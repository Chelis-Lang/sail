//! One bounded-evidence record shared by Kani and D.I.V.E. symbolic results
//! (ADR 0185).
//!
//! Unstable groundwork for the Kani phase (ADR 0180) and the D.I.V.E. symbolic
//! pickup, hidden from the documentation. No command, MCP tool, JSON report or
//! service request uses it, it carries no protocol version, and it may change
//! without notice.
//!
//! A record never holds a tier above [`Tier::Bounded`] and never says
//! `verified`. Classifying a native outcome here authenticates nothing: the
//! producer states what it observed in [`Integrity`] and [`Provenance`].
#![doc(hidden)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::assurance::EvidenceTier;

#[cfg(test)]
mod tests;

/// The only tier a bounded record can carry. Stronger tiers do not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Checked within declared finite bounds.
    Bounded,
}

impl From<Tier> for EvidenceTier {
    fn from(_: Tier) -> Self {
        Self::Bounded
    }
}

/// Backend that produced the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// Kani over CBMC.
    Kani,
    /// D.I.V.E. symbolic and symbolic-path strategies.
    Dive,
}

/// What kind of thing was checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    /// One Kani proof harness.
    Harness,
    /// Every configured safety check of one bound TLA+ model.
    Model,
    /// One named invariant of a bound TLA+ model.
    Property,
}

/// The checked subject, named exactly as its backend names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    /// Producing backend.
    pub backend: Backend,
    /// Subject kind.
    pub kind: SubjectKind,
    /// Exact native identity, such as a harness path or a property name.
    pub id: String,
}

/// Shared outcome vocabulary. Only the first two carry a tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every selected check holds, to the recorded strength and sufficiency.
    Success,
    /// A check fails on an execution inside the bounds.
    Counterexample,
    /// Completed: the tool showed its bound is too small to decide.
    BoundInsufficient,
    /// Completed: an induction or predicate obligation failed without a
    /// reachable violation.
    Inconclusive,
    /// No claim: a deadline fired.
    Timeout,
    /// No claim: memory ran out.
    OutOfMemory,
    /// No claim: a budget or interruption the tool did not identify further.
    Stopped,
    /// No claim: the tool failed.
    Crash,
    /// No claim: the caller cancelled.
    Cancelled,
    /// No claim: the input or feature is not admitted.
    Unsupported,
    /// No claim: the subject did not build.
    BuildFailure,
}

impl Outcome {
    /// The tier this outcome carries: bounded for a success or counterexample.
    #[must_use]
    pub fn tier(self) -> Option<Tier> {
        matches!(self, Self::Success | Self::Counterexample).then_some(Tier::Bounded)
    }

    /// Whether the tool ran to a result. Other outcomes are no-claim and are
    /// never served from a receipt.
    #[must_use]
    pub fn completed(self) -> bool {
        matches!(
            self,
            Self::Success | Self::Counterexample | Self::BoundInsufficient | Self::Inconclusive
        )
    }
}

/// How a tiered outcome was reached. Both strengths stay in the bounded tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strength {
    /// Exploration of every execution inside the bounds.
    Bounded,
    /// Discharged base and step obligations over the finite configured model.
    Inductive,
}

/// What the bounds cover of the subject's executions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sufficiency {
    /// The tool established that the check covers every execution.
    Sufficient,
    /// Every prefix up to the bound was checked and executions continue past it.
    Prefix,
    /// No execution reaches the bound; every shorter prefix was checked.
    NotReached,
    /// The tool showed an execution exceeds the bound.
    Insufficient,
    /// No coverage statement. A counterexample needs none.
    NotEstablished,
}

/// What a bound limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundKind {
    /// Loop and recursion unwinding.
    Unwind,
    /// CBMC object-identifier bits.
    ObjectBits,
    /// Number of `Next` transitions in a checked path.
    PathLength,
    /// Depth `K` of a k-induction base and step.
    InductionDepth,
}

/// Where a bound was declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundSource {
    /// An attribute or annotation in the checked source.
    Annotation,
    /// A command-line argument.
    Argument,
    /// A project or model configuration file.
    Configuration,
    /// The tool's default.
    ToolDefault,
    /// Read from the tool's own report, which does not say where it came from.
    Reported,
}

