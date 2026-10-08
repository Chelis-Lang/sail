//! Differential conformance: sail against plain Verus on the same unit.
//!
//! Charter rule 1 says default mode must give the same per-function verdicts
//! and the same diagnostics as plain Verus with the pinned Z3. [`compare`]
//! checks, for one unit:
//! - the exit code;
//! - stderr, byte for byte (diagnostics);
//! - Verus's `verification-results` and `func-details`;
//! - each function's verdict.
//!
//! Any difference makes the unit `different`. Per-function rlimit
//! differences are reported separately as *drift*: rlimit is not part of
//! rule 1, but it should be identical in pass-through mode, so drift is a
//! finding to investigate.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use sail_verus::run::RunRecord;
use serde::Serialize;

/// The outcome for one unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Identical in everything rule 1 covers.
    Same,
    /// At least one difference.
    Different,
    /// The harness could not run one side (not a conformance result).
    InfraError,
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

fn first_difference(a: &str, b: &str) -> String {
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
    if plain.stderr != sail.stderr {
        c.differences.push(format!(
            "diagnostics (stderr) differ at {}",
            first_difference(&plain.stderr, &sail.stderr)
        ));
    }
    match (&plain.json, &sail.json) {
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => c
            .differences
            .push("only one side produced --output-json".to_owned()),
        (Some(p), Some(s)) => {
            for key in ["verification-results", "func-details"] {
                if p.get(key) != s.get(key) {
                    c.differences.push(format!("{key} differs"));
                }
            }
        }
    }
    let index = |r: &RunRecord| -> BTreeMap<(String, String), (bool, u64)> {
        r.functions()
            .into_iter()
            .map(|f| ((f.module, f.function), (f.success, f.rlimit)))
            .collect()
    };
    let (p, s) = (index(plain), index(sail));
    c.functions = p.len();
    for (key, (p_ok, p_rlimit)) in &p {
        match s.get(key) {
            None => c
                .differences
                .push(format!("function {} missing from sail run", key.1)),
            Some((s_ok, _)) if s_ok != p_ok => c.differences.push(format!(
                "function {}: plain success={p_ok} / sail success={s_ok}",
                key.1
            )),
            Some((_, s_rlimit)) if s_rlimit != p_rlimit => {
                c.rlimit_drift.push((key.1.clone(), *p_rlimit, *s_rlimit));
            }
            Some(_) => {}
        }
    }
    for key in s.keys().filter(|k| !p.contains_key(*k)) {
        c.differences
            .push(format!("function {} missing from plain run", key.1));
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(exit: i32, stderr: &str, fns: &[(&str, bool, u64)]) -> RunRecord {
        let breakdown: Vec<_> = fns
            .iter()
            .map(|(f, ok, r)| json!({"function": f, "success": ok, "rlimit": r, "time-micros": 1}))
            .collect();
        RunRecord {
            exit_code: Some(exit),
            stderr: stderr.to_owned(),
            json: Some(json!({
                "verification-results": {"verified": 1, "errors": 0},
                "func-details": {},
                "times-ms": {"smt": {"smt-run-module-times": [{"module": "", "function-breakdown": breakdown}]}},
            })),
            wall_ms: 1.0,
            user_s: None,
            sys_s: None,
            max_rss_bytes: None,
        }
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
    fn rlimit_drift_alone_is_not_a_difference() {
        let a = record(0, "", &[("m::f", true, 10)]);
        let b = record(0, "", &[("m::f", true, 11)]);
        let c = compare(&a, &b);
        assert_eq!(c.verdict(), Verdict::Same);
        assert_eq!(c.rlimit_drift, vec![("m::f".to_owned(), 10, 11)]);
    }

    #[test]
    fn missing_functions_are_differences() {
        let a = record(0, "", &[("m::f", true, 10), ("m::g", true, 1)]);
        let b = record(0, "", &[("m::f", true, 10), ("m::h", true, 1)]);
        assert_eq!(compare(&a, &b).differences.len(), 2);
    }
}
