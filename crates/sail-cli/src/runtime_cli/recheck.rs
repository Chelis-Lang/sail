//! A prior check can justify a named invariant failure after a CFG-only edit.
use sail_api::runtime::{Input, Output, Request, VERSION};
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
    if old.wall_time_ms == new.wall_time_ms
        && old.request != new.request
        && sail_dive::equivalent_check_config(&old.request, &new.request)
        && let Some(basis) = runtime
            .lookup_temporal(old, cancellation)
            .map_err(|e| e.to_string())?
        && let Some(trace) = basis.response.as_ref().and_then(invariant_trace)
    {
        let mut validate = current.clone();
        let Input::Temporal { job } = &mut validate.input else {
            return Err(super::usage());
        };
        let budget = Duration::from_millis(new.wall_time_ms);
        let remaining = budget.saturating_sub(start.elapsed());
        job.wall_time_ms = u64::try_from(remaining.as_millis()).map_err(|e| e.to_string())?;
        job.request.operation = Operation::Validate {
            trace: trace.clone(),
        };
        if job.wall_time_ms > 0 {
            let validated = runtime
                .execute(&validate, cancellation)
                .map_err(|e| e.to_string())?;
            if let Output::Temporal { result } = &validated.output
                && result.worker_sha256 == basis.worker_sha256
                && result.backend_revision == basis.backend_revision
                && let Some(evidence) = &result.response
                && evidence.outcome == Outcome::Valid
                && evidence.itf_traces.as_slice() == std::slice::from_ref(trace)
                && matches!(evidence.validations.as_slice(), [report] if report["result"] == "valid")
                && !cancellation.is_cancelled()
                && start.elapsed() < budget
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
    let completed = matches!(&full.output, Output::Temporal { result }
        if result.response.as_ref().is_some_and(|r| matches!(r.outcome, Outcome::Holds | Outcome::Refuted)));
    super::write_json(
        out,
        &json!({"version":1, "route":"current-check", "response":full}),
    )?;
    Ok(u8::from(!completed))
}
