//! Opt-in scalar literal-contract counterexample queries, separate from Verus verdicts.
use serde::{Deserialize, Serialize};

/// Exact query-generation semantics version.
pub const VERSION: u32 = 1;
/// Distinct evidence family; never the native mutation policy verdict.
pub const POLICY: &str = "neutral-scalar-literal-contract-witness-v1";

/// Deterministic admitted query and its complete source/contract attribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepared {
    /// Version of the separate witness translation.
    pub version: u32,
    /// Equals [`POLICY`].
    pub policy: String,
    /// Exact original candidate, including unchanged source and guarantee hashes.
    pub candidate: crate::mutation::Candidate,
    /// Exact SMT transcript digest, independent of the solver response.
    pub query_sha256: String,
    /// Original scalar input names mapped to generated SMT symbols, sorted by name.
    pub inputs: Vec<(String, String)>,
    /// Query through the existing pinned runtime; no local fallback is implied.
    pub query: crate::runtime::Query,
}

/// A bounded inventory with all omitted and unsupported scope explicit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// Equals [`VERSION`].
    pub version: u32,
    /// Equals [`POLICY`].
    pub policy: String,
    /// Complete unchanged original source, not a hash-only assertion.
    pub baseline: crate::contracts::Submission,
    /// Original selected inventory budget, required for exact regeneration.
    pub max_mutants: usize,
    /// Admitted literal queries in deterministic mutation inventory order.
    pub prepared: Vec<Prepared>,
    /// Every unsupported original target or candidate, with its reason.
    pub unsupported: Vec<String>,
    /// Eligible candidates excluded by the requested mutation budget.
    pub omitted: usize,
}

impl Plan {
    /// Recheck original source identities, body-only replacements and query links.
    /// Source admission and query translation remain the producing Sail service's
    /// trust boundary; this method does not authenticate backend execution.
    /// # Errors
    /// Rejects content drift, unsupported versions and mismatched attribution.
    pub fn validate(&self) -> Result<(), String> {
        self.baseline.validate()?;
        if !(1..=crate::mutation::MAX_MUTANTS).contains(&self.max_mutants)
            || self.version != VERSION
            || self.policy != POLICY
            || self.prepared.len() > self.max_mutants
            || !self.unsupported.windows(2).all(|pair| pair[0] <= pair[1])
        {
            return Err("SAIL3145: unsupported or inconsistent literal witness plan".into());
        }
        for prepared in &self.prepared {
            let candidate = &prepared.candidate;
            candidate.source.validate()?;
            let mut expected = self.baseline.source.clone();
            if candidate.body_start >= candidate.body_end
                || candidate.body_end > expected.len()
                || !expected.is_char_boundary(candidate.body_start)
                || !expected.is_char_boundary(candidate.body_end)
                || !["false", "true", "0", "1"].contains(&candidate.replacement.as_str())
            {
                return Err("SAIL3145: invalid literal replacement range".into());
            }
            expected.replace_range(
                candidate.body_start..candidate.body_end,
                &format!("{{ {} }}", candidate.replacement),
            );
            let id = crate::mutation::digest(
                format!(
                    "{}:{}:{}:{}",
                    crate::mutation::POLICY,
                    self.baseline.source_sha256,
                    candidate.function,
                    candidate.replacement
                )
                .as_bytes(),
            );
            if prepared.version != VERSION
                || prepared.policy != POLICY
                || candidate.id != id
                || candidate.source.source != expected
                || candidate.source.profile != self.baseline.profile
                || candidate.source.crate_name != self.baseline.crate_name
                || !digest_valid(&candidate.guarantee_sha256)
                || prepared.query_sha256
                    != crate::mutation::digest(prepared.query.script.as_bytes())
                || prepared.query.obligation_id
                    != format!("{POLICY}:{}:{}", candidate.id, prepared.query_sha256)
                || prepared.query.requirement_ids != [candidate.requirement_id.clone()]
                || prepared.query.backend != "z3"
                || prepared.query.limits != crate::runtime::Limits::default()
            {
                return Err(
                    "SAIL3145: literal witness source, body or query identity differs".into(),
                );
            }
        }
        Ok(())
    }
}

/// Observation of the separate existential contract-violation query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Observation {
    /// SAT: some admitted scalar input satisfies the preconditions and violates
    /// the defined postcondition for this exact literal body.
    SatisfiableContractQuery,
    /// UNSAT for this bounded witness translation; no proof-tier promotion.
    NoWitness,
    /// Unknown, unavailable, incomplete, unattributed or mismatched evidence.
    Inconclusive,
}

/// Classify an existing attributed runtime response without running a solver.
/// This never changes a source proof, native mutation kill or evidence tier.
#[must_use]
pub fn observe(prepared: &Prepared, result: &crate::runtime::QueryResult) -> Observation {
    use crate::runtime::{Answer, Outcome};
    if prepared.version != VERSION
        || prepared.policy != POLICY
        || prepared.candidate.source.validate().is_err()
        || prepared.query.backend != "z3"
        || prepared.query.limits != crate::runtime::Limits::default()
        || prepared.query.requirement_ids != [prepared.candidate.requirement_id.clone()]
        || prepared.query.obligation_id
            != format!(
                "{POLICY}:{}:{}",
                prepared.candidate.id, prepared.query_sha256
            )
        || prepared.query_sha256 != crate::mutation::digest(prepared.query.script.as_bytes())
        || result.script_sha256 != prepared.query_sha256
        || result.obligation_id != prepared.query.obligation_id
        || result.requirement_ids != prepared.query.requirement_ids
        || result.backend != prepared.query.backend
        || result.limits != prepared.query.limits
        || result.outcome != Outcome::Completed
        || result.error.is_some()
        || result
            .solver
            .as_ref()
            .is_none_or(|s| s.backend != "z3" || s.version.is_empty() || !digest_valid(&s.sha256))
        || result.checks.len() != 1
        || result.stdout.len() > result.limits.output_bytes
        || result.stderr.len() > result.limits.output_bytes
        || result
            .checks
            .first()
            .is_some_and(|check| !check.products.is_empty())
    {
        return Observation::Inconclusive;
    }
    match (result.checks[0].answer, result.stdout.trim_ascii()) {
        (Answer::Sat, b"sat") => Observation::SatisfiableContractQuery,
        (Answer::Unsat, b"unsat") => Observation::NoWitness,
        _ => Observation::Inconclusive,
    }
}

/// Derive the exact second transcript requesting concrete scalar assignments.
/// Execute this only after a SAT observation; unavailable models stay explicit.
#[must_use]
pub fn model_query(prepared: &Prepared) -> crate::runtime::Query {
    let mut query = prepared.query.clone();
    query.obligation_id.push_str(":model");
    let symbols = prepared
        .inputs
        .iter()
        .map(|(_, symbol)| symbol.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    query.script.push_str("(get-value (");
    query.script.push_str(&symbols);
    query.script.push_str(" true))\n");
    query
}

/// A concrete original-source counterexample, independently evaluated by Sail.
/// This is neither a Verus SAT verdict nor a native mutation kill.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validated {
    /// Original versioned query and exact body/contract binding.
    pub prepared: Prepared,
    /// Complete solver response containing the scalar assignment.
    pub model_result: crate::runtime::QueryResult,
    /// Original source input names and exact scalar values, sorted by name.
    pub values: Vec<(String, crate::output::ItfValue)>,
}

fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
