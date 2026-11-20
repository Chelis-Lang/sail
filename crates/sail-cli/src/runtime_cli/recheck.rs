//! Reuse authenticated complete proofs or validate a qualified prior failure.
use sail_api::runtime::{Input, Output, Request, Response, VERSION};
use sail_api::temporal::{ItfTrace, Job, Operation, Outcome};
use sail_runtime::{Cancellation, Runtime};
use serde_json::json;
use std::io::Write;
use std::time::{Duration, Instant};

fn check(request: &Request) -> Result<&Job, String> {
    if request.version == VERSION
        && let Input::Temporal { job } = &request.input
        && matches!(job.request.operation, Operation::Check { .. })
    {
        return Ok(job);
    }
    Err("error[SAIL6101]: recheck requires two native temporal check requests".into())
}

fn invariant_trace(response: &sail_api::temporal::Response) -> Option<&ItfTrace> {
    let [trace] = response.itf_traces.as_slice() else {
        return None;
    };
    let name = trace.ending.get("name")?.as_str()?;
    (response.outcome == Outcome::Refuted
        && response.native_report["verdict"] == "invariant-violation"
        && trace.ending["kind"] == "invariant"
        && response
            .requested_checks
            .as_ref()?
            .invariants
            .iter()
            .any(|n| n == name))
    .then_some(trace)
}

pub(super) fn run(
    runtime: &Runtime,
    previous: &Request,
    current: &Request,
    cancellation: &Cancellation,
    out: &mut dyn Write,
) -> Result<u8, String> {
    let old = check(previous)?;
    let new = check(current)?;
    let start = Instant::now();
    if old.request != new.request && old.wall_time_ms == new.wall_time_ms {
        // A current receipt already contains the validated result for these
        // exact bytes. A hint from another check must never displace it.
        if let Some(result) = runtime
            .lookup_temporal(new, cancellation)
            .map_err(|e| e.to_string())?
        {
            return write_current(
                out,
                &Response {
                    version: VERSION,
                    output: Output::Temporal {
                        result: Box::new(result),
                    },
                },
            );
        }
        if let Ok(Some(basis)) = runtime.lookup_temporal(old, cancellation) {
            if let Some(decision) = projected(old, new, &basis, cancellation, start) {
                super::write_json(out, &decision)?;
                return Ok(0);
            }
            if let Some(trace) = basis.response.as_ref().and_then(invariant_trace)
                && sail_dive::equivalent_check_config(&old.request, &new.request)
                && let Some(validated) = validate(runtime, new, &basis, trace, cancellation, start)
            {
                let digest =
                    sail_api::temporal::request_sha256(&new.request).map_err(|e| e.to_string())?;
                super::write_json(
                    out,
                    &json!({
                        "version":1, "route":"equivalent-cfg-rechecked-refutation", "outcome":"refuted",
                        "obligation_id":new.obligation_id, "requirement_ids":new.requirement_ids,
                        "request_sha256":digest, "previous_request_sha256":basis.request_sha256,
                        "previous_receipt_key":basis.cache_key, "validation":validated
                    }),
                )?;
                return Ok(0);
            }
        }
    }
    let full = runtime
        .execute(current, cancellation)
        .map_err(|e| e.to_string())?;
    write_current(out, &full)
}

#[derive(serde::Serialize)]
#[serde(tag = "route", rename_all = "kebab-case")]
enum ProjectionProof {
    UnchangedLoadedProgram {
        loaded_modules: Vec<String>,
    },
    #[serde(rename = "established-invariant-subset")]
    Invariants {
        invariants: Vec<String>,
    },
    #[serde(rename = "established-first-temporal-claim")]
    FirstTemporalClaim {
        property: String,
        basis_counts: sail_dive::edits::SearchCounts,
    },
}

#[derive(serde::Serialize)]
struct Projected<'a> {
    version: u32,
    outcome: Outcome,
    obligation_id: &'a str,
    requirement_ids: &'a [String],
    request_sha256: String,
    previous_request_sha256: String,
    previous_receipt_key: &'a str,
    worker_sha256: String,
    backend_source: String,
    #[serde(flatten)]
    evidence: ProjectionProof,
}

fn projected<'a>(
    old: &Job,
    current: &'a Job,
    basis: &'a sail_api::temporal::Result,
    cancel: &Cancellation,
    start: Instant,
) -> Option<Projected<'a>> {
    use sail_api::temporal::Completion;
    use sail_runtime::temporal::Worker;
    let evidence = basis.response.as_ref()?;
    if evidence.completion == Completion::None
        || evidence.outcome != Outcome::Holds
        || evidence.request_sha256 != basis.request_sha256
        || !basis.cache_hit
        || evidence.native_report["statistics"]["left_on_queue"] != 0
        || old.request.operation != current.request.operation
    {
        return None;
    }
    let previous_receipt_key = basis.cache_key.as_deref()?;
    let budget = Duration::from_millis(current.wall_time_ms);
    if cancel.is_cancelled() || start.elapsed() >= budget {
        return None;
    }
    let worker = Worker::discover(&std::env::current_exe().ok()?).ok()??;
    if basis.worker_sha256.as_ref() != Some(&worker.sha256) {
        return None;
    }
    let (proof, previous_request_sha256, request_sha256, backend_source) =
        relation(old, current, evidence, &worker, budget, start, cancel)?;
    if previous_request_sha256 != basis.request_sha256
        || cancel.is_cancelled()
        || start.elapsed() >= budget
    {
        return None;
    }
    Some(Projected {
        version: 1,
        outcome: Outcome::Holds,
        obligation_id: &current.obligation_id,
        requirement_ids: &current.requirement_ids,
        request_sha256,
        previous_request_sha256,
        previous_receipt_key,
        worker_sha256: worker.sha256,
        backend_source,
        evidence: proof,
    })
}

