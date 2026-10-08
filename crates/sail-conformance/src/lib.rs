//! Differential conformance: sail against plain Verus on the same unit.
//!
//! Charter rule 1 says default mode must give the same per-function verdicts
//! and the same diagnostics as plain Verus with the pinned Z3. [`compare`]
//! checks, for one unit:
//! - the exit code;
//! - stderr, byte for byte (diagnostics);
//! - the number of `--output-json` documents (one per verified crate) and,
//!   document by document in stdout order, Verus's `verification-results`
//!   and `func-details`;
//! - any non-JSON text left on stdout;
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
