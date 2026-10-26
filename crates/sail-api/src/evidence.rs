//! Explanations, immutable baselines and comparisons of completed evidence.
//!
//! These read recorded project results only. They never establish proof of
//! current inputs or generate proof edits.

use serde::{Deserialize, Serialize};

use crate::output::{Diagnostic, FunctionReport, OutcomeName, VerdictName};
use crate::project::{Currentness, ProjectResult};
use crate::proof::Target;

/// Independent evidence contract version.
pub const VERSION: u32 = 1;

/// Explanation preserving exact verifier evidence instead of generating proof edits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explanation {
    /// Independent contract version.
    pub version: u32,
    /// Complete target identity.
    pub target: Target,
    /// Establishing run, including reused historical reports.
    pub establishing_run: String,
    /// Currentness of the project evidence.
    pub currentness: Currentness,
    /// Complete unchanged function report, with obligations/models/provenance.
    pub function: FunctionReport,
    /// Diagnostics with explicit producer attribution, if available.
    pub attributed_diagnostics: Vec<Diagnostic>,
    /// Remaining run diagnostics preserved without line-window guesses.
    pub run_diagnostics: Vec<Diagnostic>,
    /// Whether diagnostic/function linkage is authoritative.
    pub diagnostic_attribution_complete: bool,
    /// Informational limitations, never an invented missing model or verdict.
    pub notes: Vec<String>,
}

/// An immutable independently retained baseline snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// Independently retained neutral contract snapshot evidence, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contracts: Option<Box<crate::contracts::Evidence>>,
    /// Baseline contract version.
    pub version: u32,
    /// Owner-selected immutable name.
    pub name: String,
    /// Original job identity.
    pub job_id: String,
    /// Independently completed evidence, unchanged by later runs or job pruning.
    pub result: ProjectResult,
}

/// Classification of one comparison entry.
///
/// Function classifications count a function that passed by Verus's totals
/// (reported `unknown`, not attributed one by one) as verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// Same reported evidence class.
    Unchanged,
    /// A previously verified function now reports a settled failure.
    Regressed,
    /// A previously verified function no longer has a confirmed proof.
    BecameUnknown,
    /// A previously unproved function now verifies (confirmed, or passed by
    /// Verus's totals).
    Recovered,
    /// Newly observed function.
    Added,
    /// No matching evidence: partial scopes do not establish removal.
    NotObserved,
    /// Absent from a finalized whole-target function inventory.
    Removed,
    /// Scope/runtime/mode differs; compare descriptively, not as a regression.
    Incomparable,
    /// Other non-proof transition.
    Changed,
}

/// One target/function transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Difference {
    /// Target identity including library/binary kind.
    pub target: Target,
    /// Exact function identity.
    pub function: String,
    /// Earlier reported verdict, absent when not observed or when the
    /// functions sharing a repeated friendly name report different verdicts.
    pub before: Option<VerdictName>,
    /// Later reported verdict, absent when not observed or when the
    /// functions sharing a repeated friendly name report different verdicts.
    pub after: Option<VerdictName>,
    /// Conservative comparison classification.
    pub change: Change,
    /// The earlier `unknown` verdict was classified as verified because the
    /// function passed by Verus's totals (not attributed one by one).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub before_passed_by_verus_totals: bool,
    /// The later `unknown` verdict was classified as verified because the
    /// function passed by Verus's totals (not attributed one by one).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub after_passed_by_verus_totals: bool,
    /// Exact attributed obligation IDs; ordinals are not semantically matched.
    pub obligations: Vec<ObligationDifference>,
}

/// A comparison of one exact attributed obligation identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObligationDifference {
    /// Exact producer identifier, never matched by a source-line heuristic.
    pub id: String,
    /// Earlier solver-derived obligation outcome.
    pub before: Option<OutcomeName>,
    /// Later solver-derived obligation outcome.
    pub after: Option<OutcomeName>,
    /// Whether a failure is advisory in the later or earlier evidence.
    pub advisory: bool,
    /// Descriptive transition; an advisory failure is not a function regression.
    pub change: Change,
}

