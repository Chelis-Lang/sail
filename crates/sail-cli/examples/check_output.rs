//! Checks a `sail verify --output json` document (used by `ci/run.sh e2e`).
//!
//! ```text
//! check_output <sail.json> <verus-stdout> [--attribution STATUS] [--source SOURCE]
//! ```
//!
//! - the document matches the JSON Schema and the schema's invariants;
//! - its per-function verdicts agree with Verus's own `--output-json`
//!   (`<verus-stdout>`, the stdout of `cargo verus verify -- --output-json
//!   --time`): `verified` exactly when Verus says `success`, except where an
//!   attributed bit-vector obligation failed (`docs/verus-issues/VI-0003`);
//! - optionally, attribution has the given status and every function's
//!   provenance has the given source summary.
#![forbid(unsafe_code)]

use std::process::ExitCode;

use sail_api::output::{JSON_SCHEMA, OutcomeName, Report, VerdictName};

fn check(args: &[String]) -> Result<String, Vec<String>> {
    let [sail, verus, rest @ ..] = args else {
        return Err(vec![
            "usage: check_output <sail.json> <verus-stdout> [--attribution S] [--source S]"
                .to_owned(),
        ]);
    };
    let read = |p: &str| std::fs::read(p).map_err(|e| vec![format!("{p}: {e}")]);
    let doc: serde_json::Value =
        serde_json::from_slice(&read(sail)?).map_err(|e| vec![format!("{sail}: {e}")])?;
    let schema: serde_json::Value =
        serde_json::from_str(JSON_SCHEMA).map_err(|e| vec![format!("schema: {e}")])?;
    let mut errors = sail_api::schema::validate(&schema, &doc);
    let report: Report =
        serde_json::from_value(doc).map_err(|e| vec![format!("{sail}: not a v1 report: {e}")])?;
    errors.extend(report.invariant_violations());
    let (docs, _) = sail_verus::attribution::json_documents(&read(verus)?);
    let expected = sail_verus::attribution::verus_verdicts(&docs);
    let functions: Vec<_> = report.crates.iter().flat_map(|c| &c.functions).collect();
    if expected.is_empty() {
        errors.push(format!("{verus}: no Verus per-function results"));
    }
    for (name, success) in &expected {
        let Some(f) = functions.iter().find(|f| &f.function == name) else {
            errors.push(format!("{name}: missing from the sail report"));
            continue;
        };
        let verified = f.verdict == VerdictName::Verified;
        let bit_vector_gap = f.obligations.iter().flatten().any(|o| {
            o.outcome == OutcomeName::Failed
                && o.spinoff
                    .as_ref()
                    .is_some_and(|s| s.reason.as_deref() == Some("bitvector"))
        });
        if Some(verified) != *success && !(bit_vector_gap && *success == Some(true)) {
            errors.push(format!(
                "{name}: sail says {:?}, Verus says success = {success:?}",
                f.verdict
            ));
        }
    }
    let mut it = rest.iter();
    while let Some(flag) = it.next() {
        let want = it.next().map(String::as_str).unwrap_or_default();
        match flag.as_str() {
            "--attribution" => {
                let got = serde_json::to_value(report.attribution.status).unwrap_or_default();
                if got != want {
                    errors.push(format!(
                        "attribution is {got}, expected {want}: {:?}",
                        report.attribution.notes
                    ));
                }
            }
            "--source" => {
                for f in &functions {
                    let got = serde_json::to_value(f.provenance.source).unwrap_or_default();
                    if got != want {
                        errors.push(format!("{}: source is {got}, expected {want}", f.function));
                    }
                }
            }
            other => errors.push(format!("unknown flag {other}")),
        }
    }
    if errors.is_empty() {
        Ok(format!(
            "{} functions agree with Verus ({} verified, {} failed); attribution {:?}",
            expected.len(),
            report.summary.verified,
            report.summary.failed,
            report.attribution.status
        ))
    } else {
        Err(errors)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match check(&args) {
        Ok(summary) => {
            println!("check_output: {summary}");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in errors {
                eprintln!("check_output: {e}");
            }
            ExitCode::FAILURE
        }
    }
}
