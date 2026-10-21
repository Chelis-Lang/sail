//! Admission of the actual compiler-loaded translation implementation.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::TranslationTrust;

use super::{digest, unsupported};
use crate::{Result, managed};

pub(super) struct Admission {
    controls: Controls,
    closure: managed::shipped::ContractSourceClosure,
    lock: Vec<u8>,
    pub(super) evidence: TranslationTrust,
}

impl Admission {
    pub(super) fn resolve(
        config: &managed::Config,
        shadow: &Path,
        controls: Controls,
    ) -> Result<Self> {
        controls.intact()?;
        let mut command = Command::new(&config.cargo);
        command
            .current_dir(shadow)
            .args(["metadata", "--format-version", "1"]);
        config.cargo_toolchain.apply(&mut command);
        crate::adapter::configure_metadata_target(
            &mut command,
            &crate::adapter::metadata_target(&config.cache)?,
        )?;
        let output = command
            .output()
            .map_err(|_| unsupported("neutral translation Cargo metadata could not execute"))?;
        if !output.status.success() {
            return Err(unsupported(
                "neutral translation Cargo metadata failed; no source closure is qualified",
            ));
        }
        let metadata = serde_json::from_slice(&output.stdout)
            .map_err(|_| unsupported("neutral translation Cargo metadata is invalid"))?;
        let closure = managed::shipped::contract_source_closure(&metadata)
            .ok_or_else(|| unsupported("actual vstd, builtin macros, selected features or source bytes differ from Sail's qualified translation closure"))?;
        let lock = std::fs::read(shadow.join("Cargo.lock"))
            .map_err(|_| unsupported("neutral translation has no readable dependency lockfile"))?;
        let evidence = TranslationTrust {
            source_policy_sha256: closure.policy_sha256().to_owned(),
            metadata_sha256: digest(&output.stdout),
            cargo_lock_sha256: digest(&lock),
        };
        let result = Self {
            controls,
            closure,
            lock,
            evidence,
        };
        result.intact(shadow)?;
        Ok(result)
    }

    pub(super) fn intact(&self, shadow: &Path) -> Result<()> {
        self.controls.intact()?;
        if !self.closure.intact()
            || std::fs::read(shadow.join("Cargo.lock")).ok().as_deref()
                != Some(self.lock.as_slice())
        {
            return Err(unsupported(
                "neutral translation source closure or dependency lockfile changed; backend evidence is unqualified",
            ));
        }
        Ok(())
    }
}

/// Private, pure control observations; values never enter diagnostics or evidence.
pub(super) struct Controls {
    workspace: PathBuf,
    environment: BTreeMap<OsString, OsString>,
}

impl Controls {
    pub(super) fn capture(workspace: &Path, pinned: &str) -> Result<Self> {
        let environment = observed(std::env::vars_os());
        validate_environment(&environment, pinned)?;
        absent_configurations(workspace, &environment)?;
        Ok(Self {
            workspace: workspace.into(),
            environment,
        })
    }

    fn intact(&self) -> Result<()> {
        if observed(std::env::vars_os()) != self.environment {
            return Err(unsupported(
                "neutral translation compiler environment changed",
            ));
        }
        absent_configurations(&self.workspace, &self.environment)
    }
}

fn relevant(name: &str) -> bool {
    [
        "PATH",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
    ]
    .contains(&name)
        || forbidden(name)
}

