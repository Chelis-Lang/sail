//! Version-one proof observations, separate from legacy job lifecycle events.
//!
//! Observations never establish a complete target proof. Only a terminal project
//! report can establish current verification after compiler and input finalization.

use serde::{Deserialize, Serialize};

use crate::output::{Diagnostic, FunctionReport, ObligationReport};

/// Independent proof-stream version.
pub const VERSION: u32 = 1;

/// A Cargo target's complete identity, including same-name library/binary targets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Target {
    /// Cargo package identifier including version/source.
    pub package_id: String,
    /// Cargo target name.
    pub name: String,
    /// Cargo target kinds.
    pub kind: Vec<String>,
}

/// Verification scope of this observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Explicit selected-function verification.
    Focus,
    /// Independent complete requested-target verification.
    Full,
}

/// Producer-side observation identity. Sequence numbers belong to the journal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputEvent {
    /// Always [`VERSION`].
    pub version: u32,
    /// Stable unique producer id, for idempotent append/reconnect.
    pub event_id: String,
    /// Establishing invocation.
    pub run_id: String,
    /// Verification context digest.
    pub context_sha256: String,
    /// Input revision associated with the observation.
    pub input_revision: String,
    /// Complete Cargo target identity.
    pub target: Target,
    /// Focused or full scope.
    pub phase: Phase,
    /// Crate-qualified function, when known authoritatively.
    pub function: Option<String>,
    /// Obligation identity, when known authoritatively.
    pub obligation: Option<String>,
    /// Observed payload; not a current complete verification claim.
    pub observation: Observation,
}

/// Bounded query progress; not a complete solver answer or proof verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationProgress {
    /// Producer description, limited to 4,096 UTF-8 bytes.
    pub description: String,
    /// The source engine finished this query; completion does not imply validity.
    pub completed: bool,
}

/// Authoritative producer facts with explicitly preliminary proof interpretation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Observation {
    /// A phase began inside its compiler invocation.
    PhaseStarted,
    /// A function's checks started.
    FunctionStarted,
    /// Progress for one exactly identified query, without solver evidence claims.
    ObligationProgress(ObligationProgress),
    /// An obligation's complete solver evidence, including raw model if any.
    ObligationObserved(ObligationReport),
    /// A function report, still preliminary before independent finalization.
    FunctionObserved(FunctionReport),
    /// A diagnostic emitted by the verifier with exact producer attribution.
    Diagnostic(Diagnostic),
    /// The source engine completed this phase's checks.
    PhaseFinished,
    /// Compiler checks and target output completed. Currentness remains separate.
    CompilerSealed,
    /// Coverage or stream failure: incomplete observations cannot confirm proofs.
    Unavailable(String),
}

impl InputEvent {
    /// Checks the stream contract without inferring missing identities.
    ///
    /// # Errors
    /// Returns a stable explanation when required scope/identity is absent.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION
            || self.event_id.is_empty()
            || self.run_id.is_empty()
            || self.context_sha256.is_empty()
            || self.input_revision.is_empty()
            || self.target.package_id.is_empty()
            || self.target.name.is_empty()
            || self.target.kind.is_empty()
        {
            return Err("SAIL0902: incomplete proof event identity".into());
        }
        if self.obligation.is_some() && self.function.is_none() {
            return Err("SAIL0902: obligation event has no function identity".into());
        }
        if matches!(self.observation, Observation::CompilerSealed) && self.phase != Phase::Full {
            return Err("SAIL0902: compiler seal cannot finalize a focused phase".into());
        }
        match &self.observation {
            Observation::FunctionStarted | Observation::FunctionObserved(_) => {
                if self.function.is_none() {
                    return Err("SAIL0902: function event has no function identity".into());
                }
            }
            Observation::ObligationProgress(progress) => {
                if progress.description.is_empty() || progress.description.len() > 4096 {
                    return Err("SAIL0902: invalid bounded obligation progress description".into());
                }
                let id = self
                    .obligation
                    .as_deref()
                    .and_then(|id| id.parse::<sail_ledger::ObligationId>().ok());
                if !id.as_ref().is_some_and(|id| {
                    Some(id.function.as_str()) == self.function.as_deref()
                        && sail_ledger::ObligationKind::is_query_kind(&id.kind.0)
                }) {
                    return Err(
                        "SAIL0902: query progress requires an exact obligation identity".into(),
                    );
                }
            }
            Observation::ObligationObserved(report)
                if self.obligation.as_deref() != Some(&report.id) =>
            {
                return Err("SAIL0902: obligation identity disagrees with evidence".into());
            }
            _ => {}
        }
        if let Observation::FunctionObserved(report) = &self.observation {
            if self.function.as_deref() != Some(&report.function) {
                return Err("SAIL0902: function identity disagrees with evidence".into());
            }
            if report.tier != crate::output::TierName::Provisional
                || report.verdict != crate::output::VerdictName::Provisional
            {
                return Err("SAIL0902: live function evidence must remain provisional".into());
            }
        }
        Ok(())
    }
}

/// One durable event. The journal alone assigns monotone sequence numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Proof-stream version.
    pub version: u32,
    /// Job id; stream identity changes when the retained job is replaced.
    pub stream_id: String,
    /// Strictly increasing journal sequence, starting at one.
    pub sequence: u64,
    /// Immutable source-engine observation.
    pub input: InputEvent,
}

/// A resumable cursor. It is meaningful only within its named stream.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    /// Named durable stream; empty on the first request.
    pub stream_id: String,
    /// Last sequence consumed; zero before the first event.
    pub after: u64,
}

/// Bounded proof event page, independent from whole-result RPC payload size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    /// Independent stream version.
    pub version: u32,
    /// Cursor to pass to the next request, including empty pages.
    pub next: Cursor,
    /// Ordered durable observations.
    pub events: Vec<Event>,
    /// True only when no further worker observations can arrive.
    pub terminal: bool,
    /// Missing or rejected events make the stream incomplete; never hide a gap.
    pub gap: Option<String>,
}