/// Comparison of explicit completed evidence; not a new proof of current inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// Independent contract version.
    pub version: u32,
    /// True only when recorded contexts, modes and requested scopes agree.
    pub compatible: bool,
    /// Earlier source revision.
    pub before_revision: String,
    /// Later source revision.
    pub after_revision: String,
    /// Later evidence currentness, never assumed from terminal lifecycle.
    pub after_currentness: Currentness,
    /// Differences and unchanged identities.
    pub differences: Vec<Difference>,
    /// Why comparisons cannot be interpreted as proof regressions.
    pub notes: Vec<String>,
}

/// An explicit earlier evidence source, without guessing a baseline from history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Reference {
    /// Completed durable job identity.
    Job(String),
    /// Immutable owner-selected baseline name.
    Baseline(String),
}

/// Selects exact evidence without guessing diagnostic ownership or model values.
///
/// # Errors
/// Returns a stable missing/ambiguous target, function, or obligation error.
pub fn explain(
    result: &ProjectResult,
    target: &Target,
    function: &str,
    obligation: Option<&str>,
) -> Result<Explanation, String> {
    let matches: Vec<_> = result
        .targets
        .iter()
        .filter(|t| {
            t.package_id == target.package_id && t.target == target.name && t.kind == target.kind
        })
        .collect();
    if matches.len() != 1 {
        return Err("SAIL0904: exact target is missing or ambiguous".into());
    }
    let selected = matches[0];
    let functions: Vec<_> = selected
        .report
        .crates
        .iter()
        .flat_map(|c| &c.functions)
        .filter(|f| f.function == function)
        .collect();
    if functions.len() != 1 {
        return Err("SAIL0904: exact function is missing or ambiguous".into());
    }
    let mut report = functions[0].clone();
    if let Some(id) = obligation {
        let Some(obligations) = report.obligations.as_mut() else {
            return Err("SAIL0904: obligation attribution is unavailable".into());
        };
        if !obligations.iter().any(|o| o.id == id) {
            return Err("SAIL0904: exact obligation was not observed".into());
        }
        obligations.retain(|o| o.id == id);
    }
    Ok(Explanation {
        version: VERSION, target: target.clone(), establishing_run: selected.establishing_run.clone(),
        currentness: result.currentness, function: report, attributed_diagnostics: Vec::new(),
        run_diagnostics: selected.report.diagnostics.clone(), diagnostic_attribution_complete: false,
        notes: vec!["Run diagnostics are retained; no line-window heuristic establishes their function ownership.".into()],
    })
}

/// One recorded function as a comparison sees it.
#[derive(Debug, Clone, Copy)]
struct Entry<'a> {
    report: &'a FunctionReport,
    /// Reported `unknown` in a crate whose Verus totals clear its unknown
    /// functions ([`crate::output::CrateReport::verus_totals_clear_unknown`]).
    by_totals: bool,
}

impl Entry<'_> {
    /// The reported verdict (`provisional` for a provisional establishment).
    fn verdict(self) -> VerdictName {
        effective(self.report)
    }

    /// The class a comparison classifies by. A function that passed by
    /// Verus's totals counts as verified here, as Verus counted it; its
    /// reported verdict stays `unknown`.
    fn class(self) -> VerdictName {
        if self.by_totals {
            VerdictName::Verified
        } else {
            self.verdict()
        }
    }
}

type Records<'a> = std::collections::BTreeMap<(Target, String), Vec<Entry<'a>>>;

fn functions(result: &ProjectResult) -> Records<'_> {
    let mut records = Records::new();
    for target in result.targets.iter().filter(|target| target.requested) {
        let id = Target {
            package_id: target.package_id.clone(),
            name: target.target.clone(),
            kind: target.kind.clone(),
        };
        for krate in &target.report.crates {
            let cleared = krate.verus_totals_clear_unknown();
            for function in &krate.functions {
                records
                    .entry((id.clone(), function.function.clone()))
                    .or_default()
                    .push(Entry {
                        report: function,
                        by_totals: cleared && effective(function) == VerdictName::Unknown,
                    });
            }
        }
    }
    records
}

fn unique<'a>(entries: &[Entry<'a>]) -> Option<&'a FunctionReport> {
    match entries {
        [only] => Some(only.report),
        _ => None,
    }
}

/// One side of a function identity, as classified.
#[derive(Debug, Default, Clone, Copy)]
struct Side {
    /// The reported verdict, when every entry reports the same one.
    verdict: Option<VerdictName>,
    /// The comparison class; `None` when not observed.
    class: Option<VerdictName>,
    /// Whether the class relies on Verus's totals.
    by_totals: bool,
}

