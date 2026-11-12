//! Verus mutation-campaign observations (ADR 0111), never proof verdicts.
//!
//! Every view uses this versioned model. The classifier accepts complete,
//! producer-attributed evidence and refuses missing identities, unstable
//! repetitions and resource-only failures. It never changes ordinary reports,
//! solver answers, ledger verdicts or evidence tiers.

mod classify;
mod service;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use classify::{classify, summarize};
pub use service::{Candidate, Detail, Exclusion, Item, Plan, State, Submission, View};

/// Campaign protocol and report version, separate from verification output.
pub const VERSION: u32 = 1;
/// Minimum independent repetitions of both baseline and each mutant.
pub const MIN_REPEATS: u32 = 2;
/// Hard campaign inventory bound.
pub const MAX_MUTANTS: u32 = 10_000;
/// Hard bound on independently owned verification copies.
pub const MAX_COPIES: u32 = 8;
/// Hard wall-time budget, including setup and confirmations.
pub const MAX_SECONDS: u64 = 24 * 60 * 60;

/// Explicit resource bounds; exhaustion retains a partial inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    /// Maximum admitted distinct source mutants.
    pub mutants: u32,
    /// Independently warmed, owned mutant copies (baseline has its own copy).
    pub copies: u32,
    /// Fresh attempts per mode, never fewer than two.
    pub repeats: u32,
    /// Total wall seconds including copies, baseline and full confirmations.
    pub seconds: u64,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            mutants: 100,
            copies: 2,
            repeats: MIN_REPEATS,
            seconds: 3600,
        }
    }
}
impl Bounds {
    /// Validates hard limits without silently changing a requested budget.
    ///
    /// # Errors
    /// `SAIL3170` for zero, excessive or insufficient-repeat bounds.
    pub fn validate(&self) -> Result<(), String> {
        if self.mutants == 0
            || self.mutants > MAX_MUTANTS
            || self.copies == 0
            || self.copies > MAX_COPIES
            || !(MIN_REPEATS..=20).contains(&self.repeats)
            || self.seconds == 0
            || self.seconds > MAX_SECONDS
        {
            return Err("SAIL3170: invalid campaign bounds; require 1..10000 mutants, 1..8 copies, 2..20 repetitions and 1..86400 seconds".to_owned());
        }
        Ok(())
    }
}

/// A compiler-qualified function, never a friendly-name or line-window guess.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Function {
    /// Complete Cargo package identity, including its source/version.
    pub package_id: String,
    /// Target name, distinguishing library and binary targets too.
    pub target: String,
    /// Cargo target kinds, in canonical sorted order.
    pub target_kinds: Vec<String>,
    /// Exact crate-qualified function identity emitted by the producer.
    pub function: String,
}

/// Identity shared by a baseline twin and its deliberately changed source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// SHA-256 of the frozen unmutated input closure.
    pub baseline_sha256: String,
    /// SHA-256 identity of the validated verifier runtime inventory.
    pub runtime_sha256: String,
    /// SHA-256 of the exact solver executable, independent of verifier identity.
    pub solver_sha256: String,
    /// SHA-256 binding arguments, explicit limits and verification configuration.
    pub options_sha256: String,
}

/// The verification scope that actually produced an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// Exact selected function/module; never full-unit confirmation.
    Scoped,
    /// Independent whole requested unit.
    Full,
}

/// An attributed failed obligation's producer classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Failure {
    /// The obligation proved or no failure was reported.
    None,
    /// A verifier rejection positively identified as non-resource failure.
    Verifier,
    /// Resource-limit exhaustion.
    ResourceLimit,
    /// Time-budget exhaustion.
    Timeout,
    /// Explicit cancellation or supersession.
    Cancelled,
    /// The producer cannot establish its failure category.
    Unattributed,
}

