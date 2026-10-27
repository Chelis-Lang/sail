//! Read-time currentness of a completed managed result.
//!
//! A run that ends Current records the content digest of every file in the
//! selected targets' dependency closure as Cargo resolved it (sources, build
//! inputs and compiler-declared includes of those packages, the workspace
//! manifests and lockfile) and every Cargo configuration path, present or
//! absent. A later read rehashes exactly those files and checks the absent
//! ones are still absent. Every byte is read again on every check; nothing is
//! inferred from timestamps. Any difference reports the result as not current,
//! and any file that cannot be read reports Unknown, so the user re-runs. The
//! context's recorded runtime identity (the Sail, Verus, Cargo and rustc
//! installation and the Cargo configuration) must also still match, or an
//! otherwise unchanged result reports Unknown.

use super::InputSnapshot;
use super::files::{digest, json};
use super::input::proposed_order;
use crate::{Error, Result};
use sail_acceptance::inputs::{InputFact, InputValue, compare_readback};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// Name of the record in a proof context's directory.
const FILE: &str = "read-check.json";
/// A record larger than this is refused (Unknown) rather than read.
const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Version 3: earlier records could omit compiler-declared files that the
/// dep-info reader split or rewrote (#408).
const VERSION: u32 = 3;

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
            version: VERSION,
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

enum Observation {
    Present(Vec<u8>),
    Absent,
    Unreadable,
}

impl Observation {
    fn value(&self) -> InputValue<'_> {
        match self {
            Self::Present(bytes) => InputValue::Present(bytes),
            Self::Absent => InputValue::Absent,
            Self::Unreadable => InputValue::Unreadable,
        }
    }
}

fn observe(path: &Path, expected_absent: bool) -> Observation {
    if expected_absent {
        return match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Observation::Absent,
            Ok(_) => Observation::Present(Vec::new()),
            Err(_) => Observation::Unreadable,
        };
    }
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Observation::Absent,
        Err(_) => return Observation::Unreadable,
    };
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 65536];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => hasher.update(&buffer[..count]),
            Err(_) => return Observation::Unreadable,
        }
    }
    Observation::Present(format!("{:x}", hasher.finalize()).into_bytes())
}

/// Rechecks the latest Current record of `context` against the files now.
///
/// The returned snapshot carries the recorded revision when every file is
/// unchanged, a different revision when any file changed, and
/// `complete == false` when a file could not be read or the context's runtime
/// identity no longer matches.
///
/// # Errors
/// `SAIL3108` when the context has no readable record (no Current run yet).
pub(super) fn check(cache: &Path, workspace: &Path, context: &str) -> Result<InputSnapshot> {
    check_with(cache, workspace, context, observe)
}

/// One path's metadata, of the name itself and of what it resolves to, or
/// `None` for a path that is definitely absent.
type Stamp = Option<(Vec<u64>, Option<Vec<u64>>)>;

fn metadata_numbers(metadata: &std::fs::Metadata) -> Vec<u64> {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or((u64::MAX, u32::MAX), |d| (d.as_secs(), d.subsec_nanos()));
    let kind = u64::from(metadata.is_file())
        | u64::from(metadata.is_dir()) << 1
        | u64::from(metadata.is_symlink()) << 2;
    #[cfg_attr(
        not(unix),
        expect(unused_mut, reason = "only Unix adds the inode and change time")
    )]
    let mut numbers = vec![metadata.len(), kind, modified.0, u64::from(modified.1)];
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        numbers.extend([
            metadata.dev(),
            metadata.ino(),
            u64::from(metadata.mode()),
            metadata.ctime().cast_unsigned(),
            metadata.ctime_nsec().cast_unsigned(),
        ]);
    }
    numbers
}

