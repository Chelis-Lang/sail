//! Pinned native temporal jobs, exact trace transport and an isolated worker.
#![forbid(unsafe_code)]

pub mod backend;
pub mod client;
mod codec;
mod input;
mod jobs;
mod wire;
pub use codec::{itf_to_native, native_to_itf};
pub use jobs::execute;
pub use sail_api::temporal::{
    Completion, DIVE_REVISION, ItfTrace, MAX_DOCUMENT_BYTES, Model, Operation, Outcome, Request,
    RequestedChecks, Response, VERSION, request_sha256, sha256,
};
pub use wire::{decode_request, decode_response};

fn requested_checks(model: &Model) -> Option<RequestedChecks> {
    let source = model.files.get(&model.config)?;
    let report = dive_api::parse_config_source(&model.config, source);
    let config = report.config()?;
    Some(RequestedChecks {
        invariants: config.invariants.iter().map(|n| n.text.clone()).collect(),
        properties: config.properties.iter().map(|n| n.text.clone()).collect(),
        postconditions: config
            .postconditions
            .iter()
            .map(|n| n.text.clone())
            .collect(),
        possible: config.possible.iter().map(|n| n.text.clone()).collect(),
        deadlock: config.checks_deadlock(),
    })
}

/// Validates native evidence and independently binds the requested property inventory.
///
/// # Errors
/// Mismatched identity, unsupported evidence or changed property inventory.
pub fn validate_response(request: &Request, response: &Response) -> Result<(), String> {
    sail_api::temporal::validate_response(request, response)?;
    if response.requested_checks != requested_checks(&request.model) {
        return Err("SAIL4213: worker requested-property inventory differs".into());
    }
    Ok(())
}
