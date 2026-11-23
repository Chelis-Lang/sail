//! Recheck a saved liveness counterexample with a trusted external Dive installation.

mod donors;
mod report;
use super::external_dive;
use report::{ordinary, periodic};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

fn failure(message: impl std::fmt::Display) -> String {
    format!("error[SAIL6102]: witness recheck: {message}")
}

fn validated(doc: &Value) -> bool {
    let repair = &doc["repair"];
    let word = &repair["trace"];
    let success_fields = |value: &Value, keys: &[&str]| {
        value.as_object().is_some_and(|fields| {
            fields.len() == keys.len() && keys.iter().all(|key| fields.contains_key(*key))
        })
    };
    // A schema-4 successful report has no validation exception, rejection or failure.
    success_fields(
        doc,
        &[
            "command",
            "schema_version",
            "result",
            "length",
            "notes",
            "repair",
        ],
    ) && success_fields(
        repair,
        &[
            "status",
            "candidate_validation",
            "reason",
            "gaps",
            "generated",
            "search_ms",
            "validation_ms",
            "trace",
        ],
    ) && doc["command"] == "validate-trace"
        && doc["schema_version"] == 4
        && doc["result"] == "valid"
        && doc["notes"]
            .as_array()
            .is_some_and(|notes| notes.iter().all(Value::is_string))
        && repair["reason"].is_null()
        && repair["candidate_validation"].is_null()
        && repair["validation_ms"].as_f64().is_some_and(|ms| ms >= 0.)
        && repair["search_ms"].as_f64().is_some_and(|ms| ms >= 0.)
        && match (
            repair["status"].as_str(),
            repair["gaps"].as_u64(),
            repair["generated"].as_u64(),
        ) {
            (Some("unchanged"), Some(0), Some(0)) => repair["search_ms"].as_f64() == Some(0.),
            (Some("repaired"), Some(gaps @ 1..=128), Some(generated @ 1..=32768)) => {
                gaps <= generated
            }
            _ => false,
        }
        && matches!(word["length"].as_u64(), Some(1..=4096))
        && doc["length"] == word["length"]
        && periodic(word)
}

enum Input<'a> {
    Word(&'a Path),
    Index(&'a Path, &'a str),
}

struct Job<'a> {
    dive: PathBuf,
    module: PathBuf,
    config: PathBuf,
    input: Input<'a>,
    budget: Duration,
}

