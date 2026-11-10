//! The Markdown job summary (`--format github-summary`).
//!
//! Sections are written in order of importance and each must fit the byte
//! limit whole: a section or diagnostic that does not fit is left out and
//! the closing line says so. The output never exceeds the limit.

use super::{Level, Place, View};
use crate::report::{Counts, Stopped};
use sail_api::evidence::{Change, Comparison, Difference};
use sail_api::output::{FunctionReport, OutcomeName, VerdictName};
use sail_api::project::{Currentness, ProjectResult, TargetResult, VerifiedAs};
use std::fmt::Write as _;

/// Rows shown in the table of functions that did not verify, and in the
/// table of baseline changes.
const ROWS: usize = 50;
/// Bytes kept back for the closing line that states what was left out.
const RESERVE: usize = 400;

/// Size and link settings.
pub(super) struct Limits<'a> {
    pub(super) bytes: usize,
    pub(super) link_base: Option<&'a str>,
}

/// What the run as a whole established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Passed,
    Failed,
    /// No function failed, and at least one crate stopped before
    /// verification.
    Stopped(Stopped),
    DidNotRun,
}

fn status(view: &View<'_>, counts: Counts) -> Status {
    let result = view.result;
    if result.error.is_some() {
        Status::DidNotRun
    } else if result.exit_code == Some(0) {
        Status::Passed
    } else {
        match counts.stopped() {
            Some(stopped) if counts.failed == 0 => Status::Stopped(stopped),
            _ => Status::Failed,
        }
    }
}

/// A Markdown code span holding `text` verbatim.
fn code(text: &str) -> String {
    let text = text.replace(['\n', '\r'], " ");
    if text.contains('`') {
        format!("`` {text} ``")
    } else {
        format!("`{text}`")
    }
}

/// Text for a table cell or list item: one line, no cell break, no markup.
fn cell(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '|' => out.push_str("\\|"),
            '\n' | '\r' => out.push(' '),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(c),
        }
    }
    out
}

/// A fenced block holding `text` verbatim, with a fence longer than any run
/// of backticks inside it.
fn fenced(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}text\n{}\n{fence}\n", text.trim_end_matches('\n'))
}

/// A location as text, linked to `link_base` when one is given.
fn location(place: &Place, link_base: Option<&str>) -> String {
    match place {
        Place::Repository { file, line, column } => {
            let text = cell(&format!("{file}:{line}:{column}"));
            match link_base {
                Some(base) => format!("[{text}]({}/{file}#L{line})", base.trim_end_matches('/')),
                None => text,
            }
        }
        Place::Outside { file, line, column } => {
            format!(
                "{} (outside the repository)",
                cell(&format!("{file}:{line}:{column}"))
            )
        }
        Place::Nowhere => "no location".to_owned(),
    }
}

fn kind(target: &TargetResult) -> String {
    if target.kind.is_empty() {
        cell(&target.target)
    } else {
        format!(
            "{} ({})",
            cell(&target.target),
            cell(&target.kind.join(", "))
        )
    }
}

/// How a target's report came to be in this result.
fn evidence(target: &TargetResult, currentness: Currentness) -> &'static str {
    let complete = sail_api::acceptance::complete_target(target).is_some();
    match (complete, target.reused, currentness) {
        (true, true, Currentness::Current) => "reused whole-target proof",
        (true, false, Currentness::Current) => "verified in this run",
        (true, _, _) => "whole-target proof, not established for the current inputs",
        (false, _, _) if target.scope_reuse.is_some() => "reused partial-scope report",
        (false, _, _) if target.report.exit_code == Some(0) => {
            "partial scope, not a whole-target proof"
        }
        (false, _, _) => match Counts::of(&target.report).nothing_checked() {
            Some(Stopped::Compilation) => "did not compile",
            Some(Stopped::Rejected) => "rejected by Verus",
            Some(Stopped::Both) => "stopped before verification",
            None => "failed in this run",
        },
    }
}

