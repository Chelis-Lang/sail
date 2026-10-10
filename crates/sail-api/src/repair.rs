//! Focused repair checks, complete validation, evidence bundles and comparisons.

use serde::{Deserialize, Serialize};

use crate::output::{Diagnostic, FunctionReport, OutcomeName, VerdictName};
use crate::project::{Currentness, ProjectResult};
use crate::proof::Target;

/// Independent repair contract version.
pub const VERSION: u32 = 1;

/// Source-engine batch output; each phase retains its independent proof report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchResult {
    /// Complete selected-function phase, absent if it did not produce evidence.
    pub focus: Option<ProjectResult>,
    /// Independent complete requested-target phase.
    pub full: ProjectResult,
    /// Hash of the validated optional engine build manifest.
    pub manifest_sha256: String,
    /// Number of compiler invocations actually started by this batch.
    pub compiler_invocations: u64,
    /// True only for engine-supported focus/full batching within compiler calls.
    pub batched: bool,
    /// Exact focus selection was unavailable or ambiguous; full evidence is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_error: Option<crate::output::ErrorInfo>,
}

impl BatchResult {
    /// Whether selection succeeded and full validation established current complete evidence.
    /// Preliminary focus success cannot authorize a repair.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.selection_error.is_none() && accepted(&self.full)
    }
}

/// Whether an independently completed full report authorizes a repair.
#[must_use]
pub fn accepted(full: &ProjectResult) -> bool {
    let selected: Vec<_> = full.targets.iter().filter(|t| t.requested).collect();
    full.currentness == Currentness::Current
        && full.exit_code == Some(0)
        && full.error.is_none()
        && !selected.is_empty()
        && selected.iter().all(|target| {
            target.complete
                && target.report.error.is_none()
                && target.report.exit_code == Some(0)
                && target.report.summary.failed == 0
                && target.report.summary.unknown == 0
                && target.report.summary.provisional == 0
                && !target.report.crates.is_empty()
                && target.report.crates.iter().all(|report| {
                    report.verus.success == Some(true)
                        && report.verus.encountered_vir_error != Some(true)
                })
        })
}

/// Explicit selected target/function; never guessed from semantic dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// Cargo package selection.
    pub package: String,
    /// Explicit binary name; library selection is unique within the package.
    pub target: Option<String>,
    /// `lib` or `bin`; target names alone are ambiguous.
    pub kind: String,
    /// Exact crate-qualified function identifier as reported by Verus.
    pub function: String,
}

/// Repair submission uses the existing environment/revision request contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submission {
    /// Verification context, arguments and originating environment.
    pub verification: crate::jobs::Submission,
    /// Explicit target/function selection.
    pub selection: Selection,
}

/// Finished repair response; accepted is derived rather than trusted from storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultDocument {
    /// Independent contract version.
    pub version: u32,
    /// Complete phase evidence.
    pub batch: BatchResult,
    /// Derived from successful selection and the independently finalized full report.
    pub accepted: bool,
}

impl From<BatchResult> for ResultDocument {
    fn from(batch: BatchResult) -> Self {
        Self {
            version: VERSION,
            accepted: batch.accepted(),
            batch,
        }
    }
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// Same reported evidence class.
    Unchanged,
    /// A previously verified function now reports a settled failure.
    Regressed,
    /// A previously verified function no longer has a confirmed proof.
    BecameUnknown,
    /// A previously unproved function is now confirmed verified.
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
    /// Earlier verdict, absent when not observed.
    pub before: Option<VerdictName>,
    /// Later verdict, absent when not observed.
    pub after: Option<VerdictName>,
    /// Conservative comparison classification.
    pub change: Change,
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

fn functions(
    result: &ProjectResult,
) -> std::collections::BTreeMap<(Target, String), Vec<&FunctionReport>> {
    let mut records: std::collections::BTreeMap<_, Vec<_>> = std::collections::BTreeMap::new();
    for target in result.targets.iter().filter(|target| target.requested) {
        let id = Target {
            package_id: target.package_id.clone(),
            name: target.target.clone(),
            kind: target.kind.clone(),
        };
        for function in target.report.crates.iter().flat_map(|c| &c.functions) {
            records
                .entry((id.clone(), function.function.clone()))
                .or_default()
                .push(function);
        }
    }
    records
}

fn unique<'a>(records: Option<&Vec<&'a FunctionReport>>) -> Option<&'a FunctionReport> {
    match records?.as_slice() {
        [only] => Some(*only),
        _ => None,
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

/// Compares recorded evidence only when exact requested contexts/scopes agree.
/// Absent functions remain unobserved; a partial run never proves their removal.
#[must_use]
pub fn compare(before: &ProjectResult, after: &ProjectResult) -> Comparison {
    let compatible =
        before.context_sha256 == after.context_sha256 && scopes(before) == scopes(after);
    let after_project = after;
    let left = functions(before);
    let right = functions(after);
    let ids: std::collections::BTreeSet<_> = left.keys().chain(right.keys()).cloned().collect();
    let ambiguous = left
        .values()
        .chain(right.values())
        .any(|entries| entries.len() > 1);
    let differences = ids
        .into_iter()
        .map(|(target, function)| {
            let key = (target.clone(), function.clone());
            let obligations =
                obligations(unique(left.get(&key)), unique(right.get(&key)), compatible);
            let repeated = left.get(&key).is_some_and(|entries| entries.len() > 1)
                || right.get(&key).is_some_and(|entries| entries.len() > 1);
            let before = (!repeated)
                .then(|| unique(left.get(&key)).map(effective))
                .flatten();
            let after = (!repeated)
                .then(|| unique(right.get(&key)).map(effective))
                .flatten();
            let change = if compatible && !repeated {
                match (before, after) {
                    (Some(a), Some(b)) if a == b => Change::Unchanged,
                    (Some(VerdictName::Verified), Some(VerdictName::Failed)) => Change::Regressed,
                    (
                        Some(VerdictName::Verified),
                        Some(VerdictName::Unknown | VerdictName::Provisional),
                    ) => Change::BecameUnknown,
                    (Some(_), Some(VerdictName::Verified)) => Change::Recovered,
                    (None, Some(_)) => Change::Added,
                    (Some(_), None) if inventory(after_project, &target) => Change::Removed,
                    (Some(_), None) => Change::NotObserved,
                    _ => Change::Changed,
                }
            } else {
                Change::Incomparable
            };
            Difference {
                target,
                function,
                before,
                after,
                change,
                obligations: if repeated { Vec::new() } else { obligations },
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
        notes: comparison_notes(compatible, ambiguous),
    }
}

fn comparison_notes(compatible: bool, ambiguous: bool) -> Vec<String> {
    let mut notes = Vec::new();
    if !compatible {
        notes.push("Requested scope, mode or runtime context differs; transitions are descriptive, not proof regressions.".into());
    }
    if ambiguous {
        notes.push("Repeated friendly function names cannot identify one implementation; their transitions are incomparable (VI-0010).".into());
    }
    notes
}

fn effective(report: &FunctionReport) -> VerdictName {
    if report.tier == crate::output::TierName::Provisional {
        VerdictName::Provisional
    } else {
        report.verdict
    }
}

fn inventory(project: &ProjectResult, id: &Target) -> bool {
    project.error.is_none()
        && project.exit_code == Some(0)
        && project.targets.iter().any(|target| {
            target.complete
                && target.package_id == id.package_id
                && target.target == id.name
                && target.kind == id.kind
                && target.report.error.is_none()
                && !target.report.crates.is_empty()
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
