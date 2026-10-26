//! The environment a managed proof depends on, following Cargo's own model.
//!
//! Two parts bind a proof's environment (see `docs/freshness.md`):
//!
//! - the proof context binds the variables the pinned tools are documented or
//!   known to read ([`input_environment_key`]);
//! - each receipt binds the current value of every variable that any unit of
//!   the run declares: rustc's `# env-dep:` dep-info lines (`env!`,
//!   `option_env!`, tracked reads, and the Verus driver's own declarations)
//!   and build scripts' `rerun-if-env-changed` ([`declared`]).
//!
//! `PATH` and `HOME` are not bound as raw values: the context binds the
//! resolved tools and homes they select. Undeclared reads by build scripts and
//! proc macros, including tools they find through `PATH`, are not visible here,
//! exactly as they are not visible to Cargo.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Sail variables that only observe or schedule a run; they never reach a proof.
const OBSERVATION_ONLY: &[&str] = &[
    // Private dependency authentication selects fetched bytes, not compiler
    // semantics. Those bytes remain source inputs; explicit env! or script
    // reads of either name remain declared receipt inputs below.
    "SAIL_DEPENDENCY_READ_TOKEN",
    "SAIL_PRIVATE_GIT_READY",
    // Whether Sail's notes are also printed in human output.
    "SAIL_EXPLAIN",
    "SAIL_PROOF_EVENTS",
    "SAIL_ENGINE_EVENTS",
    "SAIL_ENGINE_FOCUS_REPORT",
    "SAIL_ENGINE",
    "SAIL_ENGINE_FUNCTION",
    "SAIL_ENGINE_CONTEXT",
    "SAIL_JOB_EVENTS",
    "SAIL_JOB_GUARD_PARENT",
    "SAIL_MANAGED_FORCE",
    "SAIL_DAEMON_SOCKET",
    "SAIL_PROVENANCE_DIR",
    "SAIL_TRACE_DIR",
    "SAIL_PROJECT_SOCKET",
    // Whether and how much the dependency store keeps (ADR 0045); it never
    // changes what Cargo or the verifier produce.
    "SAIL_DEPENDENCY_STORE_ENTRIES",
    // Where Sail's shipped dependency build is (ADR 0048); a served
    // invocation produces exactly what compiling it produces.
    "SAIL_SHIPPED_DEPS",
    "SAIL_SHIPPED_TRACE",
];

/// Families the pinned tools read, by prefix. Sources:
/// - `CARGO_`, `__CARGO_`: Cargo configuration and internal settings (Cargo
///   book, "Environment variables Cargo reads" and "Configuration").
/// - `RUST`: `RUSTFLAGS`, `RUSTDOCFLAGS`, `RUSTC*`, `RUSTDOC`, `RUSTUP_*`,
///   `RUST_MIN_STACK`, `RUST_BACKTRACE`, `RUST_LOG` (Cargo book, rustup book,
///   std documentation); backtrace and log settings change a crash's stderr.
/// - `VERUS_`, `__VERUS_`, `VARGO_`: the Verus launcher and driver
///   (verus@7d82994 `verus/src/main.rs`, `rust_verify/src/driver.rs`,
///   `air/src/singular_manager.rs`).
/// - `Z3_`: none known for Z3 4.16.0; bound conservatively.
/// - `SAIL_`: Sail's shim, cache and daemon selection, except
///   [`OBSERVATION_ONLY`] and the private `SAIL_MANAGED_*` plumbing.
/// - `LD_`, `DYLD_`: dynamic loading of the compiler driver and the solver.
/// - `LC_`: locale.
const PREFIXES: &[&str] = &[
    "CARGO_", "__CARGO_", "RUST", "VERUS_", "__VERUS_", "VARGO_", "Z3_", "SAIL_", "LD_", "DYLD_",
    "LC_",
];

/// Single variables the pinned tools read. `VSTD_KIND` is an undeclared read
/// by Verus's `builtin_macros` (verus@7d82994 `builtin_macros/src/lib.rs`).
/// The rest select colour, layout and language of rendered diagnostics.
const NAMES: &[&str] = &[
    "VSTD_KIND",
    "NO_COLOR",
    "CLICOLOR",
    "CLICOLOR_FORCE",
    "COLUMNS",
    "LANG",
    "LANGUAGE",
];

