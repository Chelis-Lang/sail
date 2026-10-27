//! Content identity of the native tools a fresh Cargo build would run (#187).
//!
//! Copying a stored dependency closure into a new Cargo target skips the
//! native link and the C compilation that fresh Cargo would perform, so the
//! dependency store's family key must bind the bytes of those tools, as the
//! proof context binds rustc and Cargo (ADR 0041, `tool_digests`). The
//! default tools (`cc`, `c++`, `ar`, `ld`) are resolved on `PATH`, and an
//! absent default is recorded as absent. Fail closed: a tool named by the
//! environment that cannot be resolved to one readable file, or a linker
//! selected by Cargo configuration or compiler flags (whose resolution this
//! module does not reproduce), gives no identity, and the store is not used.

use super::tool_digests::ToolDigests;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Tools that rustc's default linker and the `cc` crate run by default.
const DEFAULTS: [&str; 4] = ["cc", "c++", "ar", "ld"];
/// Variables that name a native tool directly.
const NAMED: [&str; 6] = ["RUSTC_LINKER", "CC", "CXX", "AR", "HOST_CC", "TARGET_CC"];
/// Variables carrying compiler flags, which may select a linker.
const FLAGS: [&str; 3] = [
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
];

/// The identities of the native tools, as `(name, identity)` rows in a fixed
/// order, or `None` when they cannot be established.
///
/// `vars` is the process environment, `workspace` resolves relative tool
/// paths, and `configuration` lists the Cargo configuration files in effect.
pub(super) fn identities(
    vars: &[(OsString, OsString)],
    workspace: &Path,
    configuration: &[PathBuf],
    tools: &ToolDigests,
) -> Option<Vec<(String, String)>> {
    let var = |key: &str| {
        vars.iter()
            .find(|(k, _)| k == OsStr::new(key))
            .map(|(_, v)| v.clone())
    };
    for (key, value) in vars {
        let key = key.to_string_lossy();
        let flags = FLAGS.contains(&key.as_ref())
            || key.starts_with("CARGO_TARGET_") && key.ends_with("_RUSTFLAGS");
        if flags && value.as_encoded_bytes().windows(6).any(|w| w == b"linker") {
            return None;
        }
    }
    for path in configuration {
        let bytes = std::fs::read(path).ok()?;
        if bytes.windows(6).any(|w| w == b"linker") {
            return None;
        }
    }
    let path = var("PATH");
    let mut rows = Vec::new();
    let mut named: Vec<(String, OsString)> = NAMED
        .iter()
        .filter_map(|key| var(key).map(|v| ((*key).to_owned(), v)))
        .collect();
    named.extend(vars.iter().filter_map(|(k, v)| {
        let k = k.to_str()?;
        (k.starts_with("CARGO_TARGET_") && k.ends_with("_LINKER"))
            .then(|| (k.to_owned(), v.clone()))
    }));
    named.sort();
    for (key, value) in named {
        let resolved = resolve(&value, workspace, path.as_deref())?;
        rows.push((
            format!("{key}={}", resolved.display()),
            tools.identity(&resolved).ok()?,
        ));
    }
    for name in DEFAULTS {
        match resolve(OsStr::new(name), workspace, path.as_deref()) {
            Some(resolved) => {
                rows.push((
                    resolved.display().to_string(),
                    tools.identity(&resolved).ok()?,
                ));
            }
            None => rows.push((name.to_owned(), "absent".to_owned())),
        }
    }
    Some(rows)
}

/// `program` as the process would find it: a path relative to `workspace`,
/// or the first file named `program` on `path`.
fn resolve(program: &OsStr, workspace: &Path, path: Option<&OsStr>) -> Option<PathBuf> {
    let program = Path::new(program);
    if program.as_os_str().is_empty() {
        return None;
    }
    if program.components().count() > 1 {
        return Some(workspace.join(program)).filter(|p| p.is_file());
    }
    path.into_iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(program))
        .find(|p| p.is_file())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    fn fixture(name: &str) -> std::io::Result<PathBuf> {
        let root =
            std::env::temp_dir().join(format!("sail-native-tools-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin"))?;
        std::fs::create_dir_all(root.join("cache"))?;
        root.canonicalize()
    }

    fn env(pairs: &[(&str, &OsStr)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn a_replaced_linker_at_the_same_path_changes_the_identity() -> TestResult {
        // #187: the private `cc` is replaced in place; PATH is unchanged.
        let root = fixture("replaced")?;
        let cc = root.join("bin/cc");
        std::fs::write(&cc, "#!/bin/sh\nexec /usr/bin/cc \"$@\"\n")?;
        let vars = env(&[("PATH", root.join("bin").as_os_str())]);
        let before = identities(&vars, &root, &[], &ToolDigests::open(&root.join("cache")));
        std::fs::write(&cc, "#!/bin/sh\necho 'cc unavailable' >&2\nexit 42\n")?;
        let after = identities(&vars, &root, &[], &ToolDigests::open(&root.join("cache")));
        let (Some(before), Some(after)) = (before, after) else {
            return Err("both identities must be established".into());
        };
        assert_ne!(before, after);
        assert!(before.iter().any(|(name, _)| Path::new(name) == cc));
        assert!(
            before
                .iter()
                .any(|(name, id)| name == "ld" && id == "absent")
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn unresolvable_or_configured_linkers_fail_closed() -> TestResult {
        let root = fixture("closed")?;
        let tools = ToolDigests::open(&root.join("cache"));
        let bin = root.join("bin");
        let path = ("PATH", bin.as_os_str());
        for vars in [
            env(&[path, ("CC", OsStr::new("ccache gcc"))]),
            env(&[
                path,
                (
                    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
                    OsStr::new("missing"),
                ),
            ]),
            env(&[path, ("RUSTFLAGS", OsStr::new("-C linker=/opt/ld"))]),
            env(&[path, ("CARGO_ENCODED_RUSTFLAGS", OsStr::new("-Clinker=x"))]),
        ] {
            assert_eq!(identities(&vars, &root, &[], &tools), None, "{vars:?}");
        }
        let config = root.join("config.toml");
        std::fs::write(
            &config,
            "[target.x86_64-unknown-linux-gnu]\nlinker = \"clang\"\n",
        )?;
        assert_eq!(identities(&env(&[path]), &root, &[config], &tools), None);
        assert_eq!(
            identities(&env(&[path]), &root, &[root.join("missing.toml")], &tools),
            None,
            "an unreadable configuration file fails closed"
        );
        // A named tool that resolves is bound by its content.
        std::fs::write(bin.join("gcc"), "#!/bin/sh\n")?;
        let rows = identities(&env(&[path, ("CC", OsStr::new("gcc"))]), &root, &[], &tools);
        assert!(rows.is_some_and(|rows| {
            rows.iter().any(|(n, id)| {
                n.starts_with("CC=") && id.starts_with(super::super::tool_digests::PREFIX)
            })
        }));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
