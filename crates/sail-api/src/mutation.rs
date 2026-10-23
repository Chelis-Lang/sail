//! Bounded neutral-source mutation campaigns; proof outcomes are never upgraded.
use serde::{Deserialize, Serialize};

/// Campaign protocol version.
pub const VERSION: u32 = 1;
/// Versioned deterministic safe scalar-body replacement policy.
pub const POLICY: &str = "neutral-free-scalar-literals-v1";
/// Maximum actual mutant source jobs in one campaign.
pub const MAX_MUTANTS: usize = 8;
/// Maximum aggregate campaign deadline, including queued time.
pub const MAX_WALL_TIME_MS: u64 = 2_700_000;

/// A campaign over one already authenticated, completely proved source job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// Original neutral source job; callers never supply arbitrary results.
    pub baseline_job_id: String,
    /// Maximum actual jobs; omitted candidates remain explicit and prevent passing.
    pub max_mutants: usize,
    /// Aggregate deadline; each source job also retains its own fixed deadline.
    pub wall_time_ms: u64,
}

/// One body-only candidate, with unchanged contracts and surrounding source bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Content-derived candidate identity.
    pub id: String,
    /// Exact original qualified function identity.
    pub function: String,
    /// Requirement carried by the unchanged guarantee.
    pub requirement_id: String,
    /// Unchanged guarantee syntax identity.
    pub guarantee_sha256: String,
    /// Literal body replacement label.
    pub replacement: String,
    /// Original executable body byte range start, inclusive.
    pub body_start: usize,
    /// Original executable body byte range end, exclusive.
    pub body_end: usize,
    /// Exact actual mutant source submitted to the original verifier boundary.
    pub source: crate::contracts::Submission,
}

/// Entire deterministic admitted policy inventory, including coverage limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// Equals VERSION.
    pub version: u32,
    /// Equals POLICY.
    pub policy: String,
    /// Original exact source identity.
    pub baseline_source_sha256: String,
    /// Selected candidates, sorted by function and literal.
    pub candidates: Vec<Candidate>,
    /// Every contracted target outside the declared scalar policy.
    pub unsupported: Vec<String>,
    /// Eligible distinct candidates excluded by the requested job budget.
    pub omitted: usize,
}

/// Observation of actual conclusive verifier evidence, never generic `NotProved`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Actual fully attributed SAT rejection under the unchanged contract.
    Killed,
    /// Entire authenticated mutant source was proved.
    Survived,
    /// Unknown, incomplete, unsupported, unavailable or unexecuted evidence.
    Inconclusive,
}

/// Whole-campaign lifecycle, independent of property assurance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// One or more actual source jobs remain active.
    Running,
    /// Every submitted source job reached a terminal outcome.
    Completed,
    /// Explicit cancellation; no passing gate.
    Cancelled,
    /// Aggregate deadline expired; no passing gate.
    TimedOut,
    /// Interrupted service or missing evidence; no passing gate.
    Interrupted,
}

/// Exact original supervised worker streams and submitted source.
/// This is an operational receipt, not independently certified backend execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginalEvidence {
    /// Exact actual submitted source and its identity.
    pub source: crate::contracts::Submission,
    /// Verbatim original UTF-8 worker stdout, decoded anew on every validation.
    pub worker_stdout: String,
    /// Verbatim original UTF-8 worker stderr, never truncated for export.
    pub worker_stderr: String,
}

/// One authenticated mutant observation and its durable original evidence link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// Original deterministic source candidate.
    pub candidate: Candidate,
    /// Actual source job identity.
    pub job_id: String,
    /// Immutable sealed source/worker baseline, once terminal evidence exists.
    pub evidence_name: Option<String>,
    /// Original managed report identity, present only after authentication.
    pub backend_sha256: Option<String>,
    /// Complete original retained source and worker streams, when available.
    pub original: Option<OriginalEvidence>,
    /// Separate mutation classification.
    pub outcome: Outcome,
    /// Explicit scope or missing evidence explanation.
    pub reason: String,
}