/// Whether an environment variable is part of the proof context.
pub(super) fn input_environment_key(key: &OsStr) -> bool {
    let bytes = key.as_encoded_bytes();
    // Managed Cargo always receives its owned --target-dir. The caller's
    // directory cannot select build artifacts, but a source or build script
    // may still read it: those declared reads remain receipt inputs below.
    if key == "CARGO_TARGET_DIR" {
        return false;
    }
    if OBSERVATION_ONLY.iter().any(|name| key == *name) || bytes.starts_with(b"SAIL_MANAGED_") {
        return false;
    }
    PREFIXES
        .iter()
        .any(|prefix| bytes.starts_with(prefix.as_bytes()))
        || NAMES.iter().any(|name| key == *name)
}

/// The resolved Cargo and rustup homes, which `HOME` selects when the
/// explicit variables are unset (Cargo book `CARGO_HOME`; rustup book
/// `RUSTUP_HOME`).
pub(super) fn resolved_homes() -> [(Vec<u8>, Vec<u8>); 2] {
    let home = |explicit: &str, default: &str| {
        std::env::var_os(explicit)
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(default)))
            .map(|path| path.into_os_string().into_encoded_bytes())
            .unwrap_or_default()
    };
    [
        (b"sail-cargo-home".to_vec(), home("CARGO_HOME", ".cargo")),
        (b"sail-rustup-home".to_vec(), home("RUSTUP_HOME", ".rustup")),
    ]
}

/// Directories that cannot contain dep-info or build-script output.
const SKIPPED: &[&str] = &["incremental", ".fingerprint", "examples"];

/// The names declared by every unit in a Cargo target: `# env-dep:` lines of
/// each `deps/*.d` file and `rerun-if-env-changed` lines of each build
/// script's `build/*/output`. Units of earlier runs only add names.
///
/// `None` when any directory, entry or declaring file that exists cannot be
/// read: the declarations are then unknown, and nothing may be reused on
/// them.
pub(super) fn declared_names(target: &Path) -> Option<BTreeSet<String>> {
    let gone = |error: &std::io::Error| error.kind() == std::io::ErrorKind::NotFound;
    let mut names = BTreeSet::new();
    let mut pending = vec![target.to_owned()];
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if gone(&error) => continue,
            Err(_) => return None,
        };
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            let kind = entry.file_type().ok()?;
            let name = entry.file_name();
            if kind.is_dir() {
                if !SKIPPED.iter().any(|skipped| name == *skipped) {
                    pending.push(path);
                }
                continue;
            }
            let parent = directory.file_name().unwrap_or_default();
            let dep_info = parent == "deps" && path.extension().is_some_and(|e| e == "d");
            let script = name == "output"
                && directory
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|grandparent| grandparent == "build");
            if !(dep_info || script) || !kind.is_file() {
                continue;
            }
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) if gone(&error) => continue,
                Err(_) => return None,
            };
            names.extend(declarations(&text, dep_info));
        }
    }
    Some(names)
}

fn declarations(text: &str, dep_info: bool) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let rest = if dep_info {
                line.strip_prefix("# env-dep:")?
            } else {
                line.strip_prefix("cargo:rerun-if-env-changed=")
                    .or_else(|| line.strip_prefix("cargo::rerun-if-env-changed="))?
            };
            let name = rest.split_once('=').map_or(rest, |(name, _)| name).trim();
            (!name.is_empty()).then(|| name.to_owned())
        })
        .collect()
}

/// The current value, or absence, of each declared name.
pub(super) fn values(names: &BTreeSet<String>) -> BTreeMap<String, Option<Vec<u8>>> {
    names
        .iter()
        .map(|name| {
            (
                name.clone(),
                std::env::var_os(name).map(std::ffi::OsString::into_encoded_bytes),
            )
        })
        .collect()
}

