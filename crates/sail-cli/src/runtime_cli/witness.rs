//! Recheck a saved liveness counterexample with a trusted external Dive installation.

use super::external_dive;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use std::{collections::BTreeMap, fs, io::Write, path::Path, process::Command};

fn failure(message: impl std::fmt::Display) -> String {
    format!("error[SAIL6102]: witness recheck: {message}")
}

fn validated(doc: &Value) -> bool {
    let word = &doc["repair"]["trace"];
    let Some(rows) = word["states"].as_array().filter(|rows| !rows.is_empty()) else {
        return false;
    };
    let len = u64::try_from(rows.len()).ok();
    let ending = &word["ending"];
    let closes = match ending["kind"].as_str() {
        Some("stuttering") => true,
        Some("lasso") => ending["back_to"]
            .as_u64()
            .zip(len)
            .is_some_and(|(back, n)| back < n),
        _ => false,
    };
    doc["command"] == "validate-trace"
        && doc["schema_version"] == 4
        && doc["result"] == "valid"
        && matches!(
            doc["repair"]["status"].as_str(),
            Some("unchanged" | "repaired")
        )
        && closes
        && word["length"].as_u64() == len
        && doc["length"].as_u64() == len
        && word["properties"].as_array().is_some_and(|v| {
            !v.is_empty() && v.iter().all(|p| p.as_str().is_some_and(|n| !n.is_empty()))
        })
        && sail_dive::native_to_itf(word).is_ok()
}

/// Dive's default TLC exit convention is independent of the semantics profile.
fn ordinary(report: &Value, code: u8) -> bool {
    if report["command"] != "check"
        || !matches!(report["schema_version"].as_u64(), Some(3 | 4))
        || report["exit_code"].as_u64() != Some(u64::from(code))
        || !matches!(report["check_kind"].as_str(), Some("temporal" | "safety"))
    {
        return false;
    }
    match report["verdict"].as_str() {
        Some("success") => {
            code == 0
                && report["tlc"]["code"] == 2193
                && report["statistics"]["distinct"].is_u64()
                && report["counterexample"].is_null()
        }
        Some("invariant-violation") => code == 12,
        Some("liveness-violation" | "property-violation" | "action-property-violation") => {
            code == 13
        }
        Some("deadlock") => code == 11,
        Some("assumption-failure" | "postcondition-failure") => code == 10,
        Some("unsupported" | "resource-limit") => code == 3,
        Some("internal-error") => code == 4,
        Some("stopped") => code == 5,
        Some("rejected") => matches!(code, 150 | 151),
        Some(
            "model-error" | "assertion-failure" | "evaluation-error" | "overflow"
            | "incomplete-state",
        ) => {
            code != 0
                && (report["tlc"]["exit_status"].as_u64() == Some(u64::from(code))
                    || (code == 4 && report["tlc"].is_null()))
        }
        _ => false,
    }
}

