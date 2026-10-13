//! Bounded concurrent job scheduling with one running job per Cargo workspace.
//!
//! Independent jobs run on up to `SAIL_JOB_WORKERS` workers. Two jobs whose
//! project directories resolve to the same workspace key never run at the same
//! time: the later one stays queued, in FIFO order, until the earlier one ends.
//! The managed runner's target and compiler-family locks are per Cargo
//! workspace, so this keeps one daemon's jobs from contending for them.

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

/// Upper bound accepted from `SAIL_JOB_WORKERS`.
const MAX_WORKERS: usize = 64;

/// Queued job ids with their workspace keys, and the keys now running.
#[derive(Default)]
pub(super) struct Queue {
    pending: VecDeque<(String, PathBuf)>,
    running: BTreeSet<PathBuf>,
}

impl Queue {
    /// Appends a job behind every earlier job.
    pub(super) fn push(&mut self, id: String, key: PathBuf) {
        self.pending.push_back((id, key));
    }

    /// Takes the oldest job whose workspace is idle and marks it running.
    pub(super) fn take(&mut self) -> Option<(String, PathBuf)> {
        let index = self
            .pending
            .iter()
            .position(|(_, key)| !self.running.contains(key))?;
        let (id, key) = self.pending.remove(index)?;
        self.running.insert(key.clone());
        Some((id, key))
    }

    /// Marks a workspace idle again.
    pub(super) fn finish(&mut self, key: &Path) {
        self.running.remove(key);
    }
}

/// Number of concurrent verification workers: `SAIL_JOB_WORKERS` when it is a
/// whole number from 1 to 64, otherwise a quarter of the available cores,
/// between 1 and 4. Each job runs Verus and Z3 with their own parallelism, so
/// the default stays small.
pub(super) fn workers() -> usize {
    std::env::var("SAIL_JOB_WORKERS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|count| (1..=MAX_WORKERS).contains(count))
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map_or(1, std::num::NonZero::get)
                .div_euclid(4)
                .clamp(1, 4)
        })
}

/// The directory that identifies a job's Cargo workspace: the outermost
/// ancestor of the project (or of its `--manifest-path`) holding a
/// `Cargo.toml`. Choosing the outermost manifest can only serialise more,
/// never less, than Cargo's own workspace resolution.
pub(super) fn workspace_key(project: &Path, args: &[String]) -> PathBuf {
    let start = manifest_dir(project, args).unwrap_or_else(|| project.to_path_buf());
    let start = start.canonicalize().unwrap_or(start);
    start
        .ancestors()
        .filter(|dir| dir.join("Cargo.toml").is_file())
        .last()
        .map_or(start.clone(), Path::to_path_buf)
}

fn manifest_dir(project: &Path, args: &[String]) -> Option<PathBuf> {
    let mut iter = args.iter().take_while(|arg| *arg != "--");
    let manifest = loop {
        let arg = iter.next()?;
        if let Some(value) = arg.strip_prefix("--manifest-path=") {
            break value.to_owned();
        }
        if arg == "--manifest-path" {
            break iter.next()?.clone();
        }
    };
    Some(project.join(manifest).parent()?.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_workspace_waits_while_others_run_in_order() {
        let mut queue = Queue::default();
        queue.push("a1".into(), "/a".into());
        queue.push("a2".into(), "/a".into());
        queue.push("b1".into(), "/b".into());
        assert_eq!(queue.take().map(|(id, _)| id).as_deref(), Some("a1"));
        assert_eq!(queue.take().map(|(id, _)| id).as_deref(), Some("b1"));
        assert!(queue.take().is_none());
        queue.finish(Path::new("/a"));
        assert_eq!(queue.take().map(|(id, _)| id).as_deref(), Some("a2"));
    }

    #[test]
    fn workspace_key_is_the_outermost_manifest() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-schedule-{}", std::process::id()));
        let member = root.join("outer").join("member");
        std::fs::create_dir_all(member.join("src"))?;
        std::fs::write(root.join("outer").join("Cargo.toml"), "[workspace]\n")?;
        std::fs::write(member.join("Cargo.toml"), "[package]\n")?;
        let outer = root.join("outer").canonicalize()?;
        let canonical_root = root.canonicalize()?;
        let keys = (
            workspace_key(&member.join("src"), &[]),
            workspace_key(
                &root,
                &["--manifest-path".into(), "outer/member/Cargo.toml".into()],
            ),
            workspace_key(&root, &[]),
        );
        std::fs::remove_dir_all(&root)?;
        assert_eq!(keys.0, outer);
        assert_eq!(keys.1, outer);
        assert_eq!(keys.2, canonical_root);
        Ok(())
    }
}
