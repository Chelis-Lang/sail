//! Native execution, with explicit deciding evidence and independent replay.
use crate::input::{Failure, Frozen};
use crate::{Operation, Outcome, Request, Response, VERSION, itf_to_native, native_to_itf};
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{BudgetSetting, CheckRequest, Store, TemporalMode};
use serde_json::Value;

/// Executes one immutable admitted job. The isolated parent enforces lifetime.
///
/// Calling in-process provides no hard deadline or memory isolation. Never use a
/// detached thread as cancellation; use [`crate::client::run`] instead.
#[must_use]
pub fn execute(request: &Request) -> Response {
    let mut response = match run(request) {
        Ok(response) => response,
        Err((outcome, message)) => Response::failure(outcome, message),
    };
    response.requested_checks = crate::requested_checks(&request.model);
    match crate::request_sha256(request) {
        Ok(hash) => response.request_sha256 = hash,
        Err(error) => {
            response.outcome = Outcome::Error;
            response.message = Some(format!("SAIL4200: {error}"));
        }
    }
    response
}

fn unsupported(message: impl Into<String>) -> Failure {
    (Outcome::Unsupported, message.into())
}

fn api_error(error: &dive_api::ApiError) -> Failure {
    let outcome = match error {
        dive_api::ApiError::Unsupported { .. } => Outcome::Unsupported,
        dive_api::ApiError::Rejected { .. } => Outcome::Rejected,
        _ => Outcome::Error,
    };
    (outcome, format!("SAIL4204: {error}"))
}

fn model(input: &Frozen) -> Result<SpecModel, Failure> {
    SpecModel::load(
        &input.root,
        &input.config,
        &TraceOptions {
            library_paths: input.libraries.clone(),
            run: Some((0, 0)),
            ..Default::default()
        },
    )
    .map_err(|e| api_error(&e))
}

fn add_trace(response: &mut Response, source: &SpecModel, native: &Value) -> Result<(), Failure> {
    response.native_traces.push(native.clone());
    let itf = native_to_itf(native).map_err(unsupported)?;
    let replay = itf_to_native(&itf).map_err(unsupported)?;
    let document = TraceDoc::from_json(&replay).map_err(unsupported)?;
    let validation = source.validate(&document);
    response
        .validations
        .push(dive_api::trace::report_json(&validation));
    response.itf_traces.push(itf);
    match validation.validation {
        Validation::Valid => Ok(()),
        Validation::Unsupported(reason) => Err(unsupported(reason)),
        Validation::Invalid(_) => Err((
            Outcome::Error,
            "SAIL4205: backend witness failed independent semantic validation".into(),
        )),
    }
}

fn run(request: &Request) -> Result<Response, Failure> {
    if request.version != VERSION {
        return Err((
            Outcome::Rejected,
            "SAIL4200: unsupported worker request version".into(),
        ));
    }
    let input = Frozen::create(&request.model)?;
    match &request.operation {
        Operation::Check {
            max_states,
            max_depth,
        } => check(&input, *max_states, *max_depth),
        Operation::Simulate { traces, depth } => simulate(&input, *traces, *depth),
        Operation::Validate { trace } => {
            let native = itf_to_native(trace).map_err(unsupported)?;
            // Also prove the supplied ITF values survive the entire transport.
            if native_to_itf(&native).map_err(unsupported)? != *trace {
                return Err(unsupported("SAIL4203: ITF does not round-trip exactly"));
            }
            let source = model(&input)?;
            let document = TraceDoc::from_json(&native).map_err(unsupported)?;
            let validation = source.validate(&document);
            let outcome = match &validation.validation {
                Validation::Valid => Outcome::Valid,
                Validation::Invalid(_) => Outcome::Invalid,
                Validation::Unsupported(_) => Outcome::Unsupported,
            };
            let report = dive_api::trace::report_json(&validation);
            let mut response = Response::new(outcome, report.clone());
            response.native_traces.push(native);
            response.itf_traces.push(trace.clone());
            response.validations.push(report);
            Ok(response)
        }
    }
}

