//! Check a sequence of closed TLA+ requests while retaining one typed Dive artifact.
pub mod first_claim;
pub mod loaded;
pub mod projection;
use crate::input::Frozen;
use crate::{ItfTrace, Operation, Outcome, Request, Response};
use dive_api::{safety_projection, safety_reuse as safety, temporal, temporal_reuse};
use safety::Counts;
use serde::{Deserialize, Serialize};

/// Dive source revision providing the typed edit API, checked against Cargo.lock.
pub const BACKEND_SOURCE: &str = "61c654f608093abbe8df388d2d2aa3b112bfb853";

/// A group of native checks with one wall deadline for the whole process.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    /// Edit protocol version, currently one.
    pub version: u32,
    /// Milliseconds for startup, checking and transport of the entire batch.
    pub wall_time_ms: u64,
    /// Ordered immutable native check requests. Every request supplies its own limits.
    pub checks: Vec<Request>,
}

impl Batch {
    /// Validate the submitted operation, deadline and batch size.
    ///
    /// # Errors
    /// An unknown version, unsupported operation or out-of-range limit.
    pub fn admit(&self) -> Result<(), String> {
        if self.version != 1
            || !(1..=64).contains(&self.checks.len())
            || !(1..=sail_api::runtime::MAX_WALL_TIME_MS).contains(&self.wall_time_ms)
            || self.checks.iter().any(|r| {
                r.version != crate::VERSION || !matches!(r.operation, Operation::Check { .. })
            })
        {
            return Err("expected 1..64 native checks, edit version 1 and a positive wall limit of at most five minutes".into());
        }
        Ok(())
    }
}

/// Search counts established by the basis or certified for the current decision.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchCounts {
    /// Initial states generated.
    pub initial_generated: u64,
    /// Initial and successor states generated.
    pub generated: u64,
    /// Distinct states.
    pub distinct: u64,
    /// Search depth.
    pub depth: u64,
    /// States remaining in the BFS queue.
    pub left_on_queue: u64,
}
impl From<Counts> for SearchCounts {
    fn from(c: Counts) -> Self {
        Self {
            initial_generated: c.initial_generated,
            generated: c.generated,
            distinct: c.distinct,
            depth: c.depth,
            left_on_queue: c.left_on_queue,
        }
    }
}

/// Current edit outcome and the evidence that establishes it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Checked {
    /// Every reachable state was examined.
    Holds,
    /// An independently validated, currently configured counterexample.
    Refuted {
        /// Complete typed ITF trace including the current ending.
        trace: Box<ItfTrace>,
        /// Result of an independent current native validation.
        validation: Box<Response>,
    },
}

/// Evidence retained by the process that established the transition system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Structure {
    /// Ordered safety discovery rows, complete or ending in a qualified refutation.
    SafetyRows,
    /// A complete safety check and its bound invariant inventory, with zero retained state rows.
    EstablishedInvariants,
    /// Complete labelled graph, including the stuttering relation.
    TemporalGraph,
}

/// Source of the evidence returned for a check.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "route", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Evidence {
    /// The authenticated previous ordinary temporal check established its first property.
    EstablishedFirstTemporalClaim(first_claim::Claim),
    /// The ordinary native checking path, e.g. after an unsupported capture.
    Fresh {
        /// Standard validated native response.
        response: Box<Response>,
    },
    /// A new exact capture or a qualified recheck of an earlier capture.
    Checked {
        /// Type of artifact used by this decision.
        structure: Structure,
        /// True if a previous check supplied the artifact.
        reused: bool,
        /// Native payload digest of the check which established these rows.
        basis_sha256: String,
        /// Counts of the original establishing run.
        basis_counts: SearchCounts,
        /// Search position certified for this check; rechecks generate no successors.
        decision_counts: SearchCounts,
        /// Current conclusive outcome.
        decision: Checked,
    },
}

/// One result in caller order, bound to its exact input.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Elapsed worker time in microseconds, including admission, binding and
    /// independent validation. Transport and the parent clock are separate.
    pub elapsed_us: u64,
    /// Current native payload SHA-256.
    pub request_sha256: String,
    /// Proof or native fallback.
    pub evidence: Evidence,
}

