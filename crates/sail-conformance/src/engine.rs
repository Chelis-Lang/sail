//! Engine-specific checks for the source-engine conformance arm (ADR 0027).
//!
//! The arm compares the interactive engine's full-phase output with plain
//! Verus using [`crate::classify`], exactly like the other arms, on verdicts
//! and diagnostics. The check here adds what only the engine can get wrong:
//! every compiler seal must report normal termination, and a single-crate run
//! must seal exactly once, consistently with its exit code and its
//! `--output-json` document; otherwise the product would fall back to stock
//! and the engine run is not evidence.

use serde::Serialize;
use serde_json::Value;

use sail_verus::run::RunRecord;

/// What the engine reported about one run, beyond Verus's own output.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EngineStats {
    /// The exact function selected for the focused phase, if any.
    pub focus_function: Option<String>,
    /// Whether the focused phase finished (the selector matched exactly once).
    pub focus_established: bool,
    /// `compiler-sealed` observations.
    pub seals: usize,
}

/// Checks one engine run. `events` is the engine's JSONL observation journal
/// and `single_crate` is true for a `verus` (not `cargo verus`) unit.
#[must_use]
pub fn check(run: &RunRecord, events: &str, single_crate: bool) -> (Vec<String>, EngineStats) {
    let mut issues = Vec::new();
    let mut stats = EngineStats::default();
    let mut seal_success = Vec::new();
    for (index, line) in events.lines().enumerate() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            issues.push(format!("engine observation {} is not JSON", index + 1));
            continue;
        };
        match event["event"].as_str() {
            Some("compiler-sealed") => {
                stats.seals += 1;
                if event["phase"] != "full" || event["payload"]["normal_termination"] != true {
                    issues.push("engine compiler seal without normal termination".to_owned());
                }
                seal_success.push(event["payload"]["success"].as_bool());
            }
            Some("phase-finished") if event["phase"] == "focus" => stats.focus_established = true,
            _ => {}
        }
    }
    if single_crate && matches!(run.exit_code, Some(0 | 1)) {
        match seal_success.as_slice() {
            [Some(success)] => {
                if (run.exit_code == Some(0)) != *success {
                    issues.push(format!(
                        "engine seal success={success} contradicts exit code {:?}",
                        run.exit_code
                    ));
                }
                let documented = run
                    .outputs
                    .first()
                    .and_then(|doc| doc.pointer("/verification-results/success"))
                    .and_then(Value::as_bool);
                if documented.is_some_and(|documented| documented != *success) {
                    issues.push(format!(
                        "engine seal success={success} contradicts its --output-json document"
                    ));
                }
            }
            seals => issues.push(format!(
                "engine sealed {} times; exactly one complete seal is required (the product would fall back to stock)",
                seals.len()
            )),
        }
    } else if stats.seals < run.outputs.len() {
        issues.push(format!(
            "engine sealed {} of {} verified crates (the product would fall back to stock)",
            stats.seals,
            run.outputs.len()
        ));
    }
    (issues, stats)
}

/// The function to focus: the first proved function of the plain run, else
/// its first function. Deterministic for a given plain output.
#[must_use]
pub fn focus_function(plain: &RunRecord) -> Option<String> {
    let functions = plain.functions();
    functions
        .iter()
        .find(|f| f.success)
        .or_else(|| functions.first())
        .map(|f| f.function.clone())
        .filter(|name| name.contains("::"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(exit: i32, success: bool) -> RunRecord {
        RunRecord {
            exit_code: Some(exit),
            stderr: String::new(),
            outputs: vec![json!({
                "verification-results": {"success": success},
                "times-ms": {"smt": {"smt-run-module-times": [{"module": "", "function-breakdown": [
                    {"function": "demo::b", "success": true, "rlimit": 1},
                    {"function": "demo::a", "success": false, "rlimit": 1},
                ]}]}},
            })],
            stdout_rest: String::new(),
            wall_ms: 1.0,
            user_s: None,
            sys_s: None,
            max_rss_bytes: None,
        }
    }
    const SEALED_OK: &str = r#"{"version":1,"phase":"full","event":"compiler-sealed","payload":{"success":true,"normal_termination":true}}"#;
    const SEALED_FAIL: &str = r#"{"version":1,"phase":"full","event":"compiler-sealed","payload":{"success":false,"normal_termination":true}}"#;

    #[test]
    fn consistent_single_seal_passes() {
        let events = format!(
            "{}\n{SEALED_OK}\n",
            r#"{"version":1,"phase":"focus","event":"phase-finished","payload":{}}"#,
        );
        let (issues, stats) = check(&record(0, true), &events, true);
        assert_eq!(issues, Vec::<String>::new());
        assert!(stats.focus_established);
        assert_eq!(stats.seals, 1);
    }

    #[test]
    fn missing_contradictory_or_abnormal_seals_are_issues() {
        let run = record(0, true);
        assert_eq!(check(&run, "", true).0.len(), 1, "missing seal");
        assert_eq!(check(&run, &format!("{SEALED_FAIL}\n"), true).0.len(), 2);
        let failed = record(1, false);
        assert_eq!(check(&failed, &format!("{SEALED_OK}\n"), true).0.len(), 2);
        let abnormal = SEALED_OK.replace(",\"normal_termination\":true", "");
        assert_ne!(
            check(&run, &format!("{abnormal}\n"), true).0,
            Vec::<String>::new()
        );
        assert_ne!(check(&run, "{broken\n", true).0, Vec::<String>::new());
        let twice = format!("{SEALED_OK}\n{SEALED_OK}\n");
        assert_ne!(check(&run, &twice, true).0, Vec::<String>::new());
        // Cargo units: one seal per verified crate at least.
        assert_eq!(check(&run, &twice, false).0, Vec::<String>::new());
        assert_ne!(check(&run, "", false).0, Vec::<String>::new());
    }

    #[test]
    fn focus_prefers_a_proved_function() {
        assert_eq!(
            focus_function(&record(1, false)).as_deref(),
            Some("demo::b")
        );
    }
}
