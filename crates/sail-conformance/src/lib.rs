//! Differential conformance: sail against plain Verus on the same unit.
//!
//! Charter rule 1 says default mode must give the same per-function verdicts
//! and the same diagnostics as plain Verus with the pinned Z3. [`compare`]
//! checks, for one unit:
//! - the exit code;
//! - stderr (diagnostics), under ADR 0042: complete diagnostic messages as
//!   a multiset with exact bytes, everything else byte for byte and in order
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
    /// Accepted divergences (`docs/divergences.md`), one line each; not
    /// differences. Today only VI-0003: stock reports a function whose
    /// `by (bit_vector)` assertion failed as `success: true`, Sail's fork as
    /// `false`, with everything else identical.
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
    c.differences
        .extend(diagnostics::compare_stderr(&plain.stderr, &sail.stderr));
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
    let (p, s) = (function_index(plain), function_index(sail));
    c.functions = p.values().map(Vec::len).sum();
    let rest_identical = c.differences.is_empty();
    let mut flips = Vec::new();
    for (key, plain_entries) in &p {
        let Some(sail_entries) = s.get(key) else {
            c.differences
                .push(format!("function {} missing from sail run", key.2));
            continue;
        };
        if plain_entries.len() != sail_entries.len() {
            c.differences.push(format!(
                "function {}: plain {} entries / sail {} entries",
                key.2,
                plain_entries.len(),
                sail_entries.len()
            ));
            continue;
        }
        for ((p_ok, p_rlimit), (s_ok, s_rlimit)) in plain_entries.iter().zip(sail_entries) {
            if *p_ok && !s_ok {
                flips.push((key, plain_entries.len()));
            } else if s_ok != p_ok {
                c.differences.push(format!(
                    "function {}: plain success={p_ok} / sail success={s_ok}",
                    key.2
                ));
            } else if s_rlimit != p_rlimit {
                c.rlimit_drift.push((key.2.clone(), *p_rlimit, *s_rlimit));
            }
        }
    }
    for key in s.keys().filter(|k| !p.contains_key(*k)) {
        c.differences
            .push(format!("function {} missing from plain run", key.2));
    }
    // The accepted reproduction identifies both the failing function and the
    // assertion location. A bit-vector diagnostic elsewhere in the crate is
    // never evidence for a different function's changed status. Duplicate
    // names are ambiguous, so their flips remain differences.
    let vi0003 = rest_identical
        && c.differences.is_empty()
        && plain.exit_code.is_some_and(|code| code != 0)
        && flips.len() == 1
        && flips
            .iter()
            .all(|(key, count)| *count == 1 && key.1 == VI0003_MODULE && key.2 == VI0003_FUNCTION)
        && plain
            .stderr
            .lines()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|lines| lines[0] == VI0003_MESSAGE && lines[1].trim() == VI0003_LOCATION);
    for (key, _) in flips {
        if vi0003 {
            c.accepted.push(format!(
                "function {}: plain success=true / sail success=false at the recorded bit-vector assertion (VI-0003)", key.2
            ));
        } else {
            c.differences.push(format!(
                "function {}: plain success=true / sail success=false",
                key.2
            ));
        }
    }
    c
}

// Recorded in docs/validation/mutation-suite.md, vest-logic-003. Additional
// reproductions need their own function/location evidence before acceptance.
const VI0003_MESSAGE: &str = "error: bitvector assertion not satisfied";
const VI0003_MODULE: &str = "combinators::uints::spec";
const VI0003_FUNCTION: &str = "vest_lib::combinators::uints::spec::lemma_u24_be_bytes_roundtrip";
const VI0003_LOCATION: &str = "--> vest_lib/src/combinators/uints/spec.rs:212:5";

type FunctionKey = (usize, String, String);
type FunctionIndex = BTreeMap<FunctionKey, Vec<(bool, u64)>>;

