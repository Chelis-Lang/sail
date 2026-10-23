//! Actual shared service executions using the pinned installed native worker.
#![cfg(unix)]
use sail_api::{
    runtime::{self, Input, Output, transport::Client},
    temporal::{self, Job, Model, Operation},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Service {
    root: PathBuf,
    client: Client,
    handle: sail_daemon::runtime::Handle,
}
impl Service {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from(format!(
            "/tmp/sail-native-service-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        let worker = Path::new(env!("CARGO_BIN_EXE_sail-runtime-worker"));
        let config = sail_runtime::Config {
            temporal_worker: Some(sail_runtime::temporal::Worker {
                executable: worker.into(),
                sha256: temporal::sha256(&std::fs::read(worker)?),
            }),
            ..Default::default()
        };
        let socket = runtime::transport::socket_path(&root.join("cache.sock"));
        let handle = sail_daemon::runtime::bind(
            &socket,
            &sail_daemon::runtime::Config {
                runtime: config,
                ..Default::default()
            },
        )?
        .spawn()?;
        Ok(Self {
            root,
            client: Client::new(socket),
            handle,
        })
    }
    fn execute(
        &self,
        request: temporal::Request,
    ) -> Result<temporal::Result, Box<dyn std::error::Error>> {
        self.job(Job {
            obligation_id: "orders/properties".into(),
            requirement_ids: vec!["REQ-1".into()],
            request,
            wall_time_ms: 10_000,
        })
    }
    fn job(&self, job: Job) -> Result<temporal::Result, Box<dyn std::error::Error>> {
        let Output::Temporal { result } = self
            .client
            .execute(&runtime::Request {
                version: runtime::VERSION,
                input: Input::Temporal { job: Box::new(job) },
            })?
            .output
        else {
            return Err("wrong result family".into());
        };
        Ok(*result)
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.handle.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn model() -> temporal::Request {
    temporal::Request { version: temporal::VERSION, model: Model { root: "M.tla".into(), config: "M.cfg".into(), files: BTreeMap::from([
        ("M.tla".into(), "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = IF x < 2 THEN x+1 ELSE 0\nInv == x < 2\n====\n".into()),
        ("M.cfg".into(), "INIT Init\nNEXT Next\n".into()),
    ]) }, operation: Operation::Check { max_states: 100, max_depth: 100 } }
}

#[test]
fn actual_shared_endpoint_preserves_native_proof_refutation_and_trace_validation() -> TestResult {
    let service = Service::new()?;
    let capabilities = service.client.capabilities()?;
    assert!(capabilities.temporal.as_ref().is_some_and(|c| c.installed));
    let req = model();
    let result = service.execute(req.clone())?;
    assert_eq!(result.outcome, runtime::Outcome::Completed);
    assert_eq!(result.obligation_id, "orders/properties");
    assert_eq!(result.requirement_ids, ["REQ-1"]);
    assert_eq!(result.request_sha256, temporal::request_sha256(&req)?);
    assert!(!result.cache_hit);
    let response = result.response.ok_or("missing native evidence")?;
    assert_eq!(response.outcome, temporal::Outcome::Holds);
    assert_eq!(response.completion, temporal::Completion::Safety);
    let mut changed = req.clone();
    changed.model.files.insert(
        "M.cfg".into(),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n".into(),
    );
    let failed = service
        .execute(changed.clone())?
        .response
        .ok_or("missing counterexample")?;
    assert_eq!(failed.outcome, temporal::Outcome::Refuted);
    assert_ne!(failed.native_traces, [] as [serde_json::Value; 0]);
    assert!(failed.validations.iter().all(|v| v["result"] == "valid"));
    let trace = failed.itf_traces.first().ok_or("missing ITF")?.clone();
    changed.operation = Operation::Validate {
        trace: trace.clone(),
    };
    let replay = service
        .execute(changed.clone())?
        .response
        .ok_or("missing replay")?;
    assert_eq!(replay.outcome, temporal::Outcome::Valid);
    let mut forged = trace;
    forged.states[0].insert("x".into(), serde_json::json!({"#bigint":"99"}));
    changed.operation = Operation::Validate { trace: forged };
    let replay = service
        .execute(changed)?
        .response
        .ok_or("missing forged replay")?;
    assert_eq!(replay.outcome, temporal::Outcome::Invalid);
    Ok(())
}

#[test]
fn generated_step_within_and_real_action_mutant_use_same_service() -> TestResult {
    let service = Service::new()?;
    let mut request = temporal::Request {
        version: temporal::VERSION,
        model: Model {
            root: "Signal.tla".into(),
            config: "Signal.cfg".into(),
            files: BTreeMap::from([
                (
                    "Signal.tla".into(),
                    include_str!("../../sail-dive/fixtures/oyster-step-within/Signal.tla").into(),
                ),
                (
                    "Signal.cfg".into(),
                    include_str!("../../sail-dive/fixtures/oyster-step-within/Signal.cfg").into(),
                ),
                (
                    "SignalRequirements.tla".into(),
                    include_str!(
                        "../../sail-dive/fixtures/oyster-step-within/SignalRequirements.tla"
                    )
                    .into(),
                ),
            ]),
        },
        operation: Operation::Check {
            max_states: 1000,
            max_depth: 1000,
        },
    };
    let good = service
        .execute(request.clone())?
        .response
        .ok_or("missing projected result")?;
    assert_eq!(good.outcome, temporal::Outcome::Holds);
    let properties = request
        .model
        .files
        .get_mut("SignalRequirements.tla")
        .ok_or("properties")?;
    *properties = properties.replace(
        "Always_Req_464c4f572d303031 == [][Req_464c4f572d303031]_vars",
        "Always_Req_464c4f572d303031 == [][UNCHANGED vars]_vars",
    );
    let bad = service
        .execute(request)?
        .response
        .ok_or("missing changed projected result")?;
    assert_eq!(bad.outcome, temporal::Outcome::Refuted, "{bad:?}");
    assert!(bad.validations.iter().all(|v| v["result"] == "valid"));
    Ok(())
}

#[test]
fn stopped_sampled_unsupported_and_timeout_never_become_exhaustive_success() -> TestResult {
    let service = Service::new()?;
    let mut request = model();
    request.operation = Operation::Check {
        max_states: 1,
        max_depth: 100,
    };
    let stopped = service
        .execute(request.clone())?
        .response
        .ok_or("stopped")?;
    assert_eq!(stopped.outcome, temporal::Outcome::Stopped);
    assert_eq!(stopped.completion, temporal::Completion::None);
    request.operation = Operation::Simulate {
        traces: 2,
        depth: 4,
    };
    let sampled = service
        .execute(request.clone())?
        .response
        .ok_or("sampled")?;
    assert_eq!(sampled.outcome, temporal::Outcome::Sampled);
    assert_eq!(sampled.native_report["model_proved"], false);
    request.model.files.insert(
        "M.tla".into(),
        "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nInit == x = 0\nNext == x' = x\n====\n".into(),
    );
    let unsupported = service.execute(request)?.response.ok_or("unsupported")?;
    assert_eq!(unsupported.outcome, temporal::Outcome::Unsupported);
    let timeout = service.job(Job {
        obligation_id: "deadline".into(),
        requirement_ids: vec![],
        request: model(),
        wall_time_ms: 1,
    })?;
    assert_eq!(timeout.outcome, runtime::Outcome::Timeout);
    assert!(timeout.response.is_none());
    assert!(!timeout.cache_hit);
    Ok(())
}

#[test]
fn shared_native_cancellation_reaps_execution_and_restores_control_capacity() -> TestResult {
    use std::time::{Duration, Instant};
    let service = Service::new()?;
    let mut request = model();
    request.model.files.insert("M.tla".into(), "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nASSUME \\A n \\in 1..1000000000: n >= 0\nInit == x = 0\nNext == x' = x\n====\n".into());
    let request = runtime::Request {
        version: runtime::VERSION,
        input: Input::Temporal {
            job: Box::new(Job {
                obligation_id: "cancel/native".into(),
                requirement_ids: vec![],
                request,
                wall_time_ms: 20_000,
            }),
        },
    };
    let started = Instant::now();
    let response = service
        .client
        .execute_cancellable(&request, || started.elapsed() >= Duration::from_secs(2))?;
    let Output::Temporal { result } = response.output else {
        return Err("wrong response family".into());
    };
    assert_eq!(result.outcome, runtime::Outcome::Cancelled);
    assert!(result.response.is_none());
    assert!(started.elapsed() < Duration::from_secs(10));
    let until = Instant::now() + Duration::from_secs(2);
    while service.handle.activity().load(Ordering::SeqCst) != 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(service.handle.activity().load(Ordering::SeqCst), 0);
    assert!(service.client.capabilities()?.temporal.is_some());
    Ok(())
}
