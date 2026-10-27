//! Compiler-declared environment values and optional actual-command observations.
//!
//! Two parts bind a proof's environment (see `docs/freshness.md`):
//!
//! - the proof context binds the variables the pinned tools are documented or
//!   known to read ([`input_environment_key`]);
//! - each receipt binds declared variables of its target's resolved dependency
//!   closure, retaining any unowned/ambiguous declaration conservatively. Inputs
//!   come from rustc's `# env-dep:` lines and build scripts' `rerun-if-env-changed`
//!   directives ([`declaration_files`]); originally admitted same-target names
//!   remain freshly sampled when later compiler erasure drops them.
//!   Reads in ghost code alone, omitted by Verus's erased dep-info, are
//!   appended by the managed driver ([`super::ghost_environment`]) and retained
//!   as observed compiler environment before a receipt may be published.
//!
//! `PATH` and `HOME` are not bound as raw values: the context binds the
//! resolved tools and homes they select. Undeclared reads by build scripts and
//! proc macros, including tools they find through `PATH`, are not visible here,
//! exactly as they are not visible to Cargo.
//!
//! Default reuse retains the named context/declaration policy. Full raw parent
//! snapshots are used only by optional owned-command observations; they are not
//! selected as a global reuse fingerprint because unused authentication changes
//! must retain their existing context. Effective compiler inputs remain open.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Keys excluded from default named context identity unless explicitly declared.
/// Optional owned-command observations retain their actual raw parent bytes.
/// Complete ghost/opaque-read observation remains a separate input boundary.
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

/// Whether an environment key participates directly in default proof context identity.
/// Per-unit declared reads remain independently bound.
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
/// Complete successful declaration scan, retaining the actual file of each name.
pub(super) struct DeclarationFiles(BTreeMap<PathBuf, BTreeSet<String>>);

/// Untrusted ownership proposals from actual Cargo artifact path fields.
/// Unknown, ambiguous or unreported files cannot be excluded from any target.
pub(super) struct DeclarationOwners(BTreeMap<PathBuf, BTreeSet<String>>);

pub(super) fn declared_names(target: &Path) -> Option<BTreeSet<String>> {
    declaration_files(target).map(|files| files.all_names())
}

pub(super) fn declaration_files(target: &Path) -> Option<DeclarationFiles> {
    let mut files = BTreeMap::new();
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
            files.insert(path, declarations(&text, dep_info)?.into_iter().collect());
        }
    }
    Some(DeclarationFiles(files))
}

impl DeclarationFiles {
    fn all_names(&self) -> BTreeSet<String> {
        self.0.values().flatten().cloned().collect()
    }

    /// Exclude only declarations with one known owner outside an exact target closure.
    /// Failure to attribute or resolve a closure preserves the global domain.
    pub(super) fn for_target(
        &self,
        data: &serde_json::Value,
        owners: Option<&DeclarationOwners>,
        package: &str,
    ) -> BTreeSet<String> {
        let Some((owners, closure)) = owners.zip(declaration_closure(data, package)) else {
            return self.all_names();
        };
        self.0
            .iter()
            .filter(|(path, _)| {
                !owners.0.get(*path).is_some_and(|packages| {
                    packages.len() == 1 && packages.iter().all(|package| !closure.contains(package))
                })
            })
            .flat_map(|(_, names)| names.iter().cloned())
            .collect()
    }
}

/// Exact metadata dependency graph, rather than a best-effort subset.
fn declaration_closure(data: &serde_json::Value, root: &str) -> Option<BTreeSet<String>> {
    let known = package_ids(data)?;
    let mut graph = BTreeMap::new();
    for node in data["resolve"]["nodes"].as_array()? {
        let id = node["id"].as_str()?;
        if !known.contains(id) || graph.contains_key(id) {
            return None;
        }
        let dependencies: Option<Vec<_>> = node["dependencies"]
            .as_array()?
            .iter()
            .map(serde_json::Value::as_str)
            .collect();
        let dependencies = dependencies?;
        if dependencies
            .iter()
            .any(|dependency| !known.contains(*dependency))
        {
            return None;
        }
        graph.insert(id, dependencies);
    }
    let mut pending = vec![root];
    let mut closure = BTreeSet::new();
    while let Some(package) = pending.pop() {
        if closure.insert(package.to_owned()) {
            pending.extend(graph.get(package)?.iter().copied());
        }
    }
    Some(closure)
}

