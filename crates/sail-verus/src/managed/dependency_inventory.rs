//! Complete portable dependency inputs and local mutation guards for replay.
//!
//! An inventory binds absence as well as present bytes: every directory entry,
//! including empty directories, belongs to the portable identity. An observation
//! also retains local identities through compilation or restoration. Unsupported
//! inputs decline replay; no partial inventory can authorize reuse.

use super::dependency_resolution::{Resolution, Stamp, open_input, stamp};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_ENTRIES: usize = 100_000;
// The measured macOS verifier closure exceeds one GiB. This bounds streamed
// input reads, not resident allocation; hashing still uses one 64-KiB buffer.
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// One caller-named input root; labels must be stable across installations.
pub(super) struct Root {
    pub(super) label: String,
    pub(super) path: PathBuf,
    /// True for a complete directory tree, false for one regular file.
    pub(super) recursive: bool,
}

/// Portable names, types and contents, without machine-specific root paths.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Inventory(BTreeMap<String, BTreeMap<String, Entry>>);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Entry {
    Directory {
        mode: u32,
    },
    File {
        mode: u32,
        size: u64,
        sha256: String,
    },
}

#[derive(Eq, PartialEq)]
struct PreparedEntry {
    path: PathBuf,
    stamp: Stamp,
    directory: bool,
    mode: u32,
    size: u64,
}

struct PreparedRoot {
    original: PathBuf,
    canonical: PathBuf,
    label: String,
    recursive: bool,
    entries: BTreeMap<String, PreparedEntry>,
}

#[derive(Default)]
struct Budget {
    entries: usize,
    bytes: u64,
}

/// Reads only names and metadata. No compiler-input file bytes may be read
/// until one native queue covers this complete set of files and directories.
fn prepare(
    canonical: &Path,
    recursive: bool,
    budget: &mut Budget,
) -> Option<BTreeMap<String, PreparedEntry>> {
    let root = std::fs::symlink_metadata(canonical).ok()?;
    if recursive != root.is_dir() {
        return None;
    }
    let mut entries = BTreeMap::new();
    let mut pending = vec![(canonical.to_owned(), String::new())];
    while let Some((path, relative)) = pending.pop() {
        budget.entries = budget.entries.checked_add(1)?;
        if budget.entries > MAX_ENTRIES {
            return None;
        }
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        let before = stamp(&metadata)?;
        let directory = metadata.is_dir();
        if metadata.is_file() {
            budget.bytes = budget.bytes.checked_add(metadata.len())?;
            if budget.bytes > MAX_BYTES {
                return None;
            }
        } else if directory {
            let held = open_input(&path)?;
            if stamp(&held.metadata().ok()?)? != before {
                return None;
            }
            let mut children = Vec::new();
            for child in std::fs::read_dir(&path).ok()? {
                let child = child.ok()?;
                let name = child.file_name().into_string().ok()?;
                children.push((
                    child.path(),
                    if relative.is_empty() {
                        name
                    } else {
                        format!("{relative}/{name}")
                    },
                ));
                if budget
                    .entries
                    .checked_add(pending.len())?
                    .checked_add(children.len())?
                    > MAX_ENTRIES
                {
                    return None;
                }
            }
            children.sort_by(|a, b| a.1.cmp(&b.1));
            pending.extend(children.into_iter().rev());
        } else {
            return None;
        }
        if stamp(&std::fs::symlink_metadata(&path).ok()?)? != before {
            return None;
        }
        entries.insert(
            relative,
            PreparedEntry {
                path,
                stamp: before,
                directory,
                mode: mode(&metadata)?,
                size: if directory { 0 } else { metadata.len() },
            },
        );
    }
    Some(entries)
}

/// A portable inventory bracketed by exact local file and directory identities.
pub(super) struct Observation {
    inventory: Inventory,
    stamps: BTreeMap<PathBuf, Stamp>,
    files: BTreeMap<PathBuf, String>,
    roots: Vec<(PathBuf, PathBuf)>,
    resolution: Resolution,
}

