//! Short human summaries of schema-v2 reports.
//!
//! Charter rule 7: nothing provisional is ever presented as verified. The
//! summary counts a function by [`effective_verdict`], which is
//! `provisional` whenever the establishment is, whatever the verdict field
//! says.

use std::fmt::Write as _;

use sail_api::output::{EvidenceTierName, FunctionReport, OutcomeName, Report, VerdictName};

/// How many names a summary line lists before eliding the rest.
const MAX_NAMES: usize = 8;

/// The verdict to present: `provisional` whenever the establishment is
/// provisional.
#[must_use]
pub fn effective_verdict(f: &FunctionReport) -> VerdictName {
    sail_api::acceptance::present_function_verdict(f)
}

/// The evidence tier to present: the report's, for a presented verdict
/// that is settled (`verified` or `failed`), and otherwise none.
#[must_use]
pub fn effective_evidence_tier(f: &FunctionReport) -> Option<EvidenceTierName> {
    match effective_verdict(f) {
        VerdictName::Verified | VerdictName::Failed => f.evidence_tier,
        VerdictName::Unknown | VerdictName::Provisional => None,
    }
}

/// The verdict's schema name.
#[must_use]
pub fn verdict_name(v: VerdictName) -> &'static str {
    match v {
        VerdictName::Verified => "verified",
        VerdictName::Failed => "failed",
        VerdictName::Unknown => "unknown",
        VerdictName::Provisional => "provisional",
    }
}

/// One line describing a function's verdict, for lookups.
#[must_use]
pub fn function_line(f: &FunctionReport) -> String {
    let verdict = effective_verdict(f);
    let mut line = format!("{}: {}", f.function, verdict_name(verdict));
    match verdict {
        VerdictName::Provisional => {
            line.push_str(" (NOT verified: not yet confirmed by the pinned solver)");
        }
        VerdictName::Unknown => {
            line.push_str(" (verification not confirmed; not a settled failure)");
        }
        VerdictName::Verified | VerdictName::Failed => {}
    }
    if f.unstable {
        line.push_str(", unstable");
    }
    if f.provenance.mode != "default" {
        let _ = write!(line, "; mode {}", f.provenance.mode);
        if let Some(configuration) = f.provenance.configuration {
            let _ = write!(line, ", seed {}", configuration.seed);
        }
    }
    if let Some(engine) = &f.provenance.engine {
        let _ = write!(
            line,
            "; {} engine{}",
            engine.name,
            match engine.established_by {
                sail_api::output::EngineEstablishment::Engine => "",
                sail_api::output::EngineEstablishment::StockFallback =>
                    " faulted, stock Verus result",
            }
        );
    }
    line
}

fn names<'a>(it: impl Iterator<Item = &'a FunctionReport>) -> String {
    let all: Vec<&str> = it.map(|f| f.function.as_str()).collect();
    let mut text = all
        .iter()
        .take(MAX_NAMES)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if all.len() > MAX_NAMES {
        let _ = write!(text, " and {} more", all.len() - MAX_NAMES);
    }
    text
}

/// The counterexample-bearing obligations of a function.
fn counterexamples(f: &FunctionReport) -> Vec<&str> {
    f.obligations
        .iter()
        .flatten()
        .filter(|o| o.counterexample.is_some() && o.outcome != OutcomeName::Proved)
        .map(|o| o.id.as_str())
        .collect()
}

/// "First of N error(s): …", for the report's errors.
fn first_error(r: &Report) -> Option<String> {
    // rustc's closing "aborting due to N previous errors" and Cargo's
    // "could not compile" summarise the errors; they are not errors of their
    // own.
    let errors: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| {
            d.level == "error"
                && !d.message.starts_with("aborting due to")
                && !d.message.starts_with("could not compile")
        })
        .collect();
    errors.first().map(|d| {
        let at = d
            .spans
            .iter()
            .find(|s| s.primary)
            .map(|s| format!(" at {}:{}:{}", s.file, s.line, s.column))
            .unwrap_or_default();
        format!("\nFirst of {} error(s): {}{at}.", errors.len(), d.message)
    })
}