fn package_ids(data: &serde_json::Value) -> Option<BTreeSet<&str>> {
    let mut known = BTreeSet::new();
    for package in data["packages"].as_array()? {
        if !known.insert(package["id"].as_str()?) {
            return None;
        }
    }
    Some(known)
}

/// Read exact file-to-package proposals. A failed attribution never drops names.
pub(super) fn declaration_owners(
    artifacts: &Path,
    data: &serde_json::Value,
) -> Option<DeclarationOwners> {
    let known = package_ids(data)?;
    let records = super::source_inputs::provider_records(artifacts)?;
    let mut owners: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for record in records {
        let package = record["package_id"].as_str()?;
        if !known.contains(package) {
            return None;
        }
        let paths = match record["reason"].as_str()? {
            "compiler-artifact" => {
                let mut paths = Vec::new();
                for filename in record["filenames"].as_array()? {
                    let path = Path::new(filename.as_str()?);
                    let stem = path.file_stem()?.to_str()?;
                    // Cargo library filenames carry lib; binary names retain it.
                    let library = matches!(
                        path.extension().and_then(OsStr::to_str),
                        Some("rlib" | "rmeta" | "dylib" | "so" | "vir")
                    );
                    let stem = if library {
                        stem.strip_prefix("lib").unwrap_or(stem)
                    } else {
                        stem
                    };
                    paths.push(path.with_file_name(stem).with_extension("d"));
                }
                paths
            }
            "build-script-executed" => vec![
                Path::new(record["out_dir"].as_str()?)
                    .parent()?
                    .join("output"),
            ],
            _ => return None,
        };
        for path in paths {
            owners.entry(path).or_default().insert(package.to_owned());
        }
    }
    Some(DeclarationOwners(owners))
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

/// One actual parent-process environment enumeration, retained only in memory.
/// No prefix is omitted from observation, including Sail scheduling controls.
/// This observation does not select the default reusable proof context.
pub(super) struct ParentEnvironment {
    entries: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

impl ParentEnvironment {
    pub(super) fn capture() -> Self {
        Self {
            entries: std::env::vars_os().collect(),
        }
    }

    /// Immutable bytes from the original actual parent enumeration.
    pub(super) fn entries(&self) -> &[(std::ffi::OsString, std::ffi::OsString)] {
        &self.entries
    }

    /// A complete raw parent fingerprint; duplicate/invalid keys refuse authority.
    /// Only the digest is retained by optional command observations, never these values.
    pub(super) fn fingerprint(&self) -> Option<String> {
        use sail_acceptance::inputs::{InputFact, InputValue};
        use sail_acceptance::phase_inputs::{
            EnvironmentInheritance, Inventory, seal_environment_projection,
        };
        let facts: Vec<_> = self
            .entries
            .iter()
            .map(|(name, value)| InputFact {
                namespace: 3,
                name: name.as_encoded_bytes(),
                value: InputValue::Present(value.as_encoded_bytes()),
            })
            .collect();
        let (order, inverse) = crate::parallel::proposed_fact_order(&facts);
        let empty = Inventory {
            rows: &[],
            order: &[],
            inverse: &[],
        };
        let parent = Inventory {
            rows: &facts,
            order: &order,
            inverse: &inverse,
        };
        let result = Inventory {
            rows: &facts,
            order: &order,
            inverse: &inverse,
        };
        let admitted =
            seal_environment_projection(EnvironmentInheritance::Inherit, parent, empty, result)?;
        let rows = admitted.facts();
        let literal: Option<Vec<_>> = admitted
            .order()
            .iter()
            .map(|index| {
                let row = rows.get(*index)?;
                let InputValue::Present(value) = &row.value else {
                    return None;
                };
                Some((row.name, *value))
            })
            .collect();
        let bytes = serde_json::to_vec(&("sail-parent-environment-v1", literal?)).ok()?;
        Some(super::files::digest(&bytes))
    }
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
    #[test]
    fn full_parent_environment_fingerprint_tracks_ordinary_values_and_absence()
    -> Result<(), Box<dyn std::error::Error>> {
        const NAME: &str = "ORDINARY_PROOF_FLAG";
        const CHILD: &str = "SAIL_TEST_FULL_PARENT_ENV";
        const TEST: &str = "managed::environment::tests::full_parent_environment_fingerprint_tracks_ordinary_values_and_absence";
        if std::env::var_os(CHILD).is_some() {
            let snapshot = ParentEnvironment::capture();
            let digest = snapshot
                .fingerprint()
                .ok_or("full parent snapshot refused")?;
            // No raw environment values are printed or retained.
            println!("SAIL_PARENT_FINGERPRINT={digest}");
            return Ok(());
        }
        let mut digests = Vec::new();
        for value in [Some("7"), Some(""), None] {
            let mut command = std::process::Command::new(std::env::current_exe()?);
            command
                .arg("--exact")
                .arg(TEST)
                .arg("--nocapture")
                .env(CHILD, "child");
            if let Some(value) = value {
                command.env(NAME, value);
            } else {
                command.env_remove(NAME);
            }
            let output = command.output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout)?;
            let fingerprint = stdout
                .lines()
                .find_map(|line| line.strip_prefix("SAIL_PARENT_FINGERPRINT="))
                .ok_or("child fingerprint missing")?
                .to_owned();
            digests.push(fingerprint);
        }
        assert_ne!(
            digests[0], digests[1],
            "nonempty and empty original values differ"
        );
        assert_ne!(
            digests[0], digests[2],
            "an ordinary untracked variable cannot disappear from identity"
        );
        assert_ne!(
            digests[1], digests[2],
            "absence is distinct from an empty value"
        );
        Ok(())
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "One owned child trace keeps original capture, explicit clear policy, final overrides and complete observed fingerprint together."
    )]
    fn full_effective_environment_matches_actual_owned_child()
    -> Result<(), Box<dyn std::error::Error>> {
        use sail_acceptance::inputs::{InputFact, InputValue};
        use sail_acceptance::phase_inputs::{
            EnvironmentInheritance, Inventory, seal_environment_projection,
        };
        const CHILD: &str = "SAIL_TEST_OVERLAY_CHILD";
        const TEST: &str =
            "managed::environment::tests::full_effective_environment_matches_actual_owned_child";
        const NAMES: [&str; 3] = [
            "SAIL_OVERLAY_PRESENT",
            "SAIL_OVERLAY_EMPTY",
            "SAIL_OVERLAY_REMOVED",
        ];
        if std::env::var_os(CHILD).is_some() {
            let values: Vec<_> = NAMES
                .iter()
                .map(|name| std::env::var_os(name).map(std::ffi::OsString::into_encoded_bytes))
                .collect();
            println!("SAIL_OVERLAY_ACTUAL={}", serde_json::to_string(&values)?);
            println!(
                "SAIL_OVERLAY_FINGERPRINT={}",
                ParentEnvironment::capture()
                    .fingerprint()
                    .ok_or("actual child snapshot refused")?
            );
            return Ok(());
        }
        for clear in [false, true] {
            let parent = ParentEnvironment::capture();
            let parent_rows: Vec<_> = parent
                .entries
                .iter()
                .map(|(name, value)| InputFact {
                    namespace: 3,
                    name: name.as_encoded_bytes(),
                    value: InputValue::Present(value.as_encoded_bytes()),
                })
                .collect();
            let mut command = std::process::Command::new(std::env::current_exe()?);
            let policy = if clear {
                command.env_clear();
                EnvironmentInheritance::Clear
            } else {
                EnvironmentInheritance::Inherit
            };
            command
                .arg("--exact")
                .arg(TEST)
                .arg("--nocapture")
                .env(CHILD, "child")
                .env(NAMES[0], "7")
                .env(NAMES[1], "")
                .env_remove(NAMES[2]);
            let operations: Vec<_> = command
                .get_envs()
                .map(|(name, value)| InputFact {
                    namespace: 3,
                    name: name.as_encoded_bytes(),
                    value: value.map_or(InputValue::Absent, |value| {
                        InputValue::Present(value.as_encoded_bytes())
                    }),
                })
                .collect();
            let mut proposed = BTreeMap::new();
            if !clear {
                for (name, value) in &parent.entries {
                    proposed.insert(name.as_encoded_bytes(), Some(value.as_encoded_bytes()));
                }
            }
            for (name, value) in command.get_envs() {
                proposed.insert(
                    name.as_encoded_bytes(),
                    value.map(std::ffi::OsStr::as_encoded_bytes),
                );
            }
            let result: Vec<_> = proposed
                .iter()
                .map(|(name, value)| InputFact {
                    namespace: 3,
                    name,
                    value: value.map_or(InputValue::Absent, InputValue::Present),
                })
                .collect();
            let (parent_order, parent_inverse) = crate::parallel::proposed_fact_order(&parent_rows);
            let (operations_order, operations_inverse) =
                crate::parallel::proposed_fact_order(&operations);
            let (result_order, result_inverse) = crate::parallel::proposed_fact_order(&result);
            let admitted = seal_environment_projection(
                policy,
                Inventory {
                    rows: &parent_rows,
                    order: &parent_order,
                    inverse: &parent_inverse,
                },
                Inventory {
                    rows: &operations,
                    order: &operations_order,
                    inverse: &operations_inverse,
                },
                Inventory {
                    rows: &result,
                    order: &result_order,
                    inverse: &result_inverse,
                },
            )
            .ok_or("actual configured environment projection refused")?;
            let expected: Option<Vec<_>> = NAMES
                .iter()
                .map(|name| {
                    let row = admitted
                        .facts()
                        .iter()
                        .find(|row| row.name == name.as_bytes());
                    // env_clear makes a later env_remove a no-op; get_envs
                    // then has no removal row. Absence follows the explicit
                    // Clear policy and exact inventory, not a missing read.
                    let Some(row) = row else {
                        return Some(None);
                    };
                    match &row.value {
                        InputValue::Present(value) => Some(Some(value.to_vec())),
                        InputValue::Absent => Some(None),
                        InputValue::Unreadable => None,
                    }
                })
                .collect();
            let expected = expected.ok_or("expected synthetic keys missing")?;
            let literal: Vec<_> = admitted
                .order()
                .iter()
                .filter_map(|index| {
                    let row = admitted.facts().get(*index)?;
                    match &row.value {
                        InputValue::Present(value) => Some((row.name, *value)),
                        InputValue::Absent | InputValue::Unreadable => None,
                    }
                })
                .collect();
            let expected_fingerprint = super::super::files::digest(&serde_json::to_vec(&(
                "sail-parent-environment-v1",
                literal,
            ))?);
            // Projection borrows the final Command overrides, so it cannot be
            // mutated until those expected values have been extracted.
            let output = command.output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout)?;
            let actual = stdout
                .lines()
                .find_map(|line| line.strip_prefix("SAIL_OVERLAY_ACTUAL="))
                .ok_or("child environment report missing")?;
            let actual: Vec<Option<Vec<u8>>> = serde_json::from_str(actual)?;
            assert_eq!(actual, expected);
            assert_eq!(actual, vec![Some(b"7".to_vec()), Some(Vec::new()), None]);
            let actual_fingerprint = stdout
                .lines()
                .find_map(|line| line.strip_prefix("SAIL_OVERLAY_FINGERPRINT="))
                .ok_or("child complete fingerprint missing")?;
            assert_eq!(
                actual_fingerprint, expected_fingerprint,
                "every effective child variable participates"
            );
        }
        Ok(())
    }

    #[test]
    fn full_parent_snapshot_rejects_duplicates_and_keeps_scheduling_keys() {
        let ordinary = ParentEnvironment {
            entries: vec![("ORDINARY_PROOF_FLAG".into(), "7".into())],
        };
        let scheduled = ParentEnvironment {
            entries: vec![
                ("ORDINARY_PROOF_FLAG".into(), "7".into()),
                (
                    "SAIL_JOB_EVENTS".into(),
                    "literal-original-caller-value".into(),
                ),
            ],
        };
        assert!(ordinary.fingerprint().is_some());
        assert!(scheduled.fingerprint().is_some());
        assert_ne!(
            ordinary.fingerprint(),
            scheduled.fingerprint(),
            "no original caller key is silently excluded"
        );
        let duplicate = ParentEnvironment {
            entries: vec![("SAME".into(), "7".into()), ("SAME".into(), "9".into())],
        };
        assert!(
            duplicate.fingerprint().is_none(),
            "no last-wins conversion can hide duplicate raw entries"
        );
        let scheduled_fingerprint = scheduled.fingerprint();
        let reversed = ParentEnvironment {
            entries: scheduled.entries.into_iter().rev().collect(),
        };
        assert_eq!(scheduled_fingerprint, reversed.fingerprint());
    }

    #[cfg(unix)]
    #[test]
    fn full_parent_snapshot_keeps_non_utf8_bytes() {
        use std::os::unix::ffi::OsStringExt as _;
        let name = std::ffi::OsString::from_vec(vec![b'X', 255]);
        let value = std::ffi::OsString::from_vec(vec![255, 0x7f]);
        let raw = ParentEnvironment {
            entries: vec![(name.clone(), value)],
        };
        let lossy = ParentEnvironment {
            entries: vec![(name.to_string_lossy().into_owned().into(), "�\u{7f}".into())],
        };
        assert!(raw.fingerprint().is_some());
        assert_ne!(raw.fingerprint(), lossy.fingerprint());
    }

    use super::*;
    use std::fmt::Write as _;

    #[test]
    fn named_context_selects_tools_without_duplicating_full_parent_entries() {
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
    fn declaration_ownership_filters_only_known_unrelated_files() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-env-owner-{}", std::process::id()));
        let deps = root.join("debug/deps");
        let script = root.join("debug/build/app-1");
        std::fs::create_dir_all(&deps)?;
        std::fs::create_dir_all(&script)?;
        std::fs::write(deps.join("dep-1.d"), "# env-dep:DEP\n")?;
        std::fs::write(deps.join("unknown.d"), "# env-dep:UNKNOWN\n")?;
        std::fs::write(deps.join("ambiguous.d"), "# env-dep:AMBIGUOUS\n")?;
        std::fs::write(
            script.join("output"),
            "cargo:rerun-if-env-changed=APP_ONLY\n",
        )?;
        let data = serde_json::json!({"packages":[{"id":"app"},{"id":"dep"}],
            "resolve":{"nodes":[{"id":"app","dependencies":["dep"]},{"id":"dep","dependencies":[]}]}});
        let records = [
            serde_json::json!({"reason":"compiler-artifact","package_id":"dep","filenames":[deps.join("libdep-1.rmeta"),deps.join("libambiguous.rmeta")]}),
            serde_json::json!({"reason":"compiler-artifact","package_id":"app","filenames":[deps.join("libambiguous.rlib")]}),
            serde_json::json!({"reason":"build-script-executed","package_id":"app","out_dir":script.join("out")}),
        ];
        let artifacts = root.join("artifacts.jsonl");
        let raw = super::super::source_inputs::fixture_journal(&records)?;
        std::fs::write(&artifacts, &raw)?;
        let owners = declaration_owners(&artifacts, &data);
        let files =
            declaration_files(&root).ok_or_else(|| std::io::Error::other("scan refused"))?;
        let relevant = BTreeSet::from(["DEP".into(), "UNKNOWN".into(), "AMBIGUOUS".into()]);
        assert_eq!(files.for_target(&data, owners.as_ref(), "dep"), relevant);
        assert_eq!(
            files.for_target(&data, owners.as_ref(), "app"),
            files.all_names()
        );
        assert_eq!(files.for_target(&data, None, "dep"), files.all_names());
        let mut incomplete = data.clone();
        incomplete["resolve"]["nodes"][1]["dependencies"] = serde_json::json!(["missing"]);
        assert_eq!(
            files.for_target(&incomplete, owners.as_ref(), "dep"),
            files.all_names()
        );
        // Outer framing without a typed terminal cannot establish ownership.
        let mut incomplete_records: Vec<serde_json::Value> = std::str::from_utf8(&raw)
            .map_err(std::io::Error::other)?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        incomplete_records
            .retain(|record| record["reason"] != "sail-source-provider" || record["kind"] != 3);
        let incomplete = incomplete_records
            .iter()
            .map(|record| record.to_string() + "\n")
            .collect::<String>();
        std::fs::write(&artifacts, incomplete)?;
        assert!(declaration_owners(&artifacts, &data).is_none());
        // Unknown or malformed ownership never authorizes dropping a name.
        std::fs::write(
            &artifacts,
            [
                raw.as_slice(),
                b"{\"reason\":\"compiler-artifact\",\"package_id\":\"unknown\",\"filenames\":[]}\n",
            ]
            .concat(),
        )?;
        assert!(declaration_owners(&artifacts, &data).is_none());
        // A failed actual scan cannot be replaced by an ownership proposal.
        std::fs::write(deps.join("dep-1.d"), [0xff])?;
        assert!(declaration_files(&root).is_none());
        std::fs::remove_dir_all(root)?;
        Ok(())
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
