//! `sail lsp` publishes a failed proof on its line and clears it after the fix.
//!
//! The job service is real; its worker is a script that answers with project
//! results recorded from the pinned Verus (`crates/sail-lsp/fixtures`),
//! choosing by the source on disk. The same exchange against a real verifier
//! is `crates/sail-lsp/spike/lsp_client.py`, which also covers results the
//! service classifies as current, reconnection and `--stale mark`.
#![cfg(unix)]

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const FAILING: &str = include_str!("../../sail-lsp/fixtures/failing.json");
const PASSING: &str = include_str!("../../sail-lsp/fixtures/passing.json");

/// Bounds a hung run only; see `tests/watch.rs` for why it is generous.
const WAIT: Duration = Duration::from_secs(120);

/// Reads messages until one satisfies `wanted`, returning it and everything
/// read on the way.
fn until(
    incoming: &Receiver<Value>,
    what: &str,
    wanted: impl Fn(&Value) -> bool,
) -> (Value, Vec<Value>) {
    let deadline = Instant::now() + WAIT;
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let Ok(message) = incoming.recv_timeout(left) else {
            unreachable!("no {what}; read {seen:#?}")
        };
        seen.push(message.clone());
        if wanted(&message) {
            return (message, seen);
        }
    }
}

fn status(event: &'static str) -> impl Fn(&Value) -> bool {
    move |message| message["method"] == "$/sail/status" && message["params"]["event"] == event
}

/// The diagnostics of every publication for `uri` among `seen`, in order.
fn publications(seen: &[Value], uri: &str) -> Vec<Value> {
    seen.iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
        })
        .map(|message| message["params"]["diagnostics"].clone())
        .collect()
}

/// A package at `root/crate` whose source says `broken`, and a worker that
/// answers with the recorded failing result while it does and the passing
/// one afterwards. Returns the package directory and the worker.
fn project(root: &Path) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;
    let krate = root.join("crate");
    assert!(std::fs::create_dir_all(krate.join("src")).is_ok());
    assert!(
        std::fs::write(
            krate.join("Cargo.toml"),
            "[package]\nname='lsp_fixture'\nversion='0.1.0'\nedition='2024'\n"
        )
        .is_ok()
    );
    let source = krate.join("src/lib.rs");
    assert!(std::fs::write(&source, "// broken\n").is_ok());
    let workspace = krate.to_string_lossy().into_owned();
    for (name, text) in [("failing.json", FAILING), ("passing.json", PASSING)] {
        assert!(std::fs::write(root.join(name), text.replace("@WORKSPACE@", &workspace)).is_ok());
    }
    // The worker runs in the project directory and must exit as its
    // document says (101 for the failed proof, 0 for the repaired one).
    let worker = root.join("worker");
    let script = format!(
        "#!/bin/sh\nif grep -q broken src/lib.rs; then cat '{0}/failing.json'; exit 101; fi\ncat '{0}/passing.json'\n",
        root.display()
    );
    assert!(std::fs::write(&worker, script).is_ok());
    assert!(std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755)).is_ok());
    (krate, worker)
}

#[test]
fn a_failed_postcondition_is_published_on_its_line_and_cleared_after_the_fix() {
    // A short root keeps the versioned socket name inside the Unix limit.
    let root = Path::new("/tmp").join(format!("sail-lsp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (krate, worker) = project(&root);
    let source = krate.join("src/lib.rs");
    let workspace = krate.to_string_lossy().into_owned();
    let cache_socket = root.join("d.sock");
    let socket = sail_api::jobs::socket_path(&cache_socket);
    let mut daemon = sail_daemon::jobs::bind(&socket, &root.join("jobs"), &worker)
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .unwrap_or_else(|error| unreachable!("{error}"));

    // Started outside the project: the workspace folder decides where jobs run.
    let mut server = Command::new(env!("CARGO_BIN_EXE_sail"))
        .arg("lsp")
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", krate.join("target"))
        .env("SAIL_DAEMON_SOCKET", &cache_socket)
        .env("SAIL_CACHE_DIR", root.join("cache"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(
            std::fs::File::create(root.join("lsp.err")).map_or_else(|_| Stdio::null(), Stdio::from),
        )
        .spawn()
        .unwrap_or_else(|error| unreachable!("{error}"));
    let (Some(mut stdin), Some(stdout)) = (server.stdin.take(), server.stdout.take()) else {
        unreachable!("no pipes")
    };
    let (sender, incoming) = channel();
    std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        while let Ok(Some(body)) = sail_lsp::jsonrpc::read(&mut stdout) {
            let Ok(message) = serde_json::from_slice::<Value>(&body) else {
                break;
            };
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    let mut send = |message: Value| {
        assert!(sail_lsp::jsonrpc::write(&mut stdin, &message).is_ok());
    };
    let folder = sail_lsp::uri::from_path(&workspace);
    let uri = sail_lsp::uri::from_path(&source.to_string_lossy());

    send(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "processId": null, "rootUri": folder, "capabilities": {},
            "workspaceFolders": [{"uri": folder, "name": "crate"}],
        }}),
    );
    let (reply, _) = until(&incoming, "initialize reply", |m| m["id"] == 1);
    assert_eq!(reply["result"]["serverInfo"]["name"], "sail");
    send(json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));

    // The failed postcondition: `ensures r == 2 * x,` at 7:13, ten carets.
    let (_, seen) = until(&incoming, "first result", status("result"));
    assert_eq!(
        publications(&seen, &uri),
        vec![json!([{
            "range": {
                "start": {"line": 6, "character": 12},
                "end": {"line": 6, "character": 22},
            },
            // The scripted worker cannot establish Sail's input records, so
            // the service classifies its result as `unknown`; the failure is
            // still shown, demoted and labelled.
            "severity": 3,
            "source": "sail",
            "message": "[currentness unknown: verify again for a current result] postcondition not satisfied\nfailed this postcondition",
        }])]
    );

    // The fix. The squiggle goes when the new run is queued, before any
    // verdict, and nothing replaces it once the run completes.
    assert!(std::fs::write(&source, "// repaired\n").is_ok());
    send(json!({"jsonrpc": "2.0", "method": "textDocument/didSave",
        "params": {"textDocument": {"uri": uri}}}));
    let (_, seen) = until(&incoming, "second run queued", status("queued"));
    assert_eq!(publications(&seen, &uri), vec![json!([])]);
    let (result, seen) = until(&incoming, "second result", status("result"));
    assert_eq!(publications(&seen, &uri), Vec::<Value>::new());
    assert_eq!(result["params"]["exit_code"], 0);
    assert_eq!(result["params"]["currentness"], "unknown");
    assert_eq!(result["params"]["published"], false);

    send(json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown"}));
    let (reply, _) = until(&incoming, "shutdown reply", |m| m["id"] == 2);
    assert_eq!(reply["result"], Value::Null);
    send(json!({"jsonrpc": "2.0", "method": "exit"}));
    let deadline = Instant::now() + WAIT;
    let code = loop {
        if let Ok(Some(code)) = server.try_wait() {
            break code;
        }
        assert!(Instant::now() < deadline, "sail lsp did not exit");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(code.success(), "{code}");
    daemon.shutdown();
    let _ = std::fs::remove_dir_all(root);
}
