//! CLI-style requests coalesce only with complete remembered input coverage.
#![cfg(unix)]
use sail_api::jobs::{Client, Lifecycle, Request, Response, Status, Submission};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn write(path: &Path, value: &impl serde::Serialize) {
    assert!(std::fs::write(path, serde_json::to_vec(value).unwrap_or_default()).is_ok());
}
fn prepare(root: &Path) -> (PathBuf, BTreeMap<String, String>, PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;
    let project = root.join("project");
    let cache = root.join("cache");
    let worker = root.join("worker");
    let event = root.join("ready.json");
    assert!(std::fs::create_dir_all(&project).is_ok());
    assert!(std::fs::write(project.join("Cargo.toml"), "manifest").is_ok());
    assert!(std::fs::write(project.join("source.rs"), "original").is_ok());
    assert!(std::fs::write(&worker, "#!/bin/sh\ncat \"$SAIL_TEST_CONTEXT_EVENT\" > \"$SAIL_JOB_EVENTS\"\nprintf '\\n' >> \"$SAIL_JOB_EVENTS\"\nsleep 30\n").is_ok());
    assert!(std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755)).is_ok());
    let project = project.canonicalize().unwrap_or(project);
    let workspace_sha = format!(
        "{:x}",
        Sha256::digest(project.as_os_str().as_encoded_bytes())
    );
    let context = cache
        .join("projects")
        .join(workspace_sha)
        .join("fixture-context");
    assert!(std::fs::create_dir_all(&context).is_ok());
    let environment = BTreeMap::from([
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("SAIL_CACHE_DIR".to_owned(), cache.display().to_string()),
        (
            "SAIL_TEST_CONTEXT_EVENT".to_owned(),
            event.display().to_string(),
        ),
    ]);
    (worker, environment, context, event)
}
fn submit(client: &Client, root: &Path, environment: &BTreeMap<String, String>) -> Status {
    let submission = Submission {
        mode: sail_api::jobs::Mode::Stock,
        project_dir: root.join("project"),
        args: vec![],
        force: false,
        environment: environment.clone(),
        input_revision: None,
    };
    match client.call(&Request::Submit { submission }) {
        Ok(Response::Status(status)) => status,
        other => unreachable!("{other:?}"),
    }
}
fn ready(client: &Client, root: &Path, id: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let running = matches!(client.call(&Request::Status { id:id.to_owned() }), Ok(Response::Status(status)) if status.state == Lifecycle::Running);
        let observed = std::fs::read_to_string(root.join("jobs").join(id).join("events.jsonl"))
            .is_ok_and(|s| s.ends_with('\n'));
        if running && observed {
            return;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn cancel(client: &Client, id: &str) {
    assert!(
        client
            .call(&Request::Cancel {
                id: id.to_owned(),
                superseded: false
            })
            .is_ok()
    );
}
fn await_coalescing(
    client: &Client,
    root: &Path,
    environment: &BTreeMap<String, String>,
    expected: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let duplicate = submit(client, root, environment);
        if duplicate.id == expected {
            return;
        }
        cancel(client, &duplicate.id);
        assert!(
            Instant::now() < deadline,
            "complete observed inputs must eventually coalesce"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
/// The record a Current run leaves: the digest of each input it depends on.
fn record(context: &Path, project: &Path, version: u32) -> String {
    let files: Vec<(PathBuf, Option<String>)> = ["Cargo.toml", "source.rs"]
        .iter()
        .map(|name| {
            let path = project.join(name);
            let digest = format!(
                "{:x}",
                Sha256::digest(std::fs::read(&path).unwrap_or_default())
            );
            (path, Some(digest))
        })
        .chain([(project.join(".cargo/config.toml"), None)])
        .collect();
    let revision = "fixture-revision".to_owned();
    write(
        &context.join("read-check.json"),
        &serde_json::json!({"version":version,"revision":revision,"files":files,"bindings":{}}),
    );
    revision
}
fn context_event(event: &Path, project: &Path, revision: &str) {
    write(
        event,
        &serde_json::json!({"kind":"context-ready","workspace":project,"context_sha256":"fixture-context","input_revision":revision,"complete":true,"source_observation":true}),
    );
}

#[test]
fn remembered_context_coalesces_equal_inputs_but_not_cold_or_restored_mtime_edits() {
    let root = std::env::temp_dir().join(format!("sail-job-context-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    assert!(std::fs::create_dir_all(&root).is_ok());
    let root = root.canonicalize().unwrap_or(root);
    let (worker, environment, context, event) = prepare(&root);
    let project = root.join("project");
    context_event(&event, &project, "fixture-revision");
    let socket = root.join("j.sock");
    let mut handle = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &worker)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let client = Client::new(socket);
    // Without a record of a Current run, no input revision is known.
    let cold = submit(&client, &root, &environment);
    ready(&client, &root, &cold.id);
    let started = Instant::now();
    let duplicate = submit(&client, &root, &environment);
    assert_ne!(cold.id, duplicate.id, "unrecorded inputs never coalesce");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "submission must not wait for a verification"
    );
    cancel(&client, &cold.id);
    cancel(&client, &duplicate.id);
    // Older observations cannot establish coverage under the current policy.
    record(&context, &project, 2);
    let legacy = submit(&client, &root, &environment);
    ready(&client, &root, &legacy.id);
    let legacy_duplicate = submit(&client, &root, &environment);
    assert_ne!(
        legacy.id, legacy_duplicate.id,
        "legacy inputs never coalesce"
    );
    cancel(&client, &legacy.id);
    cancel(&client, &legacy_duplicate.id);
    let revision = record(&context, &project, 3);
    let warm = submit(&client, &root, &environment);
    ready(&client, &root, &warm.id);
    await_coalescing(&client, &root, &environment, &warm.id);
    assert_eq!(warm.id, submit(&client, &root, &environment).id);
    // A lost record can no longer establish equal inputs.
    let _ = std::fs::remove_file(context.join("read-check.json"));
    let lost = submit(&client, &root, &environment);
    assert_ne!(warm.id, lost.id);
    cancel(&client, &lost.id);
    assert_eq!(record(&context, &project, 3), revision);
    await_coalescing(&client, &root, &environment, &warm.id);
    // A newly created configuration file is an input change.
    assert!(std::fs::create_dir_all(project.join(".cargo")).is_ok());
    assert!(std::fs::write(project.join(".cargo/config.toml"), "").is_ok());
    let configured = submit(&client, &root, &environment);
    assert_ne!(warm.id, configured.id);
    cancel(&client, &configured.id);
    assert!(std::fs::remove_file(project.join(".cargo/config.toml")).is_ok());
    await_coalescing(&client, &root, &environment, &warm.id);
    // Restoring the modification time does not hide a content edit.
    let source = project.join("source.rs");
    let modified = std::fs::metadata(&source).and_then(|m| m.modified()).ok();
    assert!(std::fs::write(&source, "modified").is_ok());
    if let Some(modified) = modified {
        assert!(
            std::fs::OpenOptions::new()
                .write(true)
                .open(&source)
                .and_then(|file| file.set_times(std::fs::FileTimes::new().set_modified(modified)))
                .is_ok()
        );
    }
    assert_ne!(warm.id, submit(&client, &root, &environment).id);
    handle.shutdown();
    let _ = std::fs::remove_dir_all(root);
}
