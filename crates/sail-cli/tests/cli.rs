//! Smoke tests for the `sail` binary's command surface.

use std::process::{Command, Output};

fn sail(args: &[&str]) -> Output {
    match Command::new(env!("CARGO_BIN_EXE_sail")).args(args).output() {
        Ok(output) => output,
        Err(e) => unreachable!("cannot run sail: {e}"),
    }
}

#[test]
fn help_lists_every_charter_command() {
    let output = sail(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8_lossy(&output.stdout);
    for cmd in [
        "verify", "watch", "mcp", "serve", "status", "cache", "bench",
    ] {
        assert!(text.contains(cmd), "help is missing `{cmd}`:\n{text}");
    }
}

#[test]
fn bench_delegates_to_the_benchmark_binary() {
    let output = sail(&["bench", "--help"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("history-replay") && help.contains("agent-loop"),
        "{help}"
    );
}

#[test]
fn unknown_command_is_a_usage_error() {
    let output = sail(&["frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SAIL0002"));
}

#[test]
fn no_arguments_is_a_usage_error() {
    assert_eq!(sail(&[]).status.code(), Some(2));
}

/// Without an installed engine, `engine status` and `repair-check` name the
/// missing file and how to obtain the engine, and `repair-check` fails before
/// it starts a daemon or submits a job (no infrastructure-error job is left).
#[test]
fn missing_engine_is_reported_up_front() {
    let root = std::env::temp_dir().join(format!("sail-noengine-{}", std::process::id()));
    let cache = root.join("cache");
    let _ = std::fs::remove_dir_all(&root);
    assert!(std::fs::create_dir_all(&root).is_ok());
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_sail"))
            .args(args)
            .current_dir(&root)
            .env("SAIL_CACHE_DIR", &cache)
            .env_remove("SAIL_DAEMON_SOCKET")
            .output()
            .unwrap_or_else(|e| unreachable!("cannot run sail: {e}"))
    };
    for args in [
        &["engine", "status"][..],
        &[
            "repair-check",
            "--engine",
            "interactive",
            "-p",
            "demo",
            "--lib",
            "--function",
            "demo::max",
        ][..],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.starts_with("error[SAIL3120]: the optional interactive engine is not installed")
                && stderr.contains("sail-build.json")
                && stderr.contains("cargo xtask build-verus-engine")
                && stderr.contains("sail engine install --from PATH")
                && !stderr.contains("private"),
            "{args:?}: {stderr}"
        );
    }
    // No daemon was started and no job was recorded.
    assert!(!cache.join("results/projects/jobs").exists());
    let _ = std::fs::remove_dir_all(&root);
}
