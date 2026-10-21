//! Differential conformance: sail against plain Verus on the same unit.
//!
//! Charter rule 1 says default mode must give the same per-function verdicts
//! and the same diagnostics as plain Verus with the pinned Z3. [`compare`]
//! checks, for one unit:
//! - the exit code;
//! - stderr (diagnostics), under ADR 0042: complete diagnostic messages as
//!   a multiset with exact bytes; only standalone verification-progress block
//!   ordering has the owner-approved ADR 0064 exception; all other text stays exact
//!   ([`diagnostics`]);
//! - the number of `--output-json` documents (one per verified crate) and,
//!   document by document in stdout order, Verus's `verification-results`
//!   and `func-details`;
//! - any non-JSON text left on stdout;
//! - each function's verdict.
//!
//! [`classify`] applies ADR 0009 on top of [`compare`]. Plain Verus is not
//! perfectly deterministic: Z3's effort on nonlinear queries jitters. So when
//! sail differs from a plain run, the harness compares against further plain
//! runs:
//! - if the plain runs disagree among themselves, and sail matches one of
//!   them, the unit is `unstable` (reported, not counted as a sail error);
//! - otherwise it is `different`.
//!
//! Per-function rlimit
//! differences are reported separately as *drift*: rlimit is not part of
//! rule 1, but it should be identical in pass-through mode, so drift is a
//! finding to investigate.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

pub mod diagnostics;
pub mod engine;
pub mod rvt;

use sail_verus::run::RunRecord;
use serde::Serialize;

/// The outcome for one unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Identical in everything rule 1 covers.
    Same,
    /// Plain Verus itself varies between runs, and sail matches one of its
    /// outcomes (ADR 0009). Reported, not counted as a difference.
    Unstable,
    /// At least one difference.
    Different,
    /// The harness could not run one side (not a conformance result).
    InfraError,
    /// Engine arm only: the project requires a Verus feature the engine
    /// runtime is built without, so the product falls back to stock for it
    /// (SAIL3124) and the engine is not run. Counted on its own, never as
    /// same or different.
    EngineLacksFeature,
}

/// What differed between the two runs.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Comparison {
    /// Rule 1 differences, one line each.
    pub differences: Vec<String>,
    /// Functions whose rlimit differs: `(function, plain, sail)`.
    pub rlimit_drift: Vec<(String, u64, u64)>,
    /// Number of functions compared.
    pub functions: usize,
    /// Owner-approved differences, reported separately from failures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted: Vec<String>,
}

impl Comparison {
    /// The verdict this comparison implies.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        if self.differences.is_empty() {
            Verdict::Same
        } else {
            Verdict::Different
        }
    }
}

pub(crate) fn first_difference(a: &str, b: &str) -> String {
    let line = a
        .lines()
        .zip(b.lines())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.lines().count().min(b.lines().count()));
    let show = |s: &str| s.lines().nth(line).unwrap_or("<end>").to_owned();
    format!(
        "line {}: plain {:?} / sail {:?}",
        line + 1,
        show(a),
        show(b)
    )
}

/// Compares a plain-Verus run with a sail run of the same unit.
#[must_use]
pub fn compare(plain: &RunRecord, sail: &RunRecord) -> Comparison {
    let mut c = Comparison::default();
    if plain.exit_code != sail.exit_code {
        c.differences.push(format!(
            "exit code: plain {:?} / sail {:?}",
            plain.exit_code, sail.exit_code
        ));
    }
    let stderr_differences = diagnostics::compare_stderr(&plain.stderr, &sail.stderr);
    if stderr_differences.is_empty()
        && diagnostics::progress_order_differs(&plain.stderr, &sail.stderr)
    {
        c.accepted
            .push("verification-progress note order (VI-0012, ADR 0064)".to_owned());
    }
    c.differences.extend(stderr_differences);
    if plain.stdout_rest != sail.stdout_rest {
        c.differences.push(format!(
            "non-JSON stdout differs at {}",
            first_difference(&plain.stdout_rest, &sail.stdout_rest)
        ));
    }
    if plain.outputs.len() == sail.outputs.len() {
        for (i, (p, s)) in plain.outputs.iter().zip(&sail.outputs).enumerate() {
            for key in ["verification-results", "func-details"] {
                if p.get(key) != s.get(key) {
                    let which = if plain.outputs.len() == 1 {
                        String::new()
                    } else {
                        format!(" (output {i})")
                    };
                    c.differences.push(format!("{key} differs{which}"));
                }
            }
        }
    } else {
        c.differences.push(format!(
            "--output-json documents: plain {} / sail {}",
            plain.outputs.len(),
            sail.outputs.len()
        ));
    }
    let index = |r: &RunRecord| -> BTreeMap<(usize, String, String), (bool, u64)> {
        r.functions()
            .into_iter()
            .map(|f| ((f.output, f.module, f.function), (f.success, f.rlimit)))
            .collect()
    };
    let (p, s) = (index(plain), index(sail));
    c.functions = p.len();
    for (key, (p_ok, p_rlimit)) in &p {
        match s.get(key) {
            None => c
                .differences
                .push(format!("function {} missing from sail run", key.2)),
            Some((s_ok, _)) if s_ok != p_ok => c.differences.push(format!(
                "function {}: plain success={p_ok} / sail success={s_ok}",
                key.2
            )),
            Some((_, s_rlimit)) if s_rlimit != p_rlimit => {
                c.rlimit_drift.push((key.2.clone(), *p_rlimit, *s_rlimit));
            }
            Some(_) => {}
        }
    }
    for key in s.keys().filter(|k| !p.contains_key(*k)) {
        c.differences
            .push(format!("function {} missing from plain run", key.2));
    }
    c
}