/// Original solver observation; campaign interpretation never rewrites it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    /// Verbatim answer category, absent when not recorded.
    pub answer: Option<crate::output::AnswerName>,
    /// Verbatim reason-unknown response, when present.
    pub reason_unknown: Option<String>,
    /// Recorded native-solver answer provenance, absent when unrecorded.
    pub source: Option<crate::output::SourceName>,
    /// The requested rlimit (zero explicitly means unlimited).
    pub rlimit_limit: Option<u64>,
    /// Actual effort of this check, never a whole-function approximation.
    pub rlimit_used: Option<u64>,
    /// Producer-attributed failure category.
    pub failure: Failure,
    /// Native model exactly as returned, without independent-model validation.
    pub model: Option<String>,
}

/// A compiler function observation in one completed invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionObservation {
    /// Exact producer identity.
    pub identity: Function,
    /// The function's actual Verus verdict.
    pub verified: bool,
    /// Existing recorded instability, independent of campaign repetitions.
    pub unstable: bool,
    /// The producer established that the complete check inventory was captured;
    /// an authoritatively empty inventory is distinct from missing evidence.
    pub checks_complete: bool,
    /// Complete original check observations.
    pub checks: Vec<Check>,
}

/// A diagnostic and its explicit compiler-producer attribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    /// Exact function; absent/ambiguous attribution cannot establish detection.
    pub function: Option<Function>,
    /// Original diagnostic level.
    pub level: String,
    /// Original diagnostic code, when present.
    pub code: Option<String>,
    /// Complete original message. Repetitions compare message multiplicity;
    /// stderr line ordering is not a campaign stability condition (ADR 0111).
    pub message: String,
    /// Complete backend diagnostic, including original spans and notes.
    pub original: crate::output::Diagnostic,
}

/// Evidence of one baseline or mutant attempt, after independent finalization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Frozen twin/runtime/limits identity.
    pub context: Context,
    /// Actual source closure verified by this attempt.
    pub source_sha256: String,
    /// Actual verification scope.
    pub scope: Scope,
    /// Normal compiler termination and input/output finalization established.
    pub completed: bool,
    /// Rust compilation or Verus mode checking failed before verification.
    pub unviable: bool,
    /// Complete requested function inventory; partial coverage cannot score.
    pub functions_complete: bool,
    /// Function observations, retaining other baseline failures too.
    pub functions: Vec<FunctionObservation>,
    /// Original diagnostics and explicit attribution.
    pub diagnostics: Vec<Diagnostic>,
}

/// Stable exec-only operator vocabulary; spec/proof operators have no variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecClass {
    /// Arithmetic operator replacement.
    Arith,
    /// Comparison replacement.
    Compare,
    /// Executable integer literal replacement.
    Constant,
    /// Boolean operator replacement.
    Logic,
    /// Executable statement deletion.
    Statement,
    /// Wrong return in a restricted input region.
    RareRegion,
    /// Strict/non-strict loop bound replacement.
    LoopBound,
    /// One-step index/range adjustment.
    OffByOne,
    /// Adjacent call arguments exchanged.
    SwappedArgs,
    /// Final executable else branch removed.
    DroppedElse,
    /// Early return in a unit-returning function.
    EarlyReturn,
}

/// Exact executable-source change, always applied in an owned copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mutation {
    /// Deterministic inventory identity.
    pub id: String,
    /// Relative file within the frozen project closure.
    pub file: PathBuf,
    /// Exact producer-qualified target, or absent until resolvable.
    pub function: Option<Function>,
    /// Exec-only operator class.
    pub class: ExecClass,
    /// First replaced UTF-8 byte in the original file.
    pub start: u64,
    /// One past the last replaced byte; equal to start for insertion.
    pub end: u64,
    /// Exact original bytes as UTF-8.
    pub before: String,
    /// Exact replacement bytes as UTF-8.
    pub after: String,
    /// SHA-256 of the resulting complete mutant input closure.
    pub source_sha256: String,
}

