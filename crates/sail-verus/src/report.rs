//! Builds the `sail verify --output json` document (schema v1, ADR 0014)
//! from Verus's results, the attribution and the diagnostics.

use crate::attribution::{Attribution, Status};
use sail_api::output::{
    AttributionInfo, AttributionStatus, CacheInfo, CrateReport, Diagnostic, DiagnosticSpan,
    ErrorInfo, FunctionReport, Report, SCHEMA_VERSION, SolverInfo, Summary, VerusResults,
};

/// Everything a report is built from.
pub struct Inputs<'a> {
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
    pub diagnostics: &'a [crate::diagnostics::Diagnostic],
}

fn diagnostic(d: &crate::diagnostics::Diagnostic) -> Diagnostic {
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

/// Builds the report, preserving default-mode scope and uncertainty rules.
#[must_use]
pub fn build(inputs: &Inputs<'_>) -> Report {
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
        engine: None,
    };
    crate::report_scope::restrict(&mut report);
    incomplete_failures(&mut report);
    report
}

/// Verus can omit a failed bit-vector function from its failure set (VI-0003).
/// In a failing crate, an incomplete mapping cannot resolve which successful
/// timing entries are real proofs. Preserve the raw field without confirming it.
pub fn incomplete_failures(report: &mut Report) {
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
#[must_use]
pub fn setup_failure(args: &[String], code: &str, message: &str) -> Report {
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
        engine: None,
    }
}