/// Accumulates sections within the byte limit.
struct Page {
    text: String,
    limit: usize,
    /// What did not fit, for the closing line.
    omitted: Vec<String>,
}

impl Page {
    fn room(&self) -> usize {
        self.limit
            .saturating_sub(RESERVE)
            .saturating_sub(self.text.len())
    }

    /// Appends `block` if it fits whole.
    fn push(&mut self, block: &str) -> bool {
        let fits = block.len() <= self.room();
        if fits {
            self.text.push_str(block);
        }
        fits
    }

    fn section(&mut self, name: &str, block: &str) {
        if !block.is_empty() && !self.push(block) {
            self.omitted.push(name.to_owned());
        }
    }

    fn finish(mut self) -> String {
        if !self.omitted.is_empty() {
            let full = format!(
                "\n_Left out to keep this summary under {} bytes: {}. The result document, the job log and `sail report --format sarif` hold everything._\n",
                self.limit,
                self.omitted.join("; ")
            );
            // The reserve bounds this line; a longer one is replaced.
            let brief =
                "\n_Parts of this summary were left out to keep it within its size limit._\n";
            let room = self.limit.saturating_sub(self.text.len());
            if full.len() <= room {
                self.text.push_str(&full);
            } else if brief.len() <= room {
                self.text.push_str(brief);
            }
        }
        self.text
    }
}

pub(super) fn render(
    view: &View<'_>,
    baseline: Option<&ProjectResult>,
    limits: &Limits<'_>,
) -> String {
    let counts = Counts::sum(view.requested().map(|target| &target.report));
    let status = status(view, counts);
    let mut page = Page {
        text: String::new(),
        limit: limits.bytes,
        omitted: Vec::new(),
    };
    page.section("the headline", &headline(view, counts, status));
    page.section("the target table", &targets(view));
    page.section(
        "the functions that did not verify",
        &unverified(view, limits),
    );
    if let Some(baseline) = baseline {
        let block = if status == Status::DidNotRun {
            "### Compared with the baseline\n\nNot compared: this run produced no verification result.\n\n"
                .to_owned()
        } else {
            comparison(baseline, view.result)
        };
        page.section("the baseline comparison", &block);
    }
    page.section("Sail's notes", &notes(view));
    diagnostics(view, &mut page);
    page.finish()
}

fn headline(view: &View<'_>, counts: Counts, status: Status) -> String {
    let result = view.result;
    let mut text = String::new();
    let title = match status {
        Status::Passed => "passed",
        Status::Failed => "failed",
        Status::Stopped(stopped) => stopped.describe(),
        Status::DidNotRun => "did not run",
    };
    let _ = writeln!(text, "## Sail verification: {title}\n");
    if let Some(error) = &result.error {
        let _ = writeln!(
            text,
            "**{}**: {}\n",
            cell(&error.code),
            cell(&error.message)
        );
    }
    let requested = view.requested().count();
    if requested > 0 {
        let _ = writeln!(
            text,
            "**{counts}** in {requested} requested target{}.\n",
            if requested == 1 { "" } else { "s" }
        );
    }
    let exit = result.exit_code.map_or_else(
        || "none (no exit status)".to_owned(),
        |code| code.to_string(),
    );
    let _ = write!(
        text,
        "Exit code {exit} · {:.1} s · {} verifier invocation{} · {} target{} reused from proof receipts",
        result.execution.wall_ms / 1000.0,
        result.execution.verifier_invocations,
        if result.execution.verifier_invocations == 1 {
            ""
        } else {
            "s"
        },
        result.execution.reused_targets,
        if result.execution.reused_targets == 1 {
            ""
        } else {
            "s"
        },
    );
    let (mut solved, mut served, mut unrecorded) = (0u64, 0u64, 0u64);
    for function in result
        .targets
        .iter()
        .filter(|target| !target.reused)
        .flat_map(|target| &target.report.crates)
        .flat_map(|krate| &krate.functions)
    {
        solved = solved.saturating_add(function.provenance.checks_solved);
        served = served.saturating_add(function.provenance.checks_served);
        unrecorded = unrecorded.saturating_add(function.provenance.checks_unrecorded);
    }
    if solved + served + unrecorded > 0 {
        let _ = write!(
            text,
            " · solver checks in this run: {served} from cache, {solved} solved"
        );
        if unrecorded > 0 {
            let _ = write!(text, ", {unrecorded} unrecorded");
        }
    }
    text.push_str("\n\n");
    match result.currentness {
        Currentness::Current => {}
        Currentness::Historical => text.push_str(
            "> **Historical result.** The inputs changed while verification ran, so this does not describe the current sources.\n\n",
        ),
        Currentness::Unknown => {
            if result.error.is_none() && result.no_reuse.is_none() {
                text.push_str(
                    "> **Currentness unknown.** Sail could not establish that the inputs stayed unchanged during the run.\n\n",
                );
            }
        }
    }
    text
}

