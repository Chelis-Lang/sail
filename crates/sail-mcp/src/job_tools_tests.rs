//! Durable job tool regressions with a bounded fake CLI endpoint.

use super::*;
use sail_api::jobs::{Currentness, Lifecycle, Response, ResultDocument, Status};
use sail_api::output::{Counterexample, ObligationReport, OutcomeName, VerdictName};
use sail_api::project::{Execution, ProjectResult, TargetResult};
use std::path::{Path, PathBuf};

#[test]
fn owned_result_projection_matches_typed_schema_and_moves_the_full_graph() {
    let root = Path::new("/fixture");
    let repair = sail_api::repair::ResultDocument::from(sail_api::repair::BatchResult {
        focus: None,
        full: None,
        manifest_sha256: "manifest".to_owned(),
        compiler_invocations: 1,
        batched: false,
        selection_error: None,
        engine_fault: None,
    });
    for repair in [None, Some(Box::new(repair))] {
        for result in [
            None,
            Some(json!({"model": "λ\n".repeat(400_000), "array": [null, true, 1, 1.25]})),
        ] {
            let pointer = result
                .as_ref()
                .and_then(|value| value["model"].as_str())
                .map(str::as_ptr);
            let response = Response::Result(ResultDocument {
                repair: repair.clone(),
                job: status(root, Lifecycle::Completed, "id"),
                result,
            });
            let expected = serde_json::to_value(&response).unwrap_or_default();
            let actual = response_value(response).unwrap_or_default();
            assert_eq!(actual, expected);
            assert_eq!(
                serde_json::to_vec(&actual).ok(),
                serde_json::to_vec(&expected).ok()
            );
            assert_eq!(
                actual["value"]["result"]["model"].as_str().map(str::as_ptr),
                pointer
            );
            assert_eq!(actual["value"].get("repair").is_some(), repair.is_some());
        }
    }
    let response = Response::Status(status(root, Lifecycle::Running, "id"));
    assert_eq!(
        response_value(response.clone()).ok(),
        serde_json::to_value(response).ok()
    );
}

struct Fixture {
    root: PathBuf,
    context: Context,
}
impl Fixture {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-mcp-jobs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let root = root.canonicalize().unwrap_or(root);
        let sail = root.join("sail");
        // Serialized fixtures are single-line JSON; shell builtins preserve the bytes
        // while avoiding unrelated dirname/cat subprocess scheduling in the timed path.
        let script = "#!/bin/sh\nd=${0%/*}\nrespond() { IFS= read -r response < \"$1\"; printf '%s' \"$response\"; }\ncase \"$1:$2\" in\nverify:*) respond \"$d/start.json\";;\njobs:list) respond \"$d/list.json\";;\njobs:status) respond \"$d/status.json\";;\njobs:result) if [ \"$3\" = latest ]; then respond \"$d/empty.json\"; elif [ \"$3\" = running ]; then respond \"$d/running.json\"; else respond \"$d/result.json\"; fi;;\njobs:cancel) respond \"$d/cancel.json\";;\n*) exit 2;;\nesac\n";
        assert!(std::fs::write(&sail, script).is_ok());
        assert!(std::fs::set_permissions(&sail, std::fs::Permissions::from_mode(0o755)).is_ok());
        Self {
            context: Context::new(crate::tools::Config {
                sail,
                store_dir: None,
                cwd: root.clone(),
            }),
            root,
        }
    }
    /// Open fixture command text through the installed interpreter. The timed
    /// calls still launch a real child and read fresh command/response files,
    /// without making first execution of a newly created executable part of
    /// the job-service responsiveness oracle.
    fn interpreted(mut self) -> Self {
        let script = std::fs::read_to_string(&self.context.config.sail)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let script = script.replace("d=${0%/*}", "d=.");
        for name in ["verify", "jobs"] {
            let command = format!("set -- {name} \"$@\"\n{script}");
            assert!(std::fs::write(self.root.join(name), command).is_ok());
        }
        self.context.config.sail = PathBuf::from("/bin/sh");
        self
    }

    fn write(&self, name: &str, value: &impl serde::Serialize) {
        let bytes = serde_json::to_vec(value).unwrap_or_default();
        assert_ne!(bytes.len(), 0);
        assert!(
            !bytes.contains(&b'\n'),
            "fixture responses must be single-line JSON"
        );
        assert!(std::fs::write(self.root.join(name), bytes).is_ok());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
/// A real independently blocked worker, ready before the responsiveness timer.
struct BlockedWorker(std::process::Child);

impl BlockedWorker {
    fn ready() -> Self {
        use std::io::BufRead as _;
        use std::process::{Command, Stdio};
        let child = Command::new("/bin/sh")
            .args(["-c", "printf '%s\\n' \"$$\"; read -r finish"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|error| unreachable!("cannot start blocked worker: {error}"));
        let mut worker = Self(child);
        let stdout = worker
            .0
            .stdout
            .take()
            .unwrap_or_else(|| unreachable!("worker stdout missing"));
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut ready = String::new();
            let result = std::io::BufReader::new(stdout)
                .read_line(&mut ready)
                .map(|_| ready);
            let _ = tx.send(result);
        });
        let ready = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap_or_else(|error| unreachable!("worker startup handshake failed: {error}"))
            .unwrap_or_else(|error| unreachable!("worker startup read failed: {error}"));
        assert!(reader.join().is_ok());
        assert_eq!(ready.trim().parse::<u32>().ok(), Some(worker.0.id()));
        worker.assert_running();
        worker
    }

    fn assert_running(&mut self) {
        assert!(
            matches!(self.0.try_wait(), Ok(None)),
            "owned worker {} must remain blocked",
            self.0.id()
        );
    }
}

