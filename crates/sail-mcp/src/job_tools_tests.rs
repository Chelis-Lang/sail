//! Durable job tool regressions with a bounded fake CLI endpoint.

use super::*;
use sail_api::jobs::{Currentness, Lifecycle, Response, ResultDocument, Status};
use sail_api::output::{Counterexample, ObligationReport, OutcomeName, VerdictName};
use sail_api::project::{Execution, ProjectResult, TargetResult};
use std::path::{Path, PathBuf};

struct Fixture {
    root: PathBuf,
    context: Context,
}
impl Fixture {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let root = std::env::temp_dir().join(format!("sail-mcp-jobs-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let root = root.canonicalize().unwrap_or(root);
        let sail = root.join("sail");
        // Serialized fixtures are single-line JSON; shell builtins preserve the bytes
        // while avoiding unrelated dirname/cat subprocess scheduling in the timed path.
        let script = "#!/bin/sh\nd=${0%/*}\nrespond() { IFS= read -r response < \"$1\"; printf '%s' \"$response\"; }\ncase \"$1:$2\" in\nverify:*) respond \"$d/start.json\";;\njobs:list) respond \"$d/list.json\";;\njobs:status) respond \"$d/status.json\";;\njobs:result) if [ \"$3\" = latest ]; then respond \"$d/empty.json\"; else respond \"$d/result.json\"; fi;;\njobs:cancel) respond \"$d/cancel.json\";;\n*) exit 2;;\nesac\n";
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
        schema_version: 1,
        run_id: "establishing-run".to_owned(),
        workspace: root.display().to_string(),
        context_sha256: "context".to_owned(),
        input_revision: "established-revision".to_owned(),
        currentness: sail_api::project::Currentness::Historical,
        exit_code: Some(1),
        targets: vec![TargetResult {
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
    }
}

#[test]
fn async_tools_and_lookups_preserve_historical_evidence_while_another_job_runs() {
    let fixture = Fixture::new();
    let running = status(&fixture.root, Lifecycle::Running, "running");
    let completed = status(&fixture.root, Lifecycle::Completed, "completed");
    let latest = status(&fixture.root, Lifecycle::Completed, "latest");
    fixture.write("start.json", &running);
    fixture.write("status.json", &Response::Status(running.clone()));
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
    let arguments = json!({"job_id":"running"})
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut worker = BlockedWorker::ready();
    let started = std::time::Instant::now();
    let start = call(
        &fixture.context,
        "start_verification",
        &Map::new(),
        &Cancel::default(),
    );
    assert!(!start.is_error, "{}", start.text);
    assert_eq!(start.structured["id"], "running");
    for name in [
        "verification_status",
        "verification_result",
        "cancel_verification",
    ] {
        let response = call(&fixture.context, name, &arguments, &Cancel::default());
        assert!(!response.is_error, "{}", response.text);
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
        let evidence = lookup(&fixture.context, &fixture.root, &request, models)
            .unwrap_or_else(|| unreachable!("completed job evidence missing"));
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
    assert!(
        lookup(&fixture.context, &fixture.root, &missing, true)
            .is_some_and(|result| result.is_error && result.structured["error"] == "not_found")
    );
    assert_responsive(&mut worker, started);
    reject_report_as_job(&fixture);
}

fn assert_responsive(worker: &mut BlockedWorker, started: std::time::Instant) {
    worker.assert_running();
    let elapsed = started.elapsed();
    eprintln!(
        "owned worker {} remained blocked throughout job tools ({elapsed:?})",
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
