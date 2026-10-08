//! The seed, history and rlimit experiments against the pinned Z3, on the
//! recorded toy session (`crates/sail-smtlib/tests/fixtures/verus-toy.*`)
//! and small synthetic inputs.
//!
//! Needs the pinned Z3, so it is ignored by default. Run it with:
//!
//! ```sh
//! SAIL_TEST_Z3=/path/to/pinned/z3 cargo nextest run -p sail-bench --run-ignored all
//! ```
#![cfg(unix)]

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use sail_bench::history::{Agreement, HistoryOptions, history};
use sail_bench::seeds::{SeedsOptions, seeds};
use sail_bench::solver::{Outcome, Solver, run};
use sail_bench::trace::discover;
use sail_bench::transcript::Transcript;

const INPUT: &[u8] = include_bytes!("../../sail-smtlib/tests/fixtures/verus-toy.in.smt2");
const OUTPUT: &[u8] = include_bytes!("../../sail-smtlib/tests/fixtures/verus-toy.out.txt");

fn solver() -> Solver {
    match std::env::var("SAIL_TEST_Z3") {
        Ok(path) if !path.is_empty() => Solver {
            path: PathBuf::from(path),
            timeout: Duration::from_secs(120),
        },
        _ => unreachable!("set SAIL_TEST_Z3 to the pinned z3 to run this test"),
    }
}

/// A trace directory holding only the toy session.
fn toy_trace_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sail-bench-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ok = std::fs::create_dir_all(&dir).is_ok()
        && std::fs::write(dir.join("toy.in.smt2"), INPUT).is_ok()
        && std::fs::write(dir.join("toy.out"), OUTPUT).is_ok()
        && std::fs::write(
            dir.join("toy.jsonl"),
            "{\"type\":\"start\",\"trace_version\":1}\n",
        )
        .is_ok();
    assert!(ok, "cannot write {}", dir.display());
    dir
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn seed_zero_replays_the_recording_and_seeds_are_accepted() {
    let dir = toy_trace_dir("seeds");
    let sessions = discover(&dir).unwrap_or_default();
    assert_eq!(sessions.len(), 1);
    let report = seeds(
        &solver(),
        &sessions,
        &SeedsOptions {
            seeds: 3,
            jobs: 2,
            examples: 10,
            all_checks: true,
        },
    );
    let _ = std::fs::remove_dir_all(&dir);
    // The injected options are accepted: no errors, every batch paired.
    assert_eq!(report.unpaired_runs, Vec::<String>::new());
    assert_eq!(report.infra_errors, Vec::<String>::new());
    assert_eq!(report.checks, 11);
    assert_eq!(report.complete_checks, 11);
    let s0 = &report.seed0;
    assert_eq!(s0.answers_compared, 11);
    assert_eq!(
        (
            s0.answer_mismatches,
            s0.followup_mismatches,
            s0.rlimit_mismatches
        ),
        (0, 0, 0),
        "{:?}",
        s0.differences
    );
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn toy_checks_do_not_depend_on_history() {
    let dir = toy_trace_dir("history");
    let sessions = discover(&dir).unwrap_or_default();
    let report = history(
        &solver(),
        &sessions,
        &HistoryOptions {
            jobs: 2,
            examples: 10,
            all_checks: true,
        },
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(report.replayed, 11);
    assert_eq!(report.infra_errors, Vec::<String>::new());
    // Answers are a function of the v1 state for this session.
    assert_eq!(report.answer.equal, 11, "{:?}", report.examples);
    for e in &report.examples {
        assert_ne!(e.answer, Agreement::Differ, "{e:?}");
    }
}

/// `(set-option :rlimit N)` is relative to the count when the check starts,
/// and exhaustion is reported as `canceled`.
#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn rlimit_is_relative_and_exhaustion_reads_canceled() {
    // Pigeonhole with 12 pigeons and 11 holes: far beyond 300,000 units.
    let n = 12;
    let mut input = String::from("(set-option :auto_config false)\n");
    for p in 0..n {
        for h in 0..n - 1 {
            let _ = writeln!(input, "(declare-const p{p}h{h} Bool)");
        }
    }
    for p in 0..n {
        let holes: Vec<String> = (0..n - 1).map(|h| format!("p{p}h{h}")).collect();
        let _ = writeln!(input, "(assert (or {}))", holes.join(" "));
    }
    for h in 0..n - 1 {
        for a in 0..n {
            for b in a + 1..n {
                let _ = writeln!(input, "(assert (not (and p{a}h{h} p{b}h{h})))");
            }
        }
    }
    for limit in [200_000, 200_000, 300_000] {
        let _ = write!(
            input,
            "(get-info :all-statistics)\n(set-option :rlimit {limit})\n(check-sat)\n\
             (set-option :rlimit 0)\n(get-info :reason-unknown)\n"
        );
    }
    input.push_str("(get-info :all-statistics)\n");
    let r = run(&solver(), input.as_bytes());
    let Ok(r) = r else { unreachable!("{r:?}") };
    assert_eq!(r.outcome, Outcome::Exited { code: Some(0) });
    let t = Transcript::new(input.as_bytes(), &r.stdout);
    let checks = t.checks();
    assert_eq!(checks.len(), 3);
    let deltas: Vec<Option<u64>> = checks.iter().map(|c| c.rlimit_delta).collect();
    // Each check consumes exactly its own budget, however much the process
    // has already used.
    assert_eq!(deltas, [Some(200_000), Some(200_000), Some(300_000)]);
    for c in &checks {
        assert_eq!(c.answer.as_deref(), Some(&b"unknown"[..]));
        assert_eq!(
            c.reason_unknown(),
            Some(&b"(:reason-unknown \"canceled\")"[..])
        );
    }
}