impl Observation {
    /// Captures every input or refuses. Symlinks inside trees, non-UTF paths,
    /// special files, unreadable entries and excessive inventories are refused.
    #[allow(
        clippy::too_many_lines,
        reason = "One capture brackets each complete tree and its file reads before publishing any identity."
    )]
    pub(super) fn capture(roots: &[Root]) -> Option<Self> {
        if roots.is_empty() || roots.len() > MAX_ENTRIES {
            return None;
        }
        let mut prepared = Vec::new();
        let mut paths = BTreeSet::new();
        let mut labels = BTreeSet::new();
        let mut budget = Budget::default();
        for root in roots {
            if root.label.is_empty() || !labels.insert(&root.label) {
                return None;
            }
            let original = std::path::absolute(&root.path).ok()?;
            let canonical = original.canonicalize().ok()?;
            canonical.to_str()?;
            let entries = prepare(&canonical, root.recursive, &mut budget)?;
            paths.extend(entries.values().map(|entry| entry.path.clone()));
            prepared.push(PreparedRoot {
                original,
                canonical,
                label: root.label.clone(),
                recursive: root.recursive,
                entries,
            });
        }
        let originals = prepared
            .iter()
            .map(|root| root.original.clone())
            .collect::<Vec<_>>();
        // Register exactly once, with the complete path set, before hashing.
        // Native file-write and directory-membership events remain live until
        // compilation/restoration has passed its final mutation barrier.
        let resolution = Resolution::capture_inventory(&originals, &paths)?;
        let mut captured = Self {
            inventory: Inventory(BTreeMap::new()),
            stamps: BTreeMap::new(),
            files: BTreeMap::new(),
            roots: Vec::new(),
            resolution,
        };
        let mut final_budget = Budget::default();
        for root in prepared {
            if captured.resolution.canonical(&root.original)? != root.canonical
                || root.original.canonicalize().ok()? != root.canonical
                || prepare(&root.canonical, root.recursive, &mut final_budget)? != root.entries
            {
                return None;
            }
            let mut tree = BTreeMap::new();
            for (relative, entry) in root.entries {
                if stamp(&std::fs::symlink_metadata(&entry.path).ok()?)?.ne(&entry.stamp) {
                    return None;
                }
                let value = if entry.directory {
                    Entry::Directory { mode: entry.mode }
                } else {
                    let digest = read_file(&entry.path, &entry.stamp, entry.size)?;
                    if captured
                        .files
                        .get(&entry.path)
                        .is_some_and(|old| old != &digest)
                    {
                        return None;
                    }
                    captured.files.insert(entry.path.clone(), digest.clone());
                    Entry::File {
                        mode: entry.mode,
                        size: entry.size,
                        sha256: digest,
                    }
                };
                captured.remember(entry.path, entry.stamp)?;
                tree.insert(relative, value);
            }
            captured.inventory.0.insert(root.label, tree);
            captured.roots.push((root.original, root.canonical));
        }
        captured.unchanged().then_some(captured)
    }

    pub(super) fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// Checks the complete observation's bracket without trusting final mtimes
    /// alone: replacement and content/topology A-to-B-to-A changes alter ctime.
    pub(super) fn unchanged(&self) -> bool {
        self.resolution.unchanged()
            && self.roots.iter().all(|(original, canonical)| {
                original.canonicalize().ok().as_ref() == Some(canonical)
            })
            && self.stamps.iter().all(|(path, before)| {
                std::fs::symlink_metadata(path)
                    .ok()
                    .and_then(|metadata| stamp(&metadata))
                    .as_ref()
                    == Some(before)
            })
            && self.resolution.unchanged()
    }

    /// Requires a compiler-declared file's exact captured bytes, including
    /// files reached through a caller's equivalent absolute root spelling.
    pub(super) fn covers(&self, path: &Path, sha256: &str) -> bool {
        if !self.resolution.unchanged() {
            return false;
        }
        let Ok(original) = std::path::absolute(path) else {
            return false;
        };
        let Some(canonical) = self.resolution.covered_with(&original, |path, metadata| {
            self.stamps
                .get(path)
                .is_some_and(|before| stamp(metadata).as_ref() == Some(before))
        }) else {
            return false;
        };
        self.files
            .get(&canonical)
            .is_some_and(|digest| digest == sha256)
            && self.stamps.get(&canonical).is_some_and(|before| {
                std::fs::symlink_metadata(&canonical)
                    .ok()
                    .and_then(|metadata| stamp(&metadata))
                    .as_ref()
                    == Some(before)
            })
            && self.resolution.unchanged()
    }

    fn remember(&mut self, path: PathBuf, value: Stamp) -> Option<()> {
        if self.stamps.get(&path).is_some_and(|old| old != &value) {
            return None;
        }
        self.stamps.insert(path, value);
        Some(())
    }
}

