//! Smoke tests for the `sail` binary's command surface.

use std::io::Write as _;
use std::process::{Command, Output, Stdio};

fn sail(args: &[&str]) -> Output {
    match Command::new(env!("CARGO_BIN_EXE_sail")).args(args).output() {
        Ok(output) => output,
        Err(e) => unreachable!("cannot run sail: {e}"),
    }
}

/// The user help lists the supported commands. `sail bench` needs
/// `sail-bench`, which only a source build has, and experimental commands
/// are not part of the supported set, so the help leaves them out;
/// `repair-check` stays listed, marked experimental.
#[test]
fn help_lists_every_user_command() {
    let output = sail(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8_lossy(&output.stdout);
    let listed: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("    "))
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    for cmd in [
        "new",
        "verify",
        "repair-check",
        "engine",
        "watch",
        "jobs",
        "mcp",
        "status",
        "cache",
    ] {
        assert!(listed.contains(&cmd), "help is missing `{cmd}`:\n{text}");
    }
    for cmd in [
        "bench",
        "explain",
        "baseline",
        "compare",
        "serve",
        "fragility",
    ] {
        assert!(!listed.contains(&cmd), "help lists `{cmd}`:\n{text}");
    }
    assert!(text.contains("repair-check  (experimental)"), "{text}");
}

/// `--version` reports the build's `SAIL_VERSION` when set (release builds),
/// else the crate version.
#[test]
fn version_reports_the_build_version() {
    let output = sail(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    let expected = match option_env!("SAIL_VERSION") {
        Some(version) if !version.is_empty() => version,
        _ => env!("CARGO_PKG_VERSION"),
    };
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("sail {expected}\n")
    );
    assert_eq!(expected, sail_api::VERSION, "reports state this version");
}

/// The MCP server states the version `sail --version` prints (#180).
#[test]
fn mcp_server_states_the_cli_version() {
    let cache = std::env::temp_dir().join(format!("sail-cli-version-{}", std::process::id()));
    let child = Command::new(env!("CARGO_BIN_EXE_sail"))
        .arg("mcp")
        .env("SAIL_CACHE_DIR", &cache)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        unreachable!("cannot run sail mcp: {child:?}")
    };
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
    if let Some(mut stdin) = child.stdin.take() {
        assert!(writeln!(stdin, "{init}").is_ok());
    }
    let output = child.wait_with_output();
    let _ = std::fs::remove_dir_all(&cache);
    let Ok(output) = output else {
        unreachable!("sail mcp failed: {output:?}")
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let reply: serde_json::Value = text
        .lines()
        .next()
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_default();
    let version = String::from_utf8_lossy(&sail(&["--version"]).stdout).into_owned();
    let served = reply["result"]["serverInfo"]["version"]
        .as_str()
        .unwrap_or("(none)");
    assert_eq!(format!("sail {served}\n"), version, "{text}");
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

/// ADR 0043: Sail runs only its own Verus. Without it, `sail verify` fails
/// with SAIL0101 and never runs a `cargo-verus` found on `PATH`; the explicit
/// development override is used instead, announced by warning SAIL0108.
/// (Managed verification identifies the override's toolchain before it runs
/// it, through the `verus` launcher beside it, which this stand-in lacks.)
#[cfg(unix)]
#[test]
fn verify_never_runs_a_cargo_verus_from_path() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::env::temp_dir().join(format!("sail-nostock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let bin = root.join("bin");
    assert!(std::fs::create_dir_all(&bin).is_ok());
    let marker = root.join("ran");
    // Keep this engine-selection fixture in the supported managed setup;
    // overlapping package/cache roots deliberately take the plain fallback.
    let cache = root.with_extension("cache");
    let _ = std::fs::remove_dir_all(&cache);
    // A Verus project, so verification gets as far as choosing a Verus.
    assert!(std::fs::create_dir_all(root.join("src")).is_ok());
    assert!(
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[package.metadata.verus]\nverify = true\n\n[workspace]\n",
        )
        .is_ok()
    );
    assert!(std::fs::write(root.join("src/lib.rs"), "").is_ok());
    // A copy of `sail` with a stand-in `sail-z3` beside it, so setup succeeds
    // whether or not the shim was built.
    let sail = root.join("sail-bin").join("sail");
    assert!(std::fs::create_dir_all(root.join("sail-bin")).is_ok());
    assert!(std::fs::copy(env!("CARGO_BIN_EXE_sail"), &sail).is_ok());
    assert!(std::fs::write(root.join("sail-bin").join("sail-z3"), "").is_ok());
    let fake = bin.join("cargo-verus");
    let script = format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display());
    assert!(std::fs::write(&fake, script).is_ok());
    assert!(std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).is_ok());
    let path = std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap_or_default();
    let run = |cargo_verus: Option<&std::path::Path>| {
        let mut command = Command::new(&sail);
        command
            .arg("verify")
            .current_dir(&root)
            .env("PATH", &path)
            .env("SAIL_CACHE_DIR", &cache)
            // Longer than any Unix socket path: no daemon is started.
            .env("SAIL_DAEMON_SOCKET", root.join("d".repeat(120)))
            .env_remove("SAIL_VERUS_BUILD")
            .env_remove("SAIL_CARGO_VERUS")
            .env("VERUS_Z3_PATH", &fake);
        // Loader overrides (the CI image sets one) are refused before a
        // Verus is chosen (SAIL3112); this test is about the choice.
        for loader in [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_LIBRARY_PATH",
            "DYLD_FRAMEWORK_PATH",
        ] {
            command.env_remove(loader);
        }
        if let Some(cargo_verus) = cargo_verus {
            command.env("SAIL_CARGO_VERUS", cargo_verus);
        }
        command
            .output()
            .unwrap_or_else(|e| unreachable!("cannot run sail: {e}"))
    };
    let output = run(None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.starts_with("error[SAIL0101]: sail verify: Sail's Verus (the fork runtime and interactive engine) is not installed")
            && stderr.contains("never falls back"),
        "{stderr}"
    );
    assert!(!marker.exists(), "sail ran the cargo-verus on PATH");
    let output = run(Some(&fake));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("warning[SAIL0108]: SAIL_CARGO_VERUS replaces Sail's Verus"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("SAIL0101") && stderr.contains(&bin.join("verus").display().to_string()),
        "the explicit override was not used: {stderr}"
    );
    assert!(
        !marker.exists(),
        "sail ran a cargo-verus without identifying it"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&cache);
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
            .env_remove("SAIL_VERUS_BUILD")
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
            stderr.starts_with(
                "error[SAIL3120]: Sail's Verus (the fork runtime and interactive engine) is not installed"
            )
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

