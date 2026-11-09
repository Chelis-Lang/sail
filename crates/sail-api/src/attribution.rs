//! Submitter-supplied requirement attribution for ordinary Verus project jobs.
//!
//! An ordinary job proves the submitted Verus source against its own
//! specifications. A submitter may name requirement identities for exact
//! functions when submitting the job; this view reports, per requirement, what
//! the retained job evidence establishes for each attributed function. The
//! attribution itself is a submitter claim: Sail never checks that a function's
//! specification states the requirement, and this view is never neutral-contract
//! assurance (see [`crate::assurance`]). Reading a view never runs a verifier.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::assurance::{EvidenceTier, MutationStatus, ProofState};
use crate::jobs::{Currentness, Lifecycle, Mode, ResultDocument};
use crate::output::{EstablishmentName, SolverInfo};
use crate::project::ProjectResult;

/// Attributed assurance view contract version.
pub const VERSION: u32 = 1;
/// Most attributions one job may carry.
pub const MAX_ATTRIBUTIONS: usize = 4096;
/// Longest requirement identity or function name, in bytes.
pub const MAX_NAME: usize = 1024;

/// One submitter claim that an exact function serves a requirement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementAttribution {
    /// Stable caller-owned requirement identity.
    pub requirement_id: String,
    /// Exact crate-qualified function name as the verifier reports it.
    pub function: String,
}

/// Validate and canonically sort attributions before submission or storage.
///
/// # Errors
/// Refuses empty sets, empty, oversized or control-character names and
/// duplicate pairs.
pub fn canonical(
    mut attributions: Vec<RequirementAttribution>,
) -> Result<Vec<RequirementAttribution>, String> {
    let invalid = |why: &str| format!("SAIL3160: invalid requirement attribution: {why}");
    if attributions.is_empty() || attributions.len() > MAX_ATTRIBUTIONS {
        return Err(invalid("between one and 4096 attributions are required"));
    }
    for item in &attributions {
        for name in [&item.requirement_id, &item.function] {
            if name.is_empty() || name.len() > MAX_NAME || name.chars().any(char::is_control) {
                return Err(invalid("names must be non-empty printable text"));
            }
        }
    }
    attributions.sort();
    if attributions.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("duplicate requirement and function pair"));
    }
    Ok(attributions)
}

/// Whether a job mode can carry attribution: only stock project and
/// standalone-file verification, never an engine or repair batch.
#[must_use]
pub const fn mode_supported(mode: &Mode) -> bool {
    matches!(mode, Mode::Stock | Mode::Standalone)
}

/// The relationship an attributed row checks; distinct from neutral contract-to-code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relationship {
    /// The exact function satisfies its own Verus specification in the submitted source.
    VerusSpecificationToCode,
}

/// Who asserted the requirement link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionSource {
    /// Supplied by the submitting client and retained with the job, unchecked.
    Submitter,
}

/// Evidence applies to the job's recorded input revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// The project inputs whose revision the runner recorded.
    InputRevision,
}

/// One attributed function and what the retained job evidence establishes for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Content-derived identity over the revision, function, requirement and context.
    pub obligation_id: String,
    /// The relationship this evidence checks; never neutral contract-to-code.
    pub relationship: Relationship,
    /// The requirement link is a submitter claim.
    pub attribution: AttributionSource,
    /// Exact attributed function.
    pub function: String,
    /// Whether the verifier reported exactly one requested-target function with this name.
    pub function_reported: bool,
    /// Input revision recorded by the runner.
    pub input_revision: String,
    /// Original backend report digest.
    pub backend_sha256: String,
    /// Conservative outcome: proved only for a complete whole-job proof.
    pub outcome: ProofState,
    /// Present only for complete confirmed evidence with actual tool identity.
    pub establishment: Option<EstablishmentName>,
    /// Present only when that strength has actually been established.
    pub tier: Option<EvidenceTier>,
    /// Scope of the proof.
    pub scope: Scope,
    /// All actual distinct solver identities, sorted canonically.
    pub solvers: Vec<SolverInfo>,
    /// Explicit assumptions, trust and missing relationships, sorted.
    pub qualifiers: Vec<String>,
    /// Mutation establishment is never inferred from a passing proof.
    pub mutation: MutationStatus,
    /// Source-named inputs from the solver model of the function's first failed
    /// or unknown check, with struct fields named by position; absent when proved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterexample: Option<crate::counterexample::Decoded>,
}

/// Attributed evidence grouped by requirement identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    /// Submitter-supplied identity.
    pub requirement_id: String,
    /// Rows sorted by function.
    pub evidence: Vec<Evidence>,
}

