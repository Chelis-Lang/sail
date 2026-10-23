//! Read-time currentness of a completed managed result.
//!
//! A run that ends Current records the content digest of every file in the
//! selected targets' dependency closure as Cargo resolved it (sources, build
//! inputs and compiler-declared includes of those packages, the workspace
//! manifests and lockfile) and every Cargo configuration path, present or
//! absent. A later read rehashes exactly those files and checks the absent
//! ones are still absent. Every byte is read again on every check; nothing is
//! inferred from timestamps. Any difference reports the result as not current,
//! and any file that cannot be read reports Unknown, so the user re-runs.

use super::InputSnapshot;
use super::files::{digest, json};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// Name of the record in a proof context's directory.
const FILE: &str = "read-check.json";
/// A record larger than this is refused (Unknown) rather than read.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Record {
    version: u32,
    /// The run's input revision; a read that finds every file unchanged
    /// reports exactly this revision.
    revision: String,
    /// Each file and its content digest, or `None` for a path that was absent.
    files: Vec<(PathBuf, Option<String>)>,
    /// Source aliases and linked ancestors must still resolve to these destinations.
    bindings: BTreeMap<PathBuf, PathBuf>,
}

/// Records what a read must find unchanged for a Current run of `context`.
///
/// # Errors
/// `SAIL3102` if the record cannot be written atomically.
pub(super) fn record(
    context: &Path,
    revision: &str,
    files: Vec<(PathBuf, Option<String>)>,
    bindings: BTreeMap<PathBuf, PathBuf>,
) -> Result<()> {
    json(
        &context.join(FILE),
        &Record {
            version: 2,
            revision: revision.to_owned(),
            files,
            bindings,
        },
    )
}

/// The recorded files of one run: the run's input digests narrowed to the
/// selected closure, plus every configuration path with its digest or absence.
pub(super) fn entries(
    snapshot: &InputSnapshot,
    data: &serde_json::Value,
    closure: Option<&BTreeSet<String>>,
    configuration: &[PathBuf],
) -> Vec<(PathBuf, Option<String>)> {
    let mut files: BTreeMap<PathBuf, Option<String>> =
        super::input::closure_digests(snapshot, data, closure)
            .into_iter()
            .map(|(path, hash)| (path, Some(hash)))
            .collect();
    for path in configuration {
        files
            .entry(path.clone())
            .or_insert_with(|| snapshot.digests.get(path).cloned());
    }
    files.into_iter().collect()
}

enum State {
    Unchanged,
    Changed,
    Unreadable,
}

fn current(path: &Path, expected: Option<&String>) -> State {
    let Some(expected) = expected else {
        return match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::Unchanged,
            Ok(_) => State::Changed,
            Err(_) => State::Unreadable,
        };
    };
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return State::Changed,
        Err(_) => return State::Unreadable,
    };
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 65536];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => hasher.update(&buffer[..count]),
            Err(_) => return State::Unreadable,
        }
    }
    if format!("{:x}", hasher.finalize()) == *expected {
        State::Unchanged
    } else {
        State::Changed
    }
}

