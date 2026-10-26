//! Jobs, `watch` and MCP verify setups that managed verification cannot bind
//! to a proof receipt (`SAIL3112`, ADR 0046) as plain Cargo-Verus verifies
//! them, as `sail verify` does, and mark the result `no_reuse` (#165).
//! A fake cargo-verus replays the attribution fixture; no Verus or Z3 runs.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use sail_api::jobs::{Client, Lifecycle, Request, Response, Submission};
use sail_api::project::{Currentness, ProjectResult, VerifiedAs};
use serde_json::{Value, json};

/// The two unbound setups every path is tested with: a test target selection
/// and an explicit target directory inside the project.
const SETUPS: [&[&str]; 2] = [&["--tests"], &["--target-dir", "out"]];

const LOADERS: [&str; 5] = [
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
];

struct Fixture {
    root: PathBuf,
    project: PathBuf,
    sail: PathBuf,
    args_log: PathBuf,
    cache_socket: PathBuf,
}

fn write_exec(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    assert!(std::fs::write(path, text).is_ok());
    assert!(std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).is_ok());
}

impl Fixture {
    fn new(tag: &str) -> Self {
        // macOS's per-user TMPDIR can exceed the Unix socket limit once the
        // versioned job endpoint name is appended: use the short /tmp root.
        let root = Path::new("/tmp").join(format!("sail-nr-{tag}-{}", std::process::id()));
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
        let bin = root.join("bin");
        assert!(std::fs::create_dir_all(&bin).is_ok());
        let sail = bin.join("sail");
        assert!(std::fs::copy(env!("CARGO_BIN_EXE_sail"), &sail).is_ok());
        // Setup only checks that the shim exists; the fake Verus never runs it.
        write_exec(&bin.join("sail-z3"), "#!/bin/sh\nexit 2\n");
        assert!(std::fs::write(root.join("z3"), "").is_ok());
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/attr");
        let args_log = root.join("args.log");
        // Cargo-Verus: records its arguments, writes into an explicit target
        // directory (as Cargo would), copies the recorded attribution logs and
        // prints the recorded Verus result: two failed functions, exit 1.
        write_exec(
            &root.join("cargo-verus"),
            &format!(
                r#"#!/bin/sh
echo "$@" >> '{log}'
prev=""
for a in "$@"; do
  if [ "$prev" = "--target-dir" ]; then mkdir -p "$a" && date > "$a/built"; fi
  if [ "$prev" = "--log-dir" ]; then mkdir -p "$a" && cp '{fx}'/logs/* "$a"/; fi
  prev="$a"
done
cat '{fx}/verus.json'
cat '{fx}/stderr.txt' >&2
exit 1
"#,
                log = args_log.display(),
                fx = fixture.display()
            ),
        );
        let cache_socket = root.join("d.sock");
        Self {
            root,
            project,
            sail,
            args_log,
            cache_socket,
        }
    }

    /// The environment every Sail process of the fixture runs with.
    fn environment(&self) -> BTreeMap<String, String> {
        let mut environment = BTreeMap::new();
        for key in ["PATH", "HOME", "CARGO", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"] {
            if let Ok(value) = std::env::var(key) {
                environment.insert(key.to_owned(), value);
            }
        }
        for (key, value) in [
            ("SAIL_CARGO_VERUS", self.root.join("cargo-verus")),
            ("VERUS_Z3_PATH", self.root.join("z3")),
            ("SAIL_CACHE_DIR", self.root.join("cache")),
            ("SAIL_DAEMON_SOCKET", self.cache_socket.clone()),
        ] {
            environment.insert(key.to_owned(), value.to_string_lossy().into_owned());
        }
        environment
    }

    /// An in-process job endpoint whose worker is this build's `sail`. The
    /// worker gets no cache daemon (an over-long socket path), so it never
    /// starts one; jobs reach the endpoint through `cache_socket`.
    fn jobs(&self) -> sail_daemon::jobs::JobHandle {
        let worker = self.root.join("bin/worker");
        write_exec(
            &worker,
            &format!(
                "#!/bin/sh\nSAIL_DAEMON_SOCKET='{}' exec '{}' \"$@\"\n",
                self.root.join("d".repeat(120)).display(),
                self.sail.display()
            ),
        );
        let socket = sail_api::jobs::socket_path(&self.cache_socket);
        sail_daemon::jobs::bind(&socket, &self.root.join("jobs"), &worker)
            .and_then(sail_daemon::jobs::JobServer::spawn)
            .unwrap_or_else(|error| unreachable!("{error}"))
    }

    fn sail(&self) -> Command {
        let mut command = Command::new(&self.sail);
        command
            .current_dir(&self.project)
            .env_clear()
            .envs(self.environment());
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn wait_for(what: &str, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !predicate() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// What every path must report for an unbound setup: verified once as plain
/// Cargo-Verus verifies it, with the setup's arguments, the recorded
/// verdicts and exit code, nothing reused and never Current.
fn assert_plain(result: &ProjectResult, setup: &[&str]) {
    let no_reuse = result
        .no_reuse
        .as_ref()
        .unwrap_or_else(|| unreachable!("no `no_reuse`: {result:?}"));
    assert_eq!(no_reuse.code, "SAIL3112");
    assert_eq!(no_reuse.verified_as, VerifiedAs::PlainCargoVerus);
    assert!(no_reuse.reason.contains(setup[0]), "{}", no_reuse.reason);
    assert_eq!(result.currentness, Currentness::Unknown);
    assert!(result.context_sha256.is_empty() && result.input_revision.is_empty());
    assert_eq!(result.exit_code, Some(1));
    assert!(result.error.is_none(), "{:?}", result.error);
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.starts_with("SAIL3112: "))
    );
    assert_eq!(result.execution.reused_targets, 0);
    let [target] = result.targets.as_slice() else {
        unreachable!("one target: {:?}", result.targets)
    };
    assert_eq!(target.target, sail_api::project::PLAIN_TARGET);
    assert!(target.requested && !target.reused && !target.complete);
    assert_eq!(target.report.summary.functions, 8);
    assert_eq!(target.report.summary.failed, 2);
    assert_eq!(target.report.exit_code, Some(1));
}

/// The last fake Cargo-Verus invocation carried the setup's arguments.
fn assert_ran_with(fixture: &Fixture, setup: &[&str]) {
    let log = std::fs::read_to_string(&fixture.args_log).unwrap_or_default();
    let last = log.lines().last().unwrap_or_default();
    assert!(
        last.starts_with(&format!("verus verify {}", setup.join(" "))),
        "{log}"
    );
}

/// Human `sail verify` prints exactly Cargo-Verus's stderr: Sail's notes
/// (`SAIL3112` here, and `SAIL0003` for the unavailable cache daemon) are
/// added only with `SAIL_EXPLAIN=1`.
#[test]
fn human_output_has_sail_notes_only_when_explained() {
    let fixture = Fixture::new("human");
    let expected = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/attr/stderr.txt"),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let run = |explain: bool| {
        let mut command = fixture.sail();
        // Too long for a Unix socket: the daemon is unavailable and never started.
        command
            .args(["verify", "--tests"])
            .env("SAIL_DAEMON_SOCKET", fixture.root.join("d".repeat(120)));
        if explain {
            command.env("SAIL_EXPLAIN", "1");
        }
        command
            .output()
            .unwrap_or_else(|error| unreachable!("{error}"))
    };
    // The fixture's fake Cargo-Verus is a development override, announced by
    // its own `warning[SAIL0108]` line; that is not default mode.
    let without_override = |bytes: &[u8]| -> String {
        String::from_utf8_lossy(bytes)
            .split_inclusive('\n')
            .filter(|line| !line.starts_with("warning[SAIL0108]: "))
            .collect()
    };
    let plain = run(false);
    assert_eq!(plain.status.code(), Some(1));
    assert_eq!(without_override(&plain.stderr), expected);
    assert_ran_with(&fixture, &["--tests"]);
    let explained = run(true);
    assert_eq!(explained.status.code(), Some(1));
    let text = without_override(&explained.stderr);
    assert!(text.contains("note[SAIL3112]: "), "{text}");
    assert!(text.contains("warning[SAIL0003]: "), "{text}");
    assert!(text.ends_with(&expected), "{text}");
    assert_eq!(explained.stdout, plain.stdout);
}

#[test]
fn jobs_verify_unbound_setups_as_plain_cargo_verus() {
    let fixture = Fixture::new("jobs");
    let mut daemon = fixture.jobs();
    let client = Client::new(sail_api::jobs::socket_path(&fixture.cache_socket));
    let baseline = "tests-without-reuse";
    for setup in SETUPS {
        let submission = Submission {
            mode: sail_api::jobs::Mode::Stock,
            project_dir: fixture.project.clone(),
            args: setup.iter().map(|arg| (*arg).to_owned()).collect(),
            force: false,
            environment: fixture.environment(),
            input_revision: None,
        };
        let Ok(Response::Status(status)) = client.call(&Request::Submit { submission }) else {
            unreachable!("no submit response")
        };
        let mut document = None;
        wait_for("the job's result", || {
            match client.call(&Request::Result {
                id: status.id.clone(),
            }) {
                Ok(Response::Result(result)) if result.job.state.terminal() => {
                    document = Some(result);
                    true
                }
                _ => false,
            }
        });
        let Some(document) = document else {
            unreachable!()
        };
        assert_eq!(
            document.job.state,
            Lifecycle::Completed,
            "{:?}",
            document.job.error
        );
        assert_eq!(document.job.exit_code, Some(1));
        assert_eq!(
            document.job.currentness,
            sail_api::jobs::Currentness::Unknown
        );
        let result: ProjectResult = document
            .result
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_else(|| unreachable!("no project result"));
        assert_plain(&result, setup);
        assert_ran_with(&fixture, setup);
        // The job's diagnostics carry the note and Cargo-Verus's stderr.
        let Ok(Response::Chunk(logs)) = client.call(&Request::Logs {
            id: status.id.clone(),
            offset: 0,
        }) else {
            unreachable!("no logs")
        };
        assert!(logs.text.contains("note[SAIL3112]: "), "{}", logs.text);
        assert!(logs.text.contains("postcondition not satisfied"));
        // Read again, it is still never Current.
        let Ok(Response::Status(again)) = client.call(&Request::Status {
            id: status.id.clone(),
        }) else {
            unreachable!("no status")
        };
        assert_eq!(again.currentness, sail_api::jobs::Currentness::Unknown);
        // The human result names the missing reuse.
        let human = fixture
            .sail()
            .args(["jobs", "result", &status.id])
            .output()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let text = String::from_utf8_lossy(&human.stdout);
        assert!(
            text.contains("no proof reuse (SAIL3112)")
                && text.contains("observed result (no whole-target proof authority)")
                && !text.contains("verified now"),
            "{text}"
        );
        if setup == SETUPS[0] {
            let Ok(Response::Baseline(saved)) = client.call(&Request::SaveBaseline {
                id: status.id.clone(),
                name: baseline.to_owned(),
            }) else {
                unreachable!("no saved baseline")
            };
            assert_eq!(saved.result.currentness, Currentness::Historical);
            assert!(saved.result.no_reuse.is_some());
        } else {
            // Two different unbound setups share the synthetic target name
            // and empty context. Saving the earlier result must not make
            // those empty bindings authorize a proof comparison.
            let Ok(Response::Comparison(comparison)) = client.call(&Request::Compare {
                before: sail_api::evidence::Reference::Baseline(baseline.to_owned()),
                after: status.id.clone(),
            }) else {
                unreachable!("no baseline comparison")
            };
            assert!(!comparison.compatible);
            assert_ne!(comparison.differences, []);
            assert!(comparison.differences.iter().all(|difference| {
                difference.change == sail_api::evidence::Change::Incomparable
            }));
        }
    }
    daemon.shutdown();
}

#[test]
fn watch_verifies_unbound_setups_as_plain_cargo_verus() {
    let fixture = Fixture::new("watch");
    let mut daemon = fixture.jobs();
    for (index, setup) in SETUPS.iter().enumerate() {
        let events = fixture.root.join(format!("events-{index}.jsonl"));
        let mut command = fixture.sail();
        command
            .args(["watch", "--output", "jsonl"])
            .args(*setup)
            // Scope Cargo's own output directory, as the watch test does.
            .env("CARGO_TARGET_DIR", fixture.project.join("target"))
            .stdin(Stdio::null())
            .stdout(std::fs::File::create(&events).map_or_else(|_| Stdio::null(), Stdio::from))
            .stderr(
                std::fs::File::create(fixture.root.join(format!("watch-{index}.err")))
                    .map_or_else(|_| Stdio::null(), Stdio::from),
            );
        for loader in LOADERS {
            command.env_remove(loader);
        }
        let mut watch = command
            .spawn()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let lines = || -> Vec<Value> {
            std::fs::read_to_string(&events)
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        };
        wait_for("the no-reuse event", || {
            lines().iter().any(|event| event["event"] == "no-reuse")
        });
        // The explicit target directory is Cargo's output, not an input: the
        // run that wrote it does not start another.
        std::thread::sleep(Duration::from_millis(1200));
        let _ = Command::new("kill")
            .args(["-INT", &watch.id().to_string()])
            .status();
        wait_for("watch exit", || matches!(watch.try_wait(), Ok(Some(_))));
        let lines = lines();
        let queued = lines.iter().filter(|event| event["event"] == "queued");
        assert_eq!(queued.count(), 1, "{lines:?}");
        let event = lines
            .iter()
            .find(|event| event["event"] == "no-reuse")
            .cloned()
            .unwrap_or_default();
        assert_eq!(event["no_reuse"]["verified_as"], "plain-cargo-verus");
        assert_eq!(event["currentness"], "unknown");
        assert_eq!(event["exit_code"], 1);
        assert_ran_with(&fixture, setup);
        let err = std::fs::read_to_string(fixture.root.join(format!("watch-{index}.err")))
            .unwrap_or_default();
        assert!(err.contains("note[SAIL3112]: "), "{err}");
    }
    daemon.shutdown();
}

struct Mcp {
    server: Option<std::thread::JoinHandle<std::io::Result<()>>>,
    stdin: Option<UnixStream>,
    lines: Receiver<String>,
    next: i64,
}

impl Mcp {
    fn start(fixture: &Fixture) -> Self {
        // MCP runs `sail` as a subprocess: a wrapper gives it the fixture's
        // environment.
        use std::fmt::Write as _;
        let mut script = String::from("#!/bin/sh\n");
        for (key, value) in fixture.environment() {
            let _ = writeln!(script, "export {key}='{value}'");
        }
        for loader in LOADERS {
            let _ = writeln!(script, "unset {loader}");
        }
        let _ = writeln!(script, "exec '{}' \"$@\"", fixture.sail.display());
        let sail = fixture.root.join("bin/mcp-sail");
        write_exec(&sail, &script);
        let (server, client) = UnixStream::pair().unwrap_or_else(|error| unreachable!("{error}"));
        let input = server
            .try_clone()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let stdin = client.try_clone().ok();
        let config = sail_mcp::Config {
            sail,
            store_dir: Some(fixture.root.join("cache/mcp")),
            cwd: fixture.project.clone(),
        };
        let server =
            std::thread::spawn(move || sail_mcp::serve(config, BufReader::new(input), server));
        let (tx, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(client).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut mcp = Self {
            server: Some(server),
            stdin,
            lines,
            next: 1,
        };
        mcp.request(
            "initialize",
            &json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}),
        );
        mcp.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        mcp
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
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(120))
                .unwrap_or_else(|error| unreachable!("no response to {method}: {error}"));
            let value: Value = serde_json::from_str(&line).unwrap_or_default();
            if value["id"] == json!(id) {
                return value;
            }
        }
    }

    fn tool(&mut self, name: &str, arguments: &Value) -> Value {
        let response = self.request("tools/call", &json!({"name": name, "arguments": arguments}));
        let result = response["result"].clone();
        assert_eq!(result["isError"], false, "{response}");
        result
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        if let Some(stdin) = self.stdin.take() {
            let _ = stdin.shutdown(std::net::Shutdown::Write);
        }
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
    }
}

/// The agent view of a plain run without reuse: the failures, the reason in
/// the summary, the structured `no_reuse` and never `current`.
fn assert_agent_view(summary: &str, view: &Value) {
    assert!(
        summary.starts_with("FAILED: 2 of 8 function(s)"),
        "{summary}"
    );
    assert!(summary.contains("No proof reuse (SAIL3112): "), "{summary}");
    assert!(summary.contains("never current"), "{summary}");
    assert_eq!(view["no_reuse"]["code"], "SAIL3112", "{view}");
    assert_eq!(view["no_reuse"]["verified_as"], "plain-cargo-verus");
    assert_eq!(view["currentness"], "unknown");
    assert_eq!(view["exit_code"], 1);
    assert_eq!(view["targets"][0]["reused"], false);
    assert_eq!(
        view["targets"][0]["target"],
        sail_api::project::PLAIN_TARGET
    );
}

#[test]
fn mcp_verifies_unbound_setups_as_plain_cargo_verus() {
    let fixture = Fixture::new("mcp");
    let mut daemon = fixture.jobs();
    let mut mcp = Mcp::start(&fixture);
    for setup in SETUPS {
        let cargo_args: Vec<&str> = setup.to_vec();
        // The blocking `verify` tool.
        let result = mcp.tool("verify", &json!({"cargo_args": cargo_args}));
        let summary = result["content"][0]["text"].as_str().unwrap_or_default();
        assert_agent_view(summary, &result["structuredContent"]);
        assert_ran_with(&fixture, setup);
        // The persistent job tools.
        let started = mcp.tool("start_verification", &json!({"cargo_args": cargo_args}));
        let id = started["structuredContent"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert!(!id.is_empty(), "{started}");
        let mut view = Value::Null;
        wait_for("the job's result", || {
            let result = mcp.tool("verification_result", &json!({"job_id": id}));
            if result["structuredContent"]["value"]["result"].is_object() {
                view = result;
                true
            } else {
                false
            }
        });
        let summary = view["content"][0]["text"].as_str().unwrap_or_default();
        assert_agent_view(summary, &view["structuredContent"]["value"]["result"]);
        // `detail: "full"` returns the project result itself.
        let full = mcp.tool(
            "verification_result",
            &json!({"job_id": id, "detail": "full"}),
        );
        let project: ProjectResult =
            serde_json::from_value(full["structuredContent"]["value"]["result"].clone())
                .unwrap_or_else(|error| unreachable!("{error}: {full}"));
        assert_plain(&project, setup);
    }
    drop(mcp);
    daemon.shutdown();
}

/// Cargo may succeed without invoking Verus for any target. That exit code
/// must not become an MCP claim that the current inputs were verified.
#[test]
fn mcp_unbound_up_to_date_run_reports_no_result() {
    let fixture = Fixture::new("mcp-empty");
    write_exec(&fixture.root.join("cargo-verus"), "#!/bin/sh\nexit 0\n");
    let mut daemon = fixture.jobs();
    let mut mcp = Mcp::start(&fixture);
    let result = mcp.tool("verify", &json!({"cargo_args": ["--tests"]}));
    let summary = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(summary.starts_with("NO RESULT:"), "{result}");
    assert!(summary.contains("No proof reuse (SAIL3112)"), "{result}");
    let view = &result["structuredContent"];
    assert_eq!(view["exit_code"], 0);
    assert_eq!(view["currentness"], "unknown");
    assert_eq!(view["no_reuse"]["verified_as"], "plain-cargo-verus");
    drop(mcp);
    daemon.shutdown();
}