fn targets(view: &View<'_>) -> String {
    let result = view.result;
    if result.targets.is_empty() {
        return String::new();
    }
    // Provisional functions are not verified; they get their own column
    // whenever a requested target has one.
    let provisional = view
        .requested()
        .any(|target| target.report.summary.provisional > 0);
    let mut text = if provisional {
        String::from(
            "| Target | Result | Functions | Verified | Failed | Unknown | Provisional (not verified) | Evidence |\n|---|---|--:|--:|--:|--:|--:|---|\n",
        )
    } else {
        String::from(
            "| Target | Result | Functions | Verified | Failed | Unknown | Evidence |\n|---|---|--:|--:|--:|--:|---|\n",
        )
    };
    for target in view.requested() {
        let counts = Counts::of(&target.report);
        let outcome = if let Some(stopped) = counts.nothing_checked() {
            match stopped {
                Stopped::Compilation => "compilation failed",
                Stopped::Rejected => "rejected by Verus",
                Stopped::Both => "stopped before verification",
            }
        } else if target.report.exit_code == Some(0) && counts.failed == 0 {
            "passed"
        } else {
            "failed"
        };
        let cleared = counts.passed_by_verus_totals;
        let unknown = counts.unknown.saturating_sub(cleared);
        let unknown = if provisional {
            format!("{unknown} | {}", counts.provisional)
        } else {
            unknown.to_string()
        };
        let _ = writeln!(
            text,
            "| {} | {outcome} | {} | {}{} | {} | {unknown} | {} |",
            kind(target),
            counts.functions,
            counts.verified,
            if cleared > 0 {
                format!(" (+{cleared} passed by Verus's totals)")
            } else {
                String::new()
            },
            counts.failed,
            evidence(target, result.currentness),
        );
    }
    let dependencies: Vec<&TargetResult> = view.dependencies().collect();
    if !dependencies.is_empty() {
        let functions: u64 = dependencies
            .iter()
            .map(|target| target.report.summary.functions)
            .sum();
        let reused = dependencies.iter().filter(|target| target.reused).count();
        let failing = dependencies
            .iter()
            .filter(|target| target.report.summary.failed > 0)
            .count();
        let names: Vec<String> = dependencies
            .iter()
            .take(8)
            .map(|t| cell(&t.target))
            .collect();
        let _ = write!(
            text,
            "\nDependencies, not listed above: {}{} ({functions} function{}; {reused} of {} reused",
            names.join(", "),
            if dependencies.len() > names.len() {
                format!(" and {} more", dependencies.len() - names.len())
            } else {
                String::new()
            },
            if functions == 1 { "" } else { "s" },
            dependencies.len(),
        );
        if failing > 0 {
            let _ = write!(text, "; **{failing} with failed functions**");
        }
        text.push_str(").\n");
    }
    text.push('\n');
    text
}

