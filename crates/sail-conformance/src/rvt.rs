//! Conformance on Verus's own `rust_verify_test` suite (ADR 0019).
//!
//! The deterministic fork arm compares complete output exactly. The stock
//! oracle arm uses the diagnostic multiset and instability policy below.
//!
//! Each test runs twice, plainly and through `sail-z3`, and the two runs
//! of a test must agree on:
//! - libtest's outcome (`ok`, `FAILED`, `ignored`);
//! - the exit code;
//! - stderr: every diagnostic Verus reported, as the test harness rendered
//!   it, plus any panic message, compared like corpus units' stderr
//!   (ADR 0042: complete messages as a multiset, the rest byte for byte);
//! - stdout: libtest's own report.
//!
//! `sail_verus::rvt` blanks only the two pieces of that text that are not
//! Verus output and vary between identical runs (a panic's OS thread id
//! and libtest's timing). As for corpus units (ADR 0009), a difference
//! triggers further plain runs: if plain runs disagree among themselves
//! and sail matches one of them, the test is `unstable`, otherwise it is
//! `different`.

use sail_verus::rvt::TestRecord;
use serde::Serialize;

use crate::{Verdict, diagnostics, first_difference};

/// Every difference between a plain run and a sail run of one test.
#[must_use]
pub fn compare_tests(plain: &TestRecord, sail: &TestRecord) -> Vec<String> {
    compare_tests_with(plain, sail, false)
}

/// Compares two deterministic fork runs, including the order of stderr.
///
/// Only the test runner's existing thread-id, libtest-timing and known
/// Cargo-new temporary-path handling applies; ADR 0042's multiset comparison
/// is reserved for the stock oracle.
#[must_use]
pub fn compare_tests_exact(plain: &TestRecord, sail: &TestRecord) -> Vec<String> {
    compare_tests_with(plain, sail, true)
}

fn compare_tests_with(plain: &TestRecord, sail: &TestRecord, exact: bool) -> Vec<String> {
    let mut d = Vec::new();
    if plain.outcome != sail.outcome {
        d.push(format!(
            "outcome: plain {:?} / sail {:?}",
            plain.outcome, sail.outcome
        ));
    }
    if plain.exit_code != sail.exit_code {
        d.push(format!(
            "exit code: plain {:?} / sail {:?}",
            plain.exit_code, sail.exit_code
        ));
    }
    if exact {
        if plain.stderr != sail.stderr {
            d.push(format!(
                "diagnostics (stderr) differ at {}",
                first_difference(&plain.stderr, &sail.stderr)
            ));
        }
    } else {
        d.extend(diagnostics::compare_stderr(&plain.stderr, &sail.stderr));
    }
    if plain.stdout != sail.stdout {
        d.push(format!(
            "libtest report (stdout) differs at {}",
            first_difference(&plain.stdout, &sail.stdout)
        ));
    }
    d
}

/// The verdict for one test.
#[derive(Debug, Clone, Serialize)]
pub struct TestClassification {
    /// The verdict.
    pub verdict: Verdict,
    /// Sail compared with the first plain run.
    pub differences: Vec<String>,
    /// How many plain runs were compared.
    pub plain_runs: usize,
    /// Whether the plain runs disagreed among themselves.
    pub plain_varies: bool,
    /// Whether sail matched one complete plain run after the approved
    /// path, thread-id and libtest-timing normalization.
    pub matches_a_plain_run: bool,
}

/// Classifies a sail run of a test against one or more plain runs.
#[must_use]
pub fn classify_tests(plains: &[TestRecord], sail: &TestRecord) -> TestClassification {
    classify_tests_with(plains, sail, false)
}

/// Classifies deterministic fork runs with an ordered, exact stderr check.
#[must_use]
pub fn classify_tests_exact(plains: &[TestRecord], sail: &TestRecord) -> TestClassification {
    classify_tests_with(plains, sail, true)
}

