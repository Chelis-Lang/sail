//! `sail watch`: re-verify as files change.
//!
//! It supervises `sail verify` with the same arguments. When a watched file
//! changes (Rust sources and `Cargo.toml` under the current directory,
//! skipping `target/` and hidden directories), any in-flight run is cancelled
//! by killing its whole process group (`cargo verus`, Verus, the shims and
//! their solvers), and a new run starts once the edits settle. The cache makes
//! each rerun cheap: unchanged buckets are served, and only edited ones are
//! re-solved (ADR 0008, 0011).
//!
//! Cancelling is always safe. A killed shim stores only complete replies, so
//! cancellation costs only time (`docs/fault-injection.md`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant, SystemTime};

/// How often the tree is polled for changes.
const POLL: Duration = Duration::from_millis(250);
/// How long edits must be quiet before a run starts.
const SETTLE: Duration = Duration::from_millis(300);

/// A file's last observed state.
type Stamp = (Option<SystemTime>, u64);

/// The watched files under `root`, with their modification time and size.
pub(crate) fn snapshot(root: &Path) -> BTreeMap<PathBuf, Stamp> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                if !name.starts_with('.') && name != "target" {
                    stack.push(path);
                }
            } else if name.ends_with(".rs") || name == "Cargo.toml" {
                out.insert(path, (meta.modified().ok(), meta.len()));
            }
        }
    }
    out
}

/// Kills the process group led by `child` (it was started as a group
/// leader), then reaps it. `kill -s KILL -- -PGID` is the unambiguous POSIX
/// form: without `--`, some `kill` implementations read `-PGID` as an option.
fn cancel(child: &mut Child) {
    let pgid = child.id();
    let _ = Command::new("kill")
        .args(["-s", "KILL", "--", &format!("-{pgid}")])
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

fn start(args: &[String]) -> std::io::Result<Child> {
    let me = std::env::current_exe()?;
    let mut cmd = Command::new(me);
    cmd.arg("verify").args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Its own process group, so cancelling reaches every descendant.
        cmd.process_group(0);
    }
    cmd.spawn()
}

/// Runs `sail verify <args>` now and again after every settled change, until
/// interrupted.
pub fn watch(args: &[String], err: &mut impl std::io::Write) -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            let _ = writeln!(
                err,
                "error[SAIL0201]: sail watch: no current directory: {e}"
            );
            return ExitCode::from(2);
        }
    };
    // On Ctrl-C, kill the in-flight run's process group, then exit.
    let stop = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicU32::new(0));
    {
        let (stop, running) = (Arc::clone(&stop), Arc::clone(&running));
        let _ = ctrlc::set_handler(move || {
            stop.store(true, Ordering::SeqCst);
            let pgid = running.load(Ordering::SeqCst);
            if pgid != 0 {
                let _ = Command::new("kill")
                    .args(["-s", "KILL", "--", &format!("-{pgid}")])
                    .status();
            }
        });
    }

    let mut seen = snapshot(&root);
    let mut run = 0u64;
    let mut child: Option<Child> = None;
    let mut pending = Some(Instant::now());
    while !stop.load(Ordering::SeqCst) {
        let now = snapshot(&root);
        if now != seen {
            seen = now;
            pending = Some(Instant::now());
            if let Some(mut c) = child.take() {
                let _ = writeln!(
                    err,
                    "sail watch: change detected; cancelling run {run} (process group {})",
                    c.id()
                );
                cancel(&mut c);
                running.store(0, Ordering::SeqCst);
            }
        }
        if pending.is_some_and(|t| t.elapsed() >= SETTLE) {
            pending = None;
            run += 1;
            let _ = writeln!(err, "sail watch: run {run}");
            match start(args) {
                Ok(c) => {
                    running.store(c.id(), Ordering::SeqCst);
                    child = Some(c);
                }
                Err(e) => {
                    let _ = writeln!(
                        err,
                        "error[SAIL0202]: sail watch: cannot start sail verify: {e}"
                    );
                }
            }
        }
        if let Some(c) = child.as_mut()
            && let Ok(Some(status)) = c.try_wait()
        {
            let _ = writeln!(
                err,
                "sail watch: run {run} finished ({}); watching for changes",
                status
                    .code()
                    .map_or_else(|| "killed".to_owned(), |c| format!("exit {c}"))
            );
            child = None;
            running.store(0, Ordering::SeqCst);
        }
        std::thread::sleep(POLL);
    }
    if let Some(mut c) = child.take() {
        cancel(&mut c);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::snapshot;

    #[test]
    fn snapshot_sees_sources_and_skips_targets_and_hidden_dirs() {
        let root = std::env::temp_dir().join(format!("sail-watch-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["src", "target/debug", ".git"] {
            assert!(std::fs::create_dir_all(root.join(dir)).is_ok());
        }
        for (file, text) in [
            ("src/lib.rs", "fn a() {}"),
            ("Cargo.toml", "[package]"),
            ("target/debug/gen.rs", "x"),
            (".git/x.rs", "x"),
            ("notes.txt", "x"),
        ] {
            assert!(std::fs::write(root.join(file), text).is_ok());
        }
        let before = snapshot(&root);
        let names: Vec<String> = before
            .keys()
            .filter_map(|p| p.strip_prefix(&root).ok())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Cargo.toml", "src/lib.rs"]);
        assert!(std::fs::write(root.join("src/lib.rs"), "fn a() { }").is_ok());
        assert_ne!(snapshot(&root), before);
        let _ = std::fs::remove_dir_all(&root);
    }
}
