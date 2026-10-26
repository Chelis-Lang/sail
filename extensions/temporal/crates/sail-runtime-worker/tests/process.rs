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
fn action_only_requirement_model_checks_and_violations_replay_in_owned_worker() -> TestResult {
    let files = [
        (
            "Signal.tla",
            include_str!("../../sail-dive/fixtures/oyster-step-within/Signal.tla"),
        ),
        (
            "SignalRequirements.tla",
            include_str!("../../sail-dive/fixtures/oyster-step-within/SignalRequirements.tla"),
        ),
        (
            "Signal.cfg",
            include_str!("../../sail-dive/fixtures/oyster-step-within/Signal.cfg"),
        ),
    ];
    let mut request = Request {
        version: VERSION,
        model: Model {
            files: files
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            root: "Signal.tla".into(),
            config: "Signal.cfg".into(),
        },
        operation: Operation::Check {
            max_states: 100,
            max_depth: 100,
        },
    };
    let (worker, hash) = worker();
    for violating in [false, true] {
        if violating {
            let source = request
                .model
                .files
                .get_mut("SignalRequirements.tla")
                .ok_or("source")?;
            *source = source.replace(
                "Always_Req_464c4f572d303031 == [][Req_464c4f572d303031]_vars",
                "Always_Req_464c4f572d303031 == [][UNCHANGED vars]_vars",
            );
        }
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
        assert_eq!(
            response.outcome,
            if violating {
                Outcome::Refuted
            } else {
                Outcome::Holds
            },
            "{response:?}"
        );
        if violating {
            assert_eq!(
                response.native_report["verdict"],
                "action-property-violation"
            );
            assert_eq!(
                response.native_report["property"],
                "Always_Req_464c4f572d303031"
            );
            assert_eq!(
                response.itf_traces[0].ending["name"],
                "Always_Req_464c4f572d303031"
            );
            assert_eq!(
                response.itf_traces[0].ending,
                response.native_traces[0]["ending"]
            );
            assert_eq!(
                response.native_traces[0],
                response.native_report["counterexample"]["trace"]
            );
            assert!(response.validations.iter().all(|v| v["result"] == "valid"));
            assert_eq!(response.validations.len(), 1);
        } else {
            assert_eq!(response.completion, sail_dive::Completion::Safety);
            assert_eq!(response.native_report["check_kind"], "safety");
        }
    }
    Ok(())
}

#[test]
fn actual_worker_is_cancelled_after_spawn_for_large_assumption_input() -> TestResult {
    let (worker, hash) = worker();
    let mut request = request();
    request.model.files.insert("M.tla".into(),"---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nASSUME \\A n \\in 1..1000000000: n >= 0\nInit == x = 0\nNext == x' = x\n====\n".into());
    let cancel = AtomicBool::new(false);
    let (result, observed) = std::thread::scope(|scope| {
        let monitor = scope.spawn(|| {
            // Hashing and freezing the real binary can exceed a fixed sleep on
            // a busy runner. Observe this test process's actual owned worker.
            let prefix = format!("sail-runtime-image-{}-", std::process::id());
            let started = std::time::Instant::now();
            let observed = loop {
                let processes = std::process::Command::new("/bin/ps")
                    .args(["-axo", "command="])
                    .output();
                if processes.is_ok_and(|p| {
                    p.status.success()
                        && String::from_utf8_lossy(&p.stdout).lines().any(|line| {
                            line.contains(&prefix)
                                && line.trim_end().ends_with("/sail-runtime-worker")
                        })
                }) {
                    break true;
                }
                if started.elapsed() >= Duration::from_secs(5) {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            cancel.store(true, Ordering::Release);
            observed
        });
        let result = run(worker, &hash, &request, Duration::from_secs(10), &cancel);
        (result, monitor.join())
    });
    assert!(
        observed.map_err(|_| "worker observer panicked")?,
        "actual worker was not observed"
    );
    assert!(
        matches!(
            result,
            RunResult::Cancelled {
                worker_started: true
            }
        ),
        "{result:?}"
    );
    Ok(())
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
    // The wall budget also covers hashing the debug worker before it starts.
    // Leave room for that admission so this control reaches inherited pipes.
    let result = control("SAIL_CONTROL_PIPES", Duration::from_secs(2));
    assert!(
        matches!(
            result,
            RunResult::TimedOut {
                worker_started: true
            }
        ),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
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