impl Drop for BlockedWorker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn status(root: &Path, state: Lifecycle, id: &str) -> Status {
    Status {
        version: 1,
        id: id.to_owned(),
        project_dir: root.to_path_buf(),
        args: vec![],
        state,
        submitted_at_ms: if id == "latest" { 2 } else { 1 },
        started_at_ms: Some(1),
        finished_at_ms: None,
        exit_code: None,
        input_revision: Some("established-revision".to_owned()),
        currentness: Currentness::Historical,
        error: None,
    }
}
fn project(root: &Path) -> ProjectResult {
    let mut function = crate::store::tests::function("demo::failed", VerdictName::Failed);
    function.obligations = Some(vec![ObligationReport {
        id: "demo::failed#Function-Def/0".to_owned(),
        kind: "Function-Def".to_owned(),
        ordinal: 0,
        spinoff: None,
        advisory: false,
        span: None,
        outcome: OutcomeName::Failed,
        unstable: false,
        checks: vec![],
        counterexample: Some(Counterexample {
            check: 0,
            model: "(model)".to_owned(),
            assignments: vec![],
        }),
    }]);
    ProjectResult {
        kind: "sail-project-result".to_owned(),
        schema_version: sail_api::project::SCHEMA_VERSION,
        run_id: "establishing-run".to_owned(),
        workspace: root.display().to_string(),
        context_sha256: "context".to_owned(),
        input_revision: "established-revision".to_owned(),
        currentness: sail_api::project::Currentness::Historical,
        exit_code: Some(1),
        targets: vec![TargetResult {
            dependency_store: None,
            package_id: "demo".to_owned(),
            target: "demo".to_owned(),
            kind: vec!["lib".to_owned()],
            establishing_run: "establishing-run".to_owned(),
            reused: false,
            requested: true,
            complete: false,
            report: crate::store::tests::report(vec![function]),
        }],
        execution: Execution::default(),
        notes: vec![],
        error: None,
        engine: None,
        no_reuse: None,
    }
}

fn populate_job_fixture(fixture: &Fixture) {
    let running = status(&fixture.root, Lifecycle::Running, "running");
    let completed = status(&fixture.root, Lifecycle::Completed, "completed");
    let latest = status(&fixture.root, Lifecycle::Completed, "latest");
    fixture.write("start.json", &running);
    fixture.write("status.json", &Response::Status(running.clone()));
    fixture.write(
        "running.json",
        &Response::Result(ResultDocument {
            job: running.clone(),
            repair: None,
            result: None,
        }),
    );
    fixture.write(
        "cancel.json",
        &Response::Status(status(&fixture.root, Lifecycle::Cancelled, "running")),
    );
    fixture.write(
        "empty.json",
        &Response::Result(ResultDocument {
            repair: None,
            job: latest.clone(),
            result: None,
        }),
    );
    fixture.write(
        "list.json",
        &Response::List(vec![completed.clone(), running.clone(), latest]),
    );
    let document = ResultDocument {
        repair: None,
        job: completed,
        result: serde_json::to_value(project(&fixture.root)).ok(),
    };
    fixture.write("result.json", &Response::Result(document));
}

