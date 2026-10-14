//! Listing and pruning managed workspaces (`<cache>/projects/<workspace-key>/`).
//!
//! Each workspace directory holds that workspace's compiled Cargo targets
//! (`compiler-artifacts/<family>/target/`), its proof contexts (receipts and
//! run records) and the locks that serialise its runs. Every managed run holds
//! `workspace.lock` shared from preparation until its records are read back,
//! so pruning, which takes it exclusively without waiting, never removes a
//! directory that a run is using. A run that was waiting on the lock of a
//! directory that has since been pruned notices the lock file is gone and takes
//! the new one (`lock`).
//!
//! Pruning removes a workspace's receipts together with its targets, so no
//! receipt survives for a deleted target: the next run is a cold run, never a
//! reused one.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The lock file, in each workspace directory, that runs hold shared and
/// pruning takes exclusively.
pub(super) const WORKSPACE_LOCK: &str = "workspace.lock";

/// Prefix of a workspace directory being removed. Such directories are never
/// listed, and a later prune removes any left by an interrupted one.
const TRASH: &str = ".prune-";

/// One managed workspace in the cache.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Project {
    /// The workspace key (directory name under `projects/`).
    pub key: String,
    /// The workspace path its latest run recorded, if any run recorded one
    /// that matches the key.
    pub workspace: Option<PathBuf>,
    /// Whether `workspace` still exists (`None` when the path is unknown or
    /// cannot be checked).
    pub exists: Option<bool>,
    /// Bytes on disk, counting each hard-linked file once.
    pub bytes: u64,
    /// When a run of this workspace last started, in seconds since the Unix
    /// epoch.
    pub last_used_unix: u64,
}

/// Which workspaces `prune` removes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Every workspace (no other criterion applies).
    pub all: bool,
    /// Only workspaces last used at least this long ago.
    pub older_than: Option<Duration>,
    /// Only workspaces whose recorded path no longer exists.
    pub missing: bool,
}

impl Selection {
    /// Whether the selection names any workspace at all: `--all` or at least
    /// one criterion.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.all && self.older_than.is_none() && !self.missing
    }

    /// Whether `project` is selected at `now` (seconds since the Unix
    /// epoch). Criteria combine: a workspace must meet every one given. An
    /// empty selection selects nothing.
    #[must_use]
    pub fn selects(&self, project: &Project, now: u64) -> bool {
        if self.all {
            return true;
        }
        if self.is_empty() {
            return false;
        }
        let old = self
            .older_than
            .is_none_or(|age| now.saturating_sub(project.last_used_unix) >= age.as_secs());
        let missing = !self.missing || project.exists == Some(false);
        old && missing
    }
}

/// The result of pruning one workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pruned {
    /// The workspace's directory was removed, freeing about this many bytes.
    Removed(u64),
    /// A managed run or job holds the workspace; nothing was removed.
    Busy,
}

/// The directory holding every managed workspace of `cache`.
#[must_use]
pub fn root(cache: &Path) -> PathBuf {
    cache.join("projects")
}

/// Every managed workspace under `cache`, largest first.
///
/// # Errors
/// When `projects/` exists but cannot be read.
pub fn list(cache: &Path) -> std::io::Result<Vec<Project>> {
    let root = root(cache);
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut projects = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some(key) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !valid_key(&key) || !entry.file_type()?.is_dir() {
            continue;
        }
        projects.push(project(&entry.path(), key));
    }
    projects.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.key.cmp(&b.key)));
    Ok(projects)
}

/// Total bytes under `projects/`, including directories an interrupted prune
/// left behind. Zero when there is none.
#[must_use]
pub fn total_bytes(cache: &Path) -> u64 {
    size(&root(cache))
}