/// Versioned durable view; reading it reauthenticates all linked original bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// Equals VERSION.
    pub version: u32,
    /// Content-bound campaign identity.
    pub id: String,
    /// Operational campaign start used only to bind the original durable record.
    pub started_at_ms: u64,
    /// Complete original baseline source and worker streams.
    pub baseline_original: OriginalEvidence,
    /// Exact original caller request and budgets.
    pub submission: Submission,
    /// Exact original baseline source, authenticated from its retained seal.
    pub baseline_source: crate::contracts::Submission,
    /// Immutable original baseline evidence name.
    pub baseline_evidence_name: String,
    /// Original baseline backend report identity.
    pub baseline_backend_sha256: String,
    /// Complete policy inventory.
    pub plan: Plan,
    /// Operational lifecycle, never a proof outcome.
    pub state: State,
    /// Actual observations in deterministic plan order.
    pub observations: Vec<Observation>,
    /// True only when every candidate was executed and killed, without omitted or unsupported scope.
    pub passed: bool,
    /// Translation trust, bounded scope and interpretation of kills.
    pub qualifiers: Vec<String>,
}

/// Canonical declared interpretation and remaining trust for every version-one receipt.
#[must_use]
pub fn qualifiers() -> Vec<String> {
    vec!["bounded free scalar literal-body mutation policy; unsupported and omitted targets remain explicit".into(),"killed means authenticated conclusive SAT verifier rejection, not a concrete source counterexample".into(),"neutral translation, source admission and pinned verifier/solver remain trusted; proof verdict and tier are unchanged".into()]
}

/// SHA-256 content identity using the shared exact-byte hash.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    sail_key::QueryKey(sail_key::sha256(bytes)).to_string()
}