fn relation(
    old: &Job,
    current: &Job,
    evidence: &sail_api::temporal::Response,
    worker: &sail_runtime::temporal::Worker,
    budget: Duration,
    start: Instant,
    cancel: &Cancellation,
) -> Option<(ProjectionProof, String, String, String)> {
    use sail_api::temporal::Completion;
    Some(
        if sail_dive::edits::loaded::eligible(&old.request, &current.request) {
            let query = sail_dive::edits::loaded::Query {
                previous: old.request.clone(),
                current: current.request.clone(),
            };
            let sail_dive::client::RunResult::Completed(relation) =
                sail_dive::client::compare_loaded(
                    &worker.executable,
                    &worker.sha256,
                    &query,
                    budget,
                    start,
                    cancel,
                )
            else {
                return None;
            };
            (
                ProjectionProof::UnchangedLoadedProgram {
                    loaded_modules: relation.loaded_modules?,
                },
                relation.previous_sha256,
                relation.current_sha256,
                relation.backend_source,
            )
        } else {
            match evidence.completion {
                Completion::Safety => {
                    let query = sail_dive::edits::projection::Query {
                        previous: old.request.clone(),
                        current: current.request.clone(),
                    };
                    let sail_dive::client::RunResult::Completed(relation) =
                        sail_dive::client::project(
                            &worker.executable,
                            &worker.sha256,
                            &query,
                            budget,
                            start,
                            cancel,
                        )
                    else {
                        return None;
                    };
                    (
                        ProjectionProof::Invariants {
                            invariants: relation.invariants?,
                        },
                        relation.previous_sha256,
                        relation.current_sha256,
                        relation.backend_source,
                    )
                }
                Completion::Temporal
                    if sail_dive::edits::first_claim::eligible(
                        &old.request,
                        &current.request,
                        evidence,
                    ) =>
                {
                    let query = sail_dive::edits::first_claim::Query {
                        previous: old.request.clone(),
                        current: current.request.clone(),
                        previous_response: evidence.clone(),
                    };
                    let sail_dive::client::RunResult::Completed(relation) =
                        sail_dive::client::project_first_claim(
                            &worker.executable,
                            &worker.sha256,
                            &query,
                            budget,
                            start,
                            cancel,
                        )
                    else {
                        return None;
                    };
                    let claim = relation.claim?;
                    (
                        ProjectionProof::FirstTemporalClaim {
                            property: claim.property,
                            basis_counts: claim.basis_counts,
                        },
                        relation.previous_sha256,
                        relation.current_sha256,
                        relation.backend_source,
                    )
                }
                _ => return None,
            }
        },
    )
}

// Validation is optional work. For example, adding a long previous trace
// can exceed the payload bound even though the current check itself fits.
fn validate(
    runtime: &Runtime,
    current: &Job,
    basis: &sail_api::temporal::Result,
    trace: &ItfTrace,
    cancellation: &Cancellation,
    start: Instant,
) -> Option<Response> {
    let budget = Duration::from_millis(current.wall_time_ms);
    let remaining_ms = u64::try_from(budget.saturating_sub(start.elapsed()).as_millis()).ok()?;
    if remaining_ms == 0 || cancellation.is_cancelled() {
        return None;
    }
    let mut job = current.clone();
    job.wall_time_ms = remaining_ms;
    job.request.operation = Operation::Validate {
        trace: trace.clone(),
    };
    let validation = Request {
        version: VERSION,
        input: Input::Temporal { job: Box::new(job) },
    };
    let validated = runtime.execute(&validation, cancellation).ok()?;
    let Output::Temporal { result } = &validated.output else {
        return None;
    };
    let evidence = result.response.as_ref()?;
    (result.worker_sha256 == basis.worker_sha256
        && result.backend_revision == basis.backend_revision
        && evidence.outcome == Outcome::Valid
        && evidence.itf_traces.as_slice() == std::slice::from_ref(trace)
        && matches!(evidence.validations.as_slice(), [report] if report["result"] == "valid")
        && !cancellation.is_cancelled()
        && start.elapsed() < budget)
        .then_some(validated)
}

fn write_current(out: &mut dyn Write, full: &Response) -> Result<u8, String> {
    let completed = matches!(&full.output, Output::Temporal { result }
        if result.response.as_ref().is_some_and(|r| matches!(r.outcome, Outcome::Holds | Outcome::Refuted)));
    super::write_json(
        out,
        &json!({"version":1, "route":"current-check", "response":full}),
    )?;
    Ok(u8::from(!completed))
}
