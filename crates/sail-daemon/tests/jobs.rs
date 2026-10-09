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
