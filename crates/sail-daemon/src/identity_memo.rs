//! Solver identity reuse under a settled complete stat stamp (ADR 0082).
//!
//! A handshake identifies its solver by the version it prints and the
//! SHA-256 of its executable. Both are a function of the executable, so a
//! settled executable whose complete stamp (device, inode, size, mode,
//! mtime and ctime with nanoseconds) is unchanged keeps its identity. Equal
//! path, length and mtime alone do not establish that (#213): a same-path
//! replacement changes the inode and ctime, and an in-place rewrite, even
//! one that restores mtime with `utimes`, changes ctime, which no user
//! program can set. An identity is recorded only when the stamp taken
//! before the fresh identification equals the one after it and its ctime
//! is [`SETTLED`](sail_api::stamp::SETTLED) older than the start of that
//! identification, so a later change cannot share the recorded stamp even
//! on a coarse clock. Interpreter scripts (`#!`) are never recorded: what
//! they print can depend on files the stamp does not cover.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use sail_api::stamp::Stamp;

/// Distinct solver paths retained at once.
const ENTRIES: usize = 8;

/// A solver's identity: its version output and executable digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Known {
    pub(crate) version: String,
    pub(crate) digest: [u8; 32],
}

/// What [`IdentityMemo::lookup`] found.
pub(crate) enum Found {
    /// The settled executable still has the recorded stamp.
    Hit(Known),
    /// Identify afresh, then offer the result to [`IdentityMemo::record`]
    /// with this observation.
    Miss(Observation),
}

/// When a fresh identification started and the stamp it started from.
pub(crate) struct Observation {
    start: SystemTime,
    before: Option<Stamp>,
}

/// Recently identified solvers, by absolute path and complete stamp.
pub(crate) struct IdentityMemo {
    entries: Mutex<Vec<(PathBuf, Stamp, Known)>>,
}

fn stamp(path: &Path) -> Option<Stamp> {
    std::fs::metadata(path).ok().map(|m| Stamp::of(&m))
}

fn is_script(path: &Path) -> bool {
    let mut head = [0u8; 2];
    match std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)) {
        Ok(()) => head == *b"#!",
        // Shorter than two bytes cannot be a native executable either.
        Err(_) => true,
    }
}

impl IdentityMemo {
    pub(crate) fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// The recorded identity of `path`, if its current stamp is exactly
    /// the recorded one. Relative paths, unreadable metadata and a poisoned
    /// memo all miss.
    pub(crate) fn lookup(&self, path: &Path) -> Found {
        let start = SystemTime::now();
        let before = if path.is_absolute() {
            stamp(path)
        } else {
            None
        };
        if let Some(now) = before
            && let Ok(entries) = self.entries.lock()
            && let Some((_, _, known)) = entries.iter().find(|(p, s, _)| p == path && *s == now)
        {
            return Found::Hit(known.clone());
        }
        Found::Miss(Observation { start, before })
    }

