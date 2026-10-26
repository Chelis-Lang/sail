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
/// script's `build/*/output`. A later compiler phase can overwrite dep-info and
/// remove names from this fresh scan. Receipt admission separately retains and
/// freshly reobserves names from independently valid original receipts. A missing,
/// unreadable, malformed or untraversable observation is `None`, distinct from
/// a successful empty scan. This enumerates declarations, not all compiler I/O.
pub(super) fn declared_names(target: &Path) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let mut pending = vec![target.to_owned()];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            let kind = entry.file_type().ok()?;
            // A link may hide a declaration-bearing directory or file. Without
            // a cycle-safe owned traversal it supplies no complete scan.
            if kind.is_symlink() {
                return None;
            }
            let name = entry.file_name();
            let parent = directory.file_name().unwrap_or_default();
            let dep_info = parent == "deps" && path.extension().is_some_and(|e| e == "d");
            let script = name == "output"
                && directory
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|grandparent| grandparent == "build");
            if kind.is_dir() {
                if dep_info || script {
                    return None;
                }
                if !SKIPPED.iter().any(|skipped| name == *skipped) {
                    pending.push(path);
                }
                continue;
            }
            if !(dep_info || script) {
                continue;
            }
            if !kind.is_file() {
                return None;
            }
            let text = std::fs::read_to_string(&path).ok()?;
            names.extend(declarations(&text, dep_info)?);
        }
    }
    Some(names)
}

fn declarations(text: &str, dep_info: bool) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for line in text.lines() {
        // Cargo trims instruction-line framing, not the payload after '='.
        // Rustc dep-info names instead retain every ordinary name byte.
        let line = if dep_info { line } else { line.trim() };
        if dep_info {
            if let Some(rest) = line.strip_prefix("# env-dep:") {
                let encoded = rest.split_once('=').map_or(rest, |(name, _)| name);
                names.push(dep_info_name(encoded)?);
            }
        } else if let Some(name) = line
            .strip_prefix("cargo:rerun-if-env-changed=")
            .or_else(|| line.strip_prefix("cargo::rerun-if-env-changed="))
        {
            // Cargo removes trailing line whitespace before interpreting a
            // directive; leading name whitespace remains significant.
            let name = name.trim_end();
            if name.is_empty() || name.contains(['=', '\0']) {
                return None;
            }
            names.push(name.to_owned());
        }
    }
    Some(names)
}

