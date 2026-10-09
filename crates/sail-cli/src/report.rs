//! Builds the `sail verify --output json` document (schema v1, ADR 0014)
//! from Verus's results, the attribution and the diagnostics.

use sail_api::output::{
    AttributionInfo, AttributionStatus, CacheInfo, CrateReport, Diagnostic, DiagnosticSpan,
    ErrorInfo, FunctionReport, Report, SCHEMA_VERSION, SolverInfo, Summary, VerusResults,
};
use sail_verus::attribution::{Attribution, Status};

/// Everything a report is built from.
pub(crate) struct Inputs<'a> {
    /// The arguments passed through to `cargo verus verify`.
    pub args: &'a [String],
    /// Its exit code.
    pub exit_code: Option<i32>,
    /// The pinned solver's path and SHA-256.
    pub solver_path: Option<String>,
    /// SHA-256 of the solver binary.
    pub solver_sha256: Option<String>,
    /// Whether cached sessions were enabled.
    pub daemon: bool,
    /// The attribution.
    pub attribution: &'a Attribution,
    /// Verus's diagnostics.
    pub diagnostics: &'a [sail_verus::diagnostics::Diagnostic],
}

fn diagnostic(d: &sail_verus::diagnostics::Diagnostic) -> Diagnostic {
    Diagnostic {
        level: d.level.clone(),
        code: d.code.clone(),
        message: d.message.clone(),
        spans: d
            .spans
            .iter()
            .map(|s| DiagnosticSpan {
                file: s.file.clone(),
                line: s.line,
                column: s.column,
                primary: s.primary,
            })
            .collect(),
        rendered: d.rendered.clone(),
    }
}

/// Builds the report.
pub(crate) fn build(inputs: &Inputs<'_>) -> Report {
    let a = inputs.attribution;
    let crates: Vec<CrateReport> = a
        .crates
        .iter()
        .map(|c| {
            let mut functions: Vec<FunctionReport> =
                c.functions.iter().map(FunctionReport::from).collect();
            functions.sort_by(|x, y| x.function.cmp(&y.function));
            CrateReport {
                name: c.name.clone(),
                verus: VerusResults {
                    verified: c.verified,
                    errors: c.errors,
                    success: c.success,
                    encountered_vir_error: c.encountered_vir_error,
                },
                functions,
            }
        })
        .collect();
    let mut report = Report {
        portfolio: None,
        schema_version: SCHEMA_VERSION,
        sail_version: env!("CARGO_PKG_VERSION").to_owned(),
        mode: "default".to_owned(),
        command: inputs.args.to_vec(),
        exit_code: inputs.exit_code,
        solver: Some(SolverInfo {
            name: "z3".to_owned(),
            version: a.solver_version.clone(),
            path: inputs.solver_path.clone(),
            sha256: inputs.solver_sha256.clone(),
        }),
        cache: CacheInfo {
            daemon: inputs.daemon,
        },
        attribution: AttributionInfo {
            status: match a.status {
                Status::Complete => AttributionStatus::Complete,
                Status::Partial => AttributionStatus::Partial,
                Status::Unavailable => AttributionStatus::Unavailable,
            },
            notes: a.notes.clone(),
            unmatched_sessions: a.unmatched_sessions,
        },
        summary: Summary::of(&crates),
        crates,
        diagnostics: inputs.diagnostics.iter().map(diagnostic).collect(),
        error: None,
    };
    crate::scope::restrict(&mut report);
    incomplete_failures(&mut report);
    report
}

/// Verus can omit a failed bit-vector function from its failure set (VI-0003).
/// In a failing crate, an incomplete mapping cannot resolve which successful
/// timing entries are real proofs. Preserve the raw field without confirming it.
fn incomplete_failures(report: &mut Report) {
    if report.attribution.status == AttributionStatus::Complete {
        return;
    }
    let mut uncertain = 0;
    for c in &mut report.crates {
        if c.verus.errors.is_some_and(|n| n > 0)
            || c.verus.success == Some(false)
            || c.verus.encountered_vir_error == Some(true)
        {
            for f in &mut c.functions {
                if f.verdict == sail_api::output::VerdictName::Verified {
                    f.verdict = sail_api::output::VerdictName::Unknown;
                    uncertain += 1;
                }
            }
        }
    }
    if uncertain > 0 {
        report.attribution.notes.push(format!(
            "{uncertain} successful timing entries in failing crates are uncertain because attribution is incomplete (VI-0003)"
        ));
        report.summary = Summary::of(&report.crates);
    }
}