    /// Offers a fresh identification of `path` that began at `observed`.
    /// It is retained only if the stamp did not change across it, that
    /// stamp is settled, and the executable is not an interpreter script.
    pub(crate) fn record(&self, path: &Path, observed: &Observation, known: &Known) {
        let Some(before) = observed.before else {
            return;
        };
        if !before.settled(observed.start) || is_script(path) || stamp(path) != Some(before) {
            return;
        }
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        entries.retain(|(p, _, _)| p != path);
        if entries.len() >= ENTRIES {
            entries.remove(0);
        }
        entries.push((path.to_owned(), before, known.clone()));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;
    use std::time::Duration;

    fn known(tag: u8) -> Known {
        Known {
            version: format!("4.16.{tag}"),
            digest: [tag; 32],
        }
    }

    /// Identifies `path` as `as_known` through the memo, as the server does.
    fn identify(memo: &IdentityMemo, path: &Path, as_known: &Known) -> bool {
        match memo.lookup(path) {
            Found::Hit(found) => {
                assert_eq!(&found, as_known);
                true
            }
            Found::Miss(observed) => {
                memo.record(path, &observed, as_known);
                false
            }
        }
    }

    fn executable(path: &Path, body: &[u8]) -> std::io::Result<()> {
        std::fs::write(path, body)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
    }

    fn set_mtime(path: &Path, mtime: SystemTime) -> std::io::Result<()> {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)?
            .set_times(std::fs::FileTimes::new().set_modified(mtime))
    }

    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(tag: &str) -> std::io::Result<Scratch> {
        let dir = std::env::temp_dir().join(format!("sail-id-memo-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Scratch(dir))
    }

    #[test]
    fn settled_unchanged_executables_hit_and_any_rewrite_or_replacement_misses()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = scratch("settled")?;
        let (unchanged, rewritten, replaced) = (
            dir.0.join("unchanged"),
            dir.0.join("rewritten"),
            dir.0.join("replaced"),
        );
        let old = b"\x7fELF native solver A";
        for path in [&unchanged, &rewritten, &replaced] {
            executable(path, old)?;
        }
        // Real elapsed time, so the recorded ctimes are genuinely settled
        // and later changes genuinely move ctime on any clock granularity.
        std::thread::sleep(sail_api::stamp::SETTLED + Duration::from_millis(100));
        let memo = IdentityMemo::new();
        for path in [&unchanged, &rewritten, &replaced] {
            assert!(!identify(&memo, path, &known(1)), "first identification");
            assert!(identify(&memo, path, &known(1)), "settled and unchanged");
        }

        // In place: same inode, size and restored mtime; ctime moves.
        let mtime = rewritten.metadata()?.modified()?;
        std::fs::write(&rewritten, b"\x7fELF native solver B")?;
        set_mtime(&rewritten, mtime)?;
        assert_eq!(rewritten.metadata()?.modified()?, mtime);
        assert_eq!(rewritten.metadata()?.len(), old.len() as u64);
        assert!(
            !identify(&memo, &rewritten, &known(2)),
            "rewrite re-identified"
        );

        // Same path, size and mtime by rename: a new inode and ctime.
        let mtime = replaced.metadata()?.modified()?;
        let staged = dir.0.join("staged");
        executable(&staged, b"\x7fELF native solver C")?;
        set_mtime(&staged, mtime)?;
        std::fs::rename(&staged, &replaced)?;
        assert_eq!(replaced.metadata()?.modified()?, mtime);
        assert!(
            !identify(&memo, &replaced, &known(3)),
            "replacement re-identified"
        );

        // Both changed files are now too recent to record again.
        assert!(!identify(&memo, &rewritten, &known(2)));
        assert!(!identify(&memo, &replaced, &known(3)));
        assert!(identify(&memo, &unchanged, &known(1)));

        // Permission and removal changes miss; the caller's fresh launch
        // then refuses the handshake.
        std::fs::set_permissions(&unchanged, std::fs::Permissions::from_mode(0o600))?;
        assert!(!identify(&memo, &unchanged, &known(1)));
        std::fs::remove_file(&unchanged)?;
        assert!(!identify(&memo, &unchanged, &known(1)));
        Ok(())
    }

    #[test]
    fn recent_scripts_relative_and_unstable_identifications_are_not_recorded()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = scratch("unrecorded")?;
        let memo = IdentityMemo::new();

        // A ctime less than two seconds old is never recorded.
        let recent = dir.0.join("recent");
        executable(&recent, b"\x7fELF native")?;
        assert!(!identify(&memo, &recent, &known(1)));
        assert!(!identify(&memo, &recent, &known(1)));

        // A settled observation is simulated by a later start time.
        let later = SystemTime::now() + Duration::from_secs(3);
        let settled = |path: &Path| Observation {
            start: later,
            before: stamp(path),
        };
        let script = dir.0.join("script");
        executable(&script, b"#!/bin/sh\necho 'Z3 version 4.16.0'\n")?;
        memo.record(&script, &settled(&script), &known(1));
        assert!(matches!(memo.lookup(&script), Found::Miss(_)), "script");

        // A change during identification leaves the before stamp stale.
        let moving = dir.0.join("moving");
        executable(&moving, b"\x7fELF native")?;
        let observed = settled(&moving);
        std::fs::write(&moving, b"\x7fELF other, longer")?;
        memo.record(&moving, &observed, &known(1));
        assert!(matches!(memo.lookup(&moving), Found::Miss(_)), "changed");

        let stable = dir.0.join("stable");
        executable(&stable, b"\x7fELF native")?;
        memo.record(&stable, &settled(&stable), &known(1));
        assert!(matches!(memo.lookup(&stable), Found::Hit(k) if k == known(1)));

        let relative = Path::new("stable");
        memo.record(relative, &settled(&stable), &known(1));
        assert!(matches!(memo.lookup(relative), Found::Miss(_)), "relative");
        Ok(())
    }

    #[test]
    fn a_hit_skips_the_launch_and_a_restored_mtime_rewrite_identifies_afresh()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::digest_memo::{DigestMemo, MAX_RETAINED_BYTES};
        use crate::server::identify;
        let dir = scratch("server")?;
        // Not launchable: only a memo hit can identify it.
        let solver = dir.0.join("solver");
        executable(&solver, b"\x7fELF not really an executable")?;
        let (digests, memo) = (DigestMemo::new(MAX_RETAINED_BYTES), IdentityMemo::new());
        assert!(
            identify(&solver, &digests, &memo).is_err(),
            "fresh launch fails"
        );
        let observed = Observation {
            start: SystemTime::now() + Duration::from_secs(3),
            before: stamp(&solver),
        };
        memo.record(&solver, &observed, &known(7));
        let hit = identify(&solver, &digests, &memo)?;
        assert_eq!(hit.version, known(7).version);
        assert_eq!(hit.binary_sha256, "07".repeat(32));

        // #213: same bytes length, restored mtime; ctime moves, so the
        // handshake identifies afresh and here refuses.
        // The settle rule was simulated above; let real time pass a coarse
        // ctime tick so the rewrite's ctime differs.
        std::thread::sleep(Duration::from_millis(50));
        let mtime = solver.metadata()?.modified()?;
        std::fs::write(&solver, b"\x7fELF not really an executablf")?;
        set_mtime(&solver, mtime)?;
        assert!(identify(&solver, &digests, &memo).is_err(), "re-identified");
        Ok(())
    }

    #[test]
    fn the_memo_is_bounded_and_keeps_one_entry_per_path() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = scratch("bounded")?;
        let memo = IdentityMemo::new();
        let later = SystemTime::now() + Duration::from_secs(3);
        let paths: Vec<_> = (0..=ENTRIES).map(|i| dir.0.join(format!("s{i}"))).collect();
        for path in &paths {
            executable(path, b"\x7fELF native")?;
            let observed = Observation {
                start: later,
                before: stamp(path),
            };
            memo.record(path, &observed, &known(1));
            memo.record(path, &observed, &known(2));
        }
        let entries = memo.entries.lock().map(|e| e.len()).unwrap_or_default();
        assert_eq!(entries, ENTRIES);
        assert!(matches!(memo.lookup(&paths[0]), Found::Miss(_)), "evicted");
        assert!(matches!(memo.lookup(&paths[ENTRIES]), Found::Hit(k) if k == known(2)));
        Ok(())
    }
}