/// Whether every recorded declared variable still has its recorded value.
pub(super) fn unchanged(recorded: &BTreeMap<String, Option<Vec<u8>>>) -> bool {
    recorded.iter().all(|(name, value)| {
        std::env::var_os(name).map(std::ffi::OsString::into_encoded_bytes) == *value
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_context_binds_tool_inputs_and_not_session_variables() {
        for key in [
            "RUSTFLAGS",
            "RUSTC",
            "RUSTUP_TOOLCHAIN",
            "RUST_MIN_STACK",
            "__CARGO_DEFAULT_LIB_METADATA",
            "VERUS_Z3_PATH",
            "VSTD_KIND",
            "SAIL_CACHE_DIR",
            "LD_LIBRARY_PATH",
            "LC_ALL",
            "COLUMNS",
            "NO_COLOR",
        ] {
            assert!(input_environment_key(OsStr::new(key)), "{key}");
        }
        for key in [
            "CARGO_TARGET_DIR",
            "PATH",
            "HOME",
            "TERM",
            "COLORTERM",
            "TERM_SESSION_ID",
            "SSH_AUTH_SOCK",
            "CLAUDECODE",
            "CLAUDE_CODE_SESSION_ID",
            "CODEX_SANDBOX",
            "VSCODE_PID",
            "GITHUB_RUN_ID",
            "TMPDIR",
            "SAIL_JOB_EVENTS",
            "SAIL_EXPLAIN",
            "SAIL_MANAGED_FORCE",
            "SAIL_MANAGED_PROXY",
            "SAIL_DEPENDENCY_READ_TOKEN",
            "SAIL_PRIVATE_GIT_READY",
        ] {
            assert!(!input_environment_key(OsStr::new(key)), "{key}");
        }
    }

    #[test]
    fn declarations_come_from_dep_info_and_build_script_output() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-declared-env-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let deps = root.join("debug/deps");
        let script = root.join("debug/build/demo-0123/output");
        let ignored = root.join("debug/incremental/demo/s.d");
        std::fs::create_dir_all(&deps)?;
        std::fs::create_dir_all(script.parent().unwrap_or(&root))?;
        std::fs::create_dir_all(ignored.parent().unwrap_or(&root))?;
        std::fs::write(
            deps.join("demo-0123.d"),
            "/t/libdemo.rmeta: src/lib.rs\n\nsrc/lib.rs:\n\n# env-dep:CARGO_PKG_NAME=demo\n# env-dep:AGENT_FLAG\n# env-dep:VERUS_Z3_PATH=/z3\n",
        )?;
        std::fs::write(
            &script,
            "cargo:rustc-env=X=1\ncargo:rerun-if-env-changed=CC\ncargo::rerun-if-env-changed=TERM\n",
        )?;
        std::fs::write(&ignored, "# env-dep:IGNORED\n")?;
        std::fs::write(deps.join("notes.txt"), "# env-dep:NOT_DEP_INFO\n")?;
        let names = declared_names(&root);
        // An unreadable dep-info file leaves the declarations unknown.
        std::fs::write(deps.join("bad-0123.d"), [0xff, 0xfe])?;
        let unreadable = declared_names(&root);
        std::fs::remove_dir_all(&root)?;
        assert!(unreadable.is_none());
        assert_eq!(
            names.into_iter().flatten().collect::<Vec<_>>(),
            [
                "AGENT_FLAG",
                "CARGO_PKG_NAME",
                "CC",
                "TERM",
                "VERUS_Z3_PATH"
            ]
        );
        Ok(())
    }

    #[test]
    fn recorded_values_bind_presence_and_bytes() {
        let names = BTreeSet::from(["SAIL_TEST_SURELY_UNSET_L16".to_owned()]);
        let recorded = values(&names);
        assert_eq!(recorded.get("SAIL_TEST_SURELY_UNSET_L16"), Some(&None));
        assert!(unchanged(&recorded));
        let mut changed = recorded;
        changed.insert("SAIL_TEST_SURELY_UNSET_L16".into(), Some(b"1".to_vec()));
        assert!(!unchanged(&changed));
    }

    #[test]
    fn authentication_plumbing_still_binds_explicit_declared_reads() {
        let names = declarations(
            "# env-dep:SAIL_DEPENDENCY_READ_TOKEN\n# env-dep:SAIL_PRIVATE_GIT_READY\n",
            true,
        );
        assert_eq!(names.len(), 2);
        for name in names {
            assert!(!input_environment_key(OsStr::new(&name)));
            let mut recorded = values(&BTreeSet::from([name.clone()]));
            assert!(unchanged(&recorded));
            let mut different = recorded
                .get(&name)
                .and_then(Clone::clone)
                .unwrap_or_default();
            different.push(b'x');
            recorded.insert(name, Some(different));
            assert!(!unchanged(&recorded));
        }
    }

    #[test]
    fn target_directory_declared_reads_still_bind_exact_values() {
        let name = "CARGO_TARGET_DIR".to_owned();
        assert_eq!(
            declarations("# env-dep:CARGO_TARGET_DIR=caller-target\n", true),
            std::slice::from_ref(&name)
        );
        assert_eq!(
            declarations("cargo:rerun-if-env-changed=CARGO_TARGET_DIR\n", false),
            std::slice::from_ref(&name)
        );
        let mut recorded = values(&BTreeSet::from([name.clone()]));
        assert!(unchanged(&recorded));
        let mut different = recorded
            .get(&name)
            .and_then(Clone::clone)
            .unwrap_or_default();
        different.push(b'x');
        recorded.insert(name, Some(different));
        assert!(
            !unchanged(&recorded),
            "declared reads cannot share proof evidence after a value change"
        );
    }
}
