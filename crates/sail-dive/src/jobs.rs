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
    // A sampled behavior, simulation deadlock or terminal word (a sample that
    // stopped early with deadlock checking off) is replayed under sampling
    // rules, as D.I.V.E.'s own simulation validates it; any other ending, and
    // every exhaustive-checking witness, uses ordinary validation.
    let sampled = matches!(witness, Witness::Sample)
        && matches!(
            document.ending,
            Some(Ending::Behavior | Ending::Deadlock | Ending::Terminal)
        );
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

/// Traces sampled first to size a larger job's kept evidence. D.I.V.E.'s
/// default-profile traces are prefix-stable for a fixed seed: the probe's
/// traces are exactly the first traces of the full run.
const PROBE_TRACES: u64 = 16;

fn simulate(input: &Frozen, traces: u64, depth: usize) -> Result<Response, Failure> {
    simulate_within(input, traces, depth, crate::MAX_DOCUMENT_BYTES)
}

/// Simulates under a response byte bound (#767). D.I.V.E. samples and checks
/// every requested trace. The response keeps the lowest-id traces whose
/// native, ITF and validated forms fit `limit`, plus any deciding trace, and
/// replays each independently; the native report's `statistics.traces_kept`
/// and `traces_dropped` count them. A job larger than [`PROBE_TRACES`] first
/// samples that many traces to size how many to keep. When the kept traces
/// still exceed the bound, the job runs once more keeping only the traces that
/// fitted, or only the deciding trace. A job is refused with `SAIL4216` only
/// when not one trace, or the deciding trace alone, fits.
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
    let keep = if traces > PROBE_TRACES {
        match sample(input, PROBE_TRACES, depth, PROBE_TRACES, limit)? {
            Sample::Fits(probe) if probe.outcome == Outcome::Sampled => {
                keep_estimate(&probe, traces, limit)?
            }
            Sample::Fits(_) => traces,
            Sample::Exceeds { deciding: true, .. } => 0,
            Sample::Exceeds { fitted: 0, .. } => {
                return Err(oversized(limit, "not even the first trace fits"));
            }
            Sample::Exceeds { fitted, .. } => fitted,
        }
    } else {
        traces
    };
    let (fitted, deciding) = match sample(input, traces, depth, keep, limit)? {
        Sample::Fits(response) => return Ok(*response),
        Sample::Exceeds { fitted, deciding } => (fitted, deciding),
    };
    let retry = if deciding { 0 } else { fitted };
    if retry == 0 && !deciding {
        return Err(oversized(limit, "not even the first trace fits"));
    }
    match sample(input, traces, depth, retry, limit)? {
        Sample::Fits(response) => Ok(*response),
        Sample::Exceeds { .. } if deciding => {
            Err(oversized(limit, "the deciding trace alone does not fit"))
        }
        Sample::Exceeds { fitted, .. } => Err(oversized(
            limit,
            &format!("{retry} traces fitted once but only {fitted} fit when rerun"),
        )),
    }
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

/// Traces to keep in a job of `traces`, from the probe's measured evidence per
/// trace, with a tenth held back for traces longer than the probe's. Refuses
/// a job in which not one trace fits `limit`.
fn keep_estimate(probe: &Response, traces: u64, limit: usize) -> Result<u64, Failure> {
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
        return Ok(traces);
    };
    if sampled == 0 || per_trace == 0 {
        return Ok(traces);
    }
    let admitted = limit_wide.saturating_sub(fixed) * sampled / per_trace;
    if admitted == 0 {
        return Err(oversized(
            limit,
            &format!(
                "the first {sampled} traces encode to {total} bytes, so not one trace fits; \
                 request a shorter depth"
            ),
        ));
    }
    let margin = (admitted - admitted / 10).max(1);
    Ok(u64::try_from(margin).unwrap_or(u64::MAX).min(traces))
}

/// A simulation run's evidence: the whole response when it fits the bound, or
/// how many leading sample traces fitted and whether a deciding trace exists.
enum Sample {
    Fits(Box<Response>),
    Exceeds { fitted: u64, deciding: bool },
}

