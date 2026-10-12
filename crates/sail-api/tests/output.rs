//! The output schema: serde round trips, the JSON Schema, and invariants.

use sail_api::output::{
    AnswerName, Assignment, AttributionInfo, AttributionStatus, CacheInfo, CheckReport,
    Counterexample, CrateReport, Diagnostic, DiagnosticSpan, ErrorInfo, Fragility, FunctionReport,
    JSON_SCHEMA, ObligationReport, OutcomeName, ProvenanceReport, Report, SCHEMA_VERSION,
    SolverInfo, SourceName, SourceSpan, SourceSummaryName, SpinoffInfo, Summary, TierName, Timing,
    VerdictName, VerusResults,
};
use serde_json::Value;

fn schema() -> Value {
    match serde_json::from_str(JSON_SCHEMA) {
        Ok(v) => v,
        Err(e) => unreachable!("the JSON Schema does not parse: {e}"),
    }
}

/// A report with every optional field set, so every schema branch is used.
#[allow(clippy::too_many_lines, reason = "one literal document")]
fn full() -> Report {
    let obligation = ObligationReport {
        id: "demo::bad#Function-Def/spinoff-1".to_owned(),
        kind: "Function-Def".to_owned(),
        ordinal: 0,
        spinoff: Some(SpinoffInfo {
            index: 1,
            reason: Some("bitvector".to_owned()),
        }),
        advisory: false,
        span: Some(SourceSpan::parse("src/lib.rs:10:1: 10:20 (#0)")),
        outcome: OutcomeName::Failed,
        unstable: false,
        checks: vec![CheckReport {
            answer: Some(AnswerName::Sat),
            rlimit_limit: Some(30_000_000),
            rlimit_used: Some(120),
            reason_unknown: Some("(:reason-unknown \"x\")".to_owned()),
            source: Some(SourceName::Cache),
        }],
        counterexample: Some(Counterexample {
            check: 0,
            model: "(\n  (define-fun x! () Int 0)\n)".to_owned(),
            assignments: vec![Assignment {
                name: "x!".to_owned(),
                sort: "Int".to_owned(),
                value: "0".to_owned(),
            }],
        }),
    };
    let function = FunctionReport {
        function: "demo::bad".to_owned(),
        module: String::new(),
        fn_mode: Some("exec".to_owned()),
        verdict: VerdictName::Failed,
        tier: TierName::Confirmed,
        unstable: false,
        verus_success: Some(true),
        provenance: ProvenanceReport {
            configuration: None,
            mode: "default".to_owned(),
            source: SourceSummaryName::Cache,
            checks_solved: 0,
            checks_served: 1,
            checks_unrecorded: 0,
            engine: None,
        },
        timing: Timing {
            smt_micros: Some(447),
            rlimit: Some(581),
        },
        fragility: Some(Fragility {
            score: 0.25,
            runs: 4,
            proved: 3,
        }),
        obligations: Some(vec![obligation]),
    };
    let crates = vec![CrateReport {
        name: Some("demo".to_owned()),
        verus: VerusResults {
            verified: Some(1),
            errors: Some(1),
            success: Some(false),
            encountered_vir_error: Some(false),
        },
        functions: vec![function],
    }];
    Report {
        portfolio: None,
        schema_version: SCHEMA_VERSION,
        sail_version: "0.0.0".to_owned(),
        mode: "default".to_owned(),
        command: vec!["--".to_owned(), "--rlimit".to_owned(), "20".to_owned()],
        exit_code: Some(1),
        solver: Some(SolverInfo {
            name: "z3".to_owned(),
            version: Some("4.16.0".to_owned()),
            path: Some("/opt/z3".to_owned()),
            sha256: Some("ab".repeat(32)),
        }),
        cache: CacheInfo { daemon: true },
        attribution: AttributionInfo {
            status: AttributionStatus::Partial,
            notes: vec!["one log was overwritten".to_owned()],
            unmatched_sessions: 1,
        },
        summary: Summary::of(&crates),
        crates,
        diagnostics: vec![Diagnostic {
            level: "error".to_owned(),
            code: Some("E0308".to_owned()),
            message: "postcondition not satisfied".to_owned(),
            spans: vec![DiagnosticSpan {
                file: "src/lib.rs".to_owned(),
                line: 10,
                column: 13,
                primary: true,
            }],
            rendered: "error: postcondition not satisfied".to_owned(),
        }],
        error: Some(ErrorInfo {
            code: "SAIL0101".to_owned(),
            message: "x".to_owned(),
        }),
        engine: None,
    }
}

