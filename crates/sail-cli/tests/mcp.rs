//! MCP's legacy compatibility path over a local duplex stream.
//! A configured older Sail, one without durable jobs, answers `verify` with
//! the schema-v1 report its `sail verify --output json` printed: recorded
//! here from the attribution fixture (`report.golden.json`, `legacy-*.json`).
//! Other commands go to this build's CLI, with a fake cargo-verus.
//! Current managed jobs are tested separately by `job_tools` and real CI acceptance.
//! No real Verus or Z3 runs here.
#![cfg(unix)]

use std::fmt::Write as _;
use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{Value, json};

struct Client {
    server: Option<JoinHandle<std::io::Result<()>>>,
    stdin: Option<UnixStream>,
    lines: Receiver<String>,
    next: i64,
}

impl Client {
    fn start(dir: &Path, env: &[(&str, PathBuf)]) -> Self {
        let root = dir.parent().unwrap_or(dir);
        let fx = fixture();
        let legacy = dir.parent().unwrap_or(dir).join("legacy-sail");
        let executable = legacy.with_file_name("real-sail");
        if !executable.is_file() {
            assert!(std::fs::copy(env!("CARGO_BIN_EXE_sail"), &executable).is_ok());
            // The replay backend never invokes a solver. This private guard
            // makes the fixture independent of workspace binary build ordering.
            write_exec(&legacy.with_file_name("sail-z3"), "#!/bin/sh\nexit 2\n");
        }
        let mut script = String::from("#!/bin/sh\n");
        for (key, value) in env {
            let _ = writeln!(script, "export {key}='{}'", value.display());
        }
        // An older Sail has no jobs. Its `verify --output json` printed one
        // report (it passed an unknown `--background` on to Cargo): the
        // recorded report for each state of the fixture project.
        let _ = write!(
            script,
            r#"if [ "$1" = jobs ]; then exit 2; fi
if [ "$1" = verify ]; then
  echo "$@" >> '{root}/sail-args.log'
  [ -f '{root}/slow' ] && {{ echo started > '{root}/slow.started'; sleep 30; echo finished > '{root}/slow.finished'; }}
  [ -f '{root}/fresh' ] && {{ cat '{fx}/legacy-fresh.json'; exit 0; }}
  [ -f '{root}/broken' ] && {{ printf 'error[E0425]: cannot find value `y` in this scope\n --> src/lib.rs:1:14\n\n' >&2; cat '{fx}/legacy-broken.json'; exit 101; }}
  cat '{fx}/report.golden.json'
  cat '{fx}/stderr.txt' >&2
  exit 1
fi
exec '{exe}' "$@"
"#,
            root = root.display(),
            fx = fx.display(),
            exe = executable.display()
        );
        write_exec(&legacy, &script);
        let (server, stdout) = UnixStream::pair().unwrap_or_else(|error| unreachable!("{error}"));
        let input = server
            .try_clone()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let stdin = stdout.try_clone().ok();
        let config = sail_mcp::Config {
            sail: legacy,
            store_dir: dir.parent().map(|root| root.join("cache/mcp")),
            cwd: dir.to_path_buf(),
        };
        let server =
            std::thread::spawn(move || sail_mcp::serve(config, BufReader::new(input), server));
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            server: Some(server),
            stdin,
            lines,
            next: 1,
        }
    }

    fn send(&mut self, message: &Value) {
        let Some(stdin) = self.stdin.as_mut() else {
            unreachable!("stdin closed")
        };
        assert!(writeln!(stdin, "{message}").is_ok());
        assert!(stdin.flush().is_ok());
    }

    fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let line = match self.lines.recv_timeout(Duration::from_secs(60)) {
                Ok(l) => l,
                Err(e) => unreachable!("no response to {method}: {e}"),
            };
            let v: Value = serde_json::from_str(&line).unwrap_or_default();
            if v["id"] == json!(id) {
                return v;
            }
        }
    }

    fn tool(&mut self, name: &str, arguments: &Value) -> Value {
        let r = self.request("tools/call", &json!({"name": name, "arguments": arguments}));
        let result = r["result"].clone();
        assert_eq!(result["resultType"], "complete", "{r}");
        // The JSON is sent once, as structuredContent, which starts with the
        // summary; the one text block is the summary.
        assert_eq!(result["content"].as_array().map(Vec::len), Some(1), "{r}");
        if result["structuredContent"].is_object() {
            assert_eq!(
                result["structuredContent"]["summary"], result["content"][0]["text"],
                "{r}"
            );
        }
        result
    }
}

