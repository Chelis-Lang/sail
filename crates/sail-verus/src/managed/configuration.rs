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
                "incremental verification cannot identify runtime configuration in {}; use ordinary sail verify",
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

pub(super) fn check(workspace: &Path, args: &[String]) -> Result<()> {
    for (key, value) in std::env::vars_os() {
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
        let loader = [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_LIBRARY_PATH",
            "DYLD_FRAMEWORK_PATH",
        ]
        .contains(&key.as_ref());
        if compiler || flags || target || loader {
            return Err(Error::new(
                "SAIL3112",
                format!(
                    "incremental verification cannot identify runtime selector {key}; use ordinary sail verify"
                ),
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
            "incremental verification does not support Cargo --config overrides; use ordinary sail verify",
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