/// Collects at most `limit` bytes and fails once more are written.
struct Bounded {
    bytes: Vec<u8>,
    limit: usize,
}

impl std::io::Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other("byte bound passed"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Samples `traces` traces, keeping `keep` of them besides any deciding trace,
/// and replays the kept traces while the response stays within `limit`.
fn sample(
    input: &Frozen,
    traces: u64,
    depth: usize,
    keep: u64,
    limit: usize,
) -> Result<Sample, Failure> {
    let mut request = dive_api::SimulateRequest::default();
    request.file.clone_from(&input.root);
    request.config = Some(input.config.clone());
    request.library_paths.clone_from(&input.libraries);
    request.seed = Some(0);
    request.workers = Some(1);
    request.traces = traces;
    request.depth = Some(depth);
    request.keep_traces = Some(keep);
    // The native report is part of the response, so it alone must fit.
    request.max_report_bytes = Some(limit);
    let report = dive_api::simulate_file(&request).map_err(|e| api_error(&e))?;
    // Write the report text without building every trace as a JSON tree
    // (dive #1040); the bounded writer guards the backend's own bound.
    let mut text = Bounded {
        bytes: Vec::new(),
        limit,
    };
    if report.write_json(&mut text, None).is_err() {
        return Err(oversized(limit, "the native report alone exceeds it"));
    }
    drop(report);
    let native: Value = serde_json::from_slice(&text.bytes).map_err(|e| {
        (
            Outcome::Error,
            format!("SAIL4208: unreadable native simulation report: {e}"),
        )
    })?;
    drop(text);
    let outcome = match native["result"].as_str() {
        Some("sampled") => Outcome::Sampled,
        Some("violation") if native["violation_validated"] == true => Outcome::Refuted,
        Some("unsupported") => Outcome::Unsupported,
        Some("resource-limit") => Outcome::Stopped,
        _ => Outcome::Error,
    };
    let deciding = native["observed_violation"]["trace"].is_u64();
    let mut response = Response::new(outcome, native);
    if !matches!(outcome, Outcome::Sampled | Outcome::Refuted) {
        return Ok(Sample::Fits(Box::new(response)));
    }
    if response.native_report["exhaustive"] != false
        || response.native_report["model_proved"] != false
    {
        return Err((
            Outcome::Error,
            "SAIL4208: simulation cannot establish an exhaustive proof".into(),
        ));
    }
    let source = model(input)?;
    let entries = response.native_report["traces"]
        .as_array()
        .cloned()
        .ok_or_else(|| unsupported("SAIL4208: simulation traces absent"))?;
    let Some(mut used) = encoded_len(&response, limit) else {
        return Err(oversized(limit, "the native report alone exceeds it"));
    };
    // D.I.V.E. drops kept sample traces until its report fits the bound; when
    // it dropped them all, not one trace fits.
    let dropped = response.native_report["statistics"]["traces_dropped"]
        .as_u64()
        .unwrap_or(0);
    if entries.is_empty() && dropped > 0 {
        return Ok(Sample::Exceeds {
            fitted: 0,
            deciding,
        });
    }
    for (index, trace) in entries.iter().enumerate() {
        let fitted = u64::try_from(index).unwrap_or(u64::MAX);
        // D.I.V.E. replaces a deciding trace that does not fit its report
        // bound by a summary without the trace.
        if trace.get("summary").is_some() {
            return Ok(Sample::Exceeds { fitted, deciding });
        }
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
            return Ok(Sample::Exceeds { fitted, deciding });
        }
    }
    if response.native_traces.is_empty() {
        response.outcome = Outcome::Error;
        response.message = Some("SAIL4208: no concrete sampled evidence".into());
    }
    Ok(Sample::Fits(Box::new(response)))
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

    fn fitted(result: Result<Sample, Failure>) -> Result<Response, String> {
        match result {
            Ok(Sample::Fits(response)) => Ok(*response),
            Ok(Sample::Exceeds { fitted, .. }) => Err(format!("only {fitted} traces fit")),
            Err(e) => Err(e.1),
        }
    }

    fn counts(response: &Response) -> (u64, u64) {
        let statistics = &response.native_report["statistics"];
        (
            statistics["traces_started"].as_u64().unwrap_or(0),
            statistics["traces_kept"].as_u64().unwrap_or(0),
        )
    }

    #[test]
    fn a_large_job_keeps_the_traces_its_probe_sizes_instead_of_refusing() -> Result<(), String> {
        let input = counter().map_err(|e| e.1)?;
        let probe = fitted(sample(&input, PROBE_TRACES, 4, PROBE_TRACES, usize::MAX))?;
        let limit = encoded_len(&probe, usize::MAX).ok_or("probe size")? * 2;
        // 100 traces would need about six times the probe's evidence.
        let response = simulate_within(&input, 100, 4, limit).map_err(|e| e.1)?;
        assert_eq!(response.outcome, Outcome::Sampled, "{response:?}");
        let (started, kept) = counts(&response);
        assert_eq!(started, 100);
        assert!(kept > PROBE_TRACES && kept < 100, "{kept}");
        assert_eq!(response.itf_traces.len() as u64, kept);
        assert!(response.validations.iter().all(|v| v["result"] == "valid"));
        assert!(encoded_len(&response, limit).is_some());
        // A slightly larger job within the bound keeps every trace.
        let response = simulate_within(&input, 20, 4, limit).map_err(|e| e.1)?;
        assert_eq!(response.outcome, Outcome::Sampled, "{response:?}");
        assert_eq!(counts(&response), (20, 20));
        assert_eq!(response.itf_traces.len(), 20);
        Ok(())
    }

    #[test]
    fn a_report_over_the_bound_is_refused() -> Result<(), String> {
        let input = counter().map_err(|e| e.1)?;
        let message = refusal(simulate_within(&input, 2, 4, 10))?;
        assert!(message.contains("native report alone exceeds"), "{message}");
        Ok(())
    }

    #[test]
    fn a_bound_below_one_trace_is_refused_rather_than_sampling_nothing() -> Result<(), String> {
        let input = counter().map_err(|e| e.1)?;
        let empty = fitted(sample(&input, 2, 4, 0, usize::MAX))?;
        let report = Response::new(Outcome::Sampled, empty.native_report.clone());
        let limit = encoded_len(&report, usize::MAX).ok_or("report size")? + 64;
        let one = fitted(sample(&input, 2, 4, 1, usize::MAX))?;
        let one = Response::new(Outcome::Sampled, one.native_report.clone());
        assert!(encoded_len(&one, limit).is_none(), "one trace fits {limit}");
        assert!(matches!(
            sample(&input, 2, 4, 2, limit),
            Ok(Sample::Exceeds {
                fitted: 0,
                deciding: false
            })
        ));
        for traces in [2, 100] {
            let message = refusal(simulate_within(&input, traces, 4, limit))?;
            assert!(
                message.contains("not even the first trace fits"),
                "{message}"
            );
        }
        Ok(())
    }

    #[test]
    fn replay_past_the_bound_reruns_keeping_the_traces_that_fitted() -> Result<(), String> {
        let input = counter().map_err(|e| e.1)?;
        let full = fitted(sample(&input, 2, 4, 2, usize::MAX))?;
        let report = Response::new(full.outcome, full.native_report.clone());
        let base = encoded_len(&report, usize::MAX).ok_or("report size")?;
        let limit = base + replay_bytes(&full, 0);
        assert!(matches!(
            sample(&input, 2, 4, 2, limit),
            Ok(Sample::Exceeds {
                fitted: 1,
                deciding: false
            })
        ));
        let response = simulate_within(&input, 2, 4, limit).map_err(|e| e.1)?;
        assert_eq!(response.outcome, Outcome::Sampled, "{response:?}");
        assert_eq!(counts(&response), (2, 1));
        assert_eq!(response.itf_traces.len(), 1);
        assert!(encoded_len(&response, limit).is_some());
        Ok(())
    }
}