/// The verdict for one unit given one or more plain runs and a sail run.
#[derive(Debug, Clone, Serialize)]
pub struct Classification {
    /// The verdict.
    pub verdict: Verdict,
    /// Sail compared with the first plain run.
    #[serde(flatten)]
    pub comparison: Comparison,
    /// How many plain runs were compared.
    pub plain_runs: usize,
    /// Functions whose plain verdict varied between plain runs, and a note if
    /// plain diagnostics or exit codes varied.
    pub unstable: Vec<String>,
}

/// What varies between plain runs: functions whose verdict differs, plus
/// `"<diagnostics>"` if any rule-1 field other than function verdicts differs.
fn plain_variation(plains: &[RunRecord]) -> Vec<String> {
    let mut varying = std::collections::BTreeSet::new();
    let mut other = false;
    let base = plains.first();
    let verdicts = |r: &RunRecord| -> BTreeMap<(usize, String, String), bool> {
        r.functions()
            .into_iter()
            .map(|f| ((f.output, f.module, f.function), f.success))
            .collect()
    };
    if let Some(base) = base {
        let b = verdicts(base);
        for run in plains.iter().skip(1) {
            let r = verdicts(run);
            for key in b.keys().chain(r.keys()) {
                if b.get(key) != r.get(key) {
                    varying.insert(key.2.clone());
                }
            }
            if compare(base, run)
                .differences
                .iter()
                .any(|d| !d.starts_with("function "))
            {
                other = true;
            }
        }
    }
    let mut out: Vec<String> = varying.into_iter().collect();
    if other {
        out.push("<diagnostics>".to_owned());
    }
    out
}

