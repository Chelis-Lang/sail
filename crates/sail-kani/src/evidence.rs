//! The decoded observation as the shared bounded-evidence record (ADR 0185).
use sail_api::bounded::{
    Backend, Bound, BoundKind, BoundSource, Cost, Evidence, Integrity, KaniOutcome, Parts,
    Provenance, Subject, SubjectKind, Tool, kani_claim,
};

use crate::{CBMC_VERSION, Error, KANI_VERSION, Observation, Outcome, refuse};

fn tool(name: &str, version: Option<&str>) -> Tool {
    Tool {
        name: name.into(),
        version: version.map(Into::into),
        sha256: None,
    }
}

impl Observation {
    /// The shared bounded-evidence record for this observation. Not yet part
    /// of any command, MCP tool or report.
    ///
    /// It states only what the decoder saw: reconciled native records, the
    /// admitted tool versions without executable identity, and the reported
    /// object bits. Unwind bounds, sufficiency, supervision and cost belong
    /// to the executor.
    ///
    /// # Errors
    /// `SAIL6301` if the record is inconsistent, which the decoder's admission
    /// rules out.
    pub fn bounded_evidence(&self) -> Result<Evidence, Error> {
        let claim = kani_claim(match self.outcome {
            Outcome::BoundedSuccess => KaniOutcome::BoundedSuccess,
            Outcome::Counterexample => KaniOutcome::Counterexample,
            Outcome::BoundInsufficient => KaniOutcome::BoundInsufficient,
        });
        let object_bits = self.backend_configuration["object_bits"]
            .as_u64()
            .ok_or_else(|| refuse("missing CBMC object bits"))?;
        let checks = self
            .check_counts
            .iter()
            .map(|(status, count)| Ok((status.clone(), u64::try_from(*count)?)))
            .collect::<Result<_, std::num::TryFromIntError>>()
            .map_err(|error| refuse(error.to_string()))?;
        Evidence::new(Parts {
            subject: Subject {
                backend: Backend::Kani,
                kind: SubjectKind::Harness,
                id: self.harness.clone(),
            },
            outcome: claim.outcome,
            tier: claim.outcome.tier(),
            strength: claim.strength,
            bounds: vec![Bound {
                kind: BoundKind::ObjectBits,
                scope: "subject".into(),
                value: object_bits,
                source: BoundSource::Reported,
            }],
            sufficiency: claim.sufficiency,
            assumptions: Vec::new(),
            checks,
            witness: None,
            tools: vec![
                tool("kani", Some(KANI_VERSION)),
                tool("cbmc", Some(CBMC_VERSION)),
                tool("cadical", None),
            ],
            integrity: Integrity::Reconciled,
            provenance: Provenance::Fresh,
            cost: Cost::default(),
        })
        .map_err(refuse)
    }
}
