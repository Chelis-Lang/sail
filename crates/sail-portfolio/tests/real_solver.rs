//! Fragility measurement against the pinned Z3, on the recorded toy session
//! (`crates/sail-smtlib/tests/fixtures/verus-toy.*`).
//!
//! Needs the pinned Z3, so it is ignored by default. Run it with:
//!
//! ```sh
//! SAIL_TEST_Z3=/path/to/pinned/z3 cargo nextest run -p sail-portfolio --run-ignored all
//! ```
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use sail_portfolio::Solver;
use sail_portfolio::fragility::{Options, Stability, attribute_single_function, measure, render};
use sail_portfolio::sessions::SessionFiles;

const INPUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sail-smtlib/tests/fixtures/verus-toy.in.smt2"
);
const OUTPUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sail-smtlib/tests/fixtures/verus-toy.out.txt"
);

fn solver() -> Solver {
    match std::env::var("SAIL_TEST_Z3") {
        Ok(path) if !path.is_empty() => Solver {
            path: PathBuf::from(path),
            timeout: Duration::from_secs(120),
        },
        _ => unreachable!("set SAIL_TEST_Z3 to the pinned z3 to run this test"),
    }
}

fn toy() -> SessionFiles {
    SessionFiles {
        id: "toy".to_owned(),
        jsonl: PathBuf::new(),
        input: PathBuf::from(INPUT),
        output: PathBuf::from(OUTPUT),
    }
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn toy_session_under_three_seeds() {
    assert!(Path::new(INPUT).is_file());
    let options = Options {
        seeds: 3,
        shuffles: 0,
        jobs: 3,
        near_limit: 0.5,
    };
    let report = measure(&solver(), &[toy()], &options, |_, _| {});
    let Ok(mut report) = report else {
        unreachable!("{report:?}")
    };
    assert_eq!(report.config_labels, ["baseline", "seed=1", "seed=2"]);
    assert!(report.infra.is_empty(), "{:?}", report.infra);
    assert!(
        report.unpaired_runs.is_empty(),
        "{:?}",
        report.unpaired_runs
    );
    let s = &report.summary;
    assert_eq!(s.checks, 11);
    // The baseline replay reproduces the capture.
    assert_eq!(s.baseline_vs_recorded.answers_compared, 11);
    assert_eq!(s.baseline_vs_recorded.answer_mismatches, 0);
    // The toy's answers do not depend on the seed: 8 proofs, 3 failures.
    assert_eq!(s.by_class.get("stable-proved"), Some(&8));
    assert_eq!(s.by_class.get("stable-unproved"), Some(&3));
    assert_eq!(s.answers.get("unsat"), Some(&24));
    assert_eq!(s.answers.get("unknown"), Some(&9));
    for c in &report.checks {
        assert_eq!(c.limit, Some(30_000_000), "check {}", c.seq);
        assert_eq!(c.answers.len(), 3);
        assert_eq!(c.answers[0], c.recorded, "check {}", c.seq);
        assert_ne!(c.stability, Stability::Unstable);
    }
    // Far from the limit: every check used under 0.1% of it.
    assert!(s.used_fraction.max.is_some_and(|m| m < 0.001), "{s:?}");
    assert!(s.unknown.max_used_fraction_not_exhausted.is_some());
    assert!(attribute_single_function(
        &mut report,
        &["toy::f".to_owned()]
    ));
    assert!(render(&report, 5).contains("function toy::f: 11 checks"));
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn assertion_shuffles_are_accepted_by_the_solver() {
    let options = Options {
        seeds: 1,
        shuffles: 2,
        jobs: 3,
        near_limit: 0.5,
    };
    let report = measure(&solver(), &[toy()], &options, |_, _| {});
    let Ok(report) = report else {
        unreachable!("{report:?}")
    };
    assert_eq!(report.config_labels, ["baseline", "shuffle=1", "shuffle=2"]);
    // A shuffle must never make Z3 reject the input or lose pairing.
    assert!(report.infra.is_empty(), "{:?}", report.infra);
    assert!(
        report.unpaired_runs.is_empty(),
        "{:?}",
        report.unpaired_runs
    );
    assert_eq!(report.summary.checks, 11);
    for c in &report.checks {
        assert!(c.answers.iter().all(Option::is_some), "check {}", c.seq);
    }
}