fn function_index(run: &RunRecord) -> FunctionIndex {
    let mut index = BTreeMap::<_, Vec<_>>::new();
    for function in run.functions() {
        index
            .entry((function.output, function.module, function.function))
            .or_default()
            .push((function.success, function.rlimit));
    }
    for entries in index.values_mut() {
        entries.sort_unstable();
    }
    index
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
    let verdicts = |run: &RunRecord| -> BTreeMap<FunctionKey, Vec<bool>> {
        function_index(run)
            .into_iter()
            .map(|(key, entries)| {
                (
                    key,
                    entries.into_iter().map(|(success, _)| success).collect(),
                )
            })
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

    fn vi0003_run(exit: i32, stderr: &str, f_ok: bool) -> RunRecord {
        RunRecord {
            exit_code: Some(exit),
            stderr: stderr.to_owned(),
            outputs: vec![{
                let mut output = doc(&[(VI0003_FUNCTION, f_ok, 1), ("g", true, 1)]);
                output["times-ms"]["smt"]["smt-run-module-times"][0]["module"] =
                    serde_json::json!(VI0003_MODULE);
                output
            }],
            stdout_rest: String::new(),
            wall_ms: 0.0,
            user_s: None,
            sys_s: None,
            max_rss_bytes: None,
        }
    }

    #[test]
    fn accepts_exactly_the_vi0003_divergence() {
        let bv = "error: bitvector assertion not satisfied\n   --> vest_lib/src/combinators/uints/spec.rs:212:5\n\nerror: aborting due to 1 previous error\n";
        // Stock: success=true despite the failed bit-vector assertion.
        let c = compare(&vi0003_run(1, bv, true), &vi0003_run(1, bv, false));
        assert_eq!(c.verdict(), Verdict::Same, "{c:?}");
        assert_eq!(c.accepted.len(), 1);
        // No bit-vector failure reported: a real difference.
        let other = "error: assertion failed\n   --> a.rs:3:5\n\n";
        let c = compare(&vi0003_run(1, other, true), &vi0003_run(1, other, false));
        assert_eq!(c.verdict(), Verdict::Different);
        // Diagnostics differ too: a difference.
        let c = compare(&vi0003_run(1, bv, true), &vi0003_run(1, other, false));
        assert_eq!(c.verdict(), Verdict::Different);
        // Stock passed: a difference.
        let c = compare(&vi0003_run(0, bv, true), &vi0003_run(0, bv, false));
        assert_eq!(c.verdict(), Verdict::Different);
        // The other direction (Sail more permissive) is never accepted.
        let c = compare(&vi0003_run(1, bv, false), &vi0003_run(1, bv, true));
        assert_eq!(c.verdict(), Verdict::Different);
    }

    #[test]
    fn bitvector_error_does_not_excuse_an_unrelated_function_or_location() {
        let bv = format!("{VI0003_MESSAGE}\n   {VI0003_LOCATION}\n");
        let plain = vi0003_run(1, &bv, true);
        let mut sail = vi0003_run(1, &bv, true);
        sail.outputs[0]["times-ms"]["smt"]["smt-run-module-times"][0]["function-breakdown"][1]["success"] =
            serde_json::json!(false);
        assert_eq!(compare(&plain, &sail).verdict(), Verdict::Different);
        let wrong = bv.replace(":212:5", ":213:5");
        assert_eq!(
            compare(&vi0003_run(1, &wrong, true), &vi0003_run(1, &wrong, false)).verdict(),
            Verdict::Different
        );
        let mut missing_exit = vi0003_run(1, &bv, true);
        missing_exit.exit_code = None;
        let mut missing_sail_exit = vi0003_run(1, &bv, false);
        missing_sail_exit.exit_code = None;
        assert_eq!(
            compare(&missing_exit, &missing_sail_exit).verdict(),
            Verdict::Different
        );
    }

    #[test]
    fn duplicate_function_entries_keep_every_verdict_and_occurrence() {
        let plain = record(1, "", &[("f", false, 1), ("f", true, 2)]);
        let same = record(1, "", &[("f", true, 2), ("f", false, 1)]);
        let comparison = compare(&plain, &same);
        assert_eq!(comparison.verdict(), Verdict::Same);
        assert_eq!(comparison.functions, 2);
        let wrong = record(1, "", &[("f", true, 1), ("f", true, 2)]);
        assert_eq!(compare(&plain, &wrong).verdict(), Verdict::Different);
        let missing = record(1, "", &[("f", true, 2)]);
        assert_eq!(compare(&plain, &missing).verdict(), Verdict::Different);
        assert_eq!(
            classify(&[plain, wrong.clone()], &wrong).verdict,
            Verdict::Unstable
        );
    }

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
