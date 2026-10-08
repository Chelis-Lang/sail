//! Short human summaries of schema-v1 reports.
//!
//! Charter rule 7: nothing provisional is ever presented as verified. The
//! summary counts a function by [`effective_verdict`], which is
//! `provisional` whenever the tier is, whatever the verdict field says.

use std::fmt::Write as _;

use sail_api::output::{FunctionReport, OutcomeName, Report, TierName, VerdictName};

/// How many names a summary line lists before eliding the rest.
const MAX_NAMES: usize = 8;

/// The verdict to present: `provisional` whenever the tier is provisional.
#[must_use]
pub fn effective_verdict(f: &FunctionReport) -> VerdictName {
    if f.tier == TierName::Provisional {
        VerdictName::Provisional
    } else {
        f.verdict
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
            line.push_str(" (Z3 gave up near its limit; not a settled failure)");
        }
        VerdictName::Verified | VerdictName::Failed => {}
    }
    if f.unstable {
        line.push_str(", unstable");
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
            "\nUnknown (Z3 gave up near its limit; not a settled failure): {}.",
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
    // Rust's closing "aborting due to N previous errors" is not an error of
    // its own.
    let errors: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.level == "error" && !d.message.starts_with("aborting due to"))
        .collect();
    if let Some(d) = errors.first() {
        let at = d
            .spans
            .iter()
            .find(|s| s.primary)
            .map(|s| format!(" at {}:{}:{}", s.file, s.line, s.column))
            .unwrap_or_default();
        let _ = write!(
            text,
            "\nFirst of {} error(s): {}{at}.",
            errors.len(),
            d.message
        );
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

    #[test]
    fn provisional_is_never_counted_as_verified() {
        let mut sneaky = function("demo::p", VerdictName::Verified);
        sneaky.tier = TierName::Provisional;
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
    fn long_lists_are_elided() {
        let r = make(
            (0..12)
                .map(|i| function(&format!("demo::f{i}"), VerdictName::Unknown))
                .collect(),
        );
        assert!(report(&r).contains("and 4 more"));
    }
}
