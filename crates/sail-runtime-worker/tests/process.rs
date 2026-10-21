//! Actual isolated worker and transport failures, without alternate verifiers.
use sail_dive::{
    Model, Operation, Outcome, Request, VERSION,
    client::{RunResult, run},
};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn request() -> Request {
    Request {version:VERSION,model:Model {
        files:BTreeMap::from([
            ("M.tla".into(),"---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = IF x < 2 THEN x+1 ELSE 0\n====\n".into()),
            ("M.cfg".into(),"INIT Init\nNEXT Next\n".into())]),
        root:"M.tla".into(),config:"M.cfg".into()
    },operation:Operation::Check {max_states:100,max_depth:100}}
}
fn worker() -> (&'static Path, String) {
    let path = Path::new(env!("CARGO_BIN_EXE_sail-runtime-worker"));
    (
        path,
        std::fs::read(path)
            .map(|bytes| sail_dive::sha256(&bytes))
            .unwrap_or_default(),
    )
}

#[test]
fn actual_worker_checks_model_and_binds_exact_request_and_binary() -> TestResult {
    let (worker, hash) = worker();
    let request = request();
    let result = run(
        worker,
        &hash,
        &request,
        Duration::from_secs(10),
        &AtomicBool::new(false),
    );
    let RunResult::Completed(response) = result else {
        return Err(format!("{result:?}").into());
    };
    assert_eq!(response.outcome, Outcome::Holds, "{response:?}");
    let direct = sail_dive::execute(&request);
    assert_eq!(response.completion, direct.completion);
    assert_eq!(response.requested_checks, direct.requested_checks);
    for field in [
        "verdict",
        "property",
        "statistics",
        "counterexample",
        "collision",
    ] {
        assert_eq!(
            response.native_report[field], direct.native_report[field],
            "{field}"
        );
    }
    assert_eq!(
        response.request_sha256,
        sail_dive::request_sha256(&request)?
    );
    assert!(matches!(
        run(
            worker,
            &"0".repeat(64),
            &request,
            Duration::from_secs(10),
            &AtomicBool::new(false)
        ),
        RunResult::Failed(_)
    ));
    assert!(matches!(
        run(
            Path::new("/missing/sail-runtime-worker"),
            &hash,
            &request,
            Duration::from_secs(10),
            &AtomicBool::new(false)
        ),
        RunResult::Failed(_)
    ));
    Ok(())
}

#[test]
fn isolated_counterexample_matches_direct_library_and_both_validate() -> TestResult {
    let (worker, hash) = worker();
    let mut req = request();
    req.model.files.insert(
        "M.cfg".into(),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n".into(),
    );
    let source = req.model.files.get_mut("M.tla").ok_or("source")?;
    *source = source.replace("====", "Inv == x < 2\n====");
    let direct = sail_dive::execute(&req);
    let result = run(
        worker,
        &hash,
        &req,
        Duration::from_secs(10),
        &AtomicBool::new(false),
    );
    let RunResult::Completed(isolated) = result else {
        return Err(format!("{result:?}").into());
    };
    assert_eq!(isolated.outcome, Outcome::Refuted, "{isolated:?}");
    assert_eq!(direct.outcome, Outcome::Refuted);
    assert_eq!(isolated.native_traces, direct.native_traces);
    assert_eq!(isolated.itf_traces, direct.itf_traces);
    assert_eq!(isolated.validations, direct.validations);
    assert!(isolated.validations.iter().all(|v| v["result"] == "valid"));
    Ok(())
}

#[test]
fn actual_worker_is_cancelled_during_large_assumption_binding() {
    let (worker, hash) = worker();
    let mut request = request();
    request.model.files.insert("M.tla".into(),"---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nASSUME \\A n \\in 1..1000000000: n >= 0\nInit == x = 0\nNext == x' = x\n====\n".into());
    let cancel = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(150));
            cancel.store(true, Ordering::Release);
        });
        run(worker, &hash, &request, Duration::from_secs(10), &cancel)
    });
    assert!(
        matches!(
            result,
            RunResult::Cancelled {
                worker_started: true
            }
        ),
        "{result:?}"
    );
}

fn control(marker: &str, deadline: Duration) -> RunResult {
    let path = Path::new(env!("SAIL_RUNTIME_TEST_WORKER"));
    let hash = std::fs::read(path)
        .map(|b| sail_dive::sha256(&b))
        .unwrap_or_default();
    let mut req = request();
    req.model.files.insert("M.cfg".into(), marker.into());
    run(path, &hash, &req, deadline, &AtomicBool::new(false))
}

#[test]
fn malformed_trailing_stderr_and_oversize_worker_output_never_become_verdicts() {
    for marker in [
        "SAIL_CONTROL_MALFORMED",
        "SAIL_CONTROL_TRAILING",
        "SAIL_CONTROL_STDERR",
        "SAIL_CONTROL_OVERSIZE",
    ] {
        let result = control(marker, Duration::from_secs(3));
        assert!(
            matches!(result, RunResult::Failed(_)),
            "{marker}: {result:?}"
        );
    }
}

#[test]
fn exited_leader_with_inherited_pipes_still_times_out_and_kills_owned_group() {
    let started = std::time::Instant::now();
    let result = control("SAIL_CONTROL_PIPES", Duration::from_millis(150));
    assert!(
        matches!(
            result,
            RunResult::TimedOut {
                worker_started: true
            }
        ),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn wrong_request_identity_and_missing_native_proof_are_refused() -> TestResult {
    let req = request();
    let mut response = sail_dive::execute(&req);
    response.request_sha256 = "0".repeat(64);
    assert!(sail_dive::validate_response(&req, &response).is_err());
    response.request_sha256 = sail_dive::request_sha256(&req)?;
    response.native_report = serde_json::Value::Null;
    assert!(sail_dive::validate_response(&req, &response).is_err());
    Ok(())
}
