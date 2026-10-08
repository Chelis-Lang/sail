//! Replays a recorded Verus session through `sail-z3` and through the pinned
//! Z3 directly, and requires the same replies.
//!
//! Needs the pinned Z3, so it is ignored by default. Run it with:
//!
//! ```sh
//! SAIL_TEST_Z3=/path/to/pinned/z3 cargo nextest run -p sail-shim --run-ignored all
//! ```
//!
//! Replies to `(get-info :all-statistics)` contain wall-clock time and memory
//! figures that differ between any two solver runs. For those, only
//! `:rlimit-count`, which is deterministic, is compared. Every other reply
//! must match byte for byte.
#![cfg(unix)]

use std::io::Write as _;
use std::process::{Command, Stdio};

use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, ResponseSplitter};

const INPUT: &[u8] = include_bytes!("../../sail-smtlib/tests/fixtures/verus-toy.in.smt2");

fn z3() -> String {
    match std::env::var("SAIL_TEST_Z3") {
        Ok(path) if !path.is_empty() => path,
        _ => unreachable!("set SAIL_TEST_Z3 to the pinned z3 to run this test"),
    }
}

fn run(program: &str, env_z3: Option<&str>) -> Vec<u8> {
    let mut cmd = Command::new(program);
    cmd.args(["-smt2", "-in"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(z3) = env_z3 {
        cmd.env("SAIL_Z3_PATH", z3).env_remove("SAIL_TRACE_DIR");
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => unreachable!("cannot start {program}: {e}"),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(INPUT);
    }
    match child.wait_with_output() {
        Ok(out) if out.status.success() => out.stdout,
        Ok(out) => unreachable!("{program} exited with {}", out.status),
        Err(e) => unreachable!("{program} did not finish: {e}"),
    }
}

/// `(is_statistics, reply)` for each reply, in order.
fn replies(output: &[u8]) -> Vec<(bool, Vec<u8>)> {
    let mut commands = Vec::new();
    let mut splitter = CommandSplitter::new();
    splitter.push(INPUT, &mut commands);
    let stats: Vec<bool> = commands
        .iter()
        .filter_map(|c| classify(&c.bytes).ok())
        .filter(|c| c.kind.output() == Output::One)
        .map(|c| {
            matches!(
                c.kind,
                CommandKind::GetInfo {
                    keyword: b":all-statistics"
                }
            )
        })
        .collect();
    let mut items = Vec::new();
    let mut responses = ResponseSplitter::new();
    responses.push(output, &mut items);
    assert_eq!(
        items.len(),
        stats.len(),
        "one reply per output-producing command"
    );
    stats
        .into_iter()
        .zip(items.into_iter().map(|i| i.bytes))
        .collect()
}

fn rlimit_count(reply: &[u8]) -> Option<&[u8]> {
    let text = reply
        .split(|&b| b == b'\n')
        .find(|l| l.windows(13).any(|w| w == b":rlimit-count"))?;
    text.split(u8::is_ascii_whitespace)
        .rfind(|w| !w.is_empty())
        .map(|w| w.strip_suffix(b")").unwrap_or(w))
}

#[test]
#[ignore = "needs the pinned z3 in SAIL_TEST_Z3"]
fn shim_replies_match_direct_z3() {
    let z3 = z3();
    let direct = replies(&run(&z3, None));
    let shimmed = replies(&run(env!("CARGO_BIN_EXE_sail-z3"), Some(&z3)));
    assert_eq!(direct.len(), shimmed.len());
    for (i, ((is_stats, a), (_, b))) in direct.iter().zip(&shimmed).enumerate() {
        if *is_stats {
            assert_eq!(
                rlimit_count(a),
                rlimit_count(b),
                "reply {i}: rlimit-count differs"
            );
            assert!(rlimit_count(a).is_some(), "reply {i}: no rlimit-count");
        } else {
            assert_eq!(a, b, "reply {i} differs");
        }
    }
}