/// Native evidence behind a stable, attributed non-resource rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Detection {
    /// At least one failed check returned recorded native `sat`; models remain
    /// unvalidated backend observations (ADR 0106).
    CounterexampleConfirmed,
    /// Verus rejected the function, including quantified `unknown` answers.
    VerifierRejected,
}

/// Why an observation cannot contribute to the mutation score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// No unambiguous compiler-qualified mutated function.
    AmbiguousFunction,
    /// Baseline or mutant identity is incomplete, inconsistent or stale.
    IdentityMismatch,
    /// Baseline did not supply the required independent repetitions.
    BaselineMissing,
    /// Baseline twin was not verified.
    BaselineFailed,
    /// Baseline exceeded half a finite check's requested rlimit.
    BaselineNearLimit,
    /// Baseline check inventory, limits, effort or provenance is missing.
    BaselineHeadroomMissing,
    /// Baseline's existing or repeated verdict/diagnostic evidence is unstable.
    BaselineUnstable,
    /// A mutant attempt ended incompletely or lacked its function inventory.
    Incomplete,
    /// Mutant repetitions differ in verdicts, attributed messages or evidence.
    Unstable,
    /// Rejection had no diagnostic attributed to the mutated function.
    DiagnosticUnattributed,
    /// The producer could not attribute a non-resource failure category.
    FailureUnattributed,
    /// The function's only failures were resource, timeout or cancellation.
    ResourceOnly,
    /// A function other than the target acquired a failure beyond baseline.
    CollateralFailure,
    /// Independent unscoped whole-unit confirmation is absent.
    FullConfirmationMissing,
    /// Backend observations contradict one another or have duplicate identities.
    InconsistentEvidence,
}

/// Campaign-only interpretation; never an ordinary verification verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "evidence", rename_all = "kebab-case")]
pub enum Outcome {
    /// Stable whole-unit rejection with an attributed diagnostic and valid twin.
    Rejected(Detection),
    /// Repeated independent whole-unit checks confirm survival relative to baseline.
    Survived,
    /// Scoped survival only, explicitly awaiting required full-unit confirmation.
    SurvivedScoped,
    /// Excluded from score, with an explicit reason.
    Inconclusive(Reason),
    /// Repeated Rust/mode rejection before verification.
    Unviable,
    /// Not run because the shape cannot be handled; reason is retained verbatim.
    Unsupported(String),
}

/// Fully bound inputs to the pure campaign classifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classification {
    /// Exact planned mutation and output closure.
    pub mutation: Mutation,
    /// Baseline identity against which all attempts are checked.
    pub context: Context,
    /// Selected repetition count, at least [`MIN_REPEATS`].
    pub repeats: u32,
    /// Repeated baseline twins, including full-unit scope.
    pub baseline: Vec<Observation>,
    /// Repeated mutant observations, preserving scoped and full runs separately.
    pub attempts: Vec<Observation>,
}

/// Counts shown alongside every score, including partial-campaign dispositions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    /// Stable detected changes.
    pub rejected: u64,
    /// Native-sat evidence subset of rejected.
    pub counterexample_confirmed: u64,
    /// Verifier-rejected evidence subset of rejected.
    pub verifier_rejected: u64,
    /// Whole-unit-confirmed survivors.
    pub survived: u64,
    /// Scoped-only survivors, always labelled incomplete pending confirmation.
    pub survived_scoped: u64,
    /// Inconclusive observations by exact reason.
    pub inconclusive: BTreeMap<Reason, u64>,
    /// Noncompiling or pre-verification-mode failures.
    pub unviable: u64,
    /// Known mutant shapes the runner could not execute.
    pub unsupported: u64,
    /// Planned mutations not attempted because a budget or filter excluded them.
    pub omitted: u64,
    /// Mutations explicitly cancelled or superseded.
    pub cancelled: u64,
    /// rejected / (rejected + survived + `survived_scoped`), absent at denominator 0.
    pub score: Option<f64>,
}
