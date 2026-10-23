//! Real CLI, stdio MCP and shared-daemon transport; no fabricated solver answers.
#![cfg(unix)]

use sail_api::runtime::{Answer, Outcome, Output, Response, transport};
use serde_json::{Value, json};
use std::{
    io::{BufRead as _, BufReader, Write as _},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, channel},
    },
    time::{Duration, Instant},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Fixture {
    root: PathBuf,
    child: Child,
    input: Option<ChildStdin>,
    lines: Receiver<String>,
    next: u64,
}
impl Fixture {
    fn start(z3: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from("/tmp").join(format!(
            "sail-rt-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        let solver = root.join("engine/verus/source/target-verus/release/z3");
        if let Some(z3) = z3 {
            std::fs::create_dir_all(solver.parent().ok_or("solver parent")?)?;
            std::os::unix::fs::symlink(z3, &solver)?;
        }
        let mut child = Self::command_at(&root)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child.stdin.take();
        let stdout = child.stdout.take().ok_or("MCP stdout")?;
        let (send, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let mut fixture = Self {
            root,
            child,
            input,
            lines,
            next: 1,
        };
        fixture.call("initialize", &json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"runtime-control","version":"1"}}))?;
        Ok(fixture)
    }
    fn command_at(root: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sail"));
        command
            .current_dir(root)
            .env("SAIL_CACHE_DIR", root.join("cache"))
            .env("SAIL_DAEMON_SOCKET", root.join("cache.sock"))
            .env("SAIL_VERUS_BUILD", root.join("engine"));
        command
    }
    fn call(&mut self, method: &str, params: &Value) -> Result<Value, Box<dyn std::error::Error>> {
        let id = self.next;
        self.next += 1;
        let input = self.input.as_mut().ok_or("MCP input")?;
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )?;
        input.flush()?;
        loop {
            let line = self.lines.recv_timeout(Duration::from_secs(10))?;
            let message: Value = serde_json::from_str(&line)?;
            if message["id"] == id {
                return Ok(message);
            }
        }
    }
    fn tool(&mut self, name: &str, arguments: &Value) -> Result<Value, Box<dyn std::error::Error>> {
        Ok(self.call("tools/call", &json!({"name":name,"arguments":arguments}))?["result"].clone())
    }
    fn execute(&self, request: &Value) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        let file = self.root.join("query.json");
        std::fs::write(&file, serde_json::to_vec(request)?)?;
        Ok(Self::command_at(&self.root)
            .args(["runtime", "execute", "--socket"])
            .arg(self.root.join("cache.sock"))
            .arg("--file")
            .arg(file)
            .output()?)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let socket = self.root.join("cache.sock");
        if let Ok(mut client) =
            sail_api::client::Client::connect_with_timeout(&socket, Duration::from_millis(200))
            && let Ok(info) = client.info()
        {
            let _ = Command::new("kill")
                .args(["-s", "TERM", "--", &info.pid.to_string()])
                .status();
            let until = Instant::now() + Duration::from_secs(2);
            while socket.exists() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn request(script: &str) -> Value {
    json!({"version":1,"input":{"kind":"smt","queries":[{"obligation_id":"consistency","requirement_ids":["REQ-1"],"backend":"z3","script":script,"limits":{"output_bytes":4096,"input_bytes":4096,"wall_time_ms":3000,"resource_limit":2_000_000}}]}})
}

#[test]
fn mcp_autostarts_real_daemon_and_cli_uses_same_unavailable_endpoint() -> TestResult {
    let mut fixture = Fixture::start(None)?;
    let tools = fixture.call("tools/list", &json!({}))?;
    let names = tools["result"]["tools"].as_array().ok_or("tools")?;
    for name in ["runtime_capabilities", "runtime_execute"] {
        assert!(names.iter().any(|tool| tool["name"] == name));
    }
    assert!(!transport::socket_path(&fixture.root.join("cache.sock")).exists());
    let capabilities = fixture.tool("runtime_capabilities", &json!({}))?;
    assert_eq!(capabilities["isError"], false);
    assert_eq!(
        capabilities["structuredContent"]["backends"][0]["installed"],
        false
    );
    assert!(transport::socket_path(&fixture.root.join("cache.sock")).exists());
    let payload = request("(check-sat)");
    let mcp = fixture.tool("runtime_execute", &json!({"request":payload}))?;
    assert_eq!(mcp["isError"], true);
    let mcp_result = &mcp["structuredContent"]["output"]["results"][0];
    assert_eq!(mcp_result["outcome"], "unavailable");
    assert_eq!(mcp_result["checks"], json!([]));
    let cli = fixture.execute(&payload)?;
    assert_eq!(
        cli.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let cli: Value = serde_json::from_slice(&cli.stdout)?;
    assert_eq!(&cli["output"]["results"][0], mcp_result);
    let refused = fixture.tool(
        "runtime_execute",
        &json!({"request":request("(echo \"unsat\") (check-sat)")}),
    )?;
    assert_eq!(refused["isError"], true);
    assert!(refused["structuredContent"]["error"].is_string());
    Ok(())
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn actual_pinned_mcp_and_cli_share_exact_solver_evidence_and_cache() -> TestResult {
    let z3 = PathBuf::from(std::env::var("SAIL_TEST_Z3")?);
    let mut fixture = Fixture::start(Some(&z3))?;
    let payload = request(
        "(set-option :produce-models true) (declare-const x Int) (assert (> x 2)) (check-sat) (get-value (x))",
    );
    let mcp = fixture.tool("runtime_execute", &json!({"request":payload}))?;
    assert_eq!(mcp["isError"], false, "{mcp}");
    let mut original = mcp["structuredContent"].clone();
    original
        .as_object_mut()
        .ok_or("response")?
        .remove("summary");
    let original: Response = serde_json::from_value(original)?;
    let Output::Smt { results: first } = original.output else {
        return Err("expected SMT family".into());
    };
    assert_eq!(first[0].outcome, Outcome::Completed);
    assert_eq!(first[0].checks[0].answer, Answer::Sat);
    assert_ne!(first[0].checks[0].products, []);
    assert!(!first[0].cache_hit);
    let cli = fixture.execute(&payload)?;
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let response: Response = serde_json::from_slice(&cli.stdout)?;
    let Output::Smt { results } = response.output else {
        return Err("expected SMT family".into());
    };
    assert!(results[0].cache_hit);
    assert_eq!(results[0].stdout, first[0].stdout);
    assert_eq!(results[0].checks, first[0].checks);
    assert_eq!(results[0].solver, first[0].solver);
    Ok(())
}