/// Rechecks the latest Current record of `context` against the files now.
///
/// The returned snapshot carries the recorded revision when every file is
/// unchanged, a different revision when any file changed, and
/// `complete == false` when a file could not be read.
///
/// # Errors
/// `SAIL3108` when the context has no readable record (no Current run yet).
pub(super) fn check(cache: &Path, workspace: &Path, context: &str) -> Result<InputSnapshot> {
    let missing = || Error::new("SAIL3108", "no current input record for this context");
    if context.is_empty()
        || context.len() > 128
        || !context
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(Error::new("SAIL3108", "invalid context identity"));
    }
    let path = cache
        .join("projects")
        .join(digest(workspace.as_os_str().as_encoded_bytes()))
        .join(context)
        .join(FILE);
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|file| file.take(MAX_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|_| missing())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(missing());
    }
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| missing())?;
    if record.version != 2 || record.files.is_empty() {
        return Err(missing());
    }
    let states = crate::parallel::in_order(&record.files, |(path, expected)| {
        current(path, expected.as_ref())
    });
    let binding_states = crate::parallel::in_order(
        &record.bindings.iter().collect::<Vec<_>>(),
        |(alias, expected)| match alias.canonicalize() {
            Ok(actual) if actual == **expected => State::Unchanged,
            Ok(_) => State::Changed,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::Changed,
            Err(_) => State::Unreadable,
        },
    );
    let states: Vec<_> = states.into_iter().chain(binding_states).collect();
    let changed = states.iter().any(|state| matches!(state, State::Changed));
    let unreadable = states
        .iter()
        .any(|state| matches!(state, State::Unreadable));
    Ok(InputSnapshot {
        workspace: workspace.to_owned(),
        revision: if changed {
            format!("changed-since-{}", record.revision)
        } else {
            record.revision
        },
        roots: Vec::new(),
        excluded: Vec::new(),
        complete: changed || !unreadable,
        observation_paths: BTreeSet::new(),
        digests: BTreeMap::new(),
        bindings: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    fn fixture(name: &str) -> std::io::Result<PathBuf> {
        let root =
            std::env::temp_dir().join(format!("sail-read-check-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn setup(root: &Path) -> TestResult {
        let workspace = root.join("ws");
        std::fs::create_dir_all(workspace.join("src"))?;
        std::fs::write(workspace.join("src/lib.rs"), b"fn f() {}")?;
        let context = root
            .join("cache/projects")
            .join(digest(workspace.as_os_str().as_encoded_bytes()))
            .join("ctx");
        std::fs::create_dir_all(&context)?;
        record(
            &context,
            "rev",
            vec![
                (
                    workspace.join("src/lib.rs"),
                    Some(super::super::files::file_digest(
                        &workspace.join("src/lib.rs"),
                    )?),
                ),
                (workspace.join(".cargo/config.toml"), None),
            ],
            BTreeMap::new(),
        )?;
        Ok(())
    }

    #[test]
    fn unchanged_files_report_the_recorded_revision() -> TestResult {
        let root = fixture("same")?;
        setup(&root)?;
        let snapshot = check(&root.join("cache"), &root.join("ws"), "ctx")?;
        assert_eq!(snapshot.revision, "rev");
        assert!(snapshot.complete);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn edits_restored_timestamps_and_new_configuration_are_changes() -> TestResult {
        let root = fixture("edit")?;
        setup(&root)?;
        let source = root.join("ws/src/lib.rs");
        let modified = std::fs::metadata(&source)?.modified()?;
        std::fs::write(&source, b"fn g() {}")?;
        std::fs::File::options()
            .write(true)
            .open(&source)?
            .set_modified(modified)?;
        let snapshot = check(&root.join("cache"), &root.join("ws"), "ctx")?;
        assert_ne!(snapshot.revision, "rev");
        std::fs::write(&source, b"fn f() {}")?;
        assert_eq!(
            check(&root.join("cache"), &root.join("ws"), "ctx")?.revision,
            "rev"
        );
        std::fs::create_dir_all(root.join("ws/.cargo"))?;
        std::fs::write(root.join("ws/.cargo/config.toml"), b"")?;
        assert_ne!(
            check(&root.join("cache"), &root.join("ws"), "ctx")?.revision,
            "rev"
        );
        std::fs::remove_file(root.join("ws/.cargo/config.toml"))?;
        std::fs::remove_file(&source)?;
        assert_ne!(
            check(&root.join("cache"), &root.join("ws"), "ctx")?.revision,
            "rev"
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn missing_or_corrupt_records_are_never_current() -> TestResult {
        let root = fixture("missing")?;
        assert!(check(&root.join("cache"), &root.join("ws"), "ctx").is_err());
        assert!(check(&root.join("cache"), &root.join("ws"), "../x").is_err());
        setup(&root)?;
        let context = root
            .join("cache/projects")
            .join(digest(root.join("ws").as_os_str().as_encoded_bytes()))
            .join("ctx");
        std::fs::write(context.join(FILE), b"{\"version\":1")?;
        assert!(check(&root.join("cache"), &root.join("ws"), "ctx").is_err());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn source_alias_retargeting_changes_revisions_and_retained_currentness() -> TestResult {
        use std::os::unix::fs::symlink;
        for internal in [false, true] {
            for directory in [false, true] {
                let root = fixture(&format!("alias-{internal}-{directory}"))?.canonicalize()?;
                let workspace = root.join("ws");
                std::fs::create_dir_all(workspace.join("src"))?;
                std::fs::write(workspace.join("Cargo.toml"), "manifest")?;
                let variants = if internal {
                    workspace.join("variants")
                } else {
                    root.join("variants")
                };
                for (name, text) in [
                    ("good", "pub fn f() { assert!(true); }"),
                    ("bad", "pub fn f() { assert!(false); }"),
                ] {
                    std::fs::create_dir_all(variants.join(name))?;
                    std::fs::write(variants.join(name).join("lib.rs"), text)?;
                }
                let leaf = if directory { "" } else { "lib.rs" };
                let alias =
                    workspace
                        .join("src")
                        .join(if directory { "selected" } else { "lib.rs" });
                let good = variants.join("good").join(leaf);
                let bad = variants.join("bad").join(leaf);
                symlink(&good, &alias)?;
                let data = serde_json::json!({"workspace_root": workspace,
                    "packages":[{"id":"selected","manifest_path":workspace.join("Cargo.toml")}]});
                let snapshot = || super::super::input::snapshot_metadata_home(&data, &[], None);
                let before = snapshot();
                assert!(before.complete);
                let context = root
                    .join("cache/projects")
                    .join(digest(workspace.as_os_str().as_encoded_bytes()))
                    .join("ctx");
                std::fs::create_dir_all(&context)?;
                record(
                    &context,
                    &before.revision,
                    entries(&before, &data, None, &[]),
                    before.bindings.clone(),
                )?;
                let check_now = || check(&root.join("cache"), &workspace, "ctx");
                assert_eq!(check_now()?.revision, before.revision);
                assert_eq!(
                    snapshot().revision,
                    before.revision,
                    "unchanged alias remains reusable"
                );
                std::fs::remove_file(&alias)?;
                symlink(&bad, &alias)?;
                let after = snapshot();
                assert!(after.complete);
                assert_ne!(
                    after.revision, before.revision,
                    "source selection changes even when both destinations were already hashed"
                );
                assert_ne!(
                    check_now()?.revision,
                    before.revision,
                    "retained evidence must be Historical"
                );
                if internal {
                    assert_eq!(
                        before.digests, after.digests,
                        "the regression changes only source binding"
                    );
                }
                std::fs::remove_file(&alias)?;
                symlink(&good, &alias)?;
                assert_eq!(
                    snapshot().revision,
                    before.revision,
                    "exact source revert remains reusable"
                );
                assert_eq!(check_now()?.revision, before.revision);
                std::fs::remove_dir_all(root)?;
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn compiler_declared_input_preserves_linked_ancestors_outside_the_package() -> TestResult {
        use std::os::unix::fs::symlink;
        let root = fixture("external-ancestor")?.canonicalize()?;
        let workspace = root.join("ws");
        std::fs::create_dir_all(&workspace)?;
        std::fs::write(workspace.join("Cargo.toml"), "manifest")?;
        for name in ["one", "two"] {
            std::fs::create_dir_all(root.join(name))?;
            std::fs::write(root.join(name).join("included.rs"), "same contents")?;
        }
        let other = root.join("other");
        std::fs::create_dir_all(&other)?;
        std::fs::write(other.join("Cargo.toml"), "other manifest")?;
        let alias = other.join("selected");
        symlink(root.join("one"), &alias)?;
        let declared = alias.join("included.rs");
        let data = serde_json::json!({"workspace_root": workspace,
            "packages":[{"id":"selected","manifest_path":workspace.join("Cargo.toml")},
                {"id":"other","manifest_path":other.join("Cargo.toml")}],
            "sail_source_inputs":[declared],
            "sail_source_inputs_by_package":{"selected":[declared]}});
        let snapshot = || super::super::input::snapshot_metadata_home(&data, &[], None);
        let before = snapshot();
        assert!(before.complete);
        assert_eq!(before.bindings.get(&alias), Some(&root.join("one")));
        let closure = BTreeSet::from(["selected".to_owned()]);
        let bindings = super::super::input::closure_bindings(&before, &data, Some(&closure));
        assert!(
            bindings.contains_key(&alias),
            "selected compiler includes retain alias bindings even inside another package"
        );
        let context = root
            .join("cache/projects")
            .join(digest(workspace.as_os_str().as_encoded_bytes()))
            .join("ctx");
        std::fs::create_dir_all(&context)?;
        record(
            &context,
            &before.revision,
            entries(&before, &data, Some(&closure), &[]),
            bindings,
        )?;
        std::fs::remove_file(&alias)?;
        symlink(root.join("two"), &alias)?;
        let after = snapshot();
        assert!(after.complete);
        assert_ne!(before.revision, after.revision);
        assert_ne!(
            check(&root.join("cache"), &workspace, "ctx")?.revision,
            before.revision
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