/// Versioned read-only view over one ordinary job and its retained attributions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// Equals [`VERSION`].
    pub version: u32,
    /// Exact retained job identity.
    pub job_id: String,
    /// The job's verification arguments, verbatim; Verus arguments follow `--`.
    pub arguments: Vec<String>,
    /// Live currentness of the job's inputs, as the job service classifies it.
    pub currentness: Currentness,
    /// Requirement groups sorted by identity.
    pub requirements: Vec<Requirement>,
}

const QUALIFIER_ATTRIBUTION: &str = "requirement attribution was supplied by the submitter; Sail does not check that the function's specification states the requirement";
const QUALIFIER_NOT_NEUTRAL: &str =
    "ordinary Verus evidence: no neutral contract was admitted, lowered or checked";
const QUALIFIER_SCOPE: &str =
    "applies to the job's recorded input revision under the function's stated preconditions";
const QUALIFIER_TRUST: &str = "the submitted Verus specifications and the pinned Verus/SMT toolchain are trusted; Sail does not review assume or admit statements, external_body or assume_specification items, or other trusted code";
const QUALIFIER_CHEATING: &str = "the job did not pass Verus --no-cheating, so assume, admit, external_body and assume_specification were allowed in the verified crate; no tier is established";
const QUALIFIER_NO_CHEATING: &str = "the job passed Verus --no-cheating, which refuses assume, admit, external_body and assume_specification in the verified crate";
const QUALIFIER_UNREVIEWED: &str =
    "the job passed Verus arguments outside the reviewed set; no tier is established:";

/// Verus arguments that cannot weaken a reported proof: selection, resource
/// limits, threading, report detail and standalone crate naming. Each pairs its name with whether it
/// takes a separate value. Anything else after `--` withholds the tier.
const REVIEWED_VERUS_ARGUMENTS: [(&str, bool); 14] = [
    ("--crate-name", true),
    ("--crate-type", true),
    ("--edition", true),
    ("--expand-errors", false),
    ("--multiple-errors", true),
    ("--no-cheating", false),
    ("--num-threads", true),
    ("--rlimit", true),
    ("--time", false),
    ("--time-expanded", false),
    ("--verify-function", true),
    ("--verify-module", true),
    ("--verify-only-module", true),
    ("--verify-root", false),
];

/// How the job's forwarded Verus arguments bear on attributed trust.
#[derive(Debug, Default, PartialEq, Eq)]
struct VerusArguments {
    no_cheating: bool,
    unreviewed: Vec<String>,
}

/// Classify the Verus arguments, those after the first `--`.
fn verus_arguments(arguments: &[String]) -> VerusArguments {
    let mut result = VerusArguments::default();
    let mut forwarded = arguments.iter().skip_while(|a| a.as_str() != "--").skip(1);
    while let Some(argument) = forwarded.next() {
        let (name, inline) = argument
            .split_once('=')
            .map_or((argument.as_str(), false), |(name, _)| (name, true));
        match REVIEWED_VERUS_ARGUMENTS.iter().find(|(n, _)| *n == name) {
            Some((_, takes_value)) if !inline || *takes_value => {
                if *takes_value && !inline && forwarded.next().is_none() {
                    result.unreviewed.push(argument.clone());
                }
                result.no_cheating |= name == "--no-cheating";
            }
            _ => result.unreviewed.push(argument.clone()),
        }
    }
    result
}
const QUALIFIER_UNREPORTED: &str =
    "the verifier reported no unique requested-target function with this exact name";
const QUALIFIER_NOT_CURRENT: &str =
    "the job's inputs are not confirmed current; no tier is established";