/// The location of the obligation that decided a function's result, as a
/// repository path when it has one.
fn decided(view: &View<'_>, function: &FunctionReport) -> Place {
    let obligations = function.obligations.as_deref().unwrap_or_default();
    let span = obligations
        .iter()
        .filter(|o| !o.advisory && o.outcome != OutcomeName::Proved)
        .chain(obligations.iter())
        .find_map(|o| o.span.as_ref());
    let Some(span) = span else {
        return Place::Nowhere;
    };
    let (Some(file), Some(line)) = (span.file.as_deref(), span.line) else {
        return Place::Nowhere;
    };
    let column = span.column.unwrap_or(1);
    match view.paths.repository(file) {
        Some(file) => Place::Repository { file, line, column },
        None => Place::Outside {
            file: file.to_owned(),
            line,
            column,
        },
    }
}

/// What the solver said for the obligations that did not prove.
fn reason(function: &FunctionReport) -> String {
    let mut reasons: Vec<String> = Vec::new();
    for obligation in function.obligations.as_deref().unwrap_or_default() {
        if obligation.advisory || obligation.outcome == OutcomeName::Proved {
            continue;
        }
        // The identity after the function path: `Function-Def/0`.
        let name = obligation
            .id
            .rsplit_once('#')
            .map_or(obligation.id.as_str(), |(_, name)| name);
        let mut text = format!("{name} {}", outcome(obligation.outcome));
        if obligation.outcome == OutcomeName::Unknown {
            match obligation.checks.first().map(|c| &c.reason_unknown) {
                Some(Some(reason)) => {
                    let _ = write!(text, ": {reason}");
                }
                Some(None) => {}
                None => text.push_str(": no solver answer recorded"),
            }
        } else if obligation.counterexample.is_some() {
            text.push_str(", with a solver model");
        }
        if !reasons.contains(&text) {
            reasons.push(text);
        }
    }
    if function.unstable {
        reasons.push("unstable (near its resource limit)".to_owned());
    }
    cell(&reasons.join("; "))
}

fn outcome(outcome: OutcomeName) -> &'static str {
    match outcome {
        OutcomeName::Proved => "proved",
        OutcomeName::Failed => "failed",
        OutcomeName::Unknown => "unknown",
    }
}

fn verdict(verdict: VerdictName) -> &'static str {
    match verdict {
        VerdictName::Verified => "verified",
        VerdictName::Failed => "failed",
        VerdictName::Unknown => "unknown",
        VerdictName::Provisional => "provisional",
    }
}

fn unverified(view: &View<'_>, limits: &Limits<'_>) -> String {
    let mut rows = Vec::new();
    for target in view.requested().chain(view.dependencies()) {
        for krate in &target.report.crates {
            // Reported `unknown`, but Verus's own totals leave no error for
            // them: counted in the target table, not listed as problems.
            let cleared = krate.verus_totals_clear_unknown();
            for function in &krate.functions {
                let listed = match function.verdict {
                    VerdictName::Verified => false,
                    VerdictName::Unknown => !cleared,
                    VerdictName::Failed | VerdictName::Provisional => true,
                };
                if listed {
                    rows.push((target, function));
                }
            }
        }
    }
    if rows.is_empty() {
        return String::new();
    }
    // Settled failures first; the order within a verdict is the report's.
    rows.sort_by_key(|(_, function)| function.verdict != VerdictName::Failed);
    let mut text = format!(
        "### Functions that did not verify ({})\n\n| Function | Verdict | Target | Location | Solver evidence |\n|---|---|---|---|---|\n",
        rows.len()
    );
    for (target, function) in rows.iter().take(ROWS) {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {} |",
            code(&function.function).replace('|', "\\|"),
            verdict(function.verdict),
            cell(&target.target),
            location(&decided(view, function), limits.link_base),
            reason(function),
        );
    }
    if rows.len() > ROWS {
        let _ = writeln!(
            text,
            "\n{} more function(s) are not listed; the result document lists every function.",
            rows.len() - ROWS
        );
    }
    text.push('\n');
    text
}