/// A report for a run that never started Verus.
pub(crate) fn setup_failure(args: &[String], code: &str, message: &str) -> Report {
    Report {
        portfolio: None,
        schema_version: SCHEMA_VERSION,
        sail_version: env!("CARGO_PKG_VERSION").to_owned(),
        mode: "default".to_owned(),
        command: args.to_vec(),
        exit_code: None,
        solver: None,
        cache: CacheInfo { daemon: false },
        attribution: AttributionInfo {
            status: AttributionStatus::Unavailable,
            notes: vec!["Verus did not run".to_owned()],
            unmatched_sessions: 0,
        },
        crates: Vec::new(),
        diagnostics: Vec::new(),
        summary: Summary::default(),
        error: Some(ErrorInfo {
            code: code.to_owned(),
            message: message.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Golden test: the fixture under `tests/fixtures/attr` is a real Verus
    //! run on `input.rs` (pinned release, `--output-json --time --log
    //! smt-transcript --log air`), with its logs trimmed to their structure
    //! (each batch keeps only `push`, `pop`, `check-sat`, `get-info`,
    //! `get-model`, `set-option` and `echo`; replies are verbatim; AIR logs
    //! keep their comments and `check-valid` lines). Regenerate the golden
    //! file with `SAIL_BLESS=1 cargo test -p sail-cli`.
    use super::*;
    use sail_api::output::SourceSummaryName as S;
    use sail_verus::attribution::{LogFile, Provenance, SessionRecord, attribute, read_logs};
    use std::path::{Path, PathBuf};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/attr")
            .join(name)
    }

    fn logs() -> Vec<LogFile> {
        read_logs(&fixture("logs")).unwrap_or_default()
    }

    fn report(provenance: &Provenance) -> Report {
        let verus = std::fs::read(fixture("verus.json")).unwrap_or_default();
        let (docs, rest) = sail_verus::attribution::json_documents(&verus);
        assert_eq!(rest, b"");
        let logs = logs();
        let attribution = attribute(&docs, Some(&logs), provenance, None);
        let stderr = std::fs::read_to_string(fixture("stderr.txt")).unwrap_or_default();
        let diagnostics = sail_verus::diagnostics::parse(&stderr);
        build(&Inputs {
            args: &[],
            exit_code: Some(1),
            solver_path: None,
            solver_sha256: None,
            daemon: matches!(provenance, Provenance::Records(_)),
            attribution: &attribution,
            diagnostics: &diagnostics,
        })
    }

    fn function<'a>(r: &'a Report, name: &str) -> Option<&'a FunctionReport> {
        r.crates
            .iter()
            .flat_map(|c| &c.functions)
            .find(|f| f.function == name)
    }

    #[test]
    fn golden_report() {
        let r = report(&Provenance::NoCache);
        let text = serde_json::to_string_pretty(&r).unwrap_or_default() + "\n";
        let golden = fixture("report.golden.json");
        if std::env::var_os("SAIL_BLESS").is_some() {
            assert!(std::fs::write(&golden, &text).is_ok());
        }
        let want = std::fs::read_to_string(&golden).unwrap_or_default();
        assert!(
            text == want,
            "report differs from {}; rerun with SAIL_BLESS=1 to update:\n{text}",
            golden.display()
        );
        let schema: serde_json::Value =
            serde_json::from_str(sail_api::output::JSON_SCHEMA).unwrap_or_default();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        assert_eq!(
            sail_api::schema::validate(&schema, &value),
            Vec::<String>::new()
        );
        assert_eq!(r.invariant_violations(), Vec::<String>::new());
    }

    #[test]
    fn verdicts_agree_with_verus_and_close_the_bit_vector_gap() {
        let r = report(&Provenance::NoCache);
        assert_eq!(
            r.attribution.status,
            AttributionStatus::Complete,
            "{:?}",
            r.attribution.notes
        );
        for f in r.crates.iter().flat_map(|c| &c.functions) {
            let verified = f.verdict == sail_api::output::VerdictName::Verified;
            if f.function == "input::arith::bv_bad" {
                // Verus's JSON says success, but its bit-vector query failed
                // (docs/verus-issues/VI-0003); its diagnostics and exit code
                // agree with sail.
                assert_eq!(f.verus_success, Some(true));
                assert!(!verified);
            } else {
                assert_eq!(Some(verified), f.verus_success, "{}", f.function);
            }
            assert!(
                f.obligations.as_ref().is_some_and(|o| !o.is_empty()),
                "{}",
                f.function
            );
        }
        let bad = function(&r, "input::bad");
        let ids: Vec<&str> = bad
            .into_iter()
            .flat_map(|f| f.obligations.iter().flatten())
            .map(|o| o.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "input::bad#Function-Def/0",
                "input::bad#Function-Recommends/0"
            ]
        );
        let def = bad
            .and_then(|f| f.obligations.as_ref())
            .and_then(|o| o.first());
        assert!(def.is_some_and(|o| {
            o.counterexample
                .as_ref()
                .is_some_and(|c| c.assignments.iter().any(|a| a.name == "x!"))
        }));
        let mul = function(&r, "input::arith::mul_pos");
        let ids: Vec<&str> = mul
            .into_iter()
            .flat_map(|f| f.obligations.iter().flatten())
            .map(|o| o.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "input::arith::mul_pos#Function-Def/0",
                "input::arith::mul_pos#Function-Def/spinoff-1"
            ]
        );
    }

    #[test]
    fn incomplete_attribution_never_confirms_hidden_bit_vector_failures() {
        let verus = std::fs::read(fixture("verus.json")).unwrap_or_default();
        let (docs, _) = sail_verus::attribution::json_documents(&verus);
        let attribution = attribute(&docs, None, &Provenance::NoCache, None);
        let r = build(&Inputs {
            args: &[],
            exit_code: Some(1),
            solver_path: None,
            solver_sha256: None,
            daemon: false,
            attribution: &attribution,
            diagnostics: &[],
        });
        assert_eq!(r.summary.verified, 0);
        let bad = function(&r, "input::arith::bv_bad");
        assert!(bad.is_some_and(|f| f.verus_success == Some(true)
            && f.verdict == sail_api::output::VerdictName::Unknown));
        assert!(r.attribution.notes.iter().any(|n| n.contains("VI-0003")));
        assert_eq!(r.invariant_violations(), Vec::<String>::new());
    }

    #[test]
    fn successful_crates_keep_confirmed_verus_results_without_logs() {
        let mut r = report(&Provenance::NoCache);
        r.attribution.status = AttributionStatus::Unavailable;
        for c in &mut r.crates {
            c.verus.errors = Some(0);
            c.verus.success = Some(true);
        }
        let verified = r.summary.verified;
        incomplete_failures(&mut r);
        assert_eq!(r.summary.verified, verified);
    }

    #[test]
    fn cache_sources_come_from_session_records() {
        // A record for the root module's process saying the cache served its
        // first 20 replies: checks before reply 20 are cached, later ones
        // solved; processes without a record are unrecorded.
        let root = logs().into_iter().find(|l| l.stem == "root");
        let Some(root) = root else {
            unreachable!("no root log in the fixture");
        };
        let records = vec![
            SessionRecord {
                version: 1,
                input_sha256: root.input_sha256.clone(),
                replies_served: 20,
                solver_started: true,
            },
            SessionRecord {
                version: 1,
                input_sha256: "00".repeat(32),
                replies_served: 3,
                solver_started: false,
            },
        ];
        let r = report(&Provenance::Records(records));
        assert_eq!(r.attribution.unmatched_sessions, 1);
        assert_eq!(r.attribution.status, AttributionStatus::Partial);
        let source = |name: &str| function(&r, name).map(|f| f.provenance.source);

        assert_eq!(source("input::Counter::incr"), Some(S::Cache));
        assert_eq!(source("input::main"), Some(S::Fresh));
        assert_eq!(source("input::arith::mul_pos"), Some(S::Unrecorded));
        assert_eq!(r.invariant_violations(), Vec::<String>::new());
    }

    #[test]
    fn without_logs_raw_verus_results_are_preserved_without_false_proofs() {
        let verus = std::fs::read(fixture("verus.json")).unwrap_or_default();
        let (docs, _) = sail_verus::attribution::json_documents(&verus);
        let a = attribute(&docs, None, &Provenance::NoCache, Some("multiple packages"));
        let r = build(&Inputs {
            args: &[],
            exit_code: Some(1),
            solver_path: None,
            solver_sha256: None,
            daemon: false,
            attribution: &a,
            diagnostics: &[],
        });
        assert_eq!(r.attribution.status, AttributionStatus::Unavailable);
        assert_eq!(
            r.attribution.notes.first().map(String::as_str),
            Some("multiple packages")
        );
        assert!(r.attribution.notes.iter().any(|n| n.contains("VI-0003")));
        assert_eq!(r.summary.functions, 8);
        assert_eq!(r.summary.failed, 1, "the known raw failure is retained");
        assert_eq!(r.summary.unknown, 7);
        assert_eq!(r.summary.verified, 0);
        assert!(r.crates.iter().flat_map(|c| &c.functions).all(|f| {
            match f.verus_success {
                Some(true) => f.verdict == sail_api::output::VerdictName::Unknown,
                Some(false) => f.verdict == sail_api::output::VerdictName::Failed,
                None => false,
            }
        }));
        assert!(
            r.crates
                .iter()
                .flat_map(|c| &c.functions)
                .all(|f| f.obligations.is_none())
        );
        assert_eq!(r.invariant_violations(), Vec::<String>::new());
    }

    #[test]
    fn setup_failures_are_valid_documents() {
        let r = setup_failure(&["x".to_owned()], "SAIL0101", "no cargo-verus");
        let schema: serde_json::Value =
            serde_json::from_str(sail_api::output::JSON_SCHEMA).unwrap_or_default();
        let value = serde_json::to_value(&r).unwrap_or_default();
        assert_eq!(
            sail_api::schema::validate(&schema, &value),
            Vec::<String>::new()
        );
    }
}