impl Client {
    fn close_input(&mut self) {
        if let Some(stdin) = self.stdin.take() {
            let _ = stdin.shutdown(std::net::Shutdown::Write);
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.close_input();
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/attr")
}

fn write_exec(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    assert!(std::fs::write(path, text).is_ok());
    assert!(std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).is_ok());
}

fn text(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn handshake_then_every_tool() {
    let root = std::env::temp_dir().join(format!("sail-mcp-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let project = root.join("input");
    assert!(std::fs::create_dir_all(project.join("src")).is_ok());
    assert!(
        std::fs::write(
            project.join("Cargo.toml"),
            "[package]\nname = \"input\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[package.metadata.verus]\nverify = true\n\n[workspace]\n"
        )
        .is_ok()
    );
    assert!(std::fs::write(project.join("src/lib.rs"), "pub fn f() {}\n").is_ok());
    let args_log = root.join("args.log");
    let fx = fixture();
    write_exec(
        &root.join("cargo-verus"),
        &format!(
            r#"#!/bin/sh
echo "$@" >> '{log}'
# `fresh`: Cargo finds the crate up to date, so Verus does not run.
[ -f '{root}/fresh' ] && exit 0
# `broken`: the build fails before Verus verifies anything.
[ -f '{root}/broken' ] && {{ printf 'error[E0425]: cannot find value `y` in this scope\n --> src/lib.rs:1:14\n\n' >&2; exit 101; }}
# `slow`: a run long enough to cancel.
[ -f '{root}/slow' ] && {{ echo started > '{root}/slow.started'; sleep 30; echo finished > '{root}/slow.finished'; }}
prev=""
for a in "$@"; do
  if [ "$prev" = "--log-dir" ]; then mkdir -p "$a" && cp '{fx}'/logs/* "$a"/; fi
  prev="$a"
done
cat '{fx}/verus.json'
cat '{fx}/stderr.txt' >&2
exit 1
"#,
            log = args_log.display(),
            root = root.display(),
            fx = fx.display()
        ),
    );
    assert!(std::fs::write(root.join("z3"), "").is_ok());
    let socket = root.join("d.sock");
    let env = [
        ("SAIL_CARGO_VERUS", root.join("cargo-verus")),
        ("VERUS_Z3_PATH", root.join("z3")),
        ("SAIL_DAEMON_SOCKET", socket.clone()),
        ("SAIL_CACHE_DIR", root.join("cache")),
    ];
    let mut c = Client::start(&project, &env);

    let init = c.request("initialize", &json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25", "{init}");
    c.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let list = c.request("tools/list", &json!({}));
    let names: Vec<String> = list["result"]["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["name"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        names,
        [
            "verify",
            "counterexample",
            "cached_result",
            "fragility",
            "start_verification",
            "verification_status",
            "verification_result",
            "cancel_verification",
            "requirement_assurance",
            "verification_events",
            "explain_failure",
            "compare_verification",
            "save_baseline",
            "list_baselines",
            "get_baseline",
            "start_repair_validation",
            "runtime_capabilities",
            "runtime_execute",
        ]
    );

    // verify (the project defaults to the server's working directory).
    let v = c.tool("verify", &json!({}));
    assert_eq!(v["isError"], false, "{v}");
    let s = &v["structuredContent"];
    assert_eq!(s["report"]["schema_version"], 2);
    assert_eq!(s["stored"], true);
    assert_eq!(s["run"], 1);
    assert_eq!(s["exit_code"], 1);
    let verdict = |name: &str| {
        s["report"]["crates"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|c| c["functions"].as_array().into_iter().flatten())
            .find(|f| f["function"] == name)
            .map(|f| f["verdict"].clone())
    };
    assert_eq!(verdict("input::max"), Some(json!("verified")));
    assert_eq!(verdict("input::bad"), Some(json!("failed")));
    assert_eq!(verdict("input::arith::bv_bad"), Some(json!("failed")));
    let summary = text(&v);
    assert!(summary.contains("verified"), "{summary}");
    assert!(summary.contains("Failed: "), "{summary}");
    assert!(summary.contains("postcondition not satisfied"), "{summary}");

    // counterexample, by short name.
    let ce = c.tool("counterexample", &json!({"function": "bad"}));
    assert_eq!(ce["isError"], false, "{ce}");
    let ce = &ce["structuredContent"];
    assert_eq!(ce["function"], "input::bad");
    assert_eq!(ce["verdict"], "failed");
    let obligations = ce["obligations"].as_array().cloned().unwrap_or_default();
    // By default a model is its source-level variables (`x!` is parameter `x`).
    assert!(
        obligations
            .iter()
            .any(|o| o["counterexample"]["variables"]["x"].is_string()
                && o["counterexample"].get("model").is_none()),
        "{ce}"
    );
    // The raw model is available on request.
    let raw = c.tool(
        "counterexample",
        &json!({"function": "bad", "detail": "full"}),
    );
    assert!(
        raw["structuredContent"]["obligations"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|o| o["counterexample"]["assignments"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x["name"] == "x!"))),
        "{raw}"
    );
    assert!(
        ce["diagnostics"].as_array().is_some_and(|d| d
            .iter()
            .any(|d| d["message"] == "postcondition not satisfied")),
        "{ce}"
    );
    // Legacy source hashes omit dependencies/configuration/runtime identity.
    // ADR 0024 requires unknown coverage even when those local files match.
    assert_eq!(ce["freshness"]["status"], "unknown");

    // cached_result, by obligation id.
    let id = obligations
        .first()
        .map(|o| o["id"].clone())
        .unwrap_or_default();
    let cr = c.tool("cached_result", &json!({"obligation": id}));
    assert_eq!(cr["isError"], false, "{cr}");
    assert_eq!(cr["structuredContent"]["obligation"]["id"], id);

    // An edit makes the recorded result stale.
    assert!(std::fs::write(project.join("src/lib.rs"), "pub fn f() { }\n").is_ok());
    let cr = c.tool("cached_result", &json!({"function": "input::max"}));
    let fresh = &cr["structuredContent"]["freshness"];
    assert_eq!(fresh["status"], "stale", "{cr}");
    assert_eq!(
        fresh["changed_files"][0],
        json!({"path": "src/lib.rs", "change": "modified"})
    );
    assert_eq!(cr["structuredContent"]["verdict"], "verified");
    assert!(text(&cr).contains("earlier sources"), "{}", text(&cr));

    // A scoped verify passes the module and function to Verus.
    let v = c.tool("verify", &json!({"project_dir": project, "module": "arith", "function": "mul_pos", "verus_args": ["--rlimit", "20"]}));
    assert_eq!(v["isError"], false, "{v}");
    let logged = std::fs::read_to_string(root.join("sail-args.log")).unwrap_or_default();
    let call_args = logged.lines().last().unwrap_or_default();
    assert_eq!(
        call_args,
        "verify --output json -- --verify-only-module arith --verify-function mul_pos --rlimit 20",
        "{logged}"
    );
    let cr = c.tool("cached_result", &json!({}));
    assert_eq!(
        cr["structuredContent"]["functions"]
            .as_array()
            .map(Vec::len),
        Some(8),
        "{cr}"
    );

    // fragility runs `sail fragility --output json`. The stand-in
    // cargo-verus replays a recorded run and never starts a solver, so no
    // session is captured: a tool error from an available command.
    let f = c.tool("fragility", &json!({}));
    let fs = &f["structuredContent"];
    assert_eq!(
        (
            f["isError"].clone(),
            fs["available"].clone(),
            fs["exit_code"].clone()
        ),
        (json!(true), json!(true), json!(2)),
        "{f}"
    );
    assert_eq!(fs["sail_args"][0], "fragility", "{f}");
    assert!(fs["stderr_tail"].to_string().contains("SAIL0112"), "{f}");
    // Invalid arguments are tool errors.
    let bad = c.tool("verify", &json!({"function": "--verify-all"}));
    assert_eq!(bad["isError"], true);
    let missing = c.tool("counterexample", &json!({"function": "nope"}));
    assert_eq!(missing["structuredContent"]["error"], "not_found");

    // When Cargo skips Verus, nothing is recorded and the latest run is
    // pointed to.
    assert!(std::fs::write(root.join("fresh"), "").is_ok());
    let v = c.tool("verify", &json!({}));
    assert_eq!(v["isError"], false, "{v}");
    assert_eq!(v["structuredContent"]["stored"], false);
    assert_eq!(
        v["structuredContent"]["latest_recorded_run"]["run"], 2,
        "{v}"
    );
    assert!(text(&v).contains("Verus did not run"), "{}", text(&v));
    assert!(std::fs::remove_file(root.join("fresh")).is_ok());

    // A build failure is reported as such, not as "up to date".
    assert!(std::fs::write(root.join("broken"), "").is_ok());
    let v = c.tool("verify", &json!({}));
    assert_eq!(v["isError"], false, "{v}");
    assert_eq!(v["structuredContent"]["stored"], false);
    let said = text(&v);
    assert!(said.contains("the build failed"), "{said}");
    assert!(said.contains("cannot find value `y`"), "{said}");
    assert!(std::fs::remove_file(root.join("broken")).is_ok());

    // Cancelling a verify kills its run; the call gets no response, and the
    // server keeps serving.
    assert!(std::fs::write(root.join("slow"), "").is_ok());
    let id = c.next;
    c.next += 1;
    c.send(&json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": "verify", "arguments": {}}}));
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while !root.join("slow.started").exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the slow run never started"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    c.send(&json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": id, "reason": "test"}}));
    let pong = c.request("ping", &json!({}));
    assert!(pong["result"].is_object(), "{pong}");
    // The run is gone well before it would have finished, and was not
    // recorded.
    let cr = c.tool("cached_result", &json!({}));
    assert_eq!(
        cr["structuredContent"]["runs"]
            .as_object()
            .map(|r| r.contains_key("3")),
        Some(false),
        "{cr}"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(!root.join("slow.finished").exists());
    assert!(std::fs::remove_file(root.join("slow")).is_ok());

    // Closing stdin ends the server.
    c.close_input();
    let status = c.server.take().map(JoinHandle::join);
    assert!(matches!(status, Some(Ok(Ok(())))));

    // A new server finds the recorded results on disk.
    let mut c = Client::start(&project, &env);
    let cr = c.tool("cached_result", &json!({"function": "max"}));
    assert_eq!(cr["structuredContent"]["verdict"], "verified", "{cr}");

    let _ = Command::new("pkill")
        .args(["-f", &format!("serve --socket {}", socket.display())])
        .status();
    let _ = std::fs::remove_dir_all(&root);
}