/// An in-process sequence of caller-selected model versions. This low-level API
/// has no hard wall limit. CLI callers use the authenticated, supervised worker.
#[derive(Default)]
pub struct Session {
    prior: Option<(String, Closure, Basis)>,
}

#[derive(Clone, PartialEq, Eq)]
struct Closure {
    root: String,
    config: String,
    files: Vec<String>,
}
impl From<&crate::Model> for Closure {
    fn from(m: &crate::Model) -> Self {
        Self {
            root: m.root.clone(),
            config: m.config.clone(),
            files: m.files.keys().cloned().collect(),
        }
    }
}

enum Basis {
    OrdinaryTemporal(Box<(Request, Response)>),
    Safety(safety::Artifact),
    Projection(Box<(Request, safety_projection::Complete)>),
    Temporal(Box<temporal_reuse::Artifact>),
}

// The hosted worker clears its environment. Direct library callers can enable a
// different ordinary checker policy; only ordinary checking can decide it.
fn default_policy() -> bool {
    ["DIVE_CORE_OPT", "DIVE_NATIVE_ATOM_AUDIT"]
        .iter()
        .all(|key| std::env::var_os(key).is_none())
}

impl Session {
    /// Check the current immutable model. On an admission or reuse miss, use
    /// the ordinary native checker. The prior artifact remains process-local.
    ///
    /// # Errors
    /// Non-check requests, invalid input identity, or invalid evidence.
    pub fn check(&mut self, request: &Request) -> Result<Entry, String> {
        let started = std::time::Instant::now();
        let Operation::Check {
            max_states,
            max_depth,
        } = request.operation
        else {
            return Err("edit sessions accept exhaustive checks only".into());
        };
        if request.version != crate::VERSION {
            return Err("unknown native check version".into());
        }
        let hash = crate::request_sha256(request).map_err(|e| e.to_string())?;
        let input = Frozen::create(&request.model);
        let evidence = input
            .as_ref()
            .ok()
            .and_then(|ready| self.checked(request, ready, &hash, max_states, max_depth))
            .unwrap_or_else(|| {
                let result =
                    input.and_then(|ready| crate::jobs::check(&ready, max_states, max_depth));
                Evidence::Fresh {
                    response: Box::new(crate::jobs::attribute(request, result)),
                }
            });
        let mut entry = Entry {
            elapsed_us: 0,
            request_sha256: hash,
            evidence,
        };
        entry.validate(request)?;
        if let Evidence::Fresh { response } = &entry.evidence
            && default_policy()
            && first_claim::candidate(request, response)
        {
            self.prior = Some((
                entry.request_sha256.clone(),
                Closure::from(&request.model),
                Basis::OrdinaryTemporal(Box::new((request.clone(), (**response).clone()))),
            ));
        }
        entry.elapsed_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        Ok(entry)
    }