/// Classifies a sail run against `plains` (at least one plain run).
#[must_use]
pub fn classify(plains: &[RunRecord], sail: &RunRecord) -> Classification {
    let Some(first) = plains.first() else {
        return Classification {
            verdict: Verdict::InfraError,
            comparison: Comparison::default(),
            plain_runs: 0,
            unstable: Vec::new(),
        };
    };
    let comparison = compare(first, sail);
    let unstable = plain_variation(plains);
    let verdict = if comparison.verdict() == Verdict::Same {
        Verdict::Same
    } else if !unstable.is_empty()
        && plains
            .iter()
            .any(|p| compare(p, sail).verdict() == Verdict::Same)
    {
        Verdict::Unstable
    } else {
        Verdict::Different
    };
    Classification {
        verdict,
        comparison,
        plain_runs: plains.len(),
        unstable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(fns: &[(&str, bool, u64)]) -> serde_json::Value {
        let breakdown: Vec<_> = fns
            .iter()
            .map(|(f, ok, r)| json!({"function": f, "success": ok, "rlimit": r, "time-micros": 1}))
            .collect();
        json!({
            "verification-results": {"verified": 1, "errors": 0},
            "func-details": {},
            "times-ms": {"smt": {"smt-run-module-times": [{"module": "", "function-breakdown": breakdown}]}},
        })
    }

    fn record(exit: i32, stderr: &str, fns: &[(&str, bool, u64)]) -> RunRecord {
        RunRecord {
            exit_code: Some(exit),
            stderr: stderr.to_owned(),
            outputs: vec![doc(fns)],
            stdout_rest: String::new(),
            wall_ms: 1.0,
            user_s: None,
            sys_s: None,
            max_rss_bytes: None,
        }
    }

    #[test]
    fn sail_matching_one_of_varying_plain_runs_is_unstable() {
        let proved = record(0, "", &[("m::f", true, 10)]);
        let gave_up = record(1, "rlimit\n", &[("m::f", false, 30)]);
        let c = classify(&[gave_up.clone(), proved.clone()], &proved);
        assert_eq!(c.verdict, Verdict::Unstable);
        assert_eq!(
            c.unstable,
            vec!["m::f".to_owned(), "<diagnostics>".to_owned()]
        );
        assert_eq!(c.plain_runs, 2);
    }

    #[test]
    fn sail_matching_no_plain_run_is_different() {
        let a = record(0, "", &[("m::f", true, 10)]);
        let b = record(1, "x\n", &[("m::f", false, 10)]);
        let other = record(1, "y\n", &[("m::f", false, 10)]);
        assert_eq!(
            classify(&[a.clone(), b], &other).verdict,
            Verdict::Different
        );
        // Stable plain runs and a differing sail run: different.
        assert_eq!(
            classify(&[a.clone(), a.clone()], &other).verdict,
            Verdict::Different
        );
        assert_eq!(
            classify(std::slice::from_ref(&a), &a).verdict,
            Verdict::Same
        );
    }

    #[test]
    fn accepted_progress_order_does_not_accept_changed_verdicts_or_exit_codes() {
        let a = record(
            0,
            "note: verifying module a\n\nnote: verifying module b\n\n",
            &[("m::f", false, 10)],
        );
        let mut b = record(
            0,
            "note: verifying module b\n\nnote: verifying module a\n\n",
            &[("m::f", false, 10)],
        );
        let comparison = compare(&a, &b);
        assert_eq!(comparison.verdict(), Verdict::Same);
        assert_eq!(comparison.accepted.len(), 1);
        b.exit_code = Some(1);
        assert_eq!(compare(&a, &b).verdict(), Verdict::Different);
        let promoted = record(0, &b.stderr, &[("m::f", true, 10)]);
        assert_eq!(compare(&a, &promoted).verdict(), Verdict::Different);
    }

    #[test]
    fn identical_runs_are_the_same() {
        let a = record(0, "ok\n", &[("m::f", true, 10)]);
        let c = compare(&a, &a.clone());
        assert_eq!(c.verdict(), Verdict::Same);
        assert_eq!(c.functions, 1);
    }

    #[test]
    fn verdict_exit_and_diagnostic_differences_are_reported() {
        let a = record(0, "x\ny\n", &[("m::f", true, 10)]);
        let b = record(1, "x\nz\n", &[("m::f", false, 10)]);
        let c = compare(&a, &b);
        assert_eq!(c.verdict(), Verdict::Different);
        assert_eq!(c.differences.len(), 3, "{:?}", c.differences);
        assert!(c.differences[1].contains("line 2"));
    }

    #[test]
    fn messages_from_different_modules_may_come_in_any_order() {
        // ADR 0042, on the real debug_expand.rs variants.
        let m4_first = include_str!("../tests/fixtures/debug-expand-order/m4-first.stderr");
        let root_first = include_str!("../tests/fixtures/debug-expand-order/root-first.stderr");
        let a = record(1, m4_first, &[("m::f", false, 10)]);
        let b = record(1, root_first, &[("m::f", false, 10)]);
        assert_eq!(compare(&a, &b).verdict(), Verdict::Same);
        assert_eq!(
            classify(std::slice::from_ref(&a), &b).verdict,
            Verdict::Same
        );
        // A message missing from sail is still a difference.
        let c = record(
            1,
            &root_first.replacen("error: assertion failed\n", "", 1),
            &[("m::f", false, 10)],
        );
        assert_eq!(classify(&[a.clone(), a], &c).verdict, Verdict::Different);
    }

    #[test]
    fn rlimit_drift_alone_is_not_a_difference() {
        let a = record(0, "", &[("m::f", true, 10)]);
        let b = record(0, "", &[("m::f", true, 11)]);
        let c = compare(&a, &b);
        assert_eq!(c.verdict(), Verdict::Same);
        assert_eq!(c.rlimit_drift, vec![("m::f".to_owned(), 10, 11)]);
    }

    #[test]
    fn every_crate_document_is_compared_in_order() {
        let mut a = record(0, "", &[("m::f", true, 10)]);
        a.outputs.push(doc(&[("m::f", true, 10)]));
        let mut b = a.clone();
        assert_eq!(compare(&a, &b).verdict(), Verdict::Same);
        assert_eq!(compare(&a, &b).functions, 2, "same name in two crates");

        // A verdict change in the second crate only.
        b.outputs[1] = doc(&[("m::f", false, 10)]);
        let c = compare(&a, &b);
        assert_eq!(c.verdict(), Verdict::Different);
        assert!(
            c.differences.iter().any(|d| d.contains("success=true")),
            "{c:?}"
        );

        // A crate's results missing entirely.
        b.outputs.truncate(1);
        let c = compare(&a, &b);
        assert!(
            c.differences
                .iter()
                .any(|d| d.contains("documents: plain 2 / sail 1")),
            "{c:?}"
        );

        // func-details in the second crate.
        let mut b = a.clone();
        b.outputs[1]["func-details"] = json!({"m::f": {"x": 1}});
        assert!(
            compare(&a, &b)
                .differences
                .iter()
                .any(|d| d == "func-details differs (output 1)")
        );
    }

    #[test]
    fn trailing_stdout_text_is_compared() {
        let a = record(0, "", &[("m::f", true, 10)]);
        let mut b = a.clone();
        b.stdout_rest = "extra".to_owned();
        assert_eq!(compare(&a, &b).verdict(), Verdict::Different);
    }

    #[test]
    fn missing_functions_are_differences() {
        let a = record(0, "", &[("m::f", true, 10), ("m::g", true, 1)]);
        let b = record(0, "", &[("m::f", true, 10), ("m::h", true, 1)]);
        assert_eq!(compare(&a, &b).differences.len(), 2);
    }
}
