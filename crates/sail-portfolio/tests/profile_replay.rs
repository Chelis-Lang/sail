//! Native pinned-Z3 controls for uncached diagnostic prefix execution.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sail_portfolio::Solver;
use sail_portfolio::profile::replay::measure;

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn exact_prefix_plain_traced_pairs_preserve_history_and_counts()
-> Result<(), Box<dyn std::error::Error>> {
    let solver = Solver {
        path: PathBuf::from(std::env::var("SAIL_TEST_Z3")?),
        timeout: Duration::from_secs(30),
    };
    let root = std::env::temp_dir().join(format!(
        "sail-profile-pairs-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    std::fs::create_dir(&root)?;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/quantifiers");
    for (case, check) in [
        ("two-checks", 0),
        ("two-checks", 1),
        ("mbqi", 0),
        ("datatype", 0),
    ] {
        let input = std::fs::read(fixtures.join(format!("{case}.smt2")))?;
        let expected = sail_portfolio::profile::prefix(&input, check)?;
        let destination = root.join(format!("{case}-{check}"));
        let measured = measure(&solver, &input, check, &destination)?;
        assert_eq!(measured.check, check);
        assert_eq!(measured.original_checks, check + 1);
        assert_eq!(measured.counts.checks, u64::try_from(check + 1)?);
        assert_eq!(measured.prefix_bytes, expected.input.len());
        assert_eq!(
            std::fs::read(destination.join("prefix.smt2"))?,
            expected.input
        );
        assert_eq!(
            std::fs::read(destination.join("plain.stdout"))?,
            std::fs::read(destination.join("traced.stdout"))?
        );
        assert!(measure(&solver, &input, check, &destination).is_err());
        assert_eq!(
            std::fs::read(destination.join("prefix.smt2"))?,
            expected.input
        );
    }
    // A valid SMT check can bypass Z3's internal begin-check trace event.
    let trivial = root.join("trivial");
    let measured = measure(&solver, b"(assert true)\n(check-sat)", 0, &trivial)?;
    assert_eq!(measured.original_checks, 1);
    assert_eq!(measured.counts.checks, 0);
    assert_eq!(measured.counts.quantifier_instances, 0);
    // A real solver syntax error is never a successful measurement, and its
    // original captures remain available to diagnose the refusal.
    let invalid = root.join("solver-error");
    assert!(
        measure(
            &solver,
            b"(assert missing_symbol)\n(check-sat)",
            0,
            &invalid
        )
        .is_err()
    );
    assert!(invalid.join("plain.stdout").is_file());
    assert!(!invalid.join("traced.stdout").exists());
    Ok(())
}