fn change(change: Change) -> &'static str {
    match change {
        Change::Unchanged => "unchanged",
        Change::Regressed => "regressed",
        Change::BecameUnknown => "became unknown",
        Change::Recovered => "recovered",
        Change::Added => "added",
        Change::NotObserved => "not observed",
        Change::Removed => "removed",
        Change::Incomparable => "incomparable",
        Change::Changed => "changed",
    }
}

fn short(digest: &str) -> &str {
    digest.get(..12).unwrap_or(digest)
}

/// Why `sail_api::evidence::compare` found two results incomparable, in the
/// terms of its own rule: proof reuse, context digest, then requested scope.
fn incomparable(before: &ProjectResult, after: &ProjectResult) -> String {
    if before.no_reuse.is_some() || after.no_reuse.is_some() {
        return "a run without proof reuse (`SAIL3112`) binds no verification context to compare"
            .to_owned();
    }
    if before.context_sha256.is_empty() || before.context_sha256 != after.context_sha256 {
        return format!(
            "the verification configuration or executable identity differs (context {} against {})",
            code(short(&before.context_sha256)),
            code(short(&after.context_sha256))
        );
    }
    let mut text = "the requested targets, mode or arguments differ".to_owned();
    if before.workspace != after.workspace {
        let _ = write!(
            text,
            "; the workspaces are at different paths ({} against {}), and a path package's identity includes its path",
            code(&before.workspace),
            code(&after.workspace)
        );
    }
    text
}

fn transition(difference: &Difference) -> String {
    let side = |verdict_name: Option<VerdictName>, by_totals: bool| match verdict_name {
        Some(name) if by_totals => format!("{} (passed by Verus's totals)", verdict(name)),
        Some(name) => verdict(name).to_owned(),
        None => "not observed".to_owned(),
    };
    format!(
        "{} → {}",
        side(difference.before, difference.before_passed_by_verus_totals),
        side(difference.after, difference.after_passed_by_verus_totals)
    )
}

fn comparison(before: &ProjectResult, after: &ProjectResult) -> String {
    let compared: Comparison = sail_api::evidence::compare(before, after);
    // A run without proof reuse records no input revision.
    let revision = |digest: &str| {
        if digest.is_empty() {
            "not recorded".to_owned()
        } else {
            code(short(digest))
        }
    };
    let mut text = format!(
        "### Compared with the baseline\n\nBaseline revision {} (run {}); this run's revision {}.\n\n",
        revision(&compared.before_revision),
        code(&before.run_id),
        revision(&compared.after_revision),
    );
    let rows: Vec<&Difference> = if compared.compatible {
        let mut counts = std::collections::BTreeMap::<u8, usize>::new();
        for difference in &compared.differences {
            *counts.entry(order(difference.change)).or_default() += 1;
        }
        let total = |wanted: Change| counts.get(&order(wanted)).copied().unwrap_or(0);
        let _ = writeln!(
            text,
            "**{} regressed, {} became unknown, {} recovered**; {} added, {} removed, {} changed otherwise, {} unchanged, {} not observed in this run.\n",
            total(Change::Regressed),
            total(Change::BecameUnknown),
            total(Change::Recovered),
            total(Change::Added),
            total(Change::Removed),
            total(Change::Changed),
            total(Change::Unchanged),
            total(Change::NotObserved),
        );
        let mut rows: Vec<&Difference> = compared
            .differences
            .iter()
            .filter(|d| !matches!(d.change, Change::Unchanged | Change::NotObserved))
            .collect();
        rows.sort_by_key(|d| order(d.change));
        rows
    } else {
        let _ = writeln!(
            text,
            "**Not comparable:** {}. The differences below are descriptive; none is reported as a regression.\n",
            incomparable(before, after)
        );
        compared
            .differences
            .iter()
            .filter(|d| d.before != d.after)
            .collect()
    };
    if !rows.is_empty() {
        text.push_str("| Function | Target | Change | Verdict |\n|---|---|---|---|\n");
        for difference in rows.iter().take(ROWS) {
            let _ = writeln!(
                text,
                "| {} | {} | {} | {} |",
                code(&difference.function).replace('|', "\\|"),
                cell(&difference.target.name),
                change(difference.change),
                transition(difference),
            );
        }
        if rows.len() > ROWS {
            let _ = writeln!(
                text,
                "\n{} more change(s) are not listed.",
                rows.len() - ROWS
            );
        }
        text.push('\n');
    }
    for note in &compared.notes {
        let _ = writeln!(text, "- {}", cell(note));
    }
    if !compared.notes.is_empty() {
        text.push('\n');
    }
    text
}

