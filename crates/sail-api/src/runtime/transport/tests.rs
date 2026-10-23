//! Framing, namespace and permission regression tests.
use super::{MAX_REQUEST_BYTES, Request, read_frame, socket_path, write_frame};
use std::{io, path::Path};

#[test]
fn frames_reject_oversize_incompatible_and_incomplete_data() {
    assert!(read_frame::<Request>(&mut [255_u8; 4].as_slice(), MAX_REQUEST_BYTES).is_err());
    let body = br#"{"version":2,"body":{"operation":"capabilities"}}"#;
    let mut frame = u32::try_from(body.len())
        .unwrap_or_default()
        .to_be_bytes()
        .to_vec();
    frame.extend_from_slice(body);
    assert!(read_frame::<Request>(&mut frame.as_slice(), MAX_REQUEST_BYTES).is_err());
    frame.pop();
    assert!(read_frame::<Request>(&mut frame.as_slice(), MAX_REQUEST_BYTES).is_err());
    assert!(write_frame(&mut Vec::new(), &Request::Capabilities, 2).is_err());
    let mut frame = Vec::new();
    assert!(write_frame(&mut frame, &Request::Capabilities, MAX_REQUEST_BYTES).is_ok());
    assert!(matches!(
        read_frame(&mut frame.as_slice(), MAX_REQUEST_BYTES),
        Ok(Request::Capabilities)
    ));
}

#[test]
fn endpoint_namespace_is_distinct_and_fits_existing_job_paths() -> io::Result<()> {
    let cache = Path::new("/tmp/sail/cache.sock");
    let runtime = socket_path(cache);
    let jobs = crate::jobs::socket_path(cache);
    assert_ne!(runtime, jobs);
    assert_ne!(runtime, cache);
    assert!(runtime.as_os_str().len() <= jobs.as_os_str().len());
    let current = std::env::current_dir()?;
    let relative = current.join("cache.sock");
    assert_eq!(socket_path(Path::new("cache.sock")), socket_path(&relative));
    Ok(())
}

#[test]
fn deadline_bounds_a_silent_unix_peer_without_socket_timeout_options() -> io::Result<()> {
    use std::{
        io::Read as _,
        os::unix::net::UnixStream,
        time::{Duration, Instant},
    };
    let (socket, _peer) = UnixStream::pair()?;
    let started = Instant::now();
    let error = super::Deadline::new(&socket, Duration::from_millis(25))
        .read(&mut [0_u8; 1])
        .err()
        .ok_or_else(|| io::Error::other("silent peer did not time out"))?;
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(1));
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One establishing fixture checks independent attribution, content, limits, identity and missing-evidence failures."
)]
fn completed_payload_is_bound_to_content_limits_identity_and_original_metadata() -> io::Result<()> {
    use crate::runtime::{
        Answer, BackendCapability, Capabilities, Check, Input, Limits, Outcome, Output, Query,
        QueryResult, Request as RuntimeRequest, Response as RuntimeResponse, SolverIdentity,
        VERSION,
    };
    use std::fmt::Write as _;
    let solver = SolverIdentity {
        backend: "z3".into(),
        version: "4.16.0".into(),
        sha256: "a".repeat(64),
        platform: "test-platform".into(),
    };
    let capabilities = Capabilities {
        temporal: None,
        version: VERSION,
        backends: vec![BackendCapability {
            backend: "z3".into(),
            implemented: true,
            installed: true,
            solver: Some(solver.clone()),
            error: None,
        }],
        incremental_transcripts: true,
        batches: true,
        max_queries: 64,
    };
    let script = "(check-sat)";
    let digest =
        sail_key::sha256(script.as_bytes())
            .iter()
            .fold(String::new(), |mut text, byte| {
                let _ = write!(text, "{byte:02x}");
                text
            });
    let request = RuntimeRequest {
        version: VERSION,
        input: Input::Smt {
            queries: vec![Query {
                obligation_id: "o".into(),
                requirement_ids: vec!["r".into()],
                backend: "z3".into(),
                script: script.into(),
                limits: Limits::default(),
            }],
        },
    };
    let result = QueryResult {
        obligation_id: "o".into(),
        requirement_ids: vec!["r".into()],
        backend: "z3".into(),
        outcome: Outcome::Completed,
        checks: vec![Check {
            command_index: 0,
            answer: Answer::Sat,
            products: Vec::new(),
        }],
        stdout: b"sat\n".to_vec(),
        stderr: Vec::new(),
        solver: Some(solver),
        script_sha256: digest,
        limits: Limits::default(),
        cache_hit: false,
        error: None,
    };
    let validate = |result| {
        super::client::validate_payload(
            &request,
            &RuntimeResponse {
                version: VERSION,
                output: Output::Smt {
                    results: vec![result],
                },
            },
            &capabilities,
        )
    };
    validate(result.clone())?;
    let mut changed = result.clone();
    changed.obligation_id = "different".into();
    assert!(validate(changed).is_err());
    let mut changed = result.clone();
    changed.requirement_ids.clear();
    assert!(validate(changed).is_err());
    let mut changed = result.clone();
    changed.backend = "cvc5".into();
    assert!(validate(changed).is_err());
    let mut changed = result.clone();
    changed.script_sha256 = "b".repeat(64);
    assert!(validate(changed).is_err());
    let mut changed = result.clone();
    changed.limits.resource_limit += 1;
    assert!(validate(changed).is_err());
    let mut changed = result.clone();
    changed.solver = None;
    assert!(validate(changed).is_err());
    let mut changed = result.clone();
    changed.stdout.clear();
    assert!(validate(changed).is_err());
    let mut changed = result;
    changed.checks.clear();
    assert!(validate(changed).is_err());
    Ok(())
}

#[test]
fn capability_extension_keeps_old_smt_clients_and_old_services_compatible() -> io::Result<()> {
    use crate::runtime::Capabilities;
    use serde::{Deserialize, Serialize};
    #[derive(Deserialize, Serialize)]
    struct OldCapabilities {
        version: u32,
        backends: Vec<crate::runtime::BackendCapability>,
        incremental_transcripts: bool,
        batches: bool,
        max_queries: usize,
    }
    let old = serde_json::json!({"version":1,"backends":[],"incremental_transcripts":true,"batches":true,"max_queries":64});
    let mut new: Capabilities = serde_json::from_value(old.clone())?;
    assert!(new.temporal.is_none());
    new.temporal = Some(crate::temporal::Capability {
        implemented: true,
        installed: false,
        backend_revision: crate::temporal::DIVE_REVISION.into(),
        worker_sha256: None,
        error: None,
    });
    let legacy: OldCapabilities = serde_json::from_value(serde_json::to_value(new)?)?;
    assert_eq!(serde_json::to_value(legacy)?, old);
    // Duplicate source or ITF keys must fail before canonical maps can hide them.
    assert!(
        crate::temporal::decode::<serde_json::Value>(
            br#"{"model":{"files":{"M.tla":"a","M.tla":"b"}}}"#
        )
        .is_err()
    );
    Ok(())
}
