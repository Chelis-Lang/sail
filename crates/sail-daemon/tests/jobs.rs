//! Durable job lifecycle, coalescing, responsiveness and process-group cancellation.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use sail_api::jobs::{Client, Lifecycle, Request, Response, Status, Submission};

fn fixture(script: &str) -> (PathBuf, PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "sail-jobs-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(std::fs::create_dir_all(root.join("project")).is_ok());
    let executable = root.join("worker");
    assert!(std::fs::write(&executable, format!("#!/bin/sh\n{script}\n")).is_ok());
    assert!(std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).is_ok());
    let socket = root.join("j.sock");
    (root, executable, socket)
}
fn submission(root: &Path, revision: &str) -> Submission {
    Submission {
        mode: sail_api::jobs::Mode::Stock,
        project_dir: root.join("project"),
        args: vec![],
        force: false,
        environment: BTreeMap::from([("PATH".to_owned(), "/usr/bin:/bin".to_owned())]),
        input_revision: Some(revision.to_owned()),
    }
}
fn submit(client: &Client, submission: Submission) -> Status {
    match client.call(&Request::Submit { submission }) {
        Ok(Response::Status(status)) => status,
        response => unreachable!("unexpected submit: {response:?}"),
    }
}
fn status(client: &Client, id: &str) -> Status {
    match client.call(&Request::Status { id: id.to_owned() }) {
        Ok(Response::Status(status)) => status,
        response => unreachable!("unexpected status: {response:?}"),
    }
}
fn wait(client: &Client, id: &str, predicate: impl Fn(Lifecycle) -> bool) -> Status {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = status(client, id);
        if predicate(status.state) {
            return status;
        }
        assert!(Instant::now() < deadline, "timed out with {status:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
const REPORT: &str = r#"printf '%s\n' '{"kind":"sail-project-result","schema_version":1,"run_id":"fixture","workspace":"fixture","context_sha256":"fixture","input_revision":"fixture","currentness":"unknown","exit_code":0,"targets":[],"execution":{"verifier_invocations":0,"reused_targets":0,"wall_ms":0},"notes":[],"error":null}'"#;

#[test]
fn bounded_event_slices_preserve_unicode_and_wait_for_incomplete_appends() {
    let (root, executable, socket) = fixture("sleep 30");
    let mut handle = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &executable)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let client = Client::new(&socket);
    let job = submit(&client, submission(&root, "revision"));
    wait(&client, &job.id, |state| state == Lifecycle::Running);
    let events = root.join("jobs").join(&job.id).join("events.jsonl");
    let text = format!("{}é\n", "a".repeat(64 * 1024 - 1));
    assert!(std::fs::write(&events, &text).is_ok());
    let mut received = String::new();
    let mut offset = 0;
    for _ in 0..3 {
        let response = client.call(&Request::Events {
            id: job.id.clone(),
            offset,
        });
        let Ok(Response::Chunk(chunk)) = response else {
            unreachable!("no events chunk: {response:?}")
        };
        offset = chunk.next;
        received.push_str(&chunk.text);
    }
    assert_eq!(received, text);
    assert!(std::fs::write(&events, [b'a', 0xc3]).is_ok());
    let Ok(Response::Chunk(chunk)) = client.call(&Request::Events {
        id: job.id.clone(),
        offset: 0,
    }) else {
        unreachable!("no events chunk")
    };
    assert_eq!(chunk.text, "a");
    assert_eq!(chunk.next, 1);
    assert!(std::fs::write(&events, "aé").is_ok());
    let Ok(Response::Chunk(chunk)) = client.call(&Request::Events {
        id: job.id,
        offset: chunk.next,
    }) else {
        unreachable!("no events chunk")
    };
    assert_eq!(chunk.text, "é");
    handle.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn queued_requests_coalesce_and_status_never_waits_for_the_worker() {
    let (root, executable, socket) = fixture("echo waiting >&2\nsleep 30");
    let server = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &executable);
    assert!(server.is_ok());
    let mut handle = server
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let client = Client::new(&socket);
    let first = submit(&client, submission(&root, "before"));
    wait(&client, &first.id, |state| state == Lifecycle::Running);
    let coalesced = submit(&client, submission(&root, "before"));
    assert_eq!(first.id, coalesced.id);
    let next = submit(&client, submission(&root, "after"));
    assert_ne!(first.id, next.id);
    let start = Instant::now();
    assert_eq!(status(&client, &next.id).state, Lifecycle::Queued);
    assert!(
        matches!(client.call(&Request::Result { id: first.id.clone() }), Ok(Response::Result(document)) if document.result.is_none())
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(
        matches!(client.call(&Request::Cancel { id: first.id.clone(), superseded: true }), Ok(Response::Status(job)) if job.state == Lifecycle::Superseded)
    );
    wait(&client, &next.id, |state| state == Lifecycle::Running);
    assert!(
        client
            .call(&Request::Cancel {
                id: next.id,
                superseded: false
            })
            .is_ok()
    );
    handle.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn completed_reports_survive_endpoint_restart_and_retention_is_bounded() {
    let (root, executable, socket) = fixture(REPORT);
    let mut handle = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &executable)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let client = Client::new(&socket);
    let mut last = None;
    for revision in 0..22 {
        let job = submit(&client, submission(&root, &revision.to_string()));
        wait(&client, &job.id, Lifecycle::terminal);
        last = Some(job.id);
    }
    let id = last.unwrap_or_default();
    handle.shutdown();
    let mut second = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &executable)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        matches!(client.call(&Request::List { project_dir: None }), Ok(Response::List(jobs)) if jobs.len() == 20)
    );
    assert!(
        matches!(client.call(&Request::Result { id }), Ok(Response::Result(document)) if document.result.is_some())
    );
    second.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn interrupted_records_are_not_resumed_or_presented_as_completed() {
    let (root, executable, socket) = fixture(REPORT);
    let jobs = root.join("jobs");
    let directory = jobs.join("fixture");
    assert!(std::fs::create_dir_all(&directory).is_ok());
    let request = submission(&root, "before");
    let record = serde_json::json!({"status":{"version":1,"id":"fixture","project_dir":root.join("project"),"args":[],"state":"running","submitted_at_ms":1,"started_at_ms":1,"finished_at_ms":null,"exit_code":null,"input_revision":"before","currentness":"unknown","error":null},"submission":request});
    assert!(std::fs::write(directory.join("record.json"), record.to_string()).is_ok());
    let mut handle = sail_daemon::jobs::bind(&socket, &jobs, &executable)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        status(&Client::new(&socket), "fixture").state,
        Lifecycle::InfrastructureError
    );
    assert!(
        status(&Client::new(&socket), "fixture")
            .error
            .unwrap_or_default()
            .starts_with("SAIL0807")
    );
    handle.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sealed_status_matches_full_result_and_refuses_restored_metadata_tampering() {
    use sail_api::jobs::Currentness;
    for (original, expected) in [
        ("unknown", Currentness::Unknown),
        ("historical", Currentness::Historical),
        ("current", Currentness::Unknown),
    ] {
        let script = REPORT.replace(
            "\"currentness\":\"unknown\"",
            &format!("\"currentness\":\"{original}\""),
        );
        let (root, executable, socket) = fixture(&script);
        let jobs = root.join("jobs");
        let mut handle = sail_daemon::jobs::bind(&socket, &jobs, &executable)
            .and_then(sail_daemon::jobs::JobServer::spawn)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let client = Client::new(&socket);
        let job = submit(&client, submission(&root, "seal"));
        wait(&client, &job.id, |state| state == Lifecycle::Completed);
        assert_eq!(status(&client, &job.id).currentness, expected);
        let Ok(Response::Result(document)) = client.call(&Request::Result { id: job.id.clone() })
        else {
            unreachable!("completed result")
        };
        assert_eq!(document.job.currentness, expected);
        let directory = jobs.join(&job.id);
        let record_path = directory.join("record.json");
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&record_path).unwrap_or_default())
                .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert_eq!(
            record["projection"]["stdout_sha256"].as_str().map(str::len),
            Some(64)
        );
        let output = directory.join("stdout");
        let file = std::fs::File::open(&output).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let modified = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let bytes = std::fs::read_to_string(&output).unwrap_or_default();
        let changed = bytes.replacen("fixture", "changed", 1);
        assert_eq!(bytes.len(), changed.len());
        assert!(std::fs::write(&output, changed).is_ok());
        assert!(
            file.set_times(std::fs::FileTimes::new().set_modified(modified))
                .is_ok()
        );
        for request in [
            Request::Status { id: job.id.clone() },
            Request::Result { id: job.id.clone() },
        ] {
            assert!(
                matches!(client.call(&request), Err(error) if error.to_string().starts_with("SAIL0816:"))
            );
        }
        assert!(std::fs::write(&output, bytes).is_ok());
        handle.shutdown();
        // A legacy record deliberately lacks the seal: full parsing and the
        // same currentness classification remain mandatory after restart.
        assert!(
            record
                .as_object_mut()
                .is_some_and(|object| object.remove("projection").is_some())
        );
        assert!(std::fs::write(&record_path, record.to_string()).is_ok());
        let mut legacy = sail_daemon::jobs::bind(&socket, &jobs, &executable)
            .and_then(sail_daemon::jobs::JobServer::spawn)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let client = Client::new(&socket);
        assert_eq!(status(&client, &job.id).currentness, expected);
        let Ok(Response::Result(document)) = client.call(&Request::Result { id: job.id }) else {
            unreachable!("legacy completed result")
        };
        assert_eq!(document.job.currentness, expected);
        legacy.shutdown();
        let _ = std::fs::remove_dir_all(root);
    }
}
