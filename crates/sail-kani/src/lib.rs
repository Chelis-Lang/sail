//! Experimental bounded observations from hand-written Kani harnesses.
//!
//! Decoding is not execution, currentness, input-closure authentication or proof
//! receipt authority. These observations never report an ordinary verified
//! verdict. The eventual executor must bind original bytes and tool identity.
#![forbid(unsafe_code)]

mod decode;
mod strict_json;

use std::collections::BTreeMap;

use sail_api::assurance::EvidenceTier;
use serde::Serialize;

/// Exact supported Kani export producer version.
pub const KANI_VERSION: &str = "0.68.0";
/// Exact supported CBMC version.
pub const CBMC_VERSION: &str = "6.11.0";
/// Maximum admitted bytes in each original export or output stream.
pub const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;

/// A bounded backend observation; it is never an ordinary verified verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// All selected checks succeed within the harness's declared bounds.
    BoundedSuccess,
    /// A native non-unwinding property fails within the declared bounds.
    Counterexample,
    /// Only unwinding checks fail; no property refutation or proof follows.
    BoundInsufficient,
}

/// Immutable decoded native observation, without reusable proof authority.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Observation {
    harness: String,
    outcome: Outcome,
    evidence_tier: Option<EvidenceTier>,
    check_counts: BTreeMap<String, usize>,
    failed_checks: Vec<serde_json::Value>,
    harness_metadata: serde_json::Value,
    backend_configuration: serde_json::Value,
    property_details: serde_json::Value,
}

impl Observation {
    /// Original exact harness identity; no suffix or printed-label matching.
    #[must_use]
    pub fn harness(&self) -> &str {
        &self.harness
    }

    /// The bounded native outcome; never `verified`.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// Bounded evidence only; insufficient bounds have no evidence tier.
    /// This classification alone does not authenticate an execution or receipt.
    #[must_use]
    pub fn evidence_tier(&self) -> Option<EvidenceTier> {
        self.evidence_tier
    }

    /// Exact check-status multiplicities from the original export.
    #[must_use]
    pub fn check_counts(&self) -> &BTreeMap<String, usize> {
        &self.check_counts
    }

    /// Original failed checks, preserving diagnostics, source and multiplicity.
    #[must_use]
    pub fn failed_checks(&self) -> &[serde_json::Value] {
        &self.failed_checks
    }
}

/// A strict export-admission refusal, never a backend proof or counterexample.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "error[SAIL6301]: {}", self.0)
    }
}

impl std::error::Error for Error {}

fn refuse(message: impl Into<String>) -> Error {
    Error(message.into())
}

/// Decode one original Kani 0.68.0 / CBMC 6.11.0 hand-written proof harness.
/// Requires the exact named result, one matching textual verdict, original
/// process exit and a nonempty consistent check inventory. No cache is read or
/// written and no source/currentness or process provenance is manufactured.
///
/// # Errors
/// `SAIL6301` for unsupported producers, contracts/generated/should-panic
/// harnesses, tool errors, incomplete exports or contradictory native outcomes.
pub fn decode(
    harness: &str,
    exit: i32,
    stdout: &[u8],
    export: &[u8],
) -> Result<Observation, Error> {
    decode::decode(harness, exit, stdout, export)
}
