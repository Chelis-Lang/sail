//! Conformance on Verus's own `rust_verify_test` suite (ADR 0019).
//!
//! Each test runs twice, plainly and through `sail-z3`, and the two runs
//! of a test must agree on:
//! - libtest's outcome (`ok`, `FAILED`, `ignored`);
//! - the exit code;
//! - stderr, byte for byte: every diagnostic Verus reported, as the test
//!   harness rendered it, plus any panic message;
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

use crate::{Verdict, first_difference};

/// Every difference between a plain run and a sail run of one test.
#[must_use]
pub fn compare_tests(plain: &TestRecord, sail: &TestRecord) -> Vec<String> {
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
    if plain.stderr != sail.stderr {
        d.push(format!(
            "diagnostics (stderr) differ at {}",
            first_difference(&plain.stderr, &sail.stderr)
        ));
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
    /// For an `unstable` test: `true` if sail matched one plain run
    /// exactly, `false` if it differed from the first plain run only on
    /// lines where the plain runs also differ among themselves.
    pub matches_a_plain_run: bool,
}

/// Indices of the lines where any of `others` differs from `base`, or
/// `None` if some run has a different number of lines.
fn varying_lines<'a>(base: &str, others: impl Iterator<Item = &'a str>) -> Option<Vec<usize>> {
    let b: Vec<&str> = base.lines().collect();
    let mut varying = std::collections::BTreeSet::new();
    for other in others {
        let o: Vec<&str> = other.lines().collect();
        if o.len() != b.len() {
            return None;
        }
        varying.extend(
            b.iter()
                .zip(&o)
                .enumerate()
                .filter(|(_, (x, y))| x != y)
                .map(|(i, _)| i),
        );
    }
    Some(varying.into_iter().collect())
}

/// Whether `sail` differs from `plains[0]` only where the plain runs
/// themselves vary: the same outcome and exit code, the same number of
/// lines in stderr and stdout, and every differing line one on which some
/// plain run also differs from the first.
fn within_plain_variation(plains: &[TestRecord], sail: &TestRecord) -> bool {
    let Some(first) = plains.first() else {
        return false;
    };
    if first.outcome != sail.outcome || first.exit_code != sail.exit_code {
        return false;
    }
    let rest = || plains.iter().skip(1);
    let ok = |field: fn(&TestRecord) -> &str| {
        let (Some(plain), Some(with_sail)) = (
            varying_lines(field(first), rest().map(field)),
            varying_lines(field(first), std::iter::once(field(sail))),
        ) else {
            return false;
        };
        with_sail.iter().all(|i| plain.contains(i))
    };
    ok(|r| r.stderr.as_str()) && ok(|r| r.stdout.as_str())
}

/// Classifies a sail run of a test against one or more plain runs.
#[must_use]
pub fn classify_tests(plains: &[TestRecord], sail: &TestRecord) -> TestClassification {
    let Some(first) = plains.first() else {
        return TestClassification {
            verdict: Verdict::InfraError,
            differences: Vec::new(),
            plain_runs: 0,
            plain_varies: false,
            matches_a_plain_run: false,
        };
    };
    let differences = compare_tests(first, sail);
    let plain_varies = plains
        .iter()
        .skip(1)
        .any(|p| !compare_tests(first, p).is_empty());
    let matches_a_plain_run = plains.iter().any(|p| compare_tests(p, sail).is_empty());
    let verdict = if differences.is_empty() {
        Verdict::Same
    } else if plain_varies && (matches_a_plain_run || within_plain_variation(plains, sail)) {
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
    fn plain_flakiness_is_unstable_never_hidden() {
        let ok = rec(Outcome::Ok, "");
        let failed = rec(Outcome::Failed, "error: rlimit\n");
        // Plain varies and sail matches one plain run: unstable.
        let c = classify_tests(&[failed.clone(), ok.clone()], &ok);
        assert_eq!(c.verdict, Verdict::Unstable);
        assert!(c.plain_varies);
        assert!(!c.differences.is_empty(), "the difference stays reported");
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
    fn differences_only_where_plain_varies_are_unstable() {
        // A random temporary path on line 2, in every run.
        let run = |tmp: &str| rec(Outcome::Failed, &format!("error: e\nopen {tmp}\nnote\n"));
        let plains = [run("/t/a"), run("/t/b")];
        let c = classify_tests(&plains, &run("/t/c"));
        assert_eq!(c.verdict, Verdict::Unstable);
        assert!(!c.matches_a_plain_run);
        // A difference on a line where plain is stable: different.
        let other = rec(Outcome::Failed, "error: x\nopen /t/c\nnote\n");
        assert_eq!(classify_tests(&plains, &other).verdict, Verdict::Different);
        // Another outcome: different, whatever the text.
        let mut passed = run("/t/c");
        passed.outcome = Outcome::Ok;
        assert_eq!(classify_tests(&plains, &passed).verdict, Verdict::Different);
        // An extra line: different.
        let longer = rec(Outcome::Failed, "error: e\nopen /t/c\nnote\nmore\n");
        assert_eq!(classify_tests(&plains, &longer).verdict, Verdict::Different);
    }
}