pub(super) fn run(args: &[String], out: &mut dyn Write) -> Result<u8, String> {
    let started = Instant::now();
    let usage = || {
        "error[SAIL6101]: usage: sail runtime witness-recheck --dive FILE --module MODULE --config CFG --trace WORD --time-limit SECONDS".to_owned()
    };
    let mut flags = BTreeMap::new();
    let mut it = args.iter();
    while let Some(name) = it.next() {
        if !matches!(
            name.as_str(),
            "--dive" | "--module" | "--config" | "--trace" | "--time-limit"
        ) || flags
            .insert(name.as_str(), it.next().ok_or_else(usage)?)
            .is_some()
        {
            return Err(usage());
        }
    }
    let required = |k: &str| flags.get(k).copied().ok_or_else(usage);
    let file = |k: &str| fs::canonicalize(required(k)?).map_err(failure);
    let dive = file("--dive")?;
    let module = file("--module")?;
    let config = file("--config")?;
    let trace = Path::new(required("--trace")?);
    let seconds = required("--time-limit")?
        .parse::<std::num::NonZeroU64>()
        .map_err(|_| usage())?;
    let budget = Duration::from_secs(seconds.get());
    let cancel = external_dive::cancellation()?;
    let command = |sub: &str| {
        let mut cmd = Command::new(&dive);
        cmd.arg(sub)
            .arg(&module)
            .arg("--config")
            .arg(&config)
            .args([
                "--compat=dive",
                "--seed",
                "0",
                "--fp",
                "0",
                "--output",
                "json",
            ]);
        cmd
    };
    let stopped = || cancel.load(Ordering::Relaxed) || started.elapsed() >= budget;
    let emit_stop = |out: &mut dyn Write| {
        let (code, bytes) = external_dive::stop(&cancel, "witness");
        out.write_all(&bytes).map_err(failure)?;
        writeln!(out).map_err(failure)?;
        Ok(code)
    };
    if fs::metadata(trace).is_ok_and(|f| f.is_file() && f.len() <= 8 << 20) {
        let mut cmd = command("validate-trace");
        cmd.arg("--trace").arg(trace).arg("--repair");
        // The worker returns the exact word it checked on both successful routes.
        // A stale candidate, an older worker or a failed optional exchange can retry.
        let attempt = external_dive::run(cmd, started, budget, &cancel, "witness");
        if stopped() {
            return emit_stop(out);
        }
        if let Ok((0, bytes)) = attempt
            && let Ok(report) = serde_json::from_slice::<Value>(&bytes)
            && validated(&report)
        {
            if stopped() {
                return emit_stop(out);
            }
            writeln!(
                out,
                "{}",
                json!({"route":"validated-witness","validation":report})
            )
            .map_err(failure)?;
            return Ok(13);
        }
    }
    let remaining = budget.saturating_sub(started.elapsed()).as_millis();
    if remaining == 0 || stopped() {
        return emit_stop(out);
    }
    let mut cmd = command("check");
    cmd.args(["--workers", "1", "--exit-codes", "tlc", "--time-limit"])
        .arg(format!("{remaining}ms"));
    let (code, bytes) = external_dive::run(cmd, started, budget, &cancel, "witness")?;
    if stopped() {
        return emit_stop(out);
    }
    let report: Value = serde_json::from_slice(&bytes).map_err(failure)?;
    if !ordinary(&report, code) {
        return Err(failure("ordinary check returned an inconsistent report"));
    }
    writeln!(out, "{}", json!({"route":"current-check","report":report})).map_err(failure)?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_a_complete_returned_named_periodic_word() {
        let mut report = json!({"command":"validate-trace","schema_version":4,"result":"valid","length":1,
            "repair":{"status":"unchanged","trace":{"format":"dive-trace","version":1,
            "length":1,"variables":["x"],"properties":["L"],"ending":{"kind":"lasso","back_to":0},
            "states":[{"values":{"x":0},"action":null}]}}});
        assert!(validated(&report));
        report["repair"]["status"] = json!("repaired");
        assert!(validated(&report));
        for (path, bad) in [
            ("/repair/status", json!("unrepaired")),
            ("/repair/trace/properties", json!([])),
            ("/repair/trace/states", json!([null])),
            ("/repair/trace/states/0/values", json!({"y":0})),
            ("/repair/trace/states/0/values/x", json!({"bogus":1})),
            ("/repair/trace/ending/back_to", json!(1)),
            ("/repair/trace/ending/kind", json!("behavior")),
            ("/repair/trace/length", json!(2)),
            ("/length", json!(2)),
            ("/command", json!("check")),
            ("/result", json!("invalid")),
            ("/repair/trace", Value::Null),
        ] {
            let mut wrong = report.clone();
            if let Some(slot) = wrong.pointer_mut(path) {
                *slot = bad;
            }
            assert!(!validated(&wrong), "{path}");
        }
    }

    #[test]
    fn ordinary_check_requires_a_known_verdict_with_its_own_status() {
        let mut report = json!({"command":"check","schema_version":4,"check_kind":"safety",
            "exit_code":0,"verdict":"success","tlc":{"code":2193},"statistics":{"distinct":1},
            "counterexample":null});
        assert!(ordinary(&report, 0));
        assert!(!ordinary(&report, 12));
        for (key, value) in [
            ("verdict", json!("forged")),
            ("verdict", json!("invariant-violation")),
            ("schema_version", json!(999)),
            ("statistics", Value::Null),
        ] {
            let mut wrong = report.clone();
            wrong[key] = value;
            assert!(!ordinary(&wrong, 0), "{key}");
        }
        report["verdict"] = json!("rejected");
        report["exit_code"] = json!(151);
        assert!(ordinary(&report, 151));
    }
}