fn dep_info_name(encoded: &str) -> Option<String> {
    let mut name = Vec::with_capacity(encoded.len());
    let mut bytes = encoded.bytes();
    while let Some(byte) = bytes.next() {
        name.push(if byte == b'\\' {
            match bytes.next()? {
                b'n' => b'\n',
                b'r' => b'\r',
                b'\\' => b'\\',
                _ => return None,
            }
        } else {
            byte
        });
    }
    // The native parser proposes a decoding; the exact shared byte checker
    // certifies the mapping before a different environment key can be sampled.
    sail_acceptance::inputs::environment_name_matches(encoded.as_bytes(), &name)
        .then(|| String::from_utf8(name).ok())?
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
    use std::fmt::Write as _;

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
    fn declaration_provider_errors_are_not_complete_empty_scans() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-env-errors-{}", std::process::id()));
        assert!(declared_names(&root).is_none());
        std::fs::create_dir_all(root.join("debug/deps"))?;
        assert_eq!(declared_names(&root), Some(BTreeSet::new()));
        let info = root.join("debug/deps/actual.d");
        std::fs::write(&info, b"# env-dep:ACTUAL_FLAG=present\n\xff\n")?;
        assert!(declared_names(&root).is_none());
        std::fs::write(&info, "# env-dep:ACTUAL_FLAG=present\n")?;
        assert_eq!(
            declared_names(&root),
            Some(BTreeSet::from(["ACTUAL_FLAG".into()]))
        );
        std::fs::write(&info, "# env-dep:BROKEN\\xNAME=value\n")?;
        assert!(declared_names(&root).is_none());
        std::fs::remove_file(&info)?;
        std::fs::create_dir_all(root.join("debug/build/demo/output"))?;
        assert!(
            declared_names(&root).is_none(),
            "a required file cannot be a directory"
        );
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn declaration_provider_does_not_drop_symlinked_inputs() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-env-links-{}", std::process::id()));
        std::fs::create_dir_all(root.join("debug/deps"))?;
        std::fs::write(root.join("external.d"), "# env-dep:ACTUAL_FLAG=present\n")?;
        std::os::unix::fs::symlink(root.join("external.d"), root.join("debug/deps/actual.d"))?;
        assert!(declared_names(&root).is_none());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn declaration_names_preserve_whitespace_and_decode_dep_info_escapes() {
        assert_eq!(
            declarations(
                "# env-dep: FLAG =7\n# env-dep:LINE\\nBREAK=7\n# env-dep:BACK\\\\SLASH=7\n",
                true
            ),
            Some(vec![
                " FLAG ".into(),
                "LINE\nBREAK".into(),
                "BACK\\SLASH".into()
            ])
        );
        assert_eq!(
            declarations(
                "cargo:rerun-if-env-changed= FLAG \ncargo::rerun-if-env-changed=\tTAB\t\n",
                false
            ),
            Some(vec![" FLAG".into(), "\tTAB".into()])
        );
        for malformed in ["", "BAD\\xNAME", "DANGLING\\", "BAD\0NAME"] {
            assert!(dep_info_name(malformed).is_none());
        }
    }

    #[test]
    fn actual_rustc_declarations_keep_exact_environment_names()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-env-rustc-{}", std::process::id()));
        let deps = root.join("debug/deps");
        std::fs::create_dir_all(&deps)?;
        let source = root.join("names.rs");
        let names = [
            " FLAG ",
            "LINE\nBREAK",
            "CARRIAGE\rRETURN",
            "BACK\\SLASH",
            "TAB\tKEY",
        ];
        let mut program = String::new();
        for (index, name) in names.iter().enumerate() {
            writeln!(
                program,
                "pub const FLAG_{index}: &str = env!({});",
                serde_json::to_string(name)?
            )?;
        }
        std::fs::write(&source, program)?;
        let mut command = std::process::Command::new("rustc");
        command
            .args([
                "--edition",
                "2024",
                "--crate-name",
                "env_names",
                "--crate-type",
                "lib",
                "--emit",
                "dep-info",
            ])
            .arg(&source)
            .arg("-o")
            .arg(deps.join("names.d"));
        for name in names {
            command.env(name, "7");
        }
        let output = command.output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            declared_names(&root),
            Some(names.into_iter().map(str::to_owned).collect())
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn actual_cargo_instruction_names_match_its_own_fingerprints()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-env-cargo-{}", std::process::id()));
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='env-instruction-probe'\nversion='0.1.0'\nedition='2024'\n[workspace]\n",
        )?;
        std::fs::write(root.join("src/lib.rs"), "pub const X: u8 = 1;\n")?;
        std::fs::write(
            root.join("build.rs"),
            r#"fn main() {
    println!("  cargo:rerun-if-env-changed=INDENTED");
    println!("\tcargo::rerun-if-env-changed=TAB_PREFIX");
    println!("cargo:rerun-if-env-changed= FLAG ");
    println!("cargo:rerun-if-env-changed=NBSP\u{a0}");
    println!("cargo:rerun-if-env-changed=EM\u{2003}");
}"#,
        )?;
        let target = root.join("target");
        let output =
            std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .args(["check", "--offline", "--manifest-path"])
                .arg(root.join("Cargo.toml"))
                .arg("--target-dir")
                .arg(&target)
                .env("RUSTC_WRAPPER", "")
                .env("RUSTC_WORKSPACE_WRAPPER", "")
                .env("CARGO_ENCODED_RUSTFLAGS", "")
                .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut cargo_names = BTreeSet::new();
        for directory in std::fs::read_dir(target.join("debug/.fingerprint"))? {
            let fingerprint = directory?
                .path()
                .join("run-build-script-build-script-build.json");
            if !fingerprint.is_file() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_slice(&std::fs::read(fingerprint)?)?;
            for entry in value["local"]
                .as_array()
                .ok_or("missing Cargo fingerprint locals")?
            {
                if let Some(name) = entry
                    .pointer("/RerunIfEnvChanged/var")
                    .and_then(serde_json::Value::as_str)
                {
                    cargo_names.insert(name.to_owned());
                }
            }
        }
        assert_eq!(
            cargo_names,
            ["INDENTED", "TAB_PREFIX", " FLAG", "NBSP", "EM"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(declared_names(&target), Some(cargo_names));
        std::fs::remove_dir_all(root)?;
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
        assert_eq!(names.as_ref().map(Vec::len), Some(2));
        for name in names.into_iter().flatten() {
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
            Some(vec![name.clone()])
        );
        assert_eq!(
            declarations("cargo:rerun-if-env-changed=CARGO_TARGET_DIR\n", false),
            Some(vec![name.clone()])
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
