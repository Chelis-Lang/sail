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
    let runtime = cache.join("runtime-identities");
    assert!(std::fs::create_dir_all(&context).is_ok());
    assert!(std::fs::create_dir_all(&runtime).is_ok());
    write(
        &context.join("input-state.json"),
        &serde_json::json!({"workspace_root":project,"target_directory":cache.join("target"),"sail_cache_directory":cache,"packages":[{"id":"fixture","name":"fixture","source":null,"manifest_path":project.join("Cargo.toml"),"targets":[]}]}),
    );
    let identities = vec![(
        worker.display().to_string(),
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(&worker).unwrap_or_default())
        ),
    )];
    let sha = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&identities).unwrap_or_default())
    );
    write(&runtime.join(format!("{sha}.json")), &identities);
    write(&runtime.join("fixture-context.link"), &sha);
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
fn context_event(
    cache: &Path,
    project: &Path,
    event: &Path,
    environment: &BTreeMap<String, String>,
    observed: bool,
) {
    let snapshot =
        sail_verus::managed::refresh_context(cache, project, "fixture-context", environment)
            .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(snapshot.complete, observed);
    write(
        event,
        &serde_json::json!({"kind":"context-ready","workspace":project,"context_sha256":"fixture-context","input_revision":snapshot.revision,"complete":true,"source_observation":observed}),
    );
}

#[test]
fn remembered_context_coalesces_equal_inputs_but_not_cold_or_restored_mtime_edits() {
    let root = std::env::temp_dir().join(format!("sail-job-context-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    assert!(std::fs::create_dir_all(&root).is_ok());
    let root = root.canonicalize().unwrap_or(root);
    let (worker, environment, context, event) = prepare(&root);
    let cache = root.join("cache");
    let project = root.join("project");
    context_event(&cache, &project, &event, &environment, false);
    let socket = root.join("j.sock");
    let mut handle = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &worker)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let client = Client::new(socket);
    let cold = submit(&client, &root, &environment);
    ready(&client, &root, &cold.id);
    let duplicate = submit(&client, &root, &environment);
    assert_ne!(
        cold.id, duplicate.id,
        "missing compiler observation cannot establish input coverage"
    );
    cancel(&client, &cold.id);
    cancel(&client, &duplicate.id);
    write(
        &context.join("source-inputs.json"),
        &serde_json::json!({"paths":[],"covered":true}),
    );
    context_event(&cache, &project, &event, &environment, true);
    let warm = submit(&client, &root, &environment);
    ready(&client, &root, &warm.id);
    assert_eq!(warm.id, submit(&client, &root, &environment).id);
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