    fn checked(
        &mut self,
        request: &Request,
        input: &Frozen,
        hash: &str,
        states: u64,
        depth: u64,
    ) -> Option<Evidence> {
        if !default_policy() {
            return None;
        }
        let analysis = dive_api::analyze_file(
            &input.root,
            &dive_api::AnalyzeOptions {
                library_paths: input.libraries.clone(),
                ..Default::default()
            },
        )
        .ok()?;
        let config = dive_api::parse_config_file(&input.config).ok()?;
        let closure = Closure::from(&request.model);
        if config.config()?.properties.is_empty() {
            if let Some(evidence) =
                self.rows(request, hash, &closure, &analysis, &config, states, depth)
            {
                return Some(evidence);
            }
            let program = safety_projection::Program::from_reports(&analysis, &config).ok()?;
            let limits = safety_projection::Limits {
                max_states: Some(states),
                max_depth: Some(depth),
            };
            if let Some((basis_hash, previous, Basis::Projection(prior))) = &self.prior
                && previous == &closure
                && projection::same_scope(&prior.0, request)
                && let Ok(counts) = prior.1.project(&program, limits)
            {
                return Some(projection_hold(basis_hash, true, counts));
            }
            let complete = program.capture(limits).ok()?;
            let current = projection_hold(hash, false, complete.original_counts());
            self.prior = Some((
                hash.to_owned(),
                closure,
                Basis::Projection(Box::new((request.clone(), complete))),
            ));
            Some(current)
        } else {
            if let Some((_, _, Basis::OrdinaryTemporal(basis))) = &self.prior
                && let Some(claim) = first_claim::qualify(&basis.0, request, &basis.1, || {
                    dive_api::temporal_projection::Program::from_reports(
                        &analysis,
                        &config,
                        safety_projection::Limits {
                            max_states: Some(states),
                            max_depth: Some(depth),
                        },
                    )
                    .ok()
                })
            {
                return Some(Evidence::EstablishedFirstTemporalClaim(claim));
            }
            let program = temporal_reuse::Program::from_reports(&analysis, &config).ok()?;
            let limits = temporal_reuse::Limits {
                max_states: Some(states),
                max_depth: Some(depth),
                ..Default::default()
            };
            if let Some((basis_hash, previous, Basis::Temporal(prior))) = &self.prior
                && previous == &closure
                && let Ok(report) = prior.recheck(&program, limits)
            {
                return temporal_evidence(request, true, basis_hash, prior, &report);
            }
            let (artifact, report) = program.capture(limits).ok()?;
            let current = temporal_evidence(request, false, hash, &artifact, &report)?;
            self.prior = Some((
                hash.to_owned(),
                closure,
                Basis::Temporal(Box::new(artifact)),
            ));
            Some(current)
        }
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "the row and projection paths share one admitted parse and binding input"
    )]
    fn rows(
        &mut self,
        request: &Request,
        hash: &str,
        closure: &Closure,
        analysis: &dive_api::AnalyzeReport,
        config: &dive_api::ConfigReport,
        states: u64,
        depth: u64,
    ) -> Option<Evidence> {
        let program = safety::Program::from_reports(analysis, config).ok()?;
        let limits = safety::Limits {
            max_states: Some(states),
            max_depth: Some(depth),
            ..Default::default()
        };
        if let Some((basis_hash, previous, Basis::Safety(prior))) = &self.prior
            && previous == closure
            && let Ok(decision) = prior.recheck(&program, limits)
        {
            return safety_evidence(
                request,
                true,
                basis_hash,
                prior.original_counts(),
                &decision,
            );
        }
        let (artifact, decision) = program.capture(limits).ok()?;
        let current = safety_evidence(request, false, hash, artifact.original_counts(), &decision)?;
        self.prior = Some((hash.to_owned(), closure.clone(), Basis::Safety(artifact)));
        Some(current)
    }
}

fn projection_hold(hash: &str, reused: bool, counts: Counts) -> Evidence {
    Evidence::Checked {
        structure: Structure::EstablishedInvariants,
        reused,
        basis_sha256: hash.to_owned(),
        basis_counts: counts.into(),
        decision_counts: counts.into(),
        decision: Checked::Holds,
    }
}

fn validate_witness(request: &Request, trace: &dive_api::trace::TraceDoc) -> Option<Checked> {
    let itf = crate::native_to_itf(&trace.to_json()).ok()?;
    let mut current = request.clone();
    current.operation = Operation::Validate { trace: itf.clone() };
    let validation = crate::execute(&current);
    crate::validate_response(&current, &validation).ok()?;
    (validation.outcome == Outcome::Valid).then(|| Checked::Refuted {
        trace: Box::new(itf),
        validation: Box::new(validation),
    })
}

fn safety_evidence(
    request: &Request,
    reused: bool,
    basis_hash: &str,
    basis: Counts,
    decision: &safety::Decision,
) -> Option<Evidence> {
    let current = match decision {
        safety::Decision::Holds(_) => Checked::Holds,
        safety::Decision::Refuted { trace, .. } => validate_witness(request, trace)?,
    };
    Some(Evidence::Checked {
        structure: Structure::SafetyRows,
        reused,
        basis_sha256: basis_hash.into(),
        basis_counts: basis.into(),
        decision_counts: decision.counts().into(),
        decision: current,
    })
}

