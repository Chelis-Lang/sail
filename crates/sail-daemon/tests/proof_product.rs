//! Product evidence survives job pruning and keeps exact target/function identities.
#![cfg(unix)]
use sail_api::jobs::{Client, Lifecycle, Mode, Request, Response, Status, Submission};
use sail_api::output::{Report, TierName, VerdictName};
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};
use sail_api::proof::Target;
use sail_api::repair::{Change, Reference};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
struct Fixture {
    root: PathBuf,
    worker: PathBuf,
}
impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-proof-product-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("project"))?;
        let worker = root.join("worker");
        std::fs::write(&worker, "#!/bin/sh\ncat \"$RESULT\"\nexit \"${CODE:-0}\"\n")?;
        std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755))?;
        Ok(Self { root, worker })
    }
    fn request(&self, revision: &str) -> Submission {
        Submission {
            mode: Mode::Stock,
            project_dir: self.root.join("project"),
            args: vec![],
            force: false,
            input_revision: Some(revision.into()),
            environment: BTreeMap::from([
                ("PATH".into(), "/usr/bin:/bin".into()),
                (
                    "RESULT".into(),
                    self.root.join("result.json").display().to_string(),
                ),
            ]),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn submit(client: &Client, submission: Submission) -> Result<Status, Box<dyn std::error::Error>> {
    match client.call(&Request::Submit { submission })? {
        Response::Status(status) => Ok(status),
        response => Err(format!("unexpected response {response:?}").into()),
    }
}
fn completed(client: &Client, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match client.call(&Request::Status { id: id.into() })? {
            Response::Status(status) if status.state == Lifecycle::Completed => return Ok(()),
            Response::Status(status) if status.state.terminal() => {
                return Err(format!("unexpected terminal {status:?}").into());
            }
            _ => {}
        }
        if Instant::now() > deadline {
            return Err("job deadline".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn project() -> Result<ProjectResult, Box<dyn std::error::Error>> {
    let report: Report = serde_json::from_str(include_str!(
        "../../sail-cli/tests/fixtures/attr/report.golden.json"
    ))?;
    Ok(ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: 1,
        run_id: "establishing".into(),
        workspace: "fixture".into(),
        context_sha256: "configuration".into(),
        input_revision: "first".into(),
        currentness: Currentness::Unknown,
        exit_code: Some(0),
        targets: vec![TargetResult {
            package_id: "demo@1".into(),
            target: "same".into(),
            kind: vec!["lib".into()],
            establishing_run: "establishing".into(),
            reused: false,
            requested: true,
            complete: false,
            report,
        }],
        execution: Execution::default(),
        notes: vec![],
        error: None,
    })
}
#[test]
fn immutable_baseline_survives_pruning_and_restart_without_overwriting_provenance()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let socket = fixture.root.join("jobs.sock");
    let store = fixture.root.join("jobs");
    let mut server = sail_daemon::jobs::bind(&socket, &store, &fixture.worker)?.spawn()?;
    let client = Client::new(&socket);
    let before = project()?;
    std::fs::write(
        fixture.root.join("result.json"),
        serde_json::to_vec(&before)?,
    )?;
    let first = submit(&client, fixture.request("first"))?;
    completed(&client, &first.id)?;
    let Response::Baseline(saved) = client.call(&Request::SaveBaseline {
        id: first.id.clone(),
        name: "release".into(),
    })?
    else {
        return Err("baseline response".into());
    };
    assert_eq!(saved.result.run_id, "establishing");
    assert_eq!(saved.result.currentness, Currentness::Historical);
    assert!(
        matches!(client.call(&Request::SaveBaseline{id:first.id.clone(),name:"release".into()}),Err(error) if error.to_string().contains("immutable"))
    );
    let mut after = before.clone();
    after.input_revision = "after".into();
    after.targets[0].report.crates[0].functions[0].verdict = VerdictName::Failed;
    after.targets[0].report.crates[0].functions[0].tier = TierName::Confirmed;
    let mut last = String::new();
    for number in 0..21 {
        std::fs::write(
            fixture.root.join("result.json"),
            serde_json::to_vec(&after)?,
        )?;
        let job = submit(&client, fixture.request(&format!("revision-{number}")))?;
        completed(&client, &job.id)?;
        last = job.id;
    }
    assert!(client.call(&Request::Status { id: first.id }).is_err());
    server.shutdown();
    let mut server = sail_daemon::jobs::bind(&socket, &store, &fixture.worker)?.spawn()?;
    assert!(
        matches!(client.call(&Request::Baselines)?,Response::Baselines(names) if names==vec!["release"])
    );
    let Response::Baseline(loaded) = client.call(&Request::GetBaseline {
        name: "release".into(),
    })?
    else {
        return Err("baseline read response".into());
    };
    assert_eq!(loaded, saved);
    let Response::Comparison(comparison) = client.call(&Request::Compare {
        before: Reference::Baseline("release".into()),
        after: last.clone(),
    })?
    else {
        return Err("comparison response".into());
    };
    assert!(comparison.compatible);
    assert_eq!(comparison.before_revision, "first");
    assert_eq!(comparison.after_revision, "after");
    assert!(
        comparison
            .differences
            .iter()
            .all(|diff| diff.change != Change::Removed)
    );
    let function = before.targets[0].report.crates[0].functions[0]
        .function
        .clone();
    let target = Target {
        package_id: "demo@1".into(),
        name: "same".into(),
        kind: vec!["lib".into()],
    };
    let Response::Explanation(explanation) = client.call(&Request::Explain {
        id: last.clone(),
        target: target.clone(),
        function: function.clone(),
        obligation: None,
    })?
    else {
        return Err("explanation response".into());
    };
    assert_eq!(explanation.establishing_run, "establishing");
    assert_eq!(explanation.function.function, function);
    assert!(!explanation.diagnostic_attribution_complete);
    assert_eq!(
        explanation.run_diagnostics,
        after.targets[0].report.diagnostics
    );
    assert!(
        client
            .call(&Request::Explain {
                id: last,
                target: Target {
                    kind: vec!["bin".into()],
                    ..target
                },
                function,
                obligation: None
            })
            .is_err()
    );
    server.shutdown();
    Ok(())
}

#[test]
fn baseline_get_preserves_provisional_evidence_and_refuses_corrupt_or_oversized_records()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let socket = fixture.root.join("jobs.sock");
    let store = fixture.root.join("jobs");
    let mut project = project()?;
    project.targets[0].report.crates[0].functions[0].verdict = VerdictName::Provisional;
    project.targets[0].report.crates[0].functions[0].tier = TierName::Provisional;
    std::fs::write(
        fixture.root.join("result.json"),
        serde_json::to_vec(&project)?,
    )?;
    let mut server = sail_daemon::jobs::bind(&socket, &store, &fixture.worker)?.spawn()?;
    let client = Client::new(&socket);
    let job = submit(&client, fixture.request("recorded-revision"))?;
    completed(&client, &job.id)?;
    let Response::Baseline(saved) = client.call(&Request::SaveBaseline {
        id: job.id,
        name: "literal-name".into(),
    })?
    else {
        return Err("baseline response".into());
    };
    let Response::Baseline(loaded) = client.call(&Request::GetBaseline {
        name: "literal-name".into(),
    })?
    else {
        return Err("baseline read response".into());
    };
    assert_eq!(loaded, saved);
    assert_eq!(loaded.result.currentness, Currentness::Historical);
    assert_eq!(loaded.result.targets, project.targets);
    assert!(!loaded.result.targets[0].complete);
    assert_eq!(
        loaded.result.targets[0].report.crates[0].functions[0].verdict,
        VerdictName::Provisional
    );
    for name in ["missing", "..", "../escape"] {
        assert!(
            client
                .call(&Request::GetBaseline { name: name.into() })
                .is_err()
        );
    }
    let path = store.join("baselines/literal-name.json");
    for corrupt in [
        serde_json::json!({"version":2}),
        serde_json::json!({"name":"another"}),
        serde_json::json!({"job_id":""}),
        serde_json::json!({"result":{"currentness":"current"}}),
        serde_json::json!({"result":{"kind":"wrong"}}),
        serde_json::json!({"result":{"schema_version":2}}),
    ] {
        let mut value = serde_json::to_value(&saved)?;
        let object = corrupt.as_object().ok_or("corruption fixture")?;
        for (key, replacement) in object {
            if key == "result" {
                for (field, replacement) in replacement.as_object().ok_or("result fixture")? {
                    value[key][field] = replacement.clone();
                }
            } else {
                value[key] = replacement.clone();
            }
        }
        std::fs::write(&path, serde_json::to_vec(&value)?)?;
        let failure = client.call(&Request::GetBaseline {
            name: "literal-name".into(),
        });
        assert!(matches!(failure, Err(error) if error.to_string().contains("SAIL0905")));
    }
    let file = std::fs::File::create(&path)?;
    file.set_len(u64::try_from(sail_api::jobs::MAX_FRAME)? + 1)?;
    drop(file);
    let oversized = client.call(&Request::GetBaseline {
        name: "literal-name".into(),
    });
    server.shutdown();
    assert!(matches!(oversized, Err(error) if error.to_string().contains("SAIL0814")));
    Ok(())
}

#[test]
fn baseline_save_still_refuses_unfinished_jobs() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    std::fs::write(&fixture.worker, "#!/bin/sh\nsleep 30\n")?;
    let socket = fixture.root.join("jobs.sock");
    let store = fixture.root.join("jobs");
    let mut server = sail_daemon::jobs::bind(&socket, &store, &fixture.worker)?.spawn()?;
    let client = Client::new(&socket);
    let job = submit(&client, fixture.request("unfinished"))?;
    let saved = client.call(&Request::SaveBaseline {
        id: job.id,
        name: "unfinished".into(),
    });
    server.shutdown();
    assert!(matches!(saved, Err(error) if error.to_string().contains("SAIL0904")));
    assert!(!store.join("baselines/unfinished.json").exists());
    Ok(())
}

#[test]
fn repair_job_preserves_focus_success_full_failure_and_never_promotes_preliminary_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let socket = fixture.root.join("jobs.sock");
    let store = fixture.root.join("jobs");
    let mut full = project()?;
    full.exit_code = Some(1);
    full.targets[0].report.exit_code = Some(1);
    full.targets[0].report.summary.failed = 1;
    full.targets[0].report.crates[0].verus.success = Some(false);
    let mut focus = full.clone();
    focus.exit_code = Some(0);
    focus.targets[0].report.exit_code = Some(0);
    focus.targets[0].report.summary.failed = 0;
    focus.targets[0].report.summary.unknown = 0;
    focus.targets[0].report.crates[0].verus.success = None;
    for function in focus.targets[0]
        .report
        .crates
        .iter_mut()
        .flat_map(|report| &mut report.functions)
    {
        function.verdict = VerdictName::Provisional;
        function.tier = TierName::Provisional;
    }
    let batch = sail_api::repair::BatchResult {
        focus: Some(focus.clone()),
        full: full.clone(),
        manifest_sha256: "mock-engine-manifest".into(),
        compiler_invocations: 1,
        batched: true,
        selection_error: None,
        preparation: None,
    };
    let mut stored = sail_api::repair::ResultDocument::from(batch);
    stored.accepted = true; // Never trust a serialized accepted field.
    std::fs::write(
        fixture.root.join("result.json"),
        serde_json::to_vec(&stored)?,
    )?;
    let mut server = sail_daemon::jobs::bind(&socket, &store, &fixture.worker)?.spawn()?;
    let client = Client::new(&socket);
    let mut submission = fixture.request("repair-revision");
    submission.mode = Mode::Repair(sail_api::repair::Selection {
        package: "demo".into(),
        target: None,
        kind: "lib".into(),
        function: "demo::proof".into(),
    });
    submission.environment.insert("CODE".into(), "1".into());
    let job = submit(&client, submission)?;
    completed(&client, &job.id)?;
    let Response::Result(document) = client.call(&Request::Result { id: job.id })? else {
        return Err("repair result response".into());
    };
    assert_eq!(document.job.state, Lifecycle::Completed);
    assert_eq!(document.job.exit_code, Some(1));
    let repair = document.repair.ok_or("repair bundle missing")?;
    assert!(!repair.accepted);
    assert_eq!(repair.batch.focus, Some(focus));
    assert_eq!(repair.batch.full.targets, full.targets);
    assert_eq!(repair.batch.compiler_invocations, 1);
    assert!(repair.batch.batched);
    server.shutdown();
    Ok(())
}