fn check(input: &Frozen, states: u64, depth: u64) -> Result<Response, Failure> {
    // Configured PROPERTYs include initial, state and action safety roles. Only
    // the binder's implied temporal roles require final graph analysis. The
    // pinned final checker refuses safety-only PROPERTYs rather than routing
    // them to its ordinary engine, so choose once before any search.
    let bound = model(input)?;
    let temporal = if bound.model().implied_temporals.is_empty() {
        TemporalMode::Ordinary
    } else {
        TemporalMode::FinalShort
    };
    let report = dive_api::check_file(&CheckRequest {
        file: input.root.clone(),
        config: Some(input.config.clone()),
        library_paths: input.libraries.clone(),
        store: Store::Exact,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        temporal,
        time_limit: BudgetSetting::Unlimited,
        max_memory: BudgetSetting::Unlimited,
        max_states: Some(states),
        max_depth: Some(depth),
        ..Default::default()
    })
    .map_err(|e| api_error(&e))?;
    let native = report.to_json();
    let mut response = Response::new(Outcome::Error, native.clone());
    response.outcome = match report.verdict() {
        "success" => {
            if let Some(safety) = report.result() {
                if safety.stats.as_ref().is_none_or(|s| s.left_on_queue != 0) {
                    return Err((
                        Outcome::Error,
                        "SAIL4206: safety success lacks complete exploration evidence".into(),
                    ));
                }
                response.completion = crate::Completion::Safety;
            } else if !report
                .temporal_result()
                .is_some_and(|r| matches!(r.outcome(), dive_api::temporal::Outcome::Holds(_)))
            {
                return Err((
                    Outcome::Error,
                    "SAIL4206: temporal success lacks complete graph evidence".into(),
                ));
            } else {
                response.completion = crate::Completion::Temporal;
            }
            Outcome::Holds
        }
        "invariant-violation"
        | "property-violation"
        | "action-property-violation"
        | "deadlock"
        | "liveness-violation" => Outcome::Refuted,
        "unsupported" => Outcome::Unsupported,
        "stopped" | "resource-limit" => Outcome::Stopped,
        "rejected" | "model-error" | "assumption-failure" | "incomplete-state" => Outcome::Rejected,
        _ => Outcome::Error,
    };
    if response.outcome == Outcome::Refuted {
        let trace = native
            .pointer("/counterexample/trace")
            .filter(|v| !v.is_null())
            .cloned()
            .ok_or_else(|| unsupported("SAIL4205: violation has no native trace document"))?;
        let source = model(input)?;
        if let Err((outcome, message)) = add_trace(&mut response, &source, &trace) {
            response.outcome = outcome;
            response.message = Some(message);
        }
    }
    Ok(response)
}

fn simulate(input: &Frozen, traces: u64, depth: usize) -> Result<Response, Failure> {
    if traces == 0 || depth == 0 || traces > 100_000 || depth > 1_000_000 {
        return Err((
            Outcome::Rejected,
            "SAIL4207: simulation requires finite admitted positive counts".into(),
        ));
    }
    let report = dive_api::simulate_file(&dive_api::SimulateRequest {
        file: input.root.clone(),
        config: Some(input.config.clone()),
        library_paths: input.libraries.clone(),
        seed: Some(0),
        workers: Some(1),
        traces,
        depth: Some(depth),
        ..Default::default()
    })
    .map_err(|e| api_error(&e))?;
    let native = report.to_json();
    let outcome = match native["result"].as_str() {
        Some("sampled") => Outcome::Sampled,
        Some("violation") if native["violation_validated"] == true => Outcome::Refuted,
        Some("unsupported") => Outcome::Unsupported,
        Some("resource-limit") => Outcome::Stopped,
        _ => Outcome::Error,
    };
    let mut response = Response::new(outcome, native.clone());
    if matches!(outcome, Outcome::Sampled | Outcome::Refuted) {
        if native["exhaustive"] != false || native["model_proved"] != false {
            return Err((
                Outcome::Error,
                "SAIL4208: simulation cannot establish an exhaustive proof".into(),
            ));
        }
        let source = model(input)?;
        for trace in native["traces"]
            .as_array()
            .ok_or_else(|| unsupported("SAIL4208: simulation traces absent"))?
        {
            if let Err((outcome, message)) = add_trace(&mut response, &source, &trace["document"]) {
                response.outcome = outcome;
                response.message = Some(message);
                break;
            }
        }
        if response.native_traces.is_empty() {
            response.outcome = Outcome::Error;
            response.message = Some("SAIL4208: no concrete sampled evidence".into());
        }
    }
    Ok(response)
}
