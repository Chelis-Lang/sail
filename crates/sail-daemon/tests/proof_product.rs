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
        std::fs::write(&worker, "#!/bin/sh\ncat \"$RESULT\"\n")?;
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