fn read_file(path: &Path, before: &Stamp, size: u64) -> Option<String> {
    let mut file = open_input(path)?;
    if stamp(&file.metadata().ok()?)?.ne(before) {
        return None;
    }
    let mut hash = Sha256::new();
    let mut read = 0u64;
    let mut buffer = vec![0u8; 65_536];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        read = read.checked_add(u64::try_from(count).ok()?)?;
        if read > size {
            return None;
        }
        hash.update(&buffer[..count]);
    }
    (read == size && stamp(&file.metadata().ok()?)?.eq(before))
        .then(|| format!("{:x}", hash.finalize()))
}

#[cfg(unix)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "The same interface conservatively refuses unsupported non-Unix hosts."
)]
fn mode(metadata: &Metadata) -> Option<u32> {
    use std::os::unix::fs::MetadataExt as _;
    Some(metadata.mode())
}

#[cfg(not(unix))]
fn mode(_: &Metadata) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fs::File;
    use std::sync::atomic::{AtomicU64, Ordering};

    type TestResult = Result<(), Box<dyn Error>>;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> std::io::Result<Self> {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "sail-dependency-inventory-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path)?;
            Ok(Self(path.canonicalize()?))
        }
        fn tree(&self) -> Root {
            Root {
                label: "package".to_owned(),
                path: self.0.clone(),
                recursive: true,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn portable_inventory_survives_relocation_and_requires_exact_declared_bytes() -> TestResult {
        let first = Fixture::new()?;
        let second = Fixture::new()?;
        for fixture in [&first, &second] {
            std::fs::create_dir(fixture.0.join("empty"))?;
            std::fs::write(fixture.0.join("lib.rs"), "pub fn example() {}")?;
        }
        let a = Observation::capture(&[first.tree()]).ok_or("first capture")?;
        let b = Observation::capture(&[second.tree()]).ok_or("relocated capture")?;
        assert_eq!(a.inventory(), b.inventory());
        let serialized = serde_json::to_vec(a.inventory())?;
        assert_eq!(
            &serde_json::from_slice::<Inventory>(&serialized)?,
            a.inventory()
        );
        let digest = format!("{:x}", Sha256::digest(b"pub fn example() {}"));
        assert!(a.covers(&first.0.join("lib.rs"), &digest));
        assert!(!a.covers(&first.0.join("lib.rs"), "wrong digest"));
        assert!(!a.covers(&second.0.join("lib.rs"), &digest));
        assert!(a.unchanged());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn declared_paths_require_observed_literal_and_expanded_components() -> TestResult {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new()?;
        let rogue = Fixture::new()?;
        let source = fixture.0.join("source");
        let ordinary = fixture.0.join("ordinary");
        std::fs::create_dir_all(source.join("known"))?;
        std::fs::create_dir(&ordinary)?;
        std::fs::create_dir_all(rogue.0.join("source"))?;
        std::fs::create_dir(rogue.0.join("branch"))?;
        std::fs::write(source.join("lib.rs"), "good")?;
        std::fs::write(rogue.0.join("source/lib.rs"), "bad!")?;
        let directory_alias = fixture.0.join("directory-alias");
        let file_alias = fixture.0.join("file-alias");
        symlink(&source, &directory_alias)?;
        symlink(source.join("lib.rs"), &file_alias)?;
        let observation = Observation::capture(&[Root {
            label: "source".to_owned(),
            path: source.clone(),
            recursive: true,
        }])
        .ok_or("source capture")?;
        let digest = format!("{:x}", Sha256::digest(b"good"));
        let declared = ordinary.join("../source/lib.rs");
        let saved = fixture.0.join("ordinary-saved");
        std::fs::rename(&ordinary, &saved)?;
        symlink(rogue.0.join("branch"), &ordinary)?;
        assert_eq!(std::fs::read(&declared)?, b"bad!");
        std::fs::remove_file(&ordinary)?;
        std::fs::rename(&saved, &ordinary)?;
        assert!(
            observation.unchanged(),
            "the mutation was outside the source tree"
        );
        assert!(!observation.covers(&declared, &digest));
        assert!(!observation.covers(&directory_alias.join("lib.rs"), &digest));
        assert!(!observation.covers(&file_alias, &digest));
        assert!(observation.covers(&source.join("known/../lib.rs"), &digest));
        let alias_observation = Observation::capture(&[Root {
            label: "source".to_owned(),
            path: directory_alias.clone(),
            recursive: true,
        }])
        .ok_or("observed alias capture")?;
        assert!(alias_observation.covers(&directory_alias.join("known/../lib.rs"), &digest));
        assert!(alias_observation.unchanged());
        Ok(())
    }

    #[test]
    fn added_module_path_and_empty_directories_change_the_portable_identity() -> TestResult {
        let fixture = Fixture::new()?;
        std::fs::write(fixture.0.join("lib.rs"), "mod leaf;")?;
        std::fs::write(fixture.0.join("leaf.rs"), "pub fn leaf() {}")?;
        let before = Observation::capture(&[fixture.tree()]).ok_or("before")?;
        std::fs::create_dir(fixture.0.join("leaf"))?;
        let empty = Observation::capture(&[fixture.tree()]).ok_or("empty directory")?;
        assert_ne!(before.inventory(), empty.inventory());
        std::fs::write(fixture.0.join("leaf/mod.rs"), "pub fn different() {}")?;
        let ambiguous = Observation::capture(&[fixture.tree()]).ok_or("ambiguous source")?;
        assert_ne!(empty.inventory(), ambiguous.inventory());
        assert!(!before.unchanged());
        assert!(!empty.unchanged());
        Ok(())
    }

    #[test]
    fn source_write_and_restore_cannot_publish_a_preceding_compiler_result() -> TestResult {
        let fixture = Fixture::new()?;
        let file = fixture.0.join("lib.rs");
        std::fs::write(&file, "good")?;
        let modified = std::fs::metadata(&file)?.modified()?;
        let before = Observation::capture(&[fixture.tree()]).ok_or("before")?;
        std::thread::sleep(std::time::Duration::from_millis(2));
        std::fs::write(&file, "bad!")?;
        assert!(!before.unchanged());
        std::fs::write(&file, "good")?;
        File::options()
            .write(true)
            .open(&file)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))?;
        let restored = Observation::capture(&[fixture.tree()]).ok_or("restored")?;
        assert_eq!(before.inventory(), restored.inventory());
        assert!(
            !before.unchanged(),
            "equal bytes and restored mtime cannot hide an in-run mutation"
        );
        Ok(())
    }

    #[test]
    fn compiler_file_roots_are_guarded_and_duplicate_labels_refuse() -> TestResult {
        let fixture = Fixture::new()?;
        let file = fixture.0.join("compiler");
        std::fs::write(&file, "compiler bytes")?;
        let root = || Root {
            label: "compiler".to_owned(),
            path: file.clone(),
            recursive: false,
        };
        let observed = Observation::capture(&[root()]).ok_or("compiler")?;
        std::fs::write(&file, "different compiler")?;
        assert!(!observed.unchanged());
        assert!(Observation::capture(&[root(), root()]).is_none());
        assert!(Observation::capture(&[]).is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn interior_and_unobserved_external_symlinks_refuse() -> TestResult {
        let fixture = Fixture::new()?;
        let outside = Fixture::new()?;
        let source = fixture.0.join("lib.rs");
        std::fs::write(&source, "source")?;
        let observed = Observation::capture(&[fixture.tree()]).ok_or("before symlink")?;
        let alias = outside.0.join("alias.rs");
        std::os::unix::fs::symlink(&source, &alias)?;
        let hash = format!("{:x}", Sha256::digest(b"source"));
        assert!(!observed.covers(&alias, &hash));
        std::os::unix::fs::symlink(&source, fixture.0.join("alias.rs"))?;
        assert!(Observation::capture(&[fixture.tree()]).is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn observed_root_alias_cannot_be_retargeted_and_restored_during_compilation() -> TestResult {
        let fixture = Fixture::new()?;
        let aliases = Fixture::new()?;
        std::fs::write(fixture.0.join("lib.rs"), "source")?;
        let alias = aliases.0.join("package");
        std::os::unix::fs::symlink(&fixture.0, &alias)?;
        let root = || Root {
            label: "package".to_owned(),
            path: alias.clone(),
            recursive: true,
        };
        let observed = Observation::capture(&[root()]).ok_or("alias")?;
        let hash = format!("{:x}", Sha256::digest(b"source"));
        assert!(observed.covers(&alias.join("lib.rs"), &hash));
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(&aliases.0, &alias)?;
        std::fs::remove_file(&alias)?;
        std::os::unix::fs::symlink(&fixture.0, &alias)?;
        let restored = Observation::capture(&[root()]).ok_or("restored alias")?;
        assert_eq!(observed.inventory(), restored.inventory());
        assert!(!observed.unchanged());
        Ok(())
    }

    #[test]
    fn ordinary_ancestor_swap_and_restore_is_latched() -> TestResult {
        let fixture = Fixture::new()?;
        for (name, bytes) in [("a", "good"), ("other", "bad!")] {
            std::fs::create_dir_all(fixture.0.join(name).join("b"))?;
            std::fs::write(fixture.0.join(name).join("b/lib.rs"), bytes)?;
        }
        let path = fixture.0.join("a/b");
        let observed = Observation::capture(&[Root {
            label: "package".to_owned(),
            path: path.clone(),
            recursive: true,
        }])
        .ok_or("capture")?;
        std::fs::rename(fixture.0.join("a"), fixture.0.join("saved"))?;
        std::fs::rename(fixture.0.join("other"), fixture.0.join("a"))?;
        assert_eq!(std::fs::read_to_string(path.join("lib.rs"))?, "bad!");
        std::fs::rename(fixture.0.join("a"), fixture.0.join("other"))?;
        std::fs::rename(fixture.0.join("saved"), fixture.0.join("a"))?;
        assert!(!observed.unchanged(), "ancestor ABA must refuse restore");
        assert!(
            !observed.unchanged(),
            "draining an event cannot clear refusal"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn indirect_symlink_target_swap_and_restore_is_latched() -> TestResult {
        let fixture = Fixture::new()?;
        for (name, bytes) in [("good", "good"), ("bad", "bad!")] {
            std::fs::create_dir_all(fixture.0.join(name).join("b"))?;
            std::fs::write(fixture.0.join(name).join("b/lib.rs"), bytes)?;
        }
        let hop = fixture.0.join("hop");
        std::os::unix::fs::symlink(fixture.0.join("good"), &hop)?;
        let alias = fixture.0.join("alias");
        std::os::unix::fs::symlink(fixture.0.join("hop/b"), &alias)?;
        let observed = Observation::capture(&[Root {
            label: "package".to_owned(),
            path: alias.clone(),
            recursive: true,
        }])
        .ok_or("capture")?;
        std::fs::remove_file(&hop)?;
        std::os::unix::fs::symlink(fixture.0.join("bad"), &hop)?;
        assert_eq!(std::fs::read_to_string(alias.join("lib.rs"))?, "bad!");
        std::fs::remove_file(&hop)?;
        std::os::unix::fs::symlink(fixture.0.join("good"), &hop)?;
        assert!(!observed.unchanged(), "hidden target-chain ABA must refuse");
        assert!(!observed.unchanged());
        Ok(())
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn native_resolution_guard_allows_sibling_cargo_output() -> TestResult {
        let fixture = Fixture::new()?;
        let file = fixture.0.join("existing.rmeta");
        std::fs::write(&file, "immutable dependency")?;
        let observed = Observation::capture(&[Root {
            label: "extern".to_owned(),
            path: file,
            recursive: false,
        }])
        .ok_or("capture")?;
        std::fs::write(fixture.0.join("new-vstd.rmeta"), "compiler output")?;
        std::fs::create_dir(fixture.0.join("new-output-directory"))?;
        assert!(
            observed.unchanged(),
            "sibling writes do not retarget the input"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn temporary_ancestor_permission_change_is_latched() -> TestResult {
        use std::os::unix::fs::PermissionsExt as _;
        let fixture = Fixture::new()?;
        let file = fixture.0.join("input");
        std::fs::write(&file, "source")?;
        let observed = Observation::capture(&[Root {
            label: "compiler".to_owned(),
            path: file,
            recursive: false,
        }])
        .ok_or("capture")?;
        let previous = std::fs::metadata(&fixture.0)?.permissions();
        std::fs::set_permissions(
            &fixture.0,
            std::fs::Permissions::from_mode(previous.mode() ^ 0o100),
        )?;
        std::fs::set_permissions(&fixture.0, previous)?;
        assert!(!observed.unchanged());
        assert!(!observed.unchanged());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn regular_file_replaced_by_fifo_after_stat_refuses_without_blocking() -> TestResult {
        let fixture = Fixture::new()?;
        let path = fixture.0.join("source");
        std::fs::write(&path, "source")?;
        let before = stamp(&std::fs::metadata(&path)?).ok_or("before")?;
        std::fs::remove_file(&path)?;
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&path)
                .status()?
                .success()
        );
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let _ = send.send(read_file(&path, &before, 6).is_none());
        });
        let refused = receive.recv_timeout(std::time::Duration::from_secs(2));
        assert_eq!(
            refused.ok(),
            Some(true),
            "FIFO replacement must refuse without a peer"
        );
        assert!(worker.join().is_ok());
        Ok(())
    }

    #[test]
    fn oversized_input_declines_before_reading_its_bytes() -> TestResult {
        let fixture = Fixture::new()?;
        File::create(fixture.0.join("too-large"))?.set_len(MAX_BYTES + 1)?;
        assert!(Observation::capture(&[fixture.tree()]).is_none());
        Ok(())
    }
}