/// The entries' common class, or `None` when they disagree.
fn side(entries: &[Entry<'_>]) -> Option<Side> {
    let Some(first) = entries.first() else {
        return Some(Side::default());
    };
    entries
        .iter()
        .all(|e| e.class() == first.class())
        .then(|| Side {
            verdict: entries
                .iter()
                .all(|e| e.verdict() == first.verdict())
                .then(|| first.verdict()),
            class: Some(first.class()),
            by_totals: entries.iter().any(|e| e.by_totals),
        })
}

/// Resolves one identity on both sides. One entry per side compares
/// directly. A repeated friendly name (VI-0010) compares only when both sides
/// hold the same number of entries and every entry on each side has the same
/// class: every implementation then makes the same transition, whichever is
/// which. Otherwise `None` (incomparable).
fn resolve(left: &[Entry<'_>], right: &[Entry<'_>]) -> Option<(Side, Side)> {
    let repeated = left.len() > 1 || right.len() > 1;
    if repeated && left.len() != right.len() {
        return None;
    }
    Some((side(left)?, side(right)?))
}

/// Classifies one transition between comparison classes.
fn classify(before: Option<VerdictName>, after: Option<VerdictName>, removed: bool) -> Change {
    match (before, after) {
        (Some(a), Some(b)) if a == b => Change::Unchanged,
        (Some(VerdictName::Verified), Some(VerdictName::Failed)) => Change::Regressed,
        (Some(VerdictName::Verified), Some(VerdictName::Unknown | VerdictName::Provisional)) => {
            Change::BecameUnknown
        }
        (Some(_), Some(VerdictName::Verified)) => Change::Recovered,
        (None, Some(_)) => Change::Added,
        (Some(_), None) if removed => Change::Removed,
        (Some(_), None) => Change::NotObserved,
        _ => Change::Changed,
    }
}

fn scopes(result: &ProjectResult) -> Vec<(Target, String, Vec<String>)> {
    let mut scopes: Vec<_> = result
        .targets
        .iter()
        .filter(|t| t.requested)
        .map(|target| {
            (
                Target {
                    package_id: target.package_id.clone(),
                    name: target.target.clone(),
                    kind: target.kind.clone(),
                },
                target.report.mode.clone(),
                target.report.command.clone(),
            )
        })
        .collect();
    scopes.sort();
    scopes
}

/// What a comparison's notes report.
#[derive(Debug, Default)]
struct Tally {
    /// Classified functions whose class relies on Verus's totals.
    by_totals: usize,
    /// Repeated friendly names compared as uniform groups.
    repeated_compared: usize,
    /// Functions holding those names (the larger side).
    repeated_functions: usize,
    /// Repeated friendly names left incomparable.
    repeated_incomparable: usize,
}

/// Compares recorded evidence only when bound requested contexts/scopes agree.
/// Runs without proof reuse do not establish a comparable runtime context.
/// Absent functions remain unobserved; a partial run never proves their removal.
///
/// A function reported `unknown` that passed by Verus's totals
/// ([`crate::output::CrateReport::verus_totals_clear_unknown`]) is classified
/// as verified on either side, as Verus counted it. Its reported verdict stays
/// `unknown` in [`Difference::before`] or [`Difference::after`], and the
/// difference says the class relied on the totals.
#[must_use]
pub fn compare(before: &ProjectResult, after: &ProjectResult) -> Comparison {
    let compatible = before.no_reuse.is_none()
        && after.no_reuse.is_none()
        && !before.context_sha256.is_empty()
        && before.context_sha256 == after.context_sha256
        && scopes(before) == scopes(after);
    let after_project = after;
    let left = functions(before);
    let right = functions(after);
    let ids: std::collections::BTreeSet<_> = left.keys().chain(right.keys()).cloned().collect();
    let mut tally = Tally::default();
    let differences = ids
        .into_iter()
        .map(|(target, function)| {
            let key = (target.clone(), function.clone());
            let l = left.get(&key).map_or(&[][..], Vec::as_slice);
            let r = right.get(&key).map_or(&[][..], Vec::as_slice);
            let repeated = l.len() > 1 || r.len() > 1;
            let resolved = resolve(l, r);
            if repeated && resolved.is_some() {
                tally.repeated_compared += 1;
                tally.repeated_functions += l.len().max(r.len());
            } else if repeated {
                tally.repeated_incomparable += 1;
            }
            let (old, new) = resolved.unwrap_or_default();
            let change = if compatible && resolved.is_some() {
                classify(old.class, new.class, inventory(after_project, &target))
            } else {
                Change::Incomparable
            };
            let classified = change != Change::Incomparable;
            if classified {
                let count = |entries: &[Entry<'_>]| entries.iter().filter(|e| e.by_totals).count();
                tally.by_totals += count(l).max(count(r));
            }
            Difference {
                target,
                function,
                before: old.verdict,
                after: new.verdict,
                change,
                before_passed_by_verus_totals: classified && old.by_totals,
                after_passed_by_verus_totals: classified && new.by_totals,
                obligations: if repeated {
                    Vec::new()
                } else {
                    obligations(unique(l), unique(r), compatible)
                },
            }
        })
        .collect();
    Comparison {
        version: VERSION,
        compatible,
        before_revision: before.input_revision.clone(),
        after_revision: after.input_revision.clone(),
        after_currentness: after.currentness,
        differences,
        notes: comparison_notes(compatible, &tally),
    }
}

fn comparison_notes(compatible: bool, tally: &Tally) -> Vec<String> {
    let mut notes = Vec::new();
    if !compatible {
        notes.push("Requested scope, mode or runtime context differs; transitions are descriptive, not proof regressions.".into());
    }
    if tally.by_totals > 0 {
        notes.push(format!(
            "{} function(s) reported `unknown` passed by Verus's totals (not attributed one by one); the comparison counts them as passing, as Verus did. Their verdicts stay `unknown`.",
            tally.by_totals
        ));
    }
    if tally.repeated_compared > 0 {
        notes.push(format!(
            "Entries are friendly function names: {} repeated name(s), shared by {} functions (VI-0010), were each compared as a group, since every function sharing the name has the same result on each side.",
            tally.repeated_compared, tally.repeated_functions
        ));
    }
    if tally.repeated_incomparable > 0 {
        notes.push(format!(
            "Repeated friendly function names cannot identify one implementation; {} such name(s) whose functions differ in result or number are incomparable (VI-0010).",
            tally.repeated_incomparable
        ));
    }
    notes
}

fn effective(report: &FunctionReport) -> VerdictName {
    if report.establishment == crate::output::EstablishmentName::Provisional {
        VerdictName::Provisional
    } else {
        report.verdict
    }
}

fn inventory(project: &ProjectResult, id: &Target) -> bool {
    project.error.is_none()
        && project.exit_code == Some(0)
        && project.targets.iter().any(|target| {
            crate::acceptance::complete_target(target).is_some()
                && target.package_id == id.package_id
                && target.target == id.name
                && target.kind == id.kind
                && target.report.crates.iter().all(|report| {
                    report.verus.success == Some(true)
                        && report.verus.encountered_vir_error != Some(true)
                })
        })
}

fn obligations(
    before: Option<&FunctionReport>,
    after: Option<&FunctionReport>,
    compatible: bool,
) -> Vec<ObligationDifference> {
    let index = |function: Option<&FunctionReport>| {
        function
            .and_then(|f| f.obligations.as_ref())
            .into_iter()
            .flatten()
            .map(|obligation| (obligation.id.clone(), obligation.clone()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let left = index(before);
    let right = index(after);
    left.keys()
        .chain(right.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|id| {
            let old = left.get(&id);
            let new = right.get(&id);
            let before = old.map(|o| o.outcome);
            let after = new.map(|o| o.outcome);
            let advisory = new.or(old).is_some_and(|o| o.advisory);
            let change = if compatible {
                match (before, after) {
                    (Some(a), Some(b)) if a == b => Change::Unchanged,
                    (Some(OutcomeName::Proved), Some(OutcomeName::Failed)) if !advisory => {
                        Change::Regressed
                    }
                    (Some(OutcomeName::Proved), Some(OutcomeName::Unknown)) => {
                        Change::BecameUnknown
                    }
                    (Some(_), Some(OutcomeName::Proved)) => Change::Recovered,
                    (None, Some(_)) => Change::Added,
                    (Some(_), None) => Change::NotObserved,
                    _ => Change::Changed,
                }
            } else {
                Change::Incomparable
            };
            ObligationDifference {
                id,
                before,
                after,
                advisory,
                change,
            }
        })
        .collect()
}