/// The smallest valid report: every optional field unset.
fn minimal() -> Report {
    Report {
        portfolio: None,
        schema_version: SCHEMA_VERSION,
        sail_version: "0.0.0".to_owned(),
        mode: "default".to_owned(),
        command: Vec::new(),
        exit_code: None,
        solver: None,
        cache: CacheInfo { daemon: false },
        attribution: AttributionInfo {
            status: AttributionStatus::Unavailable,
            notes: Vec::new(),
            unmatched_sessions: 0,
        },
        crates: Vec::new(),
        diagnostics: Vec::new(),
        summary: Summary::default(),
        error: None,
        engine: None,
    }
}

fn round_trip(report: &Report) {
    let text = match serde_json::to_string_pretty(report) {
        Ok(t) => t,
        Err(e) => unreachable!("serialize: {e}"),
    };
    let back: Report = match serde_json::from_str(&text) {
        Ok(r) => r,
        Err(e) => unreachable!("deserialize: {e}\n{text}"),
    };
    assert_eq!(&back, report);
    let value: Value = serde_json::from_str(&text).unwrap_or_default();
    assert_eq!(
        sail_api::schema::validate(&schema(), &value),
        Vec::<String>::new(),
        "{text}"
    );
    assert_eq!(report.invariant_violations(), Vec::<String>::new());
}

#[test]
fn full_report_round_trips_and_matches_the_schema() {
    round_trip(&full());
}

#[test]
fn minimal_report_round_trips_and_matches_the_schema() {
    round_trip(&minimal());
}

#[test]
fn unknown_fields_are_ignored_by_types_and_rejected_by_the_schema() {
    // Consumers must accept fields added later in version 1; the published
    // schema is strict so sail's own tests catch drift.
    let mut value = serde_json::to_value(minimal()).unwrap_or_default();
    if let Some(o) = value.as_object_mut() {
        o.insert("surprise".to_owned(), Value::Bool(true));
    }
    assert_eq!(
        serde_json::from_value::<Report>(value.clone()).ok(),
        Some(minimal())
    );
    assert_eq!(sail_api::schema::validate(&schema(), &value).len(), 1);
}

#[test]
fn every_field_is_required_by_the_schema() {
    // Removing any field the types write makes the document invalid, so the
    // schema cannot fall behind the types.
    let value = serde_json::to_value(full()).unwrap_or_default();
    let mut paths = Vec::new();
    collect_fields(&value, "", &mut paths);
    assert!(paths.len() > 50, "{}", paths.len());
    for path in paths {
        let mut doc = value.clone();
        remove_field(&mut doc, &path);
        assert_ne!(
            sail_api::schema::validate(&schema(), &doc),
            Vec::<String>::new(),
            "removing {path} still validates"
        );
    }
}

/// Every object field's path, as `/a/0/b` pointers.
fn collect_fields(v: &Value, at: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            for (k, child) in o {
                let path = format!("{at}/{k}");
                out.push(path.clone());
                collect_fields(child, &path, out);
            }
        }
        Value::Array(a) => {
            for (i, child) in a.iter().enumerate() {
                collect_fields(child, &format!("{at}/{i}"), out);
            }
        }
        _ => {}
    }
}

fn remove_field(doc: &mut Value, path: &str) {
    let Some((parent, key)) = path.rsplit_once('/') else {
        return;
    };
    if let Some(Value::Object(o)) = doc.pointer_mut(parent) {
        o.remove(key);
    }
}

#[test]
fn schema_rejects_wrong_values() {
    let mut value = serde_json::to_value(full()).unwrap_or_default();
    if let Some(f) = value.pointer_mut("/crates/0/functions/0") {
        f["verdict"] = Value::String("passed".to_owned());
        f["tier"] = Value::Null;
    }
    if let Some(v) = value.get_mut("schema_version") {
        *v = Value::from(2);
    }
    assert_eq!(sail_api::schema::validate(&schema(), &value).len(), 3);
}

#[test]
fn invariants_catch_provisional_verified_and_bad_counts() {
    let mut report = full();
    if let Some(f) = report
        .crates
        .first_mut()
        .and_then(|c| c.functions.first_mut())
    {
        f.verdict = VerdictName::Verified;
        f.tier = TierName::Provisional;
        f.provenance.source = SourceSummaryName::Fresh;
    }
    let v = report.invariant_violations();
    assert!(
        v.iter().any(|m| m.contains("verified but not confirmed")),
        "{v:?}"
    );
    assert!(v.iter().any(|m| m.contains("provenance counts")), "{v:?}");
    assert!(v.iter().any(|m| m.contains("summary")), "{v:?}");
}