/// Removes one workspace's directory unless a run holds it.
///
/// # Errors
/// When the key is not a workspace key, or the directory cannot be locked,
/// moved or removed. A failed removal leaves a hidden directory that a later
/// prune removes; it is never used by a run.
pub fn prune(cache: &Path, key: &str) -> std::io::Result<Pruned> {
    if !valid_key(key) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("`{key}` is not a managed workspace key"),
        ));
    }
    let root = root(cache);
    remove_leftovers(&root);
    let directory = root.join(key);
    if !directory.is_dir() {
        return Ok(Pruned::Removed(0));
    }
    // Hold every lock a run could hold: the workspace lock (this version),
    // each compiler family's (runs from before the workspace lock) and each
    // legacy per-context target's. None is waited for.
    let mut held = Vec::new();
    let workspace = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(WORKSPACE_LOCK))?;
    if !try_exclusive(&workspace)? {
        return Ok(Pruned::Busy);
    }
    held.push(workspace);
    for lock in older_locks(&directory)? {
        let file = match OpenOptions::new().read(true).write(true).open(&lock) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if !try_exclusive(&file)? {
            return Ok(Pruned::Busy);
        }
        held.push(file);
    }
    let bytes = size(&directory);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let trash = root.join(format!("{TRASH}{key}-{}-{nanos}", std::process::id()));
    // The rename is atomic: a run that starts now finds no directory and
    // creates a fresh one, never a partly removed one.
    std::fs::rename(&directory, &trash)?;
    let removed = std::fs::remove_dir_all(&trash);
    drop(held);
    removed?;
    Ok(Pruned::Removed(bytes))
}

/// Opens and locks `path`, waiting until `deadline`; `shared` for a run's
/// workspace lock, exclusive for its compiler family.
///
/// After locking, the file must still be the one at `path`: when a prune
/// removed the directory while this run waited, the lock it holds guards
/// nothing, so it takes the lock at the path again (creating it).
pub(super) fn lock(
    path: &Path,
    shared: bool,
    deadline: std::time::Instant,
) -> Result<File, std::io::Error> {
    loop {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        loop {
            let attempt = if shared {
                file.try_lock_shared()
            } else {
                file.try_lock()
            };
            match attempt {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        "another run holds it",
                    ));
                }
                Err(std::fs::TryLockError::Error(error)) => return Err(error),
            }
        }
        if is_at(&file, path) {
            return Ok(file);
        }
        if std::time::Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "the managed directory was removed while waiting",
            ));
        }
    }
}

fn try_exclusive(file: &File) -> std::io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(std::fs::TryLockError::WouldBlock) => Ok(false),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

/// Whether `file` is the file now at `path`.
fn is_at(file: &File, path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        match (file.metadata(), std::fs::metadata(path)) {
            (Ok(held), Ok(current)) => held.dev() == current.dev() && held.ino() == current.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        // Elsewhere an open file cannot be renamed away, so pruning fails
        // rather than leaving a waiter with a stale lock.
        let _ = (file, path);
        true
    }
}

/// Lock files of runs that do not take the workspace lock: each compiler
/// family's, and each legacy proof context's own target lock.
fn older_locks(directory: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut locks = Vec::new();
    for parent in [
        directory.join("compiler-artifacts"),
        directory.to_path_buf(),
    ] {
        let entries = match std::fs::read_dir(&parent) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                locks.push(entry.path().join("ownership.lock"));
            }
        }
    }
    Ok(locks)
}

/// Removes directories left by interrupted prunes. Best effort: another prune
/// may be removing the same directory.
fn remove_leftovers(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(TRASH) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn valid_key(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit())
}

fn project(directory: &Path, key: String) -> Project {
    let mut last_used = modified(directory);
    let mut latest: Option<(u64, PathBuf)> = None;
    for context in std::fs::read_dir(directory).into_iter().flatten().flatten() {
        let runs = context.path().join("runs");
        let Ok(entries) = std::fs::read_dir(&runs) else {
            continue;
        };
        // A run creates its record directory when it starts.
        last_used = last_used.max(modified(&runs));
        for run in entries.flatten() {
            let result = run.path().join("result.json");
            let when = modified(&result);
            if result.is_file() && latest.as_ref().is_none_or(|(t, _)| when > *t) {
                latest = Some((when, result));
            }
        }
    }
    let workspace = latest.and_then(|(_, result)| recorded_workspace(&result, &key));
    let exists = workspace.as_ref().and_then(|path| path.try_exists().ok());
    Project {
        bytes: size(directory),
        key,
        workspace,
        exists,
        last_used_unix: last_used,
    }
}