/// The stamp of every path, or `None` when any path cannot be examined.
fn stamp_all(paths: &[&Path]) -> Option<Vec<Stamp>> {
    crate::parallel::in_order(paths, |&path| match std::fs::symlink_metadata(path) {
        Ok(name) => {
            let resolved = match std::fs::metadata(path) {
                Ok(resolved) => Some(metadata_numbers(&resolved)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => return None,
            };
            Some(Some((metadata_numbers(&name), resolved)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(None),
        Err(_) => None,
    })
    .into_iter()
    .collect()
}

fn check_with(
    cache: &Path,
    workspace: &Path,
    context: &str,
    observe: impl Fn(&Path, bool) -> Observation + Sync,
) -> Result<InputSnapshot> {
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
    if record.version != VERSION || record.files.is_empty() {
        return Err(missing());
    }
    let files: Vec<_> = record
        .files
        .iter()
        .map(|(path, expected)| (path.as_path(), expected.is_none()))
        .collect();
    // The reads of one check must describe one state of the files: every
    // recorded path, alias and alias destination is stamped before and after
    // the whole batch, and any difference, or any path that cannot be
    // stamped, leaves the result unknown (#387). Without the bracket, a file
    // read before a concurrent edit and another read after a concurrent
    // restore could together reproduce the recorded revision although no
    // such state existed at any moment of the check.
    let mut guarded: Vec<&Path> = files.iter().map(|(path, _)| *path).collect();
    guarded.extend(record.bindings.keys().map(PathBuf::as_path));
    guarded.extend(record.bindings.values().map(PathBuf::as_path));
    guarded.sort();
    guarded.dedup();
    let before = stamp_all(&guarded);
    let reads =
        crate::parallel::in_order(&files, |&(path, absent)| (0, path, observe(path, absent)));
    let aliases: Vec<_> = record.bindings.keys().map(PathBuf::as_path).collect();
    let alias_reads = crate::parallel::in_order(&aliases, |&alias| {
        let value = match alias.canonicalize() {
            Ok(actual) => Observation::Present(actual.as_os_str().as_encoded_bytes().to_vec()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Observation::Absent,
            Err(_) => Observation::Unreadable,
        };
        (1, alias, value)
    });
    let after = stamp_all(&guarded);
    let observed: Vec<_> = reads.into_iter().chain(alias_reads).collect();
    let (changed, mut unreadable) = compare_observed(&record, &observed);
    if before.is_none() || before != after {
        unreadable = true;
    }
    if !changed && !unreadable && !super::runner::runtime_current(cache, context) {
        // A changed toolchain is not an input change the record can name,
        // but the stored result no longer describes this installation.
        unreadable = true;
    }
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
        capture: None,
    })
}

fn compare_observed(record: &Record, reads: &[(u8, &Path, Observation)]) -> (bool, bool) {
    let expected: Vec<_> = record
        .files
        .iter()
        .map(|(path, hash)| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: hash.as_ref().map_or(InputValue::Absent, |hash| {
                InputValue::Present(hash.as_bytes())
            }),
        })
        .chain(record.bindings.iter().map(|(alias, target)| InputFact {
            namespace: 1,
            name: alias.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(target.as_os_str().as_encoded_bytes()),
        }))
        .collect();
    let observed: Vec<_> = reads
        .iter()
        .map(|(namespace, path, value)| InputFact {
            namespace: *namespace,
            name: path.as_os_str().as_encoded_bytes(),
            value: value.value(),
        })
        .collect();
    let (before_order, before_inverse) = proposed_order(&expected);
    let (after_order, after_inverse) = proposed_order(&observed);
    let comparison = compare_readback(
        &expected,
        &observed,
        &before_order,
        &before_inverse,
        &after_order,
        &after_inverse,
    );
    (comparison.changed(), comparison.unknown())
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
        runtime(&root.join("cache"), &root.join("rustc"), "ctx")?;
        Ok(())
    }

    /// Records a one-file runtime identity for `context`, as a run does.
    fn runtime(cache: &Path, toolchain: &Path, context: &str) -> TestResult {
        std::fs::write(toolchain, b"toolchain")?;
        let rows = vec![(
            toolchain.to_string_lossy().into_owned(),
            super::super::tool_digests::ToolDigests::open(cache).identity(toolchain)?,
        )];
        let state = cache.join("runtime-identities");
        std::fs::create_dir_all(&state)?;
        let sha = digest(&serde_json::to_vec(&rows)?);
        json(&state.join(format!("{sha}.json")), &rows)?;
        json(&state.join(format!("{context}.link")), &sha)?;
        Ok(())
    }

    #[test]
    fn a_changed_runtime_is_never_current() -> TestResult {
        let root = fixture("runtime")?;
        setup(&root)?;
        let cache = root.join("cache");
        let current = check(&cache, &root.join("ws"), "ctx")?;
        assert!(current.complete);
        assert_eq!(current.revision, "rev");
        // A toolchain binary with other bytes changes its recorded identity.
        std::fs::write(root.join("rustc"), b"another toolchain build")?;
        let changed = check(&cache, &root.join("ws"), "ctx")?;
        assert!(!changed.complete, "a changed runtime reads as Unknown");
        // A missing runtime record is not Current either.
        std::fs::remove_dir_all(cache.join("runtime-identities"))?;
        assert!(!check(&cache, &root.join("ws"), "ctx")?.complete);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn raw_readback_results_cannot_swap_file_identity() -> TestResult {
        let root = fixture("tagged-readback")?;
        let a = root.join("a.rs");
        let b = root.join("b.rs");
        let missing = root.join("config.toml");
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "second")?;
        let record = Record {
            version: VERSION,
            revision: "rev".into(),
            files: vec![
                (a.clone(), Some(super::super::files::file_digest(&a)?)),
                (b.clone(), Some(super::super::files::file_digest(&b)?)),
                (missing.clone(), None),
            ],
            bindings: BTreeMap::new(),
        };
        let paths = [
            (a.as_path(), false),
            (b.as_path(), false),
            (missing.as_path(), true),
        ];
        let read = || {
            crate::parallel::in_order(&paths, |&(path, absent)| (0, path, observe(path, absent)))
        };
        let mut unchanged = read();
        unchanged.reverse();
        assert_eq!(compare_observed(&record, &unchanged), (false, false));
        unchanged.pop();
        assert_ne!(compare_observed(&record, &unchanged), (false, false));
        let mut extra = read();
        extra.push((0, root.as_path(), Observation::Absent));
        assert_ne!(compare_observed(&record, &extra), (false, false));
        let duplicate = vec![
            (0, a.as_path(), observe(&a, false)),
            (0, a.as_path(), observe(&a, false)),
            (0, missing.as_path(), Observation::Absent),
        ];
        assert_eq!(compare_observed(&record, &duplicate), (false, true));
        std::fs::write(&a, "second")?;
        std::fs::write(&b, "first")?;
        let mut swapped = read();
        swapped.reverse();
        assert_eq!(compare_observed(&record, &swapped), (true, false));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn retained_readback_refuses_a_torn_two_file_revision() -> TestResult {
        use std::sync::atomic::{AtomicBool, Ordering};
        let root = fixture("torn")?;
        setup(&root)?;
        let cache = root.join("cache");
        let workspace = root.join("ws");
        let a = workspace.join("src/a.rs");
        let b = workspace.join("src/b.rs");
        std::fs::write(&a, "a0")?;
        std::fs::write(&b, "b0")?;
        let context = cache
            .join("projects")
            .join(digest(workspace.as_os_str().as_encoded_bytes()))
            .join("ctx");
        let recorded = |path: &Path| -> std::io::Result<(PathBuf, Option<String>)> {
            let hash = super::super::files::file_digest(path).map_err(std::io::Error::other)?;
            Ok((path.to_owned(), Some(hash)))
        };
        record(
            &context,
            "old-a0-b0",
            vec![recorded(&a)?, recorded(&b)?],
            BTreeMap::new(),
        )?;
        assert_eq!(check(&cache, &workspace, "ctx")?.revision, "old-a0-b0");
        // Before the check, B changes. The check reads A as recorded; then a
        // writer replaces A and restores B before the check reads B. A0/B0
        // never existed during the check.
        std::fs::write(&b, "b1-longer")?;
        let written = AtomicBool::new(false);
        let torn = |path: &Path, absent: bool| {
            if path == b {
                while !written.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
                return observe(path, absent);
            }
            let observed = observe(path, absent);
            if path == a {
                let _ = std::fs::write(&a, "a1-longer");
                let _ = std::fs::write(&b, "b0");
                written.store(true, Ordering::Release);
            }
            observed
        };
        let snapshot = check_with(&cache, &workspace, "ctx", torn)?;
        assert!(
            !snapshot.complete,
            "a torn readback is never current: {}",
            snapshot.revision
        );
        // The files now settled at A1/B0 read as a change, as before.
        let settled = check(&cache, &workspace, "ctx")?;
        assert!(settled.complete && settled.revision != "old-a0-b0");
        std::fs::remove_dir_all(root)?;
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
    fn duplicate_input_rows_and_unreadable_inputs_cannot_be_current() -> TestResult {
        let root = fixture("invalid-inventory")?;
        setup(&root)?;
        let workspace = root.join("ws");
        let context = root
            .join("cache/projects")
            .join(digest(workspace.as_os_str().as_encoded_bytes()))
            .join("ctx");
        let path = context.join(FILE);
        let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        let duplicate = value["files"][0].clone();
        value["files"]
            .as_array_mut()
            .ok_or("missing rows")?
            .push(duplicate);
        std::fs::write(&path, serde_json::to_vec(&value)?)?;
        let duplicate = check(&root.join("cache"), &workspace, "ctx")?;
        assert!(!duplicate.complete);
        // A directory cannot provide file-content bytes. A read failure stays
        // Unknown, rather than becoming an observation of missing config.
        record(
            &context,
            "rev",
            vec![(workspace.clone(), Some("digest".into()))],
            BTreeMap::new(),
        )?;
        let failed = check(&root.join("cache"), &workspace, "ctx")?;
        assert!(!failed.complete);
        assert_eq!(failed.revision, "rev");
        record(
            &context,
            "rev",
            vec![(workspace.clone(), None)],
            BTreeMap::new(),
        )?;
        let appeared = check(&root.join("cache"), &workspace, "ctx")?;
        assert!(appeared.complete);
        assert_ne!(appeared.revision, "rev");
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