/// Table order: what needs attention first.
fn order(change: Change) -> u8 {
    match change {
        Change::Regressed => 0,
        Change::BecameUnknown => 1,
        Change::Changed => 2,
        Change::Recovered => 3,
        Change::Removed => 4,
        Change::Added => 5,
        Change::Incomparable => 6,
        Change::NotObserved => 7,
        Change::Unchanged => 8,
    }
}

/// Sail's own notes: why proofs were not reused, and coverage limits.
fn notes(view: &View<'_>) -> String {
    let result = view.result;
    let mut lines = Vec::new();
    if let Some(no_reuse) = &result.no_reuse {
        let how = match no_reuse.verified_as {
            VerifiedAs::PlainCargoVerus => {
                "verified as plain `cargo verus verify`, with the solver cache"
            }
            VerifiedAs::Managed => "verified by managed verification",
        };
        lines.push(format!(
            "{}: no proof reuse, because {}; {how}. Nothing was reused or recorded.",
            cell(&no_reuse.code),
            cell(&no_reuse.reason)
        ));
    }
    lines.extend(result.notes.iter().map(|note| cell(note)));
    for target in view.requested() {
        for note in &target.report.attribution.notes {
            lines.push(format!("{}: {}", cell(&target.target), cell(note)));
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut text = String::from("### Notes from Sail\n\n");
    for line in lines.iter().take(ROWS) {
        let _ = writeln!(text, "- {line}");
    }
    if lines.len() > ROWS {
        let _ = writeln!(
            text,
            "- {} more note(s) are not listed.",
            lines.len() - ROWS
        );
    }
    text.push('\n');
    text
}

fn diagnostics(view: &View<'_>, page: &mut Page) {
    let (errors, warnings) = (view.count(Level::Error), view.count(Level::Warning));
    if errors + warnings == 0 {
        return;
    }
    let mut heading = format!(
        "### Verus diagnostics ({errors} error{}, {warnings} warning{})\n\n",
        if errors == 1 { "" } else { "s" },
        if warnings == 1 { "" } else { "s" },
    );
    if view.trailers + view.remarks > 0 {
        let _ = writeln!(
            heading,
            "Not listed: {} closing compiler line(s) that restate the errors, and {} top-level note or help block(s).\n",
            view.trailers, view.remarks
        );
    }
    if view.duplicates > 0 {
        let _ = writeln!(
            heading,
            "Listed once: {} more diagnostic(s) repeat one of these word for word (Cargo prints a diagnostic again for another target of the same package).\n",
            view.duplicates
        );
    }
    if !page.push(&heading) {
        page.omitted
            .push(format!("all {} Verus diagnostics", errors + warnings));
        return;
    }
    let mut shown = 0;
    for finding in &view.findings {
        let block = format!(
            "<details><summary>{}: {} ({})</summary>\n\n{}\n</details>\n\n",
            finding.level.name(),
            cell(&finding.diagnostic.message),
            // Markdown links are not rendered inside an HTML summary.
            location(&finding.place, None),
            fenced(&finding.diagnostic.rendered),
        );
        if !page.push(&block) {
            break;
        }
        shown += 1;
    }
    if shown < view.findings.len() {
        page.omitted.push(format!(
            "{} of {} Verus diagnostics (ranked: errors first)",
            view.findings.len() - shown,
            view.findings.len()
        ));
    }
}
