//! A prior check can justify a named invariant failure after a CFG-only edit.
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
        if let Ok(Some(basis)) = runtime.lookup_temporal(old, cancellation)
            && let Some(trace) = basis.response.as_ref().and_then(invariant_trace)
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
    let full = runtime
        .execute(current, cancellation)
        .map_err(|e| e.to_string())?;
    write_current(out, &full)
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