#[test]
fn async_tools_and_lookups_preserve_historical_evidence_while_another_job_runs() {
    let fixture = Fixture::new().interpreted();
    populate_job_fixture(&fixture);
    let arguments = json!({"job_id":"running"})
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut worker = BlockedWorker::ready();
    let started = std::time::Instant::now();
    let mut timings = Vec::with_capacity(7);
    let operation_started = std::time::Instant::now();
    let start = call(
        &fixture.context,
        "start_verification",
        &Map::new(),
        &Cancel::default(),
    );
    timings.push(("start_verification", operation_started.elapsed()));
    assert!(!start.is_error, "{}", start.text);
    assert_eq!(start.structured["id"], "running");
    for name in [
        "verification_status",
        "verification_result",
        "cancel_verification",
    ] {
        let operation_started = std::time::Instant::now();
        let response = call(&fixture.context, name, &arguments, &Cancel::default());
        timings.push((name, operation_started.elapsed()));
        assert!(!response.is_error, "{}", response.text);
        let value = &response.structured["value"];
        assert_eq!(value.get("job").unwrap_or(value)["id"], "running");
    }
    let request = crate::args::LookupArgs::parse(
        &json!({"function":"failed"})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        false,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    for models in [false, true] {
        let operation_started = std::time::Instant::now();
        let evidence = lookup(
            &fixture.context,
            &fixture.root,
            &request,
            models,
            &Cancel::default(),
        )
        .unwrap_or_else(|| unreachable!("completed job evidence missing"));
        timings.push((
            if models {
                "counterexample"
            } else {
                "cached_result"
            },
            operation_started.elapsed(),
        ));
        assert!(!evidence.is_error);
        assert_eq!(evidence.structured["job"]["currentness"], "historical");
        assert_eq!(
            evidence.structured["job"]["input_revision"],
            "established-revision"
        );
        assert_eq!(
            evidence.structured["functions"][0]["establishing_run"],
            "establishing-run"
        );
        if models {
            assert_eq!(
                evidence.structured["functions"][0]["counterexamples"][0]["model"],
                "(model)"
            );
        }
    }
    let missing = crate::args::LookupArgs::parse(
        &json!({"obligation":"failed#missing/0"})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        true,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let operation_started = std::time::Instant::now();
    assert!(
        lookup(
            &fixture.context,
            &fixture.root,
            &missing,
            true,
            &Cancel::default()
        )
        .is_some_and(|result| result.is_error && result.structured["error"] == "not_found")
    );
    timings.push(("missing_obligation", operation_started.elapsed()));
    assert_responsive(&mut worker, started, &timings);
    reject_report_as_job(&fixture);
}

fn assert_responsive(
    worker: &mut BlockedWorker,
    started: std::time::Instant,
    timings: &[(&str, std::time::Duration)],
) {
    worker.assert_running();
    let elapsed = started.elapsed();
    eprintln!(
        "owned worker {} remained blocked throughout job tools ({elapsed:?}); operation timings: {timings:?}",
        worker.0.id()
    );
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "job tools must return while the verifier is running"
    );
}

fn reject_report_as_job(fixture: &Fixture) {
    fixture.write("start.json", &crate::store::tests::report(vec![]));
    assert!(
        call(
            &fixture.context,
            "start_verification",
            &Map::new(),
            &Cancel::default()
        )
        .is_error,
        "an older verifier's report cannot masquerade as a durable job identity"
    );
}

fn changed_model(root: &Path) -> ProjectResult {
    let result = project(root);
    let mut changed = result;
    changed.targets[0].report.crates[0].functions[0]
        .obligations
        .as_mut()
        .unwrap_or_else(|| unreachable!("fixture obligations"))[0]
        .counterexample
        .as_mut()
        .unwrap_or_else(|| unreachable!("fixture model"))
        .model = "(model changed freshly)".into();
    changed
}

#[test]
fn persistent_service_reads_preserve_fresh_models_and_do_not_spawn_cli() {
    use sail_api::jobs::Request;
    let mut fixture = Fixture::new();
    let socket = fixture.root.join("read.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket)
        .unwrap_or_else(|error| unreachable!("{error}"));
    fixture.context.jobs = Some(crate::job_clients::ReadClients::at(socket));
    fixture.context.config.sail = fixture.root.join("must-not-spawn");
    let completed = status(&fixture.root, Lifecycle::Completed, "completed");
    let result = project(&fixture.root);
    let changed = changed_model(&fixture.root);
    let make = |value| {
        Response::Result(ResultDocument {
            job: completed.clone(),
            repair: None,
            result: serde_json::to_value(value).ok(),
        })
    };
    let replies = vec![
        Response::Status(completed.clone()),
        make(&result),
        make(&changed),
        Response::List(vec![completed.clone()]),
        make(&changed),
        Response::Status(status(&fixture.root, Lifecycle::Completed, "wrong-job")),
    ];
    let mut replies = replies.into_iter();
    let server = crate::test_io::Server::spawn(listener, replies.len(), move |stream| {
        let request: Request =
            sail_api::jobs::read_frame(stream).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(matches!(
            request,
            Request::Status { .. } | Request::Result { .. } | Request::List { .. }
        ));
        let response = replies
            .next()
            .unwrap_or_else(|| unreachable!("fixture reply"));
        assert!(sail_api::jobs::write_frame(stream, &response).is_ok());
    })
    .unwrap_or_else(|error| unreachable!("{error}"));
    let args = json!({"job_id":"completed"})
        .as_object()
        .cloned()
        .unwrap_or_default();
    let status_response = call(
        &fixture.context,
        "verification_status",
        &args,
        &Cancel::default(),
    );
    assert!(!status_response.is_error, "{}", status_response.text);
    let first = call(
        &fixture.context,
        "verification_result",
        &args,
        &Cancel::default(),
    );
    let second = call(
        &fixture.context,
        "verification_result",
        &args,
        &Cancel::default(),
    );
    assert!(!first.is_error && !second.is_error);
    assert_ne!(first.structured, second.structured);
    assert_eq!(
        second.structured["value"]["job"]["currentness"],
        "historical"
    );
    let request = crate::args::LookupArgs::parse(
        &json!({"function":"failed"})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        true,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let model = lookup(
        &fixture.context,
        &fixture.root,
        &request,
        true,
        &Cancel::default(),
    )
    .unwrap_or_else(|| unreachable!("missing model"));
    assert_eq!(
        model.structured["functions"][0]["counterexamples"][0]["model"],
        "(model changed freshly)"
    );
    let wrong = call(
        &fixture.context,
        "verification_status",
        &args,
        &Cancel::default(),
    );
    assert!(wrong.is_error);
    assert_eq!(wrong.structured["error"], "invalid_response");
    assert!(server.finish());
}

#[test]
fn cli_read_responses_require_exact_job_identity_version_and_variant() {
    let fixture = Fixture::new();
    let args = json!({"job_id":"requested"})
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut wrong_version = status(&fixture.root, Lifecycle::Completed, "requested");
    wrong_version.version = 2;
    let replies = [
        Response::Status(status(&fixture.root, Lifecycle::Completed, "other")),
        Response::Status(wrong_version),
        Response::List(vec![]),
    ];
    for response in replies {
        fixture.write("status.json", &response);
        let result = call(
            &fixture.context,
            "verification_status",
            &args,
            &Cancel::default(),
        );
        assert!(result.is_error);
        assert_eq!(result.structured["error"], "invalid_response");
    }
    fixture.write(
        "result.json",
        &Response::Result(ResultDocument {
            job: status(&fixture.root, Lifecycle::Completed, "other"),
            result: Some(json!({"complete":"foreign"})),
            repair: None,
        }),
    );
    let result = call(
        &fixture.context,
        "verification_result",
        &args,
        &Cancel::default(),
    );
    assert!(result.is_error);
    assert_eq!(result.structured["error"], "invalid_response");
}

#[test]
fn post_request_protocol_failure_never_retries_cli() {
    use std::io::Write as _;
    let mut fixture = Fixture::new();
    let socket = fixture.root.join("read.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket)
        .unwrap_or_else(|error| unreachable!("{error}"));
    fixture.context.jobs = Some(crate::job_clients::ReadClients::at(socket));
    let marker = fixture.root.join("cli-retry");
    assert!(
        std::fs::write(
            &fixture.context.config.sail,
            "#!/bin/sh\nprintf tried > \"${0%/*}/cli-retry\"\nexit 2\n"
        )
        .is_ok()
    );
    let server = crate::test_io::Server::spawn(listener, 1, move |stream| {
        let request: sail_api::jobs::Request =
            sail_api::jobs::read_frame(stream).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(matches!(request, sail_api::jobs::Request::Status { .. }));
        assert!(stream.write_all(&3_u32.to_be_bytes()).is_ok());
        assert!(stream.write_all(b"bad").is_ok());
    })
    .unwrap_or_else(|error| unreachable!("{error}"));
    let args = json!({"job_id":"completed"})
        .as_object()
        .cloned()
        .unwrap_or_default();
    let result = call(
        &fixture.context,
        "verification_status",
        &args,
        &Cancel::default(),
    );
    assert!(result.is_error);
    assert_eq!(result.structured["error"], "read_failed");
    assert!(result.text.starts_with("SAIL0813: job read failed:"));
    assert!(
        !marker.exists(),
        "no second request after receiving a frame"
    );
    assert!(server.finish());
}

#[test]
fn cancellation_interrupts_owned_read_and_does_not_retry_cli() {
    use std::io::Read as _;
    use std::sync::Arc;
    use std::time::Duration;
    let mut fixture = Fixture::new();
    let socket = fixture.root.join("read.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket)
        .unwrap_or_else(|error| unreachable!("{error}"));
    fixture.context.jobs = Some(crate::job_clients::ReadClients::at(socket));
    fixture.context.config.sail = fixture.root.join("must-not-spawn");
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let server = crate::test_io::Server::spawn(listener, 1, move |stream| {
        assert!(
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .is_ok()
        );
        let _: sail_api::jobs::Request =
            sail_api::jobs::read_frame(stream).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(ready_tx.send(()).is_ok());
        let mut byte = [0];
        let read = peer_read_with_interrupts(Duration::from_secs(2), |remaining| {
            stream.set_read_timeout(Some(remaining))?;
            stream.read(&mut byte)
        });
        assert_eq!(
            read.as_ref().ok().copied(),
            Some(0),
            "owned read must close: {read:?}"
        );
    })
    .unwrap_or_else(|error| unreachable!("{error}"));
    let cancel = Arc::new(Cancel::default());
    let worker_cancel = Arc::clone(&cancel);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let replacement = Context::new(fixture.context.config.clone());
    let context = std::mem::replace(&mut fixture.context, replacement);
    // The fixture owns the directory; move only the context into the call.
    let retire_cancel = Arc::clone(&cancel);
    let worker = crate::test_io::Thread::spawn(
        move || {
            let args = json!({"job_id":"completed"})
                .as_object()
                .cloned()
                .unwrap_or_default();
            let result = call(&context, "verification_result", &args, &worker_cancel);
            let _ = done_tx.send(result);
        },
        move || retire_cancel.cancel(),
    );
    assert!(ready_rx.recv_timeout(Duration::from_secs(2)).is_ok());
    cancel.cancel();
    let result = done_rx.recv_timeout(Duration::from_secs(1));
    assert!(result.is_ok(), "cancel must not wait for the read deadline");
    let result = result.unwrap_or_else(|error| unreachable!("{error}"));
    assert!(result.is_error);
    assert_eq!(result.structured["error"], "cancelled");
    assert!(worker.finish());
    assert!(server.finish());
}

// A syscall interruption is not EOF or a second RPC. Each attempt uses the
// remaining SAME original peer budget; only Interrupted can be retried.
fn peer_read_with_interrupts(
    budget: std::time::Duration,
    mut read: impl FnMut(std::time::Duration) -> std::io::Result<usize>,
) -> std::io::Result<usize> {
    let started = std::time::Instant::now();
    loop {
        let remaining = budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        let result = read(remaining);
        if started.elapsed() >= budget {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        match result {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}

#[test]
fn peer_interrupt_control_still_requires_eof_and_original_deadline() {
    use std::io::ErrorKind;
    use std::time::Duration;
    let mut reads = 0;
    assert_eq!(
        peer_read_with_interrupts(Duration::from_secs(2), |_| {
            reads += 1;
            if reads == 1 {
                Err(ErrorKind::Interrupted.into())
            } else {
                Ok(0)
            }
        })
        .ok(),
        Some(0)
    );
    assert_eq!(reads, 2);
    for value in [
        Ok(1),
        Err(ErrorKind::TimedOut.into()),
        Err(ErrorKind::ConnectionReset.into()),
    ] {
        let mut value = Some(value);
        let read = peer_read_with_interrupts(Duration::from_secs(2), |_| {
            value
                .take()
                .unwrap_or_else(|| unreachable!("nonInterrupted must not retry"))
        });
        assert_ne!(read.ok(), Some(0));
        assert!(value.is_none());
    }
    let expired = peer_read_with_interrupts(Duration::ZERO, |_| {
        unreachable!("expired peer must not read")
    });
    assert_eq!(
        expired.err().map(|error| error.kind()),
        Some(ErrorKind::TimedOut)
    );
    let mut first = true;
    let interrupted = peer_read_with_interrupts(Duration::from_secs(2), |_| {
        let previous = first;
        first = false;
        if previous {
            Err(ErrorKind::Interrupted.into())
        } else {
            Err(ErrorKind::TimedOut.into())
        }
    });
    assert_eq!(
        interrupted.err().map(|error| error.kind()),
        Some(ErrorKind::TimedOut)
    );
}

#[test]
fn missing_endpoint_uses_original_cli_read_without_caching_authority() {
    let mut fixture = Fixture::new();
    fixture.context.jobs = Some(crate::job_clients::ReadClients::at(
        fixture.root.join("missing.sock"),
    ));
    let completed = status(&fixture.root, Lifecycle::Completed, "completed");
    fixture.write("status.json", &Response::Status(completed));
    let args = json!({"job_id":"completed"})
        .as_object()
        .cloned()
        .unwrap_or_default();
    let result = call(
        &fixture.context,
        "verification_status",
        &args,
        &Cancel::default(),
    );
    assert!(!result.is_error, "{}", result.text);
    assert_eq!(result.structured["value"]["id"], "completed");
    fixture.write(
        "status.json",
        &Response::Status(status(&fixture.root, Lifecycle::Completed, "foreign")),
    );
    let changed = call(
        &fixture.context,
        "verification_status",
        &args,
        &Cancel::default(),
    );
    assert!(changed.is_error);
    assert_eq!(changed.structured["error"], "invalid_response");
}

#[test]
fn pre_cancelled_read_never_dials() {
    let mut fixture = Fixture::new();
    fixture.context.jobs = Some(crate::job_clients::ReadClients::at(
        fixture.root.join("must-not-dial.sock"),
    ));
    let args = json!({"job_id":"completed"})
        .as_object()
        .cloned()
        .unwrap_or_default();
    let cancel = Cancel::default();
    cancel.cancel();
    let result = call(&fixture.context, "verification_status", &args, &cancel);
    assert!(result.is_error);
    assert_eq!(result.structured["error"], "cancelled");
    assert_eq!(result.text, "SAIL0813: job read cancelled");
}

#[test]
fn blocking_verify_returns_reused_requested_reports_with_their_establishing_run() {
    let root = std::env::temp_dir();
    let mut result = project(&root);
    let mut dependency = result.targets[0].clone();
    dependency.target = "vstd".to_owned();
    dependency.requested = false;
    dependency.reused = true;
    result.targets[0].reused = true;
    result.targets[0].complete = true;
    result.targets.push(dependency);
    let established = result.targets[0].report.clone();
    let merged = crate::job_tools::merged_report(&result);
    assert!(
        merged
            .as_ref()
            .is_some_and(|report| report.crates == established.crates),
        "a reused requested target keeps its establishing report"
    );
    assert_eq!(
        crate::job_tools::reuse_lines(&result),
        [
            "demo: reused the complete report established by run establishing-run for unchanged inputs."
        ]
    );
}