fn classify_tests_with(
    plains: &[TestRecord],
    sail: &TestRecord,
    exact: bool,
) -> TestClassification {
    let Some(first) = plains.first() else {
        return TestClassification {
            verdict: Verdict::InfraError,
            differences: Vec::new(),
            plain_runs: 0,
            plain_varies: false,
            matches_a_plain_run: false,
        };
    };
    let differences = compare_tests_with(first, sail, exact);
    let plain_varies = plains
        .iter()
        .skip(1)
        .any(|p| !compare_tests_with(first, p, exact).is_empty());
    let matches_a_plain_run = plains
        .iter()
        .any(|p| compare_tests_with(p, sail, exact).is_empty());
    let verdict = if differences.is_empty() {
        Verdict::Same
    } else if !exact && plain_varies && matches_a_plain_run {
        Verdict::Unstable
    } else {
        Verdict::Different
    };
    TestClassification {
        verdict,
        differences,
        plain_runs: plains.len(),
        plain_varies,
        matches_a_plain_run,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_verus::rvt::Outcome;

    fn rec(outcome: Outcome, stderr: &str) -> TestRecord {
        TestRecord {
            outcome,
            exit_code: Some(i32::from(outcome != Outcome::Ok)),
            stdout: "test result: ok.\n".to_owned(),
            stderr: stderr.to_owned(),
            wall_ms: 1.0,
        }
    }

    #[test]
    fn identical_runs_are_the_same() {
        let a = rec(Outcome::Ok, "warning: w\n");
        let c = classify_tests(std::slice::from_ref(&a), &a);
        assert_eq!(c.verdict, Verdict::Same);
        assert_eq!(c.differences, Vec::<String>::new());
    }

    #[test]
    fn outcome_and_diagnostic_differences_are_reported() {
        let a = rec(Outcome::Ok, "x\n");
        let b = rec(Outcome::Failed, "y\n");
        let d = compare_tests(&a, &b);
        assert_eq!(d.len(), 3, "{d:?}");
        assert!(d[0].starts_with("outcome"));
        assert!(d[2].contains("line 1"));
        let mut c = a.clone();
        c.stdout = "other\n".to_owned();
        assert_eq!(compare_tests(&a, &c).len(), 1);
    }

    #[test]
    fn rendered_messages_may_come_in_any_order() {
        let e1 = "error: e1\n --> /tmp/t/test.rs:3:5\n  |\n\n";
        let e2 = "error: e2\n --> /tmp/t/test.rs:9:5\n  |\n\n";
        let a = rec(Outcome::Failed, &format!("{e1}{e2}panicked\n"));
        let b = rec(Outcome::Failed, &format!("{e2}{e1}panicked\n"));
        assert_eq!(compare_tests(&a, &b), Vec::<String>::new());
        assert_eq!(compare_tests_exact(&a, &b).len(), 1);
        assert_eq!(
            classify_tests_exact(std::slice::from_ref(&a), &b).verdict,
            Verdict::Different
        );
        let missing = rec(Outcome::Failed, &format!("{e2}panicked\n"));
        assert_eq!(compare_tests(&a, &missing).len(), 1);
    }

    #[test]
    fn plain_flakiness_is_unstable_never_hidden() {
        let ok = rec(Outcome::Ok, "");
        let failed = rec(Outcome::Failed, "error: rlimit\n");
        // Plain varies and sail matches one plain run: unstable.
        let c = classify_tests(&[failed.clone(), ok.clone()], &ok);
        assert_eq!(c.verdict, Verdict::Unstable);
        assert!(c.plain_varies);
        assert!(!c.differences.is_empty(), "the difference stays reported");
        // A fork that varies is a determinism defect, never an accepted
        // stock-oracle instability.
        assert_eq!(
            classify_tests_exact(&[failed.clone(), ok.clone()], &ok).verdict,
            Verdict::Different
        );
        // Plain is stable and sail differs: different.
        let c = classify_tests(&[failed.clone(), failed.clone()], &ok);
        assert_eq!(c.verdict, Verdict::Different);
        // Plain varies but sail matches neither: different.
        let other = rec(Outcome::Failed, "error: other\n");
        assert_eq!(
            classify_tests(&[failed, ok], &other).verdict,
            Verdict::Different
        );
    }

    #[test]
    fn an_unobserved_message_on_a_varying_line_is_different() {
        let a = rec(Outcome::Failed, "error: first\n");
        let b = rec(Outcome::Failed, "error: second\n");
        let wrong = rec(Outcome::Failed, "error: wrong\n");
        let c = classify_tests(&[a, b], &wrong);
        assert_eq!(c.verdict, Verdict::Different);
        assert!(c.plain_varies);
        assert!(!c.matches_a_plain_run);
    }

    #[test]
    fn combining_lines_from_distinct_plain_runs_is_different() {
        let a = rec(Outcome::Failed, "error: first\nnote: first\n");
        let b = rec(Outcome::Failed, "error: second\nnote: second\n");
        let hybrid = rec(Outcome::Failed, "error: first\nnote: second\n");
        assert_eq!(classify_tests(&[a, b], &hybrid).verdict, Verdict::Different);
    }
}