/// One declared bound.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bound {
    /// What it limits.
    pub kind: BoundKind,
    /// What it applies to: `subject`, or a native loop or function name.
    pub scope: String,
    /// The bound.
    pub value: u64,
    /// Where it was declared.
    pub source: BoundSource,
}

/// Something the result takes on trust instead of checking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssumptionKind {
    /// A function replaced by a stub.
    Stub,
    /// A function contract assumed at its call sites.
    AssumedContract,
    /// A loop abstracted by its invariant.
    LoopInvariant,
    /// Termination was not checked.
    TerminationUnchecked,
    /// The harness was generated, not written by hand.
    GeneratedHarness,
    /// One monomorphization stands for a generic function.
    SingleMonomorphization,
    /// A finite layout or type invariant selected for the encoding.
    FiniteLayout,
    /// A state or action constraint restricting the explored model.
    StateConstraint,
}

/// One named assumption.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assumption {
    /// Its kind.
    pub kind: AssumptionKind,
    /// The native name it concerns.
    pub name: String,
}

/// A concrete counterexample and who replayed it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Witness {
    /// Native format, such as `dive-trace` or `itf`.
    pub format: String,
    /// The independent validator that replayed it; `None` when only the
    /// producing tool vouches for it.
    pub validated_by: Option<String>,
}

/// One tool that took part in the result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    /// Tool name, such as `kani`, `cbmc` or `cadical`.
    pub name: String,
    /// Version the tool reported, when it reported one.
    pub version: Option<String>,
    /// SHA-256 of the executable, when Sail authenticated it.
    pub sha256: Option<String>,
}

/// How much of the execution Sail itself observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Integrity {
    /// Nothing checked beyond the tool's own statement.
    NotEstablished,
    /// The tool's exit, text, summary and per-check records agree.
    Reconciled,
    /// Reconciled, and Sail supervised the process: observed exit, no signal,
    /// no out-of-memory event, complete output, deadline not fired, process
    /// group empty afterwards.
    Supervised,
}

/// Whether the result was computed now or replayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Computed by this invocation.
    Fresh,
    /// Replayed from a receipt in this machine's store.
    ReceiptLocal,
    /// Replayed from a signed receipt in a shared store.
    ReceiptShared,
}

/// Measured cost of the original run; absent values were not measured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    /// Wall time in milliseconds.
    pub wall_ms: Option<u64>,
    /// Peak resident memory in bytes.
    pub peak_memory_bytes: Option<u64>,
}

/// The fields of a record, before [`Evidence::new`] checks them together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parts {
    /// What was checked.
    pub subject: Subject,
    /// What happened.
    pub outcome: Outcome,
    /// Bounded for a success or counterexample, otherwise `None`.
    pub tier: Option<Tier>,
    /// Present exactly when `tier` is.
    pub strength: Option<Strength>,
    /// Every declared bound the producer knows.
    pub bounds: Vec<Bound>,
    /// What the bounds cover.
    pub sufficiency: Sufficiency,
    /// What the result takes on trust.
    pub assumptions: Vec<Assumption>,
    /// Check counts keyed by the tool's own status names.
    pub checks: BTreeMap<String, u64>,
    /// The counterexample, when the producer retained one.
    pub witness: Option<Witness>,
    /// Tools that took part.
    pub tools: Vec<Tool>,
    /// How much Sail observed.
    pub integrity: Integrity,
    /// Fresh or replayed.
    pub provenance: Provenance,
    /// Measured cost.
    pub cost: Cost,
}

/// A checked bounded-evidence record. Read its fields through `Deref`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Parts", into = "Parts")]
pub struct Evidence(Parts);

