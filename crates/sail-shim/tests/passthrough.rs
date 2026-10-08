//! The shim must forward bytes, arguments and exit status unchanged.
//!
//! These tests use small shell scripts as stand-in solvers, so they run
//! without Z3. `z3_transcript.rs` covers the real solver.
#![cfg(unix)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sail-shim-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        unreachable!("cannot create {}: {e}", dir.display());
    }
    dir
}

fn script(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("fake-solver");
    let written = std::fs::write(&path, format!("#!/bin/sh\n{body}\n"))
        .and_then(|()| std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)));
    if let Err(e) = written {
        unreachable!("cannot write {}: {e}", path.display());
    }
    path
}

fn shim(solver: Option<&Path>, args: &[&str], input: &[u8], trace: Option<&Path>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail-z3"));
    cmd.args(args)
        .env_remove("SAIL_Z3_PATH")
        .env_remove("SAIL_TRACE_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(solver) = solver {
        cmd.env("SAIL_Z3_PATH", solver);
    }
    if let Some(trace) = trace {
        cmd.env("SAIL_TRACE_DIR", trace);
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => unreachable!("cannot start sail-z3: {e}"),
    };
    let mut stdin = child.stdin.take();
    let input = input.to_vec();
    let writer = std::thread::spawn(move || {
        if let Some(stdin) = stdin.as_mut() {
            let _ = stdin.write_all(&input);
        }
    });
    let output = child.wait_with_output();
    let _ = writer.join();
    match output {
        Ok(output) => output,
        Err(e) => unreachable!("sail-z3 did not finish: {e}"),
    }
}

#[test]
fn forwards_arbitrary_bytes_unchanged() {
    let dir = scratch("bytes");
    let solver = script(&dir, "exec cat");
    // Every byte value, invalid UTF-8, unbalanced parens and a large tail.
    let mut input: Vec<u8> = (0..=255u8).collect();
    input.extend_from_slice(b"(((\")) |x ;\n");
    input.extend((0..300_000u32).map(|i| u8::try_from(i % 251).unwrap_or(0)));
    let out = shim(Some(&solver), &["-smt2", "-in"], &input, None);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout == input, "output differs from input");
}

#[test]
fn forwards_arguments_stderr_and_exit_code() {
    let dir = scratch("args");
    let argv = dir.join("argv");
    let solver = script(
        &dir,
        &format!(
            "printf '%s\\n' \"$@\" > '{}'; echo oops >&2; cat >/dev/null; exit 7",
            argv.display()
        ),
    );
    let out = shim(
        Some(&solver),
        &["-smt2", "-in", "a b"],
        b"(check-sat)\n",
        None,
    );
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(out.stderr, b"oops\n");
    assert_eq!(
        std::fs::read_to_string(&argv).unwrap_or_default(),
        "-smt2\n-in\na b\n"
    );
}

#[test]
fn solver_killed_by_signal_maps_to_128_plus_signal() {
    let dir = scratch("signal");
    let solver = script(&dir, "kill -9 $$");
    assert_eq!(shim(Some(&solver), &[], b"", None).status.code(), Some(137));
}

#[test]
fn refuses_to_start_without_a_solver() {
    let out = shim(None, &["-smt2", "-in"], b"", None);
    assert_eq!(out.status.code(), Some(125));
    assert!(String::from_utf8_lossy(&out.stderr).contains("SAIL2001"));
    let out = shim(Some(Path::new("/nonexistent/z3")), &[], b"", None);
    assert!(String::from_utf8_lossy(&out.stderr).contains("SAIL2002"));
}

#[test]
fn refuses_to_run_itself() {
    let out = shim(
        Some(Path::new(env!("CARGO_BIN_EXE_sail-z3"))),
        &[],
        b"",
        None,
    );
    assert_eq!(out.status.code(), Some(125));
    assert!(String::from_utf8_lossy(&out.stderr).contains("SAIL2003"));
}

#[test]
fn tracing_never_changes_the_forwarded_bytes() {
    let dir = scratch("trace");
    let trace = dir.join("trace");
    // A solver that answers nonsense, including an error, to stress the tap.
    let solver = script(
        &dir,
        "cat >/dev/null; printf 'sat\\n(error \"x\")\\n((((\\n'",
    );
    let input = b"(push)(assert (> x 0))(check-sat)(pop)(pop)(pop)(frobnicate)(((";
    let out = shim(Some(&solver), &["-smt2", "-in"], input, Some(&trace));
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, b"sat\n(error \"x\")\n((((\n");
    let mut files: Vec<String> = std::fs::read_dir(&trace)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    assert_eq!(files.len(), 3, "{files:?}");
    let raw_in = files
        .iter()
        .find(|f| {
            Path::new(f)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("smt2"))
        })
        .map(|f| trace.join(f));
    assert_eq!(
        raw_in.and_then(|p| std::fs::read(p).ok()).as_deref(),
        Some(&input[..])
    );
    let jsonl = files
        .iter()
        .find(|f| {
            Path::new(f)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
        })
        .map(|f| trace.join(f));
    let text = jsonl
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let last = text.lines().last().unwrap_or_default();
    assert!(last.contains("\"type\":\"end\""), "{text}");
    assert!(last.contains("\"checks\":1"), "{text}");
    assert!(last.contains("\"paired\":false"), "{text}");
}

#[test]
fn unwritable_trace_dir_does_not_affect_forwarding() {
    let dir = scratch("badtrace");
    let solver = script(&dir, "exec cat");
    let blocker = dir.join("file");
    let _ = std::fs::write(&blocker, b"");
    let out = shim(
        Some(&solver),
        &[],
        b"(check-sat)\n",
        Some(&blocker.join("sub")),
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, b"(check-sat)\n");
}