fn temporal_evidence(
    request: &Request,
    reused: bool,
    basis_hash: &str,
    basis: &temporal_reuse::Artifact,
    report: &temporal::Report,
) -> Option<Evidence> {
    let stats = basis.original_safety().stats.as_ref()?;
    let counts = SearchCounts {
        initial_generated: stats.initial_generated,
        generated: stats.generated,
        distinct: stats.distinct,
        depth: stats.depth,
        left_on_queue: stats.left_on_queue,
    };
    let decision = match report.outcome() {
        temporal::Outcome::NativeHolds(_) => Checked::Holds,
        temporal::Outcome::NativeViolation(word) => validate_witness(request, word.document())?,
        _ => return None,
    };
    Some(Evidence::Checked {
        structure: Structure::TemporalGraph,
        reused,
        basis_sha256: basis_hash.into(),
        basis_counts: counts.clone(),
        decision_counts: counts,
        decision,
    })
}

impl Entry {
    fn validate(&self, request: &Request) -> Result<(), String> {
        if self.request_sha256 != crate::request_sha256(request).map_err(|e| e.to_string())? {
            return Err("edit response belongs to a different input".into());
        }
        match &self.evidence {
            // The in-process constructor has compared the actual programs. Transport
            // validation below must also find the establishing ordinary response.
            Evidence::EstablishedFirstTemporalClaim(_) => Ok(()),
            Evidence::Fresh { response } => crate::validate_response(request, response),
            Evidence::Checked {
                decision: Checked::Holds,
                decision_counts,
                ..
            } => {
                if decision_counts.left_on_queue == 0 {
                    Ok(())
                } else {
                    Err("holding edit has an incomplete queue".into())
                }
            }
            Evidence::Checked {
                structure,
                decision: Checked::Refuted { trace, validation },
                ..
            } => {
                let inventory = crate::requested_checks(&request.model)
                    .ok_or("missing current property inventory")?;
                let kind = trace.ending.get("kind").and_then(serde_json::Value::as_str);
                let named = match structure {
                    Structure::EstablishedInvariants => false,
                    Structure::SafetyRows => {
                        kind == Some("invariant")
                            && trace.ending["name"]
                                .as_str()
                                .is_some_and(|n| inventory.invariants.iter().any(|p| p == n))
                    }
                    Structure::TemporalGraph => {
                        matches!(kind, Some("lasso" | "stuttering"))
                            && !trace.properties.is_empty()
                            && trace
                                .properties
                                .iter()
                                .all(|name| inventory.properties.contains(name))
                    }
                };
                if !named {
                    return Err("refutation does not name the configured check".into());
                }
                let mut current = request.clone();
                current.operation = Operation::Validate {
                    trace: (**trace).clone(),
                };
                crate::validate_response(&current, validation)?;
                if validation.outcome == Outcome::Valid
                    && validation.itf_traces.as_slice() == std::slice::from_ref(trace.as_ref())
                {
                    Ok(())
                } else {
                    Err("edited witness failed current validation".into())
                }
            }
        }
    }
}

/// A complete worker response with no partially delivered decisions.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchResult {
    /// Edit protocol version.
    pub version: u32,
    /// One entry for each input, in the original order.
    pub entries: Vec<Entry>,
}

struct Established<'a> {
    request: &'a Request,
    hash: &'a str,
    structure: Structure,
    counts: &'a SearchCounts,
    complete: bool,
}