/// `--help` prints help and exits 0 for every command, without running it
/// (`sail watch --help` used to start a watcher; `sail cache gc --help` was
/// rejected).
#[test]
fn every_command_has_help() {
    for command in [
        "verify",
        "repair-check",
        "engine",
        "explain",
        "baseline",
        "compare",
        "watch",
        "jobs",
        "mcp",
        "serve",
        "status",
        "cache",
        "fragility",
    ] {
        for args in [
            vec![command, "--help"],
            vec![command, "-h"],
            vec!["help", command],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_sail"))
                .args(&args)
                .env("SAIL_CARGO_VERUS", "/nonexistent/cargo-verus")
                .output()
                .unwrap_or_else(|e| unreachable!("cannot run sail: {e}"));
            assert_eq!(output.status.code(), Some(0), "{args:?}");
            assert!(!output.stdout.is_empty(), "{args:?}");
        }
    }
    // Experimental commands keep their help, which says they are experimental.
    for args in [
        &["repair-check", "--help"][..],
        &["explain", "--help"],
        &["baseline", "--help"],
        &["compare", "--help"],
        &["serve", "--help"],
        &["fragility", "--help"],
        &["cache", "keygen", "--help"],
        &["cache", "trust", "list", "--help"],
        &["cache", "publish", "--help"],
        &["cache", "serve", "--help"],
        &["cache", "connect", "--help"],
    ] {
        let output = sail(args);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout).starts_with("EXPERIMENTAL: "),
            "{args:?}"
        );
    }
    let cache = sail(&["cache", "--help"]);
    let cache = String::from_utf8_lossy(&cache.stdout);
    assert!(
        !cache.contains("EXPERIMENTAL") && !cache.contains("keygen"),
        "{cache}"
    );
    let gc = sail(&["cache", "gc", "--help"]);
    assert_eq!(gc.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&gc.stdout).contains("gc --max-bytes N"));
    let verify = sail(&["verify", "--help"]);
    let verify = String::from_utf8_lossy(&verify.stdout);
    // Managed verification is the default; `--incremental` stays accepted
    // and is documented as a deprecated no-op.
    assert!(
        verify.contains("--incremental                 deprecated, does nothing"),
        "{verify}"
    );
    // The verdict-changing portfolio and experimental fragility options
    // still run but are not listed.
    assert!(
        !verify.contains("--portfolio") && !verify.contains("--measure-fragility"),
        "{verify}"
    );
    assert_eq!(sail(&["help", "frobnicate"]).status.code(), Some(2));
}