/// The workspace a run result names, when it is the one the key identifies.
fn recorded_workspace(result: &Path, key: &str) -> Option<PathBuf> {
    #[derive(serde::Deserialize)]
    struct Recorded {
        workspace: String,
    }
    let recorded: Recorded = serde_json::from_slice(&std::fs::read(result).ok()?).ok()?;
    let path = PathBuf::from(recorded.workspace);
    (super::digest(path.as_os_str().as_encoded_bytes()) == key).then_some(path)
}

fn modified(path: &Path) -> u64 {
    std::fs::symlink_metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// Bytes under `path`, without following symbolic links and counting each
/// hard-linked file once.
fn size(path: &Path) -> u64 {
    let mut seen = std::collections::HashSet::<(u64, u64)>::new();
    let mut total = 0u64;
    let mut pending = vec![path.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(metadata) = std::fs::symlink_metadata(&next) else {
            continue;
        };
        if metadata.is_dir() {
            pending.extend(
                std::fs::read_dir(&next)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path()),
            );
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            if metadata.nlink() > 1 && !seen.insert((metadata.dev(), metadata.ino())) {
                continue;
            }
        }
        #[cfg(not(unix))]
        let _ = &mut seen;
        total = total.saturating_add(metadata.len());
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 86_400;

    fn project(last_used_unix: u64, exists: Option<bool>) -> Project {
        Project {
            key: "a".repeat(64),
            workspace: exists.map(|_| PathBuf::from("/w")),
            exists,
            bytes: 1,
            last_used_unix,
        }
    }

    #[test]
    fn selection_requires_every_given_criterion() {
        let now = 100 * DAY;
        let old_missing = project(now - 30 * DAY, Some(false));
        let old_present = project(now - 30 * DAY, Some(true));
        let new_missing = project(now - DAY, Some(false));
        let unknown = project(now - 30 * DAY, None);
        let nothing = Selection::default();
        assert!(nothing.is_empty());
        assert!(!nothing.selects(&old_missing, now));
        let all = Selection {
            all: true,
            ..Selection::default()
        };
        assert!(
            [&old_missing, &old_present, &new_missing, &unknown]
                .iter()
                .all(|p| all.selects(p, now))
        );
        let missing = Selection {
            missing: true,
            ..Selection::default()
        };
        assert!(missing.selects(&old_missing, now));
        assert!(missing.selects(&new_missing, now));
        assert!(!missing.selects(&old_present, now));
        // A workspace whose path is unknown is never "missing".
        assert!(!missing.selects(&unknown, now));
        let old = Selection {
            older_than: Some(Duration::from_secs(7 * DAY)),
            ..Selection::default()
        };
        assert!(old.selects(&old_present, now));
        assert!(old.selects(&unknown, now));
        assert!(!old.selects(&new_missing, now));
        // Exactly the age counts as old; a clock behind the record is not.
        assert!(old.selects(&project(now - 7 * DAY, None), now));
        assert!(!old.selects(&project(now + DAY, None), now));
        let both = Selection {
            missing: true,
            ..old
        };
        assert!(both.selects(&old_missing, now));
        assert!(!both.selects(&new_missing, now));
        assert!(!both.selects(&old_present, now));
    }

    fn fixture(name: &str) -> std::io::Result<(PathBuf, PathBuf, String)> {
        let cache =
            std::env::temp_dir().join(format!("sail-projects-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);
        let workspace = cache.join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let key = super::super::digest(workspace.as_os_str().as_encoded_bytes());
        let directory = root(&cache).join(&key);
        let family = directory.join("compiler-artifacts/family");
        std::fs::create_dir_all(family.join("target/debug"))?;
        std::fs::write(family.join("target/debug/lib.rlib"), vec![0u8; 1000])?;
        std::fs::write(family.join("ownership.lock"), [])?;
        let run = directory.join("context/runs/run-1");
        std::fs::create_dir_all(&run)?;
        std::fs::create_dir_all(directory.join("context/receipts"))?;
        std::fs::write(directory.join("context/receipts/r.json"), "{}")?;
        std::fs::write(
            run.join("result.json"),
            serde_json::json!({ "workspace": workspace }).to_string(),
        )?;
        Ok((cache, workspace, key))
    }

    #[test]
    fn lists_path_existence_and_size() -> Result<(), Box<dyn std::error::Error>> {
        let (cache, workspace, key) = fixture("list")?;
        std::fs::create_dir_all(root(&cache).join(format!("{TRASH}leftover")))?;
        let listed = list(&cache)?;
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0].key, key);
        assert_eq!(listed[0].workspace.as_deref(), Some(workspace.as_path()));
        assert_eq!(listed[0].exists, Some(true));
        assert!(listed[0].bytes >= 1000, "{listed:?}");
        assert!(listed[0].last_used_unix > 0);
        std::fs::remove_dir_all(&workspace)?;
        assert_eq!(list(&cache)?[0].exists, Some(false));
        // A recorded path that does not match the key is not trusted.
        std::fs::write(
            root(&cache)
                .join(&key)
                .join("context/runs/run-1/result.json"),
            r#"{"workspace":"/elsewhere"}"#,
        )?;
        assert_eq!(list(&cache)?[0].workspace, None);
        assert_eq!(list(&cache)?[0].exists, None);
        assert!(list(&cache.join("absent"))?.is_empty());
        std::fs::remove_dir_all(cache)?;
        Ok(())
    }

    #[test]
    fn prune_skips_busy_workspaces_and_removes_idle_ones() -> Result<(), Box<dyn std::error::Error>>
    {
        let (cache, _, key) = fixture("prune")?;
        let directory = root(&cache).join(&key);
        // A run of this version holds the workspace lock shared.
        let run = lock(
            &directory.join(WORKSPACE_LOCK),
            true,
            std::time::Instant::now(),
        )?;
        assert_eq!(prune(&cache, &key)?, Pruned::Busy);
        drop(run);
        // A run of an earlier version holds only its family's lock.
        let family = File::open(directory.join("compiler-artifacts/family/ownership.lock"))?;
        family.lock()?;
        assert_eq!(prune(&cache, &key)?, Pruned::Busy);
        assert!(directory.join("context/receipts/r.json").is_file());
        drop(family);
        let Pruned::Removed(bytes) = prune(&cache, &key)? else {
            return Err("idle workspace not pruned".into());
        };
        assert!(bytes >= 1000);
        assert!(!directory.exists());
        assert!(
            std::fs::read_dir(root(&cache))?.next().is_none(),
            "trash left behind"
        );
        assert_eq!(prune(&cache, &key)?, Pruned::Removed(0));
        assert!(prune(&cache, "../escape").is_err());
        std::fs::remove_dir_all(cache)?;
        Ok(())
    }

    #[test]
    fn a_waiting_run_takes_the_new_lock_after_a_prune() -> Result<(), Box<dyn std::error::Error>> {
        let (cache, _, key) = fixture("waiter")?;
        let directory = root(&cache).join(&key);
        let path = directory.join(WORKSPACE_LOCK);
        // Stand in for a prune: hold the lock exclusively, move the directory
        // away while a run waits on the old lock file, then release it.
        let pruning = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        pruning.lock()?;
        let waiter = std::thread::spawn({
            let path = path.clone();
            move || {
                lock(
                    &path,
                    true,
                    std::time::Instant::now() + Duration::from_secs(30),
                )
                .map(|file| is_at(&file, &path))
                .map_err(|e| e.to_string())
            }
        });
        std::thread::sleep(Duration::from_millis(200));
        assert!(!waiter.is_finished());
        std::fs::rename(&directory, root(&cache).join(format!("{TRASH}test")))?;
        drop(pruning);
        assert_eq!(waiter.join().map_err(|_| "waiter panicked")?, Ok(true));
        assert!(
            path.is_file(),
            "the waiter must recreate the workspace lock"
        );
        std::fs::remove_dir_all(cache)?;
        Ok(())
    }
}