/// A few lines summarising a run's report.
#[must_use]
pub fn report(r: &Report) -> String {
    let functions: Vec<&FunctionReport> = r.crates.iter().flat_map(|c| &c.functions).collect();
    let count = |v: VerdictName| {
        functions
            .iter()
            .filter(|f| effective_verdict(f) == v)
            .count()
    };
    let mut text = format!(
        "{} functions: {} verified, {} failed, {} unknown, {} provisional",
        functions.len(),
        count(VerdictName::Verified),
        count(VerdictName::Failed),
        count(VerdictName::Unknown),
        count(VerdictName::Provisional)
    );
    match r.exit_code {
        Some(c) => {
            let _ = write!(text, " (exit {c})");
        }
        None => text.push_str(" (no exit code)"),
    }
    let (solved, served) = functions.iter().fold((0, 0), |(s, c), f| {
        (
            s + f.provenance.checks_solved,
            c + f.provenance.checks_served,
        )
    });
    if solved + served > 0 {
        let _ = write!(text, "; checks: {solved} solved fresh, {served} from cache");
    }
    text.push('.');
    if r.mode != "default" {
        let _ = write!(text, "\nMode: {}", r.mode);
        if let Some(portfolio) = &r.portfolio {
            if let Some(seed) = portfolio.winner_seed {
                let _ = write!(text, "; complete proof from seed {seed}.");
            } else {
                text.push_str("; no complete proof.");
            }
        }
    }
    let of = |v: VerdictName| {
        functions
            .iter()
            .copied()
            .filter(move |f| effective_verdict(f) == v)
    };
    let failed: Vec<&FunctionReport> = of(VerdictName::Failed).collect();
    if !failed.is_empty() {
        let _ = write!(text, "\nFailed: {}.", names(failed.iter().copied()));
        let models: Vec<&str> = failed.iter().flat_map(|f| counterexamples(f)).collect();
        if !models.is_empty() {
            let _ = write!(
                text,
                "\nCounterexamples (use the counterexample tool): {}.",
                models
                    .iter()
                    .take(MAX_NAMES)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    if of(VerdictName::Unknown).next().is_some() {
        let _ = write!(
            text,
            "\nUnknown (verification not confirmed; not a settled failure): {}.",
            names(of(VerdictName::Unknown))
        );
    }
    if of(VerdictName::Provisional).next().is_some() {
        let _ = write!(
            text,
            "\nProvisional, NOT verified (not yet confirmed by the pinned solver): {}.",
            names(of(VerdictName::Provisional))
        );
    }
    if functions.iter().any(|f| f.unstable) {
        let _ = write!(
            text,
            "\nUnstable (could differ between runs): {}.",
            names(functions.iter().copied().filter(|f| f.unstable))
        );
    }
    if let Some(line) = first_error(r) {
        text.push_str(&line);
    }
    if let Some(e) = &r.error {
        let _ = write!(text, "\nsail error {}: {}", e.code, e.message);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{function, report as make};
    use sail_api::output::EstablishmentName;

    #[test]
    fn provisional_is_never_counted_as_verified() {
        let mut sneaky = function("demo::p", VerdictName::Verified);
        sneaky.establishment = EstablishmentName::Provisional;
        assert_eq!(effective_verdict(&sneaky), VerdictName::Provisional);
        let r = make(vec![
            function("demo::a", VerdictName::Verified),
            function("demo::b", VerdictName::Failed),
            sneaky,
        ]);
        let text = report(&r);
        assert!(
            text.starts_with("3 functions: 1 verified, 1 failed, 0 unknown, 1 provisional"),
            "{text}"
        );
        assert!(text.contains("Provisional, NOT verified"), "{text}");
        assert!(text.contains("Failed: demo::b."), "{text}");
        assert!(function_line(&r.crates[0].functions[2]).contains("NOT verified"));
    }

    #[test]
    fn shared_presentation_preserves_all_existing_field_combinations() {
        for verdict in [
            VerdictName::Verified,
            VerdictName::Failed,
            VerdictName::Unknown,
            VerdictName::Provisional,
        ] {
            for establishment in [EstablishmentName::Provisional, EstablishmentName::Confirmed] {
                let mut record = function("demo::fields", verdict);
                record.establishment = establishment;
                let expected = if establishment == EstablishmentName::Provisional {
                    VerdictName::Provisional
                } else {
                    verdict
                };
                assert_eq!(effective_verdict(&record), expected);
                assert_eq!(
                    sail_api::acceptance::present_function_verdict(&record),
                    expected
                );
            }
        }
    }

    #[test]
    fn engine_provenance_is_labelled_and_stock_lines_are_unchanged() {
        let stock = function("demo::a", VerdictName::Verified);
        let plain = function_line(&stock);
        assert!(!plain.contains("engine"), "{plain}");
        let mut engine = stock.clone();
        let mut provenance = sail_api::output::EngineProvenance {
            name: "interactive".into(),
            manifest_sha256: "ab".repeat(32),
            established_by: sail_api::output::EngineEstablishment::Engine,
        };
        engine.provenance.engine = Some(provenance.clone());
        assert_eq!(
            function_line(&engine),
            format!("{plain}; interactive engine")
        );
        provenance.established_by = sail_api::output::EngineEstablishment::StockFallback;
        engine.provenance.engine = Some(provenance);
        assert!(function_line(&engine).ends_with("interactive engine faulted, stock Verus result"));
    }

    #[test]
    fn long_lists_are_elided() {
        let r = make(
            (0..12)
                .map(|i| function(&format!("demo::f{i}"), VerdictName::Unknown))
                .collect(),
        );
        assert!(report(&r).contains("and 4 more"));
    }

    #[test]
    fn unknown_does_not_claim_an_unobserved_resource_failure() {
        let f = function("demo::uncertain", VerdictName::Unknown);
        assert!(function_line(&f).contains("verification not confirmed"));
        assert!(!function_line(&f).contains("near its limit"));
        assert!(!report(&make(vec![f])).contains("near its limit"));
    }

    #[test]
    fn summary_errors_are_not_counted() {
        use sail_api::output::{Diagnostic, DiagnosticSpan};
        let diag = |message: &str, spans: Vec<DiagnosticSpan>| Diagnostic {
            level: "error".to_owned(),
            code: None,
            message: message.to_owned(),
            spans,
            rendered: String::new(),
        };
        let mut r = make(vec![function("demo::bad", VerdictName::Failed)]);
        r.diagnostics = vec![
            diag(
                "postcondition not satisfied",
                vec![DiagnosticSpan {
                    file: "src/lib.rs".to_owned(),
                    line: 10,
                    column: 13,
                    primary: true,
                }],
            ),
            diag("aborting due to 1 previous error", Vec::new()),
            diag("could not compile `demo` (lib)", Vec::new()),
        ];
        let text = report(&r);
        assert!(
            text.contains("First of 1 error(s): postcondition not satisfied at src/lib.rs:10:13."),
            "{text}"
        );
    }
}