/// Project an ordinary job result and its retained attributions.
///
/// The job service reads the sealed backend output before calling this
/// function; a caller-built document is not authentication.
///
/// # Errors
/// Refuses neutral, repair, unfinished or malformed jobs and invalid attributions.
pub fn from_project_result(
    document: &ResultDocument,
    attributions: &[RequirementAttribution],
) -> Result<View, String> {
    let invalid = || "SAIL3160: invalid or incomplete attributed job evidence".to_owned();
    let attributions = canonical(attributions.to_vec())?;
    let backend: ProjectResult =
        serde_json::from_value(document.result.clone().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
    if document.job.version != crate::jobs::VERSION
        || document.job.state != Lifecycle::Completed
        || document.job.id.is_empty()
        || document.job.error.is_some()
        || document.repair.is_some()
        || backend.kind != "sail-project-result"
        || backend.schema_version != crate::project::SCHEMA_VERSION
        || backend.engine.is_some()
        || document.job.exit_code.is_none()
        || document.job.exit_code != backend.exit_code
        || backend.input_revision.is_empty()
    {
        return Err(invalid());
    }
    let backend_sha256 =
        crate::assurance::digest(&serde_json::to_vec(&backend).map_err(|_| invalid())?);
    let (checks, _) = crate::assurance::check_counts(&backend);
    let complete_run = crate::assurance::run_complete(&backend, &checks);
    let (complete_identity, solvers) =
        crate::assurance::solver_identities(&backend).map_err(|()| invalid())?;
    let current = document.job.currentness == Currentness::Current;
    let verus = verus_arguments(&document.job.args);
    let reviewed = verus.no_cheating && verus.unreviewed.is_empty();
    let targets = crate::assurance::requested_targets(&backend);
    let reported = targets
        .iter()
        .flat_map(|t| &t.report.crates)
        .flat_map(|c| &c.functions)
        .fold(BTreeMap::<&str, usize>::new(), |mut counts, f| {
            *counts.entry(f.function.as_str()).or_default() += 1;
            counts
        });
    let mut grouped: BTreeMap<String, Vec<Evidence>> = BTreeMap::new();
    for item in &attributions {
        let unique = reported.get(item.function.as_str()) == Some(&1);
        let proved = complete_run
            && unique
            && crate::assurance::exactly_one_proved(&targets, &item.function);
        let confirmed = proved && complete_identity && current && reviewed;
        let qualifiers = qualifiers(unique, complete_identity, current, &verus);
        let identity = serde_json::to_vec(&(
            &backend.input_revision,
            &item.function,
            &item.requirement_id,
            &backend.context_sha256,
        ))
        .map_err(|_| invalid())?;
        grouped
            .entry(item.requirement_id.clone())
            .or_default()
            .push(Evidence {
                obligation_id: format!("verus:{}", crate::assurance::digest(&identity)),
                relationship: Relationship::VerusSpecificationToCode,
                attribution: AttributionSource::Submitter,
                function: item.function.clone(),
                function_reported: unique,
                input_revision: backend.input_revision.clone(),
                backend_sha256: backend_sha256.clone(),
                outcome: if proved {
                    ProofState::Proved
                } else {
                    ProofState::NotProved
                },
                establishment: confirmed.then_some(EstablishmentName::Confirmed),
                tier: confirmed.then_some(EvidenceTier::SmtVerified),
                scope: Scope::InputRevision,
                solvers: solvers.clone(),
                qualifiers,
                mutation: MutationStatus::NotEstablished,
                counterexample: (!proved && unique)
                    .then(|| decoded(&targets, &item.function))
                    .flatten(),
            });
    }
    Ok(View {
        version: VERSION,
        job_id: document.job.id.clone(),
        arguments: document.job.args.clone(),
        currentness: document.job.currentness,
        requirements: grouped
            .into_iter()
            .map(|(requirement_id, evidence)| Requirement {
                requirement_id,
                evidence,
            })
            .collect(),
    })
}

/// Model inputs of the one reported function; fields are named by position
/// because ordinary source is not retained.
fn decoded(
    targets: &[&crate::project::TargetResult],
    function: &str,
) -> Option<crate::counterexample::Decoded> {
    targets
        .iter()
        .flat_map(|t| &t.report.crates)
        .flat_map(|c| &c.functions)
        .find(|f| f.function == function)
        .and_then(|f| crate::counterexample::decode(f, &crate::counterexample::FieldNames::new()))
}

fn qualifiers(
    unique: bool,
    complete_identity: bool,
    current: bool,
    verus: &VerusArguments,
) -> Vec<String> {
    let mut qualifiers: Vec<String> = [
        QUALIFIER_ATTRIBUTION,
        QUALIFIER_NOT_NEUTRAL,
        QUALIFIER_SCOPE,
        QUALIFIER_TRUST,
    ]
    .into_iter()
    .chain(crate::assurance::SHARED_QUALIFIERS)
    .map(String::from)
    .collect();
    if !unique {
        qualifiers.push(QUALIFIER_UNREPORTED.into());
    }
    if !complete_identity {
        qualifiers.push(crate::assurance::NO_IDENTITY_QUALIFIER.into());
    }
    if !current {
        qualifiers.push(QUALIFIER_NOT_CURRENT.into());
    }
    qualifiers.push(
        if verus.no_cheating {
            QUALIFIER_NO_CHEATING
        } else {
            QUALIFIER_CHEATING
        }
        .into(),
    );
    if !verus.unreviewed.is_empty() {
        qualifiers.push(format!(
            "{QUALIFIER_UNREVIEWED} {}",
            verus.unreviewed.join(" ")
        ));
    }
    qualifiers.sort();
    qualifiers
}
