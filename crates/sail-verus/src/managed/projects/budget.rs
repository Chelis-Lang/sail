//! Opt-in, soft byte budgets over complete managed workspaces.
//!
//! Eviction uses the same nonblocking locks and atomic whole-workspace removal
//! as explicit pruning. No scheduling record is proof authority.

use std::io::{self, Read as _};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Project, Pruned};

const POLICY: &str = "project-budget.json";
const KIND: &str = "sail-project-budget";
const MAX_POLICY_BYTES: u64 = 4096;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    kind: String,
    version: u32,
    max_bytes: Option<u64>,
}

/// Read a configured soft cap. An absent policy disables automatic eviction.
///
/// # Errors
/// Returns an error for unreadable, aliased, oversized or invalid policies.
pub fn load(cache: &Path) -> io::Result<Option<u64>> {
    let path = cache.join(POLICY);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() > MAX_POLICY_BYTES {
        return Err(io::Error::other("invalid project budget policy file"));
    }
    let file = std::fs::File::open(&path)?;
    let mut bytes = Vec::new();
    (&file).take(MAX_POLICY_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_POLICY_BYTES
        || !super::is_at(&file, &path)
        || !std::fs::symlink_metadata(path)?.is_file()
    {
        return Err(io::Error::other("project budget policy changed while read"));
    }
    let policy: Policy = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if policy.kind != KIND || policy.version != 1 || policy.max_bytes == Some(0) {
        return Err(io::Error::other("invalid project budget policy"));
    }
    Ok(policy.max_bytes)
}

/// Atomically set the soft cap, or disable it with `None`.
///
/// # Errors
/// Returns an error for zero caps or failed atomic publication.
pub fn save(cache: &Path, max_bytes: Option<u64>) -> io::Result<()> {
    if max_bytes == Some(0) {
        return Err(io::Error::other("project budget must be greater than zero"));
    }
    std::fs::create_dir_all(cache)?;
    super::super::files::json(
        &cache.join(POLICY),
        &Policy {
            kind: KIND.into(),
            version: 1,
            max_bytes,
        },
    )
    .map_err(io::Error::other)
}

/// Outcome of one budget pass, including soft-cap overage and skipped data.
#[derive(Debug, Serialize)]
pub struct Report {
    /// Requested cap in accounted workspace bytes.
    pub max_bytes: u64,
    /// Whether deletion was only simulated.
    pub dry_run: bool,
    /// Bytes accounted before the pass.
    pub before_bytes: u64,
    /// Bytes remaining, or projected to remain during a dry run.
    pub remaining_bytes: u64,
    /// Accounted bytes removed, or selected in a dry run.
    pub freed_bytes: u64,
    /// Whether protected, busy or failed data leave the cache above its cap.
    pub over_budget: bool,
    /// Most recently used workspace, protected from budget eviction.
    pub protected: Option<String>,
    /// Complete workspace records removed or selected for removal.
    pub removed: Vec<Project>,
    /// Workspaces in use, or used again since they were listed.
    pub busy: Vec<Project>,
    /// Failures retained alongside the other outcomes.
    pub failed: Vec<(String, String)>,
}

/// Remove least recently used workspaces until the cap is met.
///
/// The newest workspace is protected even if it alone exceeds the cap. Existing
/// `prune --all` remains the explicit way to remove every idle workspace.
///
/// # Errors
/// Returns an error for a zero cap or failed initial/final inventory.
pub fn enforce(cache: &Path, max_bytes: u64, dry_run: bool) -> io::Result<Report> {
    if max_bytes == 0 {
        return Err(io::Error::other("project budget must be greater than zero"));
    }
    let mut projects = super::list(cache)?;
    projects.sort_by(|a, b| {
        a.last_used_unix
            .cmp(&b.last_used_unix)
            .then_with(|| a.key.cmp(&b.key))
    });
    let before_bytes = projects
        .iter()
        .fold(0u64, |sum, p| sum.saturating_add(p.bytes));
    let protected = projects.last().map(|p| p.key.clone());
    let mut report = Report {
        max_bytes,
        dry_run,
        before_bytes,
        remaining_bytes: before_bytes,
        freed_bytes: 0,
        over_budget: before_bytes > max_bytes,
        protected,
        removed: Vec::new(),
        busy: Vec::new(),
        failed: Vec::new(),
    };
    for project in projects {
        if report.remaining_bytes <= max_bytes {
            break;
        }
        if Some(&project.key) == report.protected.as_ref() {
            continue;
        }
        let outcome = if dry_run {
            Ok(Pruned::Removed(project.bytes))
        } else {
            super::prune_observed(cache, &project.key, Some(project.last_used_unix))
        };
        match outcome {
            Ok(Pruned::Removed(bytes)) => {
                report.freed_bytes = report.freed_bytes.saturating_add(bytes);
                report.remaining_bytes = report.remaining_bytes.saturating_sub(project.bytes);
                report.removed.push(project);
            }
            Ok(Pruned::Busy) => report.busy.push(project),
            Err(error) => report.failed.push((project.key, error.to_string())),
        }
    }
    if !dry_run {
        report.remaining_bytes = super::list(cache)?
            .iter()
            .fold(0u64, |sum, p| sum.saturating_add(p.bytes));
    }
    report.over_budget = report.remaining_bytes > max_bytes;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, FileTimes};
    use std::time::{Duration, UNIX_EPOCH};

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> io::Result<Self> {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root =
                std::env::temp_dir().join(format!("sail-budget-{}-{serial}", std::process::id()));
            std::fs::create_dir_all(&root)?;
            Ok(Self(root))
        }
        fn project(&self, name: char, age: u64) -> io::Result<std::path::PathBuf> {
            let directory = super::super::root(&self.0).join(name.to_string().repeat(64));
            std::fs::create_dir_all(&directory)?;
            std::fs::write(directory.join("artifact"), [0u8; 64])?;
            std::fs::write(directory.join(super::super::WORKSPACE_LOCK), [])?;
            File::open(&directory)?
                .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(age)))?;
            Ok(directory)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn lru_eviction_keeps_newest_and_dry_run_changes_nothing() -> io::Result<()> {
        let fixture = Fixture::new()?;
        let oldest = fixture.project('a', 10)?;
        let middle = fixture.project('b', 20)?;
        let newest = fixture.project('c', 30)?;
        let dry = enforce(&fixture.0, 128, true)?;
        assert_eq!(dry.removed.len(), 1);
        assert!(oldest.is_dir());
        let report = enforce(&fixture.0, 128, false)?;
        assert_eq!(report.remaining_bytes, 128);
        assert!(!report.over_budget);
        assert!(!oldest.exists());
        assert!(middle.is_dir() && newest.is_dir());
        let small = enforce(&fixture.0, 1, false)?;
        assert!(small.over_budget);
        assert!(newest.is_dir());
        Ok(())
    }
    #[test]
    fn a_busy_old_workspace_is_not_removed() -> io::Result<()> {
        let fixture = Fixture::new()?;
        let oldest = fixture.project('a', 10)?;
        let newest = fixture.project('b', 20)?;
        let lock = File::open(oldest.join(super::super::WORKSPACE_LOCK))?;
        lock.lock_shared()?;
        let report = enforce(&fixture.0, 1, false)?;
        assert!(report.over_budget);
        assert_eq!(report.busy.len(), 1);
        assert!(oldest.is_dir() && newest.is_dir());
        Ok(())
    }
    #[test]
    fn policy_is_opt_in_bounded_and_rejects_corruption() -> io::Result<()> {
        let fixture = Fixture::new()?;
        assert_eq!(load(&fixture.0)?, None);
        save(&fixture.0, Some(123))?;
        assert_eq!(load(&fixture.0)?, Some(123));
        save(&fixture.0, None)?;
        assert_eq!(load(&fixture.0)?, None);
        assert!(save(&fixture.0, Some(0)).is_err());
        std::fs::write(fixture.0.join(POLICY), b"{}")?;
        assert!(load(&fixture.0).is_err());
        std::fs::write(fixture.0.join(POLICY), [b' '; 4097])?;
        assert!(load(&fixture.0).is_err());
        Ok(())
    }
}