fn parse(args: &[String]) -> Result<Job<'_>, String> {
    let usage = || {
        "error[SAIL6101]: usage: sail runtime witness-recheck --dive FILE --module MODULE --config CFG (--trace WORD | --donor-index INDEX --property PROPERTY) --time-limit SECONDS".to_owned()
    };
    let mut flags = BTreeMap::new();
    let mut it = args.iter();
    while let Some(name) = it.next() {
        if !matches!(
            name.as_str(),
            "--dive"
                | "--module"
                | "--config"
                | "--trace"
                | "--time-limit"
                | "--donor-index"
                | "--property"
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
    let input = match (
        flags.get("--trace").copied(),
        flags.get("--donor-index").copied(),
        flags.get("--property").copied(),
    ) {
        (Some(trace), None, None) => Input::Word(Path::new(trace)),
        (None, Some(index), Some(property)) if !property.is_empty() => {
            Input::Index(Path::new(index), property)
        }
        _ => return Err(usage()),
    };
    let seconds = required("--time-limit")?
        .parse::<std::num::NonZeroU64>()
        .map_err(|_| usage())?;
    let budget = Duration::from_secs(seconds.get());
    Ok(Job {
        dive,
        module,
        config,
        input,
        budget,
    })
}

fn command(executable: &Path, subcommand: &str, module: &Path, config: &Path) -> Command {
    let mut cmd = Command::new(executable);
    cmd.arg(subcommand)
        .arg(module)
        .arg("--config")
        .arg(config)
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
}

pub(super) fn run(args: &[String], out: &mut dyn Write) -> Result<u8, String> {
    let started = Instant::now();
    let Job {
        dive,
        module,
        config,
        input,
        budget,
    } = parse(args)?;
    let cancel = external_dive::cancellation()?;
    let command = |sub: &str| command(&dive, sub, &module, &config);
    let stopped = || cancel.load(Ordering::Relaxed) || started.elapsed() >= budget;
    let emit_stop = |out: &mut dyn Write| {
        let (code, bytes) = external_dive::stop(&cancel, "witness");
        out.write_all(&bytes).map_err(failure)?;
        writeln!(out).map_err(failure)?;
        Ok(code)
    };
    let mut selection = None;
    if let Input::Index(index, property) = input {
        let found = donors::select(
            index,
            &module,
            &config,
            donors::session(&dive, property, started, budget, &cancel),
        );
        if stopped() {
            return emit_stop(out);
        }
        if let Some(report) = found.report {
            writeln!(
                out,
                "{}",
                json!({"route":"validated-witness","validation":report,"selection":found.detail})
            )
            .map_err(failure)?;
            return Ok(13);
        }
        selection = Some(found.detail);
    }
    if let Input::Word(trace) = input
        && fs::metadata(trace).is_ok_and(|f| f.is_file() && f.len() <= 8 << 20)
    {
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
    let ordinary_started = Instant::now();
    let (code, bytes) = external_dive::run(cmd, started, budget, &cancel, "witness")?;
    if stopped() {
        return emit_stop(out);
    }
    let report: Value = serde_json::from_slice(&bytes).map_err(failure)?;
    if report["file"].as_str() != module.to_str()
        || report["config"].as_str() != config.to_str()
        || !ordinary(&report, code)
    {
        return Err(failure("ordinary check returned an inconsistent report"));
    }
    if stopped() {
        return emit_stop(out);
    }
    let mut response = json!({"route":"current-check","report":report});
    if let Some(mut selection) = selection {
        selection["ordinary_ms"] = json!(ordinary_started.elapsed().as_secs_f64() * 1000.);
        response["selection"] = selection;
    }
    writeln!(out, "{response}").map_err(failure)?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_a_complete_returned_named_periodic_word() {
        let mut report = json!({"command":"validate-trace","schema_version":4,"result":"valid","length":1,"notes":[],
            "repair":{"status":"unchanged","gaps":0,"generated":0,"search_ms":0.0,
            "validation_ms":1.2,"reason":null,"candidate_validation":null,"trace":{"format":"dive-trace","version":1,
            "length":1,"variables":["x"],"properties":["L"],"ending":{"kind":"lasso","back_to":0},
            "states":[{"values":{"x":0},"action":null}]}}});
        assert!(validated(&report));
        report["repair"]["status"] = json!("repaired");
        assert!(!validated(&report));
        report["repair"]["gaps"] = json!(1);
        report["repair"]["generated"] = json!(2);
        assert!(validated(&report));
        for (key, value) in [
            ("validation_completed", json!(false)),
            ("error", json!({"kind":"resource"})),
            ("failure", json!({"check":"next"})),
            ("unsupported", json!("unbound property")),
        ] {
            let mut wrong = report.clone();
            wrong[key] = value;
            assert!(!validated(&wrong), "{key}");
        }
        for (path, bad) in [
            ("/repair/status", json!("unrepaired")),
            ("/repair/candidate_validation", json!({"result":"invalid"})),
            (
                "/repair/reason",
                json!("replacement failed independent validation"),
            ),
            ("/repair/gaps", json!(0)),
            ("/repair/generated", json!(0)),
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
            "exit_code":0,"verdict":"success","options":{"exit_codes":"tlc","semantic_profile":"dive"},
            "tlc":{"code":2193,"name":"TLC_SUCCESS","exit_status":0,"verified":true},"statistics":{"distinct":1},
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
        report["tlc"]["exit_status"] = json!(13);
        assert!(!ordinary(&report, 0));
        report["verdict"] = json!("liveness-violation");
        report["exit_code"] = json!(13);
        assert!(!ordinary(&report, 13));
        report["verdict"] = json!("stopped");
        report["exit_code"] = json!(5);
        assert!(!ordinary(&report, 5));
        report["tlc"] = Value::Null;
        report["stopped"] = json!({"resource":"time"});
        assert!(ordinary(&report, 5));
        report["diagnostics"] = json!([{"message":"invalid cfg"}]);
        report["verdict"] = json!("rejected");
        report["exit_code"] = json!(151);
        assert!(ordinary(&report, 151));
    }
}