fn forbidden(name: &str) -> bool {
    [
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTC_BOOTSTRAP",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "DYLD_FRAMEWORK_PATH",
    ]
    .contains(&name)
        || (name.starts_with("RUSTUP_") && !["RUSTUP_HOME", "RUSTUP_TOOLCHAIN"].contains(&name))
        || name.starts_with("RUSTC_")
        || name.starts_with("DYLD_")
        || name.starts_with("LD_")
        || name.ends_with("RUSTFLAGS")
        || (name.starts_with("CARGO_BUILD_") && name != "CARGO_BUILD_JOBS")
        || (name.starts_with("CARGO_TARGET_")
            && !["CARGO_TARGET_DIR", "CARGO_TARGET_TMPDIR"].contains(&name))
        || [
            "CARGO_UNSTABLE_",
            "CARGO_PROFILE_",
            "CARGO_CFG_",
            "CARGO_FEATURE_",
            "CARGO_REGISTRIES_",
            "CARGO_REGISTRY_",
            "CARGO_SOURCE_",
            "CARGO_PATCH_",
            "CARGO_CONFIG_",
            "VERUS_",
            "__VERUS",
            "SAIL_MANAGED",
            "SAIL_SHIPPED",
            "SAIL_VSTD_BOUNDARY",
            "SAIL_RUSTC_",
            "SAIL_VIR_",
            "SAIL_RETAINED_CONTENT",
            "SAIL_RESEARCH_",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn observed(
    environment: impl Iterator<Item = (OsString, OsString)>,
) -> BTreeMap<OsString, OsString> {
    environment
        .filter(|(name, _)| relevant(&name.to_string_lossy()))
        .collect()
}

fn validate_environment(environment: &BTreeMap<OsString, OsString>, pinned: &str) -> Result<()> {
    for (name, value) in environment {
        if !value.is_empty() && forbidden(&name.to_string_lossy()) {
            return Err(unsupported(format!(
                "neutral translation refuses compiler environment selector {}",
                name.to_string_lossy()
            )));
        }
    }
    if environment
        .get(std::ffi::OsStr::new("RUSTUP_TOOLCHAIN"))
        .is_some_and(|value| value != pinned)
    {
        return Err(unsupported(
            "neutral translation requires the pinned Cargo toolchain environment",
        ));
    }
    Ok(())
}

fn absent_configurations(
    workspace: &Path,
    environment: &BTreeMap<OsString, OsString>,
) -> Result<()> {
    let mut paths = BTreeSet::new();
    for root in [std::path::absolute(workspace), workspace.canonicalize()] {
        let root =
            root.map_err(|_| unsupported("neutral translation workspace cannot be resolved"))?;
        for ancestor in root.ancestors() {
            for file in [".cargo/config", ".cargo/config.toml"] {
                paths.insert(ancestor.join(file));
            }
        }
    }
    let home = environment
        .get(std::ffi::OsStr::new("CARGO_HOME"))
        .map(PathBuf::from)
        .or_else(|| {
            environment
                .get(std::ffi::OsStr::new("HOME"))
                .map(|home| PathBuf::from(home).join(".cargo"))
        })
        .ok_or_else(|| unsupported("neutral translation cannot identify Cargo home"))?;
    let home = if home.is_absolute() {
        home
    } else {
        workspace.join(home)
    };
    for file in ["config", "config.toml"] {
        paths.insert(home.join(file));
    }
    for path in paths {
        match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(unsupported(
                    "neutral translation refuses unreviewed Cargo configuration files",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn injected_patch_wrapper_and_compiler_controls_are_refused_without_execution() {
        for name in [
            "RUSTC_WRAPPER",
            "RUSTUP_DIST_SERVER",
            "RUSTFLAGS",
            "CARGO_PATCH_CRATES_IO_VSTD_PATH",
            "CARGO_REGISTRIES_CRATES_IO_INDEX",
            "CARGO_BUILD_RUSTC",
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
            "VERUS_Z3_PATH",
            "SAIL_SHIPPED_DEPS",
            "LD_PRELOAD",
        ] {
            let inputs = observed([(name.into(), "untrusted-value".into())].into_iter());
            assert!(validate_environment(&inputs, "pinned").is_err(), "{name}");
        }
        let inputs = observed(
            [
                ("CARGO_BUILD_JOBS".into(), "2".into()),
                ("RUSTUP_TOOLCHAIN".into(), "pinned".into()),
            ]
            .into_iter(),
        );
        assert!(validate_environment(&inputs, "pinned").is_ok());
        assert!(validate_environment(&inputs, "other").is_err());
    }

    #[test]
    fn configuration_appearing_after_admission_is_refused_without_execution() -> TestResult {
        let root = std::env::temp_dir().join(format!("sail-neutral-trust-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let result = (|| -> TestResult {
            let environment = BTreeMap::from([(
                "CARGO_HOME".into(),
                root.join("cargo-home").into_os_string(),
            )]);
            absent_configurations(&root, &environment)?;
            std::fs::create_dir(root.join(".cargo"))?;
            std::fs::write(
                root.join(".cargo/config.toml"),
                "[patch.crates-io]\nvstd = { path = '/untrusted' }\n",
            )?;
            assert!(absent_configurations(&root, &environment).is_err());
            Ok(())
        })();
        std::fs::remove_dir_all(&root)?;
        result
    }
    #[cfg(unix)]
    #[test]
    fn cargo_home_and_symbolic_configuration_paths_are_refused() -> TestResult {
        let root =
            std::env::temp_dir().join(format!("sail-neutral-trust-alias-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let result = (|| -> TestResult {
            let home = root.join("home");
            let workspace = root.join("workspace");
            std::fs::create_dir(&home)?;
            std::fs::create_dir(&workspace)?;
            let environment =
                BTreeMap::from([("CARGO_HOME".into(), home.clone().into_os_string())]);
            absent_configurations(&workspace, &environment)?;
            std::fs::write(home.join("config"), "")?;
            assert!(absent_configurations(&workspace, &environment).is_err());
            std::fs::remove_file(home.join("config"))?;
            std::fs::create_dir(workspace.join(".cargo"))?;
            // Even a dangling link cannot be treated as absent configuration.
            std::os::unix::fs::symlink(root.join("missing"), workspace.join(".cargo/config.toml"))?;
            assert!(absent_configurations(&workspace, &environment).is_err());
            Ok(())
        })();
        std::fs::remove_dir_all(&root)?;
        result
    }
}
