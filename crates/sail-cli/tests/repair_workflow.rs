//! Repair command/MCP transport against an explicitly synthetic engine worker.
#![cfg(unix)]
use sail_api::jobs::{Client, Lifecycle, Request, Response};
use sail_api::output::{EstablishmentName, Report, VerdictName};
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};
use sail_api::proof::{InputEvent, Observation, Phase, Target};
use serde_json::{Value, json};
use std::io::{BufRead as _, Write as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Fixture {
    root: PathBuf,
    socket: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Fixture {
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail"));
        cmd.args(args)
            .current_dir(self.root.join("project"))
            .env("SAIL_CACHE_DIR", self.root.join("cache"))
            .env("SAIL_DAEMON_SOCKET", &self.socket)
            .env("TEST_RESULT", self.root.join("result.json"))
            .env("TEST_CONTEXT", self.root.join("context.jsonl"))
            .env("TEST_PROOF", self.root.join("proof.jsonl"));
        cmd
    }
    fn json(&self, args: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
        let output = self.command(args).output()?;
        if !output.status.success() {
            return Err(format!(
                "command {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "One literal fixture of a submitted job and its complete project result."
)]
fn fixture() -> Result<(Fixture, sail_daemon::jobs::JobHandle, String), Box<dyn std::error::Error>>
{
    let root = PathBuf::from(format!("/tmp/sail-repair-cli-{}", std::process::id()));
    std::fs::create_dir_all(root.join("project"))?;
    let fixture = Fixture {
        socket: root.join("cache.sock"),
        root,
    };
    let mut report: Report =
        serde_json::from_str(include_str!("fixtures/attr/report.golden.json"))?;
    report.exit_code = Some(1);
    report.crates[0].functions[0].verdict = VerdictName::Failed;
    let function = report.crates[0].functions[0].function.clone();
    let target = Target {
        package_id: "mock-package".into(),
        name: "demo".into(),
        kind: vec!["lib".into()],
    };
    let full = ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: sail_api::project::SCHEMA_VERSION,
        run_id: "mock-run".into(),
        workspace: fixture.root.join("project").display().to_string(),
        context_sha256: "mock-context".into(),
        input_revision: "mock-revision".into(),
        currentness: Currentness::Unknown,
        exit_code: Some(1),
        targets: vec![TargetResult {
            dependency_store: None,
            package_id: target.package_id.clone(),
            target: target.name.clone(),
            kind: target.kind.clone(),
            establishing_run: "mock-run".into(),
            reused: false,
            requested: true,
            complete: false,
            report,
        }],
        execution: Execution::default(),
        notes: vec![
            "Synthetic backend transport test; no real engine proof or performance claim.".into(),
        ],
        error: None,
        engine: None,
    };
    let mut focus = full.clone();
    focus.exit_code = Some(0);
    for f in focus.targets[0]
        .report
        .crates
        .iter_mut()
        .flat_map(|c| &mut c.functions)
    {
        f.establishment = EstablishmentName::Provisional;
        f.evidence_tier = None;
        f.verdict = VerdictName::Provisional;
    }
    let bundle = sail_api::repair::ResultDocument::from(sail_api::repair::BatchResult {
        focus: Some(focus.clone()),
        full: Some(full),
        manifest_sha256: "mock-manifest".into(),
        compiler_invocations: 1,
        batched: true,
        selection_error: None,
        engine_fault: None,
    });
    std::fs::write(
        fixture.root.join("result.json"),
        serde_json::to_vec(&bundle)?,
    )?;
    std::fs::write(
        fixture.root.join("context.jsonl"),
        format!(
            "{}\n",
            json!({"kind":"context-ready","run_id":"mock-run","context_sha256":"mock-context","input_revision":"mock-revision","targets":[target]})
        ),
    )?;
    let input = InputEvent {
        version: 1,
        event_id: "mock-focus-observation".into(),
        run_id: "mock-run".into(),
        context_sha256: "mock-context".into(),
        input_revision: "mock-revision".into(),
        target,
        phase: Phase::Focus,
        function: Some(function.clone()),
        obligation: None,
        observation: Observation::FunctionObserved(
            focus.targets[0].report.crates[0].functions[0].clone(),
        ),
    };
    std::fs::write(
        fixture.root.join("proof.jsonl"),
        format!("{}\n", serde_json::to_string(&input)?),
    )?;
    let worker = worker(&fixture)?;
    let handle = sail_daemon::jobs::bind(
        &sail_api::jobs::socket_path(&fixture.socket),
        &fixture.root.join("jobs"),
        &worker,
    )?
    .spawn()?;
    Ok((fixture, handle, function))
}
fn worker(fixture: &Fixture) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt as _;
    let worker = fixture.root.join("mock-worker");
    std::fs::write(
        &worker,
        "#!/bin/sh\ncat \"$TEST_CONTEXT\" > \"$SAIL_JOB_EVENTS\"\ncat \"$TEST_PROOF\" > \"$SAIL_PROOF_EVENTS\"\ncat \"$TEST_RESULT\"\nexit 1\n",
    )?;
    std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755))?;
    Ok(worker)
}
fn wait(fixture: &Fixture, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new(sail_api::jobs::socket_path(&fixture.socket));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match client.call(&Request::Status { id: id.into() })? {
            Response::Status(status) if status.state == Lifecycle::Completed => return Ok(()),
            Response::Status(status) if status.state.terminal() => {
                return Err(format!("unexpected {status:?}").into());
            }
            _ => {}
        }
        if Instant::now() > deadline {
            return Err("mock worker deadline".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn installed_cli_and_mcp_share_repair_journal_explanation_and_baseline_contracts()
-> Result<(), Box<dyn std::error::Error>> {
    let (fixture, mut daemon, function) = fixture()?;
    let submitted = fixture.json(&[
        "repair-check",
        "--package",
        "demo",
        "--lib",
        "--function",
        &function,
        "--engine",
        "interactive",
        "--background",
        "--output",
        "json",
    ])?;
    let id = submitted["id"].as_str().ok_or("job id")?;
    wait(&fixture, id)?;
    let result = fixture.json(&["jobs", "result", id, "--output", "json"])?;
    assert_eq!(result["value"]["repair"]["accepted"], false);
    assert_eq!(
        result["value"]["repair"]["batch"]["compiler_invocations"],
        1
    );
    let page = fixture.json(&["jobs", "events", id, "--output", "json"])?;
    assert_eq!(page["events"][0]["input"]["phase"], "focus");
    assert_eq!(
        page["events"][0]["input"]["observation"]["value"]["establishment"],
        "provisional"
    );
    assert_eq!(page["gap"], Value::Null);
    let explained = fixture.json(&[
        "explain",
        id,
        "--package-id",
        "mock-package",
        "--target",
        "demo",
        "--kind",
        "lib",
        "--function",
        &function,
        "--output",
        "json",
    ])?;
    assert_eq!(explained["value"]["establishing_run"], "mock-run");
    fixture.json(&[
        "baseline", "save", "before", "--job", id, "--output", "json",
    ])?;
    assert_eq!(
        fixture.json(&["compare", id, "--baseline", "before", "--output", "json"])?["value"]["compatible"],
        true
    );
    check_mcp(&fixture, id, &function)?;
    daemon.shutdown();
    Ok(())
}

fn check_mcp(
    fixture: &Fixture,
    id: &str,
    function: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mcp = fixture
        .command(&["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut input = mcp.stdin.take().ok_or("MCP stdin")?;
    let mut output = std::io::BufReader::new(mcp.stdout.take().ok_or("MCP stdout")?);
    let mut call = |number: u64,
                    name: &str,
                    args: Value|
     -> Result<Value, Box<dyn std::error::Error>> {
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":number,"method":"tools/call","params":{"name":name,"arguments":args}})
        )?;
        input.flush()?;
        let mut line = String::new();
        output.read_line(&mut line)?;
        let response: Value = serde_json::from_str(&line)?;
        assert_eq!(response["id"], number);
        assert_eq!(response["result"]["isError"], false, "{response}");
        Ok(response["result"]["structuredContent"].clone())
    };
    assert_eq!(
        call(1, "verification_events", json!({"job_id":id}))?["next"]["after"],
        1
    );
    assert_eq!(
        call(
            2,
            "explain_failure",
            json!({"job_id":id,"package_id":"mock-package","target":"demo","kind":"lib","function":function})
        )?["value"]["establishing_run"],
        "mock-run"
    );
    assert_eq!(
        call(
            3,
            "compare_verification",
            json!({"after_job":id,"baseline":"before"})
        )?["value"]["compatible"],
        true
    );
    let started = call(
        4,
        "start_repair_validation",
        json!({"package":"demo","kind":"lib","function":function,"engine":"interactive"}),
    )?;
    assert!(started["id"].as_str().is_some());
    drop(call);
    drop(input);
    assert!(mcp.wait()?.success());
    Ok(())
}