impl BatchResult {
    pub(crate) fn validate(&self, batch: &Batch) -> Result<(), String> {
        if self.version != 1 || self.entries.len() != batch.checks.len() {
            return Err("worker returned a different number of edit results or version".into());
        }
        let mut prior: Option<Established> = None;
        let mut temporal_basis = None;
        for (request, row) in batch.checks.iter().zip(&self.entries) {
            row.validate(request)?;
            match &row.evidence {
                Evidence::Fresh { response } if first_claim::candidate(request, response) => {
                    temporal_basis = Some((request, response.as_ref()));
                    prior = None;
                }
                Evidence::EstablishedFirstTemporalClaim(claim) => {
                    if !temporal_basis
                        .is_some_and(|(old, response)| claim.matches(old, request, response))
                    {
                        return Err(
                            "first temporal claim has no matching ordinary completion".into()
                        );
                    }
                }
                Evidence::Checked { reused: false, .. } => temporal_basis = None,
                _ => {}
            }
            let Evidence::Checked {
                structure,
                reused,
                basis_sha256,
                basis_counts,
                decision_counts,
                decision,
            } = &row.evidence
            else {
                continue;
            };
            let inventory = crate::requested_checks(&request.model)
                .ok_or("missing configured property inventory")?;
            if (*structure == Structure::TemporalGraph) == inventory.properties.is_empty() {
                return Err("retained artifact has the wrong property role".into());
            }
            let holds = matches!(decision, Checked::Holds);
            if (holds || *structure == Structure::TemporalGraph) && basis_counts != decision_counts
            {
                return Err("complete-graph decision has different exploration counts".into());
            }
            if decision_counts.distinct > basis_counts.distinct
                || decision_counts.generated > basis_counts.generated
                || decision_counts.depth > basis_counts.depth
                || decision_counts.initial_generated > basis_counts.initial_generated
            {
                return Err("decision extends beyond the retained search".into());
            }
            if *reused {
                let basis = prior
                    .as_ref()
                    .ok_or("worker cited an unestablished edit artifact")?;
                if basis.hash != basis_sha256
                    || basis.structure != *structure
                    || basis.counts != basis_counts
                    || request.operation != basis.request.operation
                    || (holds && !basis.complete)
                    || (*structure == Structure::EstablishedInvariants
                        && projection::subset_inventory(basis.request, request).is_none())
                    || Closure::from(&request.model) != Closure::from(&basis.request.model)
                {
                    return Err("worker cited an ineligible edit artifact".into());
                }
            } else if basis_sha256 == &row.request_sha256 && basis_counts == decision_counts {
                prior = Some(Established {
                    request,
                    hash: basis_sha256,
                    structure: *structure,
                    counts: basis_counts,
                    complete: *structure == Structure::TemporalGraph || holds,
                });
            } else {
                return Err("worker capture differs from the establishing request".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refuted_safety_prefix_cannot_be_transported_as_a_complete_proof() -> Result<(), String> {
        let request = Request {
            version: crate::VERSION,
            model: crate::Model {
                root: "M.tla".into(), config: "M.cfg".into(),
                files: [
                    ("M.tla".into(), "---- MODULE M ----\nVARIABLE x\nInit == x = 0\nNext == x'=x\nInv == x /= 0\n====\n".into()),
                    ("M.cfg".into(), "INIT Init\nNEXT Next\nINVARIANT Inv\n".into()),
                ].into(),
            },
            operation: Operation::Check { max_states: 8, max_depth: 8 },
        };
        let mut session = Session::default();
        let mut first = session.check(&request)?;
        let Evidence::Checked {
            decision: Checked::Refuted { .. },
            basis_counts,
            decision_counts,
            ..
        } = &mut first.evidence
        else {
            return Err("expected a checked initial-state refutation".into());
        };
        // A zero queue by itself can never certify success for a failing search.
        basis_counts.left_on_queue = 0;
        decision_counts.left_on_queue = 0;
        let forged = Entry {
            request_sha256: first.request_sha256.clone(),
            elapsed_us: 0,
            evidence: Evidence::Checked {
                structure: Structure::SafetyRows,
                reused: true,
                basis_sha256: first.request_sha256.clone(),
                basis_counts: basis_counts.clone(),
                decision_counts: basis_counts.clone(),
                decision: Checked::Holds,
            },
        };
        let batch = Batch {
            version: 1,
            wall_time_ms: 100,
            checks: vec![request.clone(), request],
        };
        let response = BatchResult {
            version: 1,
            entries: vec![first, forged],
        };
        assert_eq!(
            response.validate(&batch),
            Err("worker cited an ineligible edit artifact".into())
        );
        Ok(())
    }
}