/// Classify already authenticated original worker evidence. Caller-created
/// documents do not authenticate execution; the daemon validates sealed bytes.
/// A kill means conclusive verifier rejection, not a concrete source counterexample.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "One conservative decision keeps identity admission and conclusive rejection conditions together."
)]
pub fn classify(
    baseline: &crate::contracts::WorkerResult,
    mutant: &crate::contracts::WorkerResult,
    candidate: &Candidate,
) -> (Outcome, String) {
    use crate::contracts::ProofState;
    use crate::output::{
        AnswerName, AttributionStatus, EstablishmentName, OutcomeName, VerdictName,
    };
    let inconclusive = |message: &str| (Outcome::Inconclusive, message.to_owned());
    let identities = |result: &crate::contracts::WorkerResult| -> Option<Vec<String>> {
        let mut values = Vec::new();
        for target in &result.backend.targets {
            let solver = target.report.solver.as_ref()?;
            if solver.name.is_empty()
                || solver.version.as_ref().is_none_or(String::is_empty)
                || solver.sha256.as_ref().is_none_or(|s| {
                    s.len() != 64
                        || !s
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            {
                return None;
            }
            values.push(serde_json::to_string(solver).ok()?);
        }
        values.sort();
        values.dedup();
        (!values.is_empty()).then_some(values)
    };
    let contract_keys = |result: &crate::contracts::WorkerResult| {
        result
            .contracts
            .manifest
            .bindings
            .iter()
            .map(|b| {
                (
                    b.function.clone(),
                    b.requirement_id.clone(),
                    b.guarantee_sha256.clone(),
                    b.lowered_function.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    if baseline.version != crate::contracts::VERSION
        || mutant.version != crate::contracts::VERSION
        || baseline.contracts.proof.state != ProofState::Proved
        || mutant.contracts.manifest.original_sha256 != candidate.source.source_sha256
        || contract_keys(baseline) != contract_keys(mutant)
        || baseline.contracts.translation_trust.source_policy_sha256
            != mutant.contracts.translation_trust.source_policy_sha256
        || identities(baseline).is_none()
        || identities(baseline) != identities(mutant)
        || baseline.backend.context_sha256 != mutant.backend.context_sha256
        || baseline.backend.context_sha256.len() != 64
        || baseline.contracts.proof
            != crate::assurance::observed_contract_proof(
                &baseline.contracts.manifest,
                &baseline.backend,
            )
        || mutant.contracts.proof
            != crate::assurance::observed_contract_proof(
                &mutant.contracts.manifest,
                &mutant.backend,
            )
    {
        return inconclusive(
            "baseline, source, contract, tool identity or original proof projection differs",
        );
    }
    let Some(binding) = mutant.contracts.manifest.bindings.iter().find(|b| {
        b.function == candidate.function
            && b.requirement_id == candidate.requirement_id
            && b.guarantee_sha256 == candidate.guarantee_sha256
    }) else {
        return inconclusive("target guarantee is not exactly bound");
    };
    if mutant.contracts.proof.state == ProofState::Proved {
        return (
            Outcome::Survived,
            "complete original mutant proof under unchanged contract".into(),
        );
    }
    let backend = &mutant.backend;
    let counts = &mutant.contracts.proof.checks;
    if backend.currentness != crate::project::Currentness::Current
        || backend.error.is_some()
        || backend.no_reuse.is_some()
        || backend.exit_code != Some(1)
        || counts.unknown != 0
        || counts.missing_answer != 0
        || counts.missing_source != 0
        || !backend.targets.iter().all(|t| {
            t.complete
                && t.report.attribution.status == AttributionStatus::Complete
                && t.report.attribution.unmatched_sessions == 0
                && t.report.error.is_none()
                && t.report.invariant_violations().is_empty()
                && t.report.crates.iter().flat_map(|c| &c.functions).all(|f| {
                    !f.unstable
                        && f.provenance.checks_unrecorded == 0
                        && f.obligations
                            .as_ref()
                            .is_none_or(|os| os.iter().all(|o| !o.unstable))
                })
                && t.report
                    .crates
                    .iter()
                    .all(|c| c.verus.encountered_vir_error == Some(false))
        })
    {
        return inconclusive(
            "unknown, missing, frontend, currentness or attribution evidence prevents a kill",
        );
    }
    if mutant
        .contracts
        .manifest
        .bindings
        .iter()
        .filter(|b| b.function != candidate.function)
        .any(|b| {
            !mutant
                .contracts
                .proof
                .proved_functions
                .contains(&b.lowered_function)
        })
    {
        return inconclusive("an unchanged non-target function lacks its complete proof");
    }
    let functions = backend
        .targets
        .iter()
        .filter(|t| t.requested)
        .flat_map(|t| &t.report.crates)
        .flat_map(|c| &c.functions)
        .filter(|f| f.function == binding.lowered_function)
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        return inconclusive("target function attribution is absent or ambiguous");
    };
    if function.verdict != VerdictName::Failed
        || function.establishment != EstablishmentName::Confirmed
        || function.unstable
        || function.verus_success != Some(false)
        || function.provenance.checks_unrecorded != 0
    {
        return inconclusive("target has no confirmed stable verifier failure");
    }
    let Some(obligations) = &function.obligations else {
        return inconclusive("target obligations are missing");
    };
    let primary = obligations
        .iter()
        .filter(|o| !o.advisory)
        .collect::<Vec<_>>();
    if primary.is_empty()
        || primary.iter().any(|o| {
            o.unstable
                || o.checks.is_empty()
                || o.checks.iter().any(|c| {
                    c.source.is_none()
                        || !matches!(c.answer, Some(AnswerName::Sat | AnswerName::Unsat))
                })
        })
    {
        return inconclusive("target checks are unknown, missing or unstable");
    }
    if primary.iter().any(|o| {
        o.outcome == OutcomeName::Failed
            && o.checks.iter().any(|c| c.answer == Some(AnswerName::Sat))
    }) {
        return (Outcome::Killed, "actual fully attributed SAT verifier rejection under the unchanged admitted contract; not a concrete source counterexample".into());
    }
    inconclusive("NotProved without an attributed conclusive SAT rejection is not killed")
}

/// Decode and verify self-consistency of an exact retained original worker stream.
/// Native admission and actual execution remain the producing Sail service's trust boundary.
/// # Errors
/// Refuses invalid source identity, stream limits, versions or proof projections.
pub fn decode_original(
    evidence: &OriginalEvidence,
) -> Result<crate::contracts::WorkerResult, String> {
    evidence.source.validate()?;
    if evidence.worker_stdout.len() > crate::contracts::MAX_STDOUT_BYTES
        || evidence.worker_stderr.len() > crate::contracts::MAX_STDERR_BYTES
    {
        return Err("SAIL3144: original mutation stream exceeds retained bound".into());
    }
    let result: crate::contracts::WorkerResult = serde_json::from_str(&evidence.worker_stdout)
        .map_err(|e| format!("SAIL3144: invalid original worker stream: {e}"))?;
    if result.backend.kind != "sail-project-result"
        || result.backend.schema_version != crate::project::SCHEMA_VERSION
        || result.backend.error.is_some()
        || result.backend.no_reuse.is_some()
        || result.version != crate::contracts::VERSION
        || result.contracts.version != crate::contracts::VERSION
        || result.contracts.manifest.original_sha256 != evidence.source.source_sha256
        || result.contracts.manifest.crate_name != evidence.source.crate_name
        || result.contracts.proof
            != crate::assurance::observed_contract_proof(
                &result.contracts.manifest,
                &result.backend,
            )
    {
        return Err("SAIL3144: original mutation worker source or proof projection differs".into());
    }
    Ok(result)
}
fn encoded_digest(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|b| digest(&b))
        .map_err(|e| format!("SAIL3144: mutation encoding: {e}"))
}
fn invalid() -> String {
    "SAIL3144: retained mutation evidence or derived view differs".into()
}

/// Recompute retained receipt relationships, source body edits, outcomes and passing gate.
/// This pure check starts no service and executes no verifier. It does not certify
/// the producer or repeat its native source admission and complete policy inventory.
/// Consumers retain the exact authenticated service receipt and its artifact hash.
/// # Errors
/// Refuses changed original streams, identities, derived outcomes or incomplete links.
#[allow(
    clippy::too_many_lines,
    reason = "One bounded receipt validation keeps the original record identity, every member, and aggregate gate together."
)]
pub fn validate_view(view: &View) -> Result<(), String> {
    #[derive(Serialize)]
    struct Record<'a> {
        version: u32,
        submission: &'a Submission,
        plan: &'a Plan,
        jobs: Vec<&'a str>,
        started_at_ms: u64,
    }
    let mut export_bytes = serde_json::to_vec(&view.baseline_original)
        .map_err(|_| invalid())?
        .len();
    for original in view.observations.iter().filter_map(|o| o.original.as_ref()) {
        export_bytes = export_bytes
            .checked_add(serde_json::to_vec(original).map_err(|_| invalid())?.len())
            .ok_or_else(invalid)?;
        if export_bytes > crate::jobs::MAX_FRAME / 2 {
            return Err(invalid());
        }
    }
    if export_bytes > crate::jobs::MAX_FRAME / 2
        || view.qualifiers != qualifiers()
        || view.version != VERSION
        || view.plan.version != VERSION
        || view.plan.policy != POLICY
        || !(1..=MAX_MUTANTS).contains(&view.submission.max_mutants)
        || view.submission.wall_time_ms == 0
        || view.submission.wall_time_ms > MAX_WALL_TIME_MS
        || view.plan.candidates.len() > view.submission.max_mutants
        || view.plan.candidates.len() != view.observations.len()
        || view.baseline_source != view.baseline_original.source
        || view.plan.baseline_source_sha256 != view.baseline_source.source_sha256
        || !view.plan.unsupported.windows(2).all(|w| w[0] < w[1])
    {
        return Err(invalid());
    }
    let record = Record {
        version: VERSION,
        submission: &view.submission,
        plan: &view.plan,
        jobs: view
            .observations
            .iter()
            .map(|o| o.job_id.as_str())
            .collect(),
        started_at_ms: view.started_at_ms,
    };
    if encoded_digest(&record)? != view.id
        || view.baseline_evidence_name != format!("m-{}-b", view.id)
    {
        return Err(invalid());
    }
    let baseline = decode_original(&view.baseline_original)?;
    if encoded_digest(&baseline.backend)? != view.baseline_backend_sha256 {
        return Err(invalid());
    }
    let Some(binding) = baseline.contracts.manifest.bindings.first() else {
        return Err(invalid());
    };
    let baseline_candidate = Candidate {
        id: String::new(),
        function: binding.function.clone(),
        requirement_id: binding.requirement_id.clone(),
        guarantee_sha256: binding.guarantee_sha256.clone(),
        replacement: String::new(),
        body_start: 0,
        body_end: 0,
        source: view.baseline_source.clone(),
    };
    if classify(&baseline, &baseline, &baseline_candidate).0 != Outcome::Survived {
        return Err(invalid());
    }
    let mut identities = std::collections::BTreeSet::new();
    for (index, (candidate, observation)) in view
        .plan
        .candidates
        .iter()
        .zip(&view.observations)
        .enumerate()
    {
        if candidate != &observation.candidate
            || !identities.insert(&candidate.id)
            || candidate.id
                != digest(
                    format!(
                        "{POLICY}:{}:{}:{}",
                        view.baseline_source.source_sha256,
                        candidate.function,
                        candidate.replacement
                    )
                    .as_bytes(),
                )
            || !matches!(candidate.replacement.as_str(), "false" | "true" | "0" | "1")
            || candidate.body_start >= candidate.body_end
            || !view
                .baseline_source
                .source
                .is_char_boundary(candidate.body_start)
            || !view
                .baseline_source
                .source
                .is_char_boundary(candidate.body_end)
            || candidate.body_end > view.baseline_source.source.len()
            || !baseline.contracts.manifest.bindings.iter().any(|b| {
                b.function == candidate.function
                    && b.requirement_id == candidate.requirement_id
                    && b.guarantee_sha256 == candidate.guarantee_sha256
            })
        {
            return Err(invalid());
        }
        let mut expected = view.baseline_source.source.clone();
        expected.replace_range(
            candidate.body_start..candidate.body_end,
            &format!("{{ {} }}", candidate.replacement),
        );
        if expected != candidate.source.source
            || candidate.source.crate_name != view.baseline_source.crate_name
            || expected == view.baseline_source.source
        {
            return Err(invalid());
        }
        candidate.source.validate()?;
        if let Some(original) = &observation.original {
            if original.source != candidate.source
                || observation.evidence_name != Some(format!("m-{}-{index}", view.id))
            {
                return Err(invalid());
            }
            let mutant = decode_original(original)?;
            if observation.backend_sha256 != Some(encoded_digest(&mutant.backend)?)
                || classify(&baseline, &mutant, candidate)
                    != (observation.outcome, observation.reason.clone())
            {
                return Err(invalid());
            }
        } else if observation.evidence_name.is_some()
            || observation.backend_sha256.is_some()
            || observation.outcome != Outcome::Inconclusive
            || observation.reason != "actual source job has no retained conclusive evidence"
        {
            return Err(invalid());
        }
    }
    let passed = view.state == State::Completed
        && !view.observations.is_empty()
        && view.plan.unsupported.is_empty()
        && view.plan.omitted == 0
        && view
            .observations
            .iter()
            .all(|o| o.outcome == Outcome::Killed);
    if passed != view.passed {
        return Err(invalid());
    }
    Ok(())
}

/// Attach a fully retained, validated campaign to its exact original assurance rows.
/// No proof outcome, relationship, currentness or evidence tier changes.
/// # Errors
/// Refuses changed campaign evidence or a different baseline job/source/guarantee.
pub fn attach_assurance(
    campaign: &View,
    assurance: &mut crate::assurance::View,
) -> Result<(), String> {
    validate_view(campaign)?;
    if assurance.version != crate::assurance::VERSION
        || assurance.job_id != campaign.submission.baseline_job_id
    {
        return Err(invalid());
    }
    for requirement in &mut assurance.requirements {
        for evidence in &mut requirement.evidence {
            if evidence.source_sha256 != campaign.plan.baseline_source_sha256 {
                return Err(invalid());
            }
            let observations = campaign
                .observations
                .iter()
                .filter(|o| {
                    o.candidate.function == evidence.function
                        && o.candidate.requirement_id == requirement.requirement_id
                        && o.candidate.guarantee_sha256 == evidence.guarantee_sha256
                })
                .collect::<Vec<_>>();
            if !observations.is_empty() {
                evidence.mutation = crate::assurance::MutationStatus::Bounded {
                    campaign_id: campaign.id.clone(),
                    policy: campaign.plan.policy.clone(),
                    passed: campaign.passed,
                    killed: observations
                        .iter()
                        .filter(|o| o.outcome == Outcome::Killed)
                        .count(),
                    survived: observations
                        .iter()
                        .filter(|o| o.outcome == Outcome::Survived)
                        .count(),
                    inconclusive: observations
                        .iter()
                        .filter(|o| o.outcome == Outcome::Inconclusive)
                        .count(),
                };
            }
        }
    }
    Ok(())
}
