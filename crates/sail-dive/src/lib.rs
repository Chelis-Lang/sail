//! Pinned native temporal jobs, exact trace transport and an isolated worker.
#![forbid(unsafe_code)]

pub mod backend;
pub mod client;
mod codec;
pub mod edits;
mod input;
mod jobs;
mod wire;
pub use codec::{itf_to_native, native_to_itf};
pub use jobs::execute;
pub use sail_api::temporal::{
    Completion, DIVE_VERSION, ItfTrace, MAX_DOCUMENT_BYTES, Model, Operation, Outcome, Request,
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

/// Whether two admitted check requests have identical model files and parsed CFGs.
/// Names, order and source locations in the parsed CFG participate in equality.
/// This admits trailing configuration comments. A change to any module, option,
/// file set, path or operative configuration fails this test.
///
/// Callers still need a trusted completed check at the same limits, producer
/// identity and independent validation against the edited request.
#[must_use]
pub fn equivalent_check_config(old: &Request, new: &Request) -> bool {
    if old.version != VERSION
        || new.version != VERSION
        || !matches!(old.operation, Operation::Check { .. })
        || old.operation != new.operation
    {
        return false;
    }
    let (a, b) = (&old.model, &new.model);
    if a.root != b.root
        || a.config != b.config
        || a.files.len() != b.files.len()
        || a.files
            .iter()
            .any(|(name, source)| name != &a.config && b.files.get(name) != Some(source))
    {
        return false;
    }
    let (Some(left), Some(right)) = (a.files.get(&a.config), b.files.get(&b.config)) else {
        return false;
    };
    let left = dive_api::parse_config_source(&a.config, left);
    let right = dive_api::parse_config_source(&b.config, right);
    left.accepted()
        && right.accepted()
        && left.parse().diagnostics().is_empty()
        && right.parse().diagnostics().is_empty()
        && left.config() == right.config()
}
