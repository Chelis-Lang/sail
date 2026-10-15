//! Validation of Cargo configuration that can select an unobserved runtime.

use crate::{Error, Result};
use std::path::{Path, PathBuf};

const SELECTORS: &[&str] = &[
    "PATH",
    "HOME",
    "CARGO_HOME",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
    "DYLD_INSERT_LIBRARIES",
];

fn custom_target(value: &str) -> bool {
    Path::new(value)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        || value.contains("--sysroot")
}

fn validate(value: &toml::Value, file: &Path) -> Result<()> {
    let bad = || {
        Error::new(
            "SAIL3112",
            format!(
                "managed verification cannot identify the runtime configuration in {}",
                file.display()
            ),
        )
    };
    if value.get("include").is_some() {
        return Err(bad());
    }
    if let Some(build) = value.get("build") {
        if ["rustc", "rustc-wrapper", "rustc-workspace-wrapper"]
            .iter()
            .any(|k| build.get(k).is_some())
        {
            return Err(bad());
        }
        if build.get("target").is_some_and(custom_value) {
            return Err(bad());
        }
    }
    if value
        .get("env")
        .and_then(toml::Value::as_table)
        .is_some_and(|env| {
            env.keys().any(|key| {
                SELECTORS.contains(&key.as_str())
                    || key.starts_with("VERUS_")
                    || key.starts_with("SAIL_")
                    || key == "RUSTFLAGS"
                    || key == "CARGO_ENCODED_RUSTFLAGS"
                    || key.starts_with("CARGO_BUILD_")
                    || key.starts_with("CARGO_TARGET_")
            })
        })
    {
        return Err(bad());
    }
    if value
        .get("build")
        .and_then(|v| v.get("rustflags"))
        .is_some_and(custom_value)
        || value
            .get("target")
            .and_then(toml::Value::as_table)
            .is_some_and(|targets| {
                targets
                    .values()
                    .any(|target| target.get("rustflags").is_some_and(custom_value))
            })
    {
        return Err(bad());
    }
    Ok(())
}

fn custom_value(value: &toml::Value) -> bool {
    match value {
        toml::Value::String(s) => custom_target(s),
        toml::Value::Array(values) => values.iter().any(custom_value),
        toml::Value::Table(values) => values.values().any(custom_value),
        _ => false,
    }
}

pub(super) fn paths(workspace: &Path, cargo_home: Option<&Path>) -> Vec<PathBuf> {
    super::input::configuration_paths(workspace, cargo_home)
}

/// The process-loader variables whose effect on the compiler and verifier
/// Sail cannot bind to a receipt. A run with one set proceeds without proof
/// reuse ([`check`]); it is not refused.
pub(super) const LOADER_VARIABLES: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
];

/// Refuses configuration Sail cannot identify (`SAIL3112`). Returns the
/// first loader variable set in the environment, if any: such a run is
/// still verified, as plain Cargo-Verus would, but neither reuses nor
/// records proof receipts or dependency records (ADR 0046).
pub(super) fn check(workspace: &Path, args: &[String]) -> Result<Option<String>> {
    check_environment(std::env::vars_os(), workspace, args)
}

/// [`check`] for an explicit process environment.
fn check_environment(
    environment: impl Iterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
    workspace: &Path,
    args: &[String],
) -> Result<Option<String>> {
    let mut loader_set = None;
    for (key, value) in environment {
        let key = key.to_string_lossy();
        let value = value.to_string_lossy();
        let compiler = [
            "CARGO_BUILD_RUSTC",
            "CARGO_BUILD_RUSTC_WRAPPER",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        ]
        .contains(&key.as_ref());
        let flags = (key.ends_with("RUSTFLAGS") || key == "CARGO_ENCODED_RUSTFLAGS")
            && value.contains("--sysroot");
        let target = key == "CARGO_BUILD_TARGET" && custom_target(&value);
        if LOADER_VARIABLES.contains(&key.as_ref()) {
            loader_set = loader_set.or_else(|| Some(key.clone().into_owned()));
            continue;
        }
        if compiler || flags || target {
            return Err(Error::new(
                "SAIL3112",
                format!("managed verification cannot identify the runtime selector {key}"),
            ));
        }
    }
    if args
        .iter()
        .take_while(|a| a.as_str() != "--")
        .any(|a| a == "--config" || a.starts_with("--config="))
    {
        return Err(Error::new(
            "SAIL3112",
            "Cargo `--config` overrides are not bound to a proof receipt",
        ));
    }
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cargo")));
    for path in paths(workspace, home.as_deref()).into_iter().filter(|p| {
        p.is_file()
            && p.file_name().is_some_and(|name| {
                name == "config" || (name == "config.toml" && !p.with_file_name("config").is_file())
            })
    }) {
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| Error::new("SAIL3112", format!("cannot read {}: {e}", path.display())))?;
        let value: toml::Value = toml::from_str(&contents)
            .map_err(|e| Error::new("SAIL3112", format!("cannot read {}: {e}", path.display())))?;
        validate(&value, &path)?;
    }
    Ok(loader_set)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A loader variable makes the run proceed without proof reuse (ADR
    /// 0046); compiler selectors are still refused.
    #[test]
    fn loader_variables_disable_reuse_and_compiler_selectors_are_refused() {
        let dir = std::env::temp_dir().join(format!("sail-config-check-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let env = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(k, v)| (std::ffi::OsString::from(k), std::ffi::OsString::from(v)))
                .collect::<Vec<_>>()
                .into_iter()
        };
        let check = |pairs: &[(&str, &str)]| check_environment(env(pairs), &dir, &[]);
        assert_eq!(check(&[("PATH", "/bin")]), Ok(None));
        for variable in LOADER_VARIABLES {
            assert_eq!(
                check(&[("PATH", "/bin"), (variable, "/opt/cuda/lib64")]),
                Ok(Some((*variable).to_owned()))
            );
        }
        assert_eq!(
            check(&[("LD_LIBRARY_PATH", "/x"), ("CARGO_BUILD_RUSTC", "other")]).map_err(|e| e.code),
            Err("SAIL3112")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn runtime_selectors_and_custom_sysroots_are_refused() {
        for text in [
            "[build]\nrustc='custom'",
            "[build]\nrustc-wrapper='wrapper'",
            "[env]\nPATH='custom'",
            "[env]\nRUSTUP_TOOLCHAIN={value='nightly',force=true}",
            "[env]\nVERUS_Z3_PATH='wrong'",
            "[env]\nCARGO_BUILD_RUSTC='other'",
            "[build]\ntarget='custom.json'",
            "[target.x86_64-unknown-linux-gnu]\nrustflags=['--sysroot','/custom']",
            "include=['other.toml']",
        ] {
            let value = toml::from_str(text);
            assert!(value.is_ok());
            assert!(
                value
                    .as_ref()
                    .is_ok_and(|v| validate(v, Path::new("config.toml")).is_err())
            );
        }
    }
    #[test]
    fn ordinary_configuration_does_not_change_runtime_selection() {
        let value = toml::from_str(
            "[env]\nOPENSSL_DIR={value='deps/openssl',relative=true}\n[build]\njobs=2\nrustflags=['-C','opt-level=1']\n[net]\noffline=true",
        );
        assert!(value.is_ok());
        assert!(
            value
                .as_ref()
                .is_ok_and(|v| validate(v, Path::new("config.toml")).is_ok())
        );
    }
}
