//! The rlimit experiment against the pinned Z3.
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

use sail_bench::solver::{Outcome, Solver, run};
use sail_bench::transcript::Transcript;

fn solver() -> Solver {
    match std::env::var("SAIL_TEST_Z3") {
        Ok(path) if !path.is_empty() => Solver {
            path: PathBuf::from(path),
            timeout: Duration::from_secs(120),
        },
        _ => unreachable!("set SAIL_TEST_Z3 to the pinned z3 to run this test"),
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
