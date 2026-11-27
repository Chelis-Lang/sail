//! Actual child and descendant controls for the original-driver capture layer.

use super::*;

fn directory() -> Result<PathBuf, Box<dyn std::error::Error>> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "sail-kani-execution-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::DirBuilder::new().mode(0o700).create(&path)?;
    Ok(path)
}

#[test]
fn completed_child_preserves_exit_and_exact_streams() -> Result<(), Box<dyn std::error::Error>> {
    let root = directory()?;
    let exit = capture(
        Command::new("/bin/sh").args(["-c", "printf original; printf diagnostic >&2; exit 1"]),
        &root,
        Duration::from_secs(5),
        &AtomicBool::new(false),
    )?;
    assert_eq!(exit, 1);
    assert_eq!(read(&root.join("stdout"))?, b"original");
    assert_eq!(read(&root.join("stderr"))?, b"diagnostic");
    assert_eq!(std::fs::read(root.join("exit"))?, b"1\n");
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn cancellation_before_spawn_and_signal_exit_never_complete()
-> Result<(), Box<dyn std::error::Error>> {
    let cancelled = directory()?;
    assert!(
        capture(
            Command::new("/bin/sh").args(["-c", "touch should-not-start"]),
            &cancelled,
            Duration::from_secs(5),
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert!(!cancelled.join("stdout").exists());
    let signalled = directory()?;
    assert!(
        capture(
            Command::new("/bin/sh").args(["-c", "kill -KILL $$"]),
            &signalled,
            Duration::from_secs(5),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(!signalled.join("exit").exists());
    std::fs::remove_dir_all(cancelled)?;
    std::fs::remove_dir_all(signalled)?;
    Ok(())
}

fn inactive(pid: &str) -> Result<bool, std::io::Error> {
    match std::fs::read_to_string(format!("/proc/{}/stat", pid.trim())) {
        Ok(stat) => Ok(stat
            .rsplit_once(')')
            .is_some_and(|(_, fields)| fields.split_whitespace().next() == Some("Z"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

#[test]
fn timeout_and_leader_exit_stop_inherited_descendants() -> Result<(), Box<dyn std::error::Error>> {
    for leader_exits in [false, true] {
        let root = directory()?;
        let script = if leader_exits {
            "sleep 30 & echo $! > child; exit 0"
        } else {
            "sleep 30 & echo $! > child; wait"
        };
        let result = capture(
            Command::new("/bin/sh")
                .args(["-c", script])
                .current_dir(&root),
            &root,
            Duration::from_millis(200),
            &AtomicBool::new(false),
        );
        assert_eq!(result.is_ok(), leader_exits);
        let pid = std::fs::read_to_string(root.join("child"))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while !inactive(&pid)? && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(inactive(&pid)?, "inherited child remains active");
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

#[test]
fn excessive_or_nonregular_capture_never_completes() -> Result<(), Box<dyn std::error::Error>> {
    for script in ["head -c 16777217 /dev/zero", "mkfifo export.json; sleep 30"] {
        let root = directory()?;
        assert!(
            capture(
                Command::new("/bin/sh")
                    .args(["-c", script])
                    .current_dir(&root),
                &root,
                Duration::from_secs(5),
                &AtomicBool::new(false),
            )
            .is_err()
        );
        std::fs::remove_dir_all(root)?;
    }
    Ok(())
}

#[test]
fn live_cancellation_stops_the_owned_group() -> Result<(), Box<dyn std::error::Error>> {
    let root = directory()?;
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let flag = std::sync::Arc::clone(&cancelled);
    let observed = root.join("child");
    let watcher = std::thread::spawn(move || -> std::io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !observed.exists() {
            if Instant::now() >= deadline {
                return Err(std::io::Error::other("child did not start"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        flag.store(true, Ordering::SeqCst);
        Ok(())
    });
    let result = capture(
        Command::new("/bin/sh")
            .args(["-c", "sleep 30 & echo $! > child; wait"])
            .current_dir(&root),
        &root,
        Duration::from_secs(5),
        &cancelled,
    );
    watcher
        .join()
        .map_err(|_| std::io::Error::other("cancellation watcher panicked"))??;
    assert!(result.is_err());
    let pid = std::fs::read_to_string(root.join("child"))?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while !inactive(&pid)? && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(inactive(&pid)?);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
