//! Native execution, with explicit deciding evidence and independent replay.
use crate::input::{Failure, Frozen};
use crate::{Operation, Outcome, Request, Response, VERSION, itf_to_native, native_to_itf};
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{BudgetSetting, CheckRequest, Store, TemporalMode};
use serde_json::Value;

/// Executes one immutable admitted job. The isolated parent enforces lifetime.
///
/// Calling in-process provides no hard deadline or memory isolation. Never use a
/// detached thread as cancellation; use [`crate::client::run`] instead.
#[must_use]
pub fn execute(request: &Request) -> Response {
    attribute(request, run(request))
}

pub(crate) fn attribute(request: &Request, result: Result<Response, Failure>) -> Response {
    let mut response = match result {
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

/// How a witness is replayed: as an exhaustive-checking word, or as a finite
/// simulation sample, where a deadlock means no successor satisfies the
/// model's constraints (#766).
#[derive(Clone, Copy)]
enum Witness {
    Check,
    Sample,
}

fn add_trace(
    input: &Frozen,
    response: &mut Response,
    source: &SpecModel,
    native: &Value,
    witness: Witness,
) -> Result<(), Failure> {
    response.native_traces.push(native.clone());
    let itf = native_to_itf(native).map_err(unsupported)?;
    let replay = itf_to_native(&itf).map_err(unsupported)?;
    let document = TraceDoc::from_json(&replay).map_err(unsupported)?;
    input.admit_labels(&document)?;
    // A sampled behavior or simulation deadlock is replayed under sampling
    // rules, as D.I.V.E.'s own simulation validates it; any other ending, and
    // every exhaustive-checking witness, uses ordinary validation.
    let sampled = matches!(witness, Witness::Sample)
        && matches!(document.ending, Some(Ending::Behavior | Ending::Deadlock));
    let validation = if sampled {
        source.try_validate_sample(&document).map_err(|error| {
            (
                Outcome::Error,
                format!("SAIL4205: independent sample validation did not complete: {error}"),
            )
        })?
    } else {
        source.validate(&document)
    };
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
        // A validation kind this build does not know is never acceptance.
        _ => Err((
            Outcome::Error,
            "SAIL4205: backend witness has an unrecognized validation result".into(),
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
            let document = TraceDoc::from_json(&native).map_err(unsupported)?;
            input.admit_labels(&document)?;
            let validation = model(&input)?.validate(&document);
            let outcome = match &validation.validation {
                Validation::Valid => Outcome::Valid,
                Validation::Invalid(_) => Outcome::Invalid,
                // Unknown validation kinds are never reported as valid.
                Validation::Unsupported(_) | _ => Outcome::Unsupported,
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

pub(crate) fn check(input: &Frozen, states: u64, depth: u64) -> Result<Response, Failure> {
    // D.I.V.E. 0.1.0 checks on its Core engine, which has no bounded final
    // (`FinalShort`) route: ordinary dive-profile checking runs safety roles
    // and deterministic completed-level temporal checks in one request, and
    // every temporal success must still carry complete capture evidence.
    let temporal = TemporalMode::Ordinary;
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
                .is_some_and(|r| matches!(r.outcome(), dive_api::temporal::Outcome::NativeHolds(_)))
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
        if let Err((outcome, message)) =
            add_trace(input, &mut response, &source, &trace, Witness::Check)
        {
            response.outcome = outcome;
            response.message = Some(message);
        }
    }
    Ok(response)
}

/// Traces sampled first to estimate a larger job's evidence. D.I.V.E.'s
/// default-profile traces are prefix-stable for a fixed seed: the probe's
/// traces are exactly the first traces of the full run.
const PROBE_TRACES: u64 = 16;

fn simulate(input: &Frozen, traces: u64, depth: usize) -> Result<Response, Failure> {
    simulate_within(input, traces, depth, crate::MAX_DOCUMENT_BYTES)
}

/// Simulates under a response byte bound (#767). A job larger than
/// [`PROBE_TRACES`] first samples that many traces and refuses before the full
/// run when their measured evidence, extrapolated to the requested count,
/// exceeds `limit`. The full run is refused as soon as its evidence provably
/// exceeds `limit`: right after sampling when the native traces alone exceed
/// it, otherwise during replay, before the next trace is converted.
fn simulate_within(
    input: &Frozen,
    traces: u64,
    depth: usize,
    limit: usize,
) -> Result<Response, Failure> {
    if traces == 0 || depth == 0 || traces > 100_000 || depth > 1_000_000 {
        return Err((
            Outcome::Rejected,
            "SAIL4207: simulation requires finite admitted positive counts".into(),
        ));
    }
    if traces > PROBE_TRACES {
        let probe = sample(input, PROBE_TRACES, depth, limit)?;
        if probe.outcome == Outcome::Sampled {
            check_estimate(&probe, traces, limit)?;
        }
    }
    sample(input, traces, depth, limit)
}

fn oversized(limit: usize, detail: &str) -> Failure {
    (
        Outcome::Rejected,
        format!("SAIL4216: simulation evidence exceeds the {limit}-byte response bound: {detail}"),
    )
}

/// Counts compact JSON bytes, stopping the serializer once `limit` is passed.
struct Count {
    bytes: usize,
    limit: usize,
}

impl std::io::Write for Count {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self.bytes.saturating_add(bytes.len());
        if self.bytes > self.limit {
            return Err(std::io::Error::other("byte bound passed"));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Compact JSON length of `value`, or `None` once it exceeds `limit`.
fn encoded_len(value: &impl serde::Serialize, limit: usize) -> Option<usize> {
    let mut count = Count { bytes: 0, limit };
    serde_json::to_writer(&mut count, value).ok()?;
    Some(count.bytes)
}

/// Encoded bytes replay adds for trace `index`: its native document, ITF
/// rendering and validation, each with a separator.
fn replay_bytes(response: &Response, index: usize) -> usize {
    let pieces = [
        response
            .native_traces
            .get(index)
            .and_then(|v| encoded_len(v, usize::MAX)),
        response
            .itf_traces
            .get(index)
            .and_then(|v| encoded_len(v, usize::MAX)),
        response
            .validations
            .get(index)
            .and_then(|v| encoded_len(v, usize::MAX)),
    ];
    pieces.iter().map(|n| n.unwrap_or(0) + 1).sum()
}

/// Encoded bytes the response spends on trace `index`, including its native
/// report entry.
fn trace_bytes(response: &Response, index: usize) -> usize {
    let entry = response
        .native_report
        .pointer(&format!("/traces/{index}"))
        .and_then(|v| encoded_len(v, usize::MAX))
        .unwrap_or(0);
    replay_bytes(response, index) + entry + 1
}

/// Refuses a job whose probe evidence, extrapolated per trace, exceeds `limit`.
fn check_estimate(probe: &Response, traces: u64, limit: usize) -> Result<(), Failure> {
    let total = encoded_len(probe, usize::MAX).unwrap_or(usize::MAX);
    let sampled = probe.native_traces.len();
    let per_trace: usize = (0..sampled).map(|i| trace_bytes(probe, i)).sum();
    let fixed = total.saturating_sub(per_trace);
    let (Ok(sampled), Ok(per_trace), Ok(fixed), Ok(limit_wide)) = (
        u128::try_from(sampled),
        u128::try_from(per_trace),
        u128::try_from(fixed),
        u128::try_from(limit),
    ) else {
        return Ok(());
    };
    if sampled == 0 || per_trace == 0 {
        return Ok(());
    }
    let estimate = fixed + per_trace * u128::from(traces) / sampled;
    if estimate <= limit_wide {
        return Ok(());
    }
    let admitted = limit_wide.saturating_sub(fixed) * sampled / per_trace;
    Err(oversized(
        limit,
        &format!(
            "the first {sampled} traces encode to {total} bytes, so {traces} traces need about \
             {estimate} bytes; request at most {admitted} traces at this depth, or a shorter depth"
        ),
    ))
}

fn sample(input: &Frozen, traces: u64, depth: usize, limit: usize) -> Result<Response, Failure> {
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
    // The response holds the native report, so its retained trace text alone
    // is a lower bound on the response: refuse before building any tree.
    let retained = report.retained_trace_bytes();
    if retained > limit {
        return Err(oversized(
            limit,
            &format!("the native report's {traces} traces alone hold {retained} bytes"),
        ));
    }
    // An admitted report is at most `limit` bytes of traces, and the response
    // is decoded as one tree by the parent anyway, so building it here is
    // bounded; `write_json` would only pay off with a streamed response.
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
        let entries = native["traces"]
            .as_array()
            .ok_or_else(|| unsupported("SAIL4208: simulation traces absent"))?;
        let mut used = encoded_len(&response, limit)
            .ok_or_else(|| oversized(limit, "the native report alone exceeds it"))?;
        for (index, trace) in entries.iter().enumerate() {
            if let Err((outcome, message)) = add_trace(
                input,
                &mut response,
                &source,
                &trace["document"],
                Witness::Sample,
            ) {
                response.outcome = outcome;
                response.message = Some(message);
                break;
            }
            // The native report entry is already counted.
            used = used.saturating_add(replay_bytes(&response, index));
            if used > limit {
                return Err(oversized(
                    limit,
                    &format!(
                        "replaying {} of {} traces already needs {used} bytes",
                        index + 1,
                        entries.len()
                    ),
                ));
            }
        }
        if response.native_traces.is_empty() {
            response.outcome = Outcome::Error;
            response.message = Some("SAIL4208: no concrete sampled evidence".into());
        }
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUNTER: &str = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = IF x < 2 THEN x+1 ELSE 0\n====\n";

    fn counter() -> Result<Frozen, Failure> {
        Frozen::create(&crate::Model {
            root: "M.tla".into(),
            config: "M.cfg".into(),
            files: [
                ("M.tla".into(), COUNTER.into()),
                ("M.cfg".into(), "INIT Init\nNEXT Next\n".into()),
            ]
            .into(),
        })
    }

    fn refusal(result: Result<Response, Failure>) -> Result<String, String> {
        match result {
            Err((Outcome::Rejected, message)) if message.starts_with("SAIL4216: ") => Ok(message),
            other => Err(format!("expected a SAIL4216 refusal, got {other:?}")),
        }
    }

    #[test]
    fn probe_extrapolation_refuses_before_the_full_run() -> Result<(), String> {
        let input = counter().map_err(|e| e.1)?;
        let probe = sample(&input, PROBE_TRACES, 4, usize::MAX).map_err(|e| e.1)?;
        let probe_bytes = encoded_len(&probe, usize::MAX).ok_or("probe size")?;
        // The probe fits; 100 traces would need about six times as much.
        let message = refusal(simulate_within(&input, 100, 4, probe_bytes * 2))?;
        assert!(
            message.contains("the first 16 traces") && message.contains("request at most"),
            "{message}"
        );
        // A slightly larger job within the bound still runs completely.
        let response = simulate_within(&input, 20, 4, probe_bytes * 2).map_err(|e| e.1)?;
        assert_eq!(response.outcome, Outcome::Sampled, "{response:?}");
        assert_eq!(response.itf_traces.len(), 20);
        assert!(encoded_len(&response, probe_bytes * 2).is_some());
        Ok(())
    }

    #[test]
    fn native_traces_over_the_bound_refuse_before_replay() -> Result<(), String> {
        let input = counter().map_err(|e| e.1)?;
        let message = refusal(simulate_within(&input, 2, 4, 10))?;
        assert!(message.contains("traces alone hold"), "{message}");
        Ok(())
    }

    #[test]
    fn replay_refuses_once_its_evidence_passes_the_bound() -> Result<(), String> {
        let input = counter().map_err(|e| e.1)?;
        let full = sample(&input, 2, 4, usize::MAX).map_err(|e| e.1)?;
        let report = Response::new(full.outcome, full.native_report.clone());
        let base = encoded_len(&report, usize::MAX).ok_or("report size")?;
        let message = refusal(simulate_within(
            &input,
            2,
            4,
            base + replay_bytes(&full, 0) - 1,
        ))?;
        assert!(message.contains("replaying 1 of 2 traces"), "{message}");
        let message = refusal(simulate_within(&input, 2, 4, base + replay_bytes(&full, 0)))?;
        assert!(message.contains("replaying 2 of 2 traces"), "{message}");
        Ok(())
    }
}