impl Evidence {
    /// Checks the fields against each other.
    ///
    /// # Errors
    /// `SAIL6303` when the tier, strength, sufficiency, witness, integrity or
    /// provenance contradict the outcome.
    pub fn new(parts: Parts) -> Result<Self, String> {
        let outcome = parts.outcome;
        let tiered = outcome.tier().is_some();
        let consistent = !parts.subject.id.is_empty()
            && parts.tier == outcome.tier()
            && parts.strength.is_some() == tiered
            && (parts.strength != Some(Strength::Inductive)
                || (outcome == Outcome::Success && parts.sufficiency == Sufficiency::Sufficient))
            && (parts.sufficiency == Sufficiency::Insufficient)
                == (outcome == Outcome::BoundInsufficient)
            && (outcome == Outcome::Success
                || parts.sufficiency == Sufficiency::Insufficient
                || parts.sufficiency == Sufficiency::NotEstablished)
            && (parts.witness.is_none() || outcome == Outcome::Counterexample)
            && (!tiered || (parts.integrity >= Integrity::Reconciled && !parts.tools.is_empty()))
            && (parts.provenance == Provenance::Fresh
                || (outcome.completed() && parts.integrity == Integrity::Supervised));
        if consistent {
            Ok(Self(parts))
        } else {
            Err(format!(
                "SAIL6303: inconsistent bounded evidence record for outcome {outcome:?}"
            ))
        }
    }
}

impl std::ops::Deref for Evidence {
    type Target = Parts;
    fn deref(&self) -> &Parts {
        &self.0
    }
}

impl TryFrom<Parts> for Evidence {
    type Error = String;
    fn try_from(parts: Parts) -> Result<Self, String> {
        Self::new(parts)
    }
}

impl From<Evidence> for Parts {
    fn from(evidence: Evidence) -> Self {
        evidence.0
    }
}

/// The outcome, strength and sufficiency a native result maps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    /// Shared outcome.
    pub outcome: Outcome,
    /// Strength, for a tiered outcome.
    pub strength: Option<Strength>,
    /// Coverage the native result alone establishes.
    pub sufficiency: Sufficiency,
}

impl Claim {
    const fn tiered(outcome: Outcome, strength: Strength, sufficiency: Sufficiency) -> Self {
        Self {
            outcome,
            strength: Some(strength),
            sufficiency,
        }
    }

    const fn untiered(outcome: Outcome) -> Self {
        Self {
            outcome,
            strength: None,
            sufficiency: Sufficiency::NotEstablished,
        }
    }
}

/// The three outcomes the `sail-kani` decoder admits (ADR 0128).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KaniOutcome {
    /// Every selected check succeeds.
    BoundedSuccess,
    /// A non-unwinding property fails.
    Counterexample,
    /// Only unwinding checks fail.
    BoundInsufficient,
}

/// Maps a decoded Kani outcome. The decoder does not see whether unwinding
/// checks were enabled, so a success alone establishes no sufficiency.
#[must_use]
pub const fn kani_claim(native: KaniOutcome) -> Claim {
    match native {
        KaniOutcome::BoundedSuccess => Claim::tiered(
            Outcome::Success,
            Strength::Bounded,
            Sufficiency::NotEstablished,
        ),
        KaniOutcome::Counterexample => Claim::tiered(
            Outcome::Counterexample,
            Strength::Bounded,
            Sufficiency::NotEstablished,
        ),
        KaniOutcome::BoundInsufficient => Claim {
            outcome: Outcome::BoundInsufficient,
            strength: None,
            sufficiency: Sufficiency::Insufficient,
        },
    }
}

/// Maps the `verdict` or `proof` value of a D.I.V.E. symbolic or symbolic-path
/// report row. `None` for any other value: an unknown verdict is a protocol
/// error for the caller, never a claim.
#[must_use]
pub fn dive_symbolic_claim(native: &str) -> Option<Claim> {
    Some(match native {
        "success" => Claim::tiered(Outcome::Success, Strength::Bounded, Sufficiency::Sufficient),
        "bounded-safe" => Claim::tiered(Outcome::Success, Strength::Bounded, Sufficiency::Prefix),
        "executions-too-short" => {
            Claim::tiered(Outcome::Success, Strength::Bounded, Sufficiency::NotReached)
        }
        "inductive-safety" => Claim::tiered(
            Outcome::Success,
            Strength::Inductive,
            Sufficiency::Sufficient,
        ),
        "invariant-violation" | "deadlock" => Claim::tiered(
            Outcome::Counterexample,
            Strength::Bounded,
            Sufficiency::NotEstablished,
        ),
        "induction-inconclusive" | "inconclusive" => Claim::untiered(Outcome::Inconclusive),
        "unsupported" => Claim::untiered(Outcome::Unsupported),
        "error" => Claim::untiered(Outcome::Crash),
        "stopped" => Claim::untiered(Outcome::Stopped),
        _ => return None,
    })
}
