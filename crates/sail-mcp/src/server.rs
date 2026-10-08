//! The MCP server loop over stdio.
//!
//! The server is dual-era (MCP revision 2026-07-28, "Versioning and
//! Compatibility"):
//! - a legacy client opens with `initialize` (revisions 2024-11-05 to
//!   2025-11-25) and the server answers with the client's revision if it
//!   knows it, else its newest legacy one;
//! - a modern client sends `_meta["io.modelcontextprotocol/protocolVersion"]`
//!   on every request (and may call `server/discover`); a request naming an
//!   unknown version gets `UnsupportedProtocolVersionError`.
//!
//! Requests are read one line at a time. `tools/call` runs on its own
//! thread, so `ping` and `notifications/cancelled` are served while a
//! verification runs. A cancelled call's process group is killed and the
//! call gets no response, as the specification asks. At end of input the
//! server waits for calls in flight, then returns.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};

use crate::jsonrpc::{self, Message};
use crate::run::Cancel;
use crate::tools::{self, Config, Context};

/// Modern protocol revisions (per-request `_meta`), newest first.
pub const MODERN_VERSIONS: &[&str] = &["2026-07-28"];
/// Legacy protocol revisions (`initialize` handshake), newest first.
pub const LEGACY_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";

const INSTRUCTIONS: &str = "sail verifies Verus crates with the same verdicts as `cargo verus verify`, serving unchanged solver work from a cache. After each edit, call `verify` (optionally with `module` or `function`) for per-function verdicts; `provisional` and `unknown` are never `verified`. Use `counterexample` for the models behind a failure and `cached_result` to recall verdicts without re-running (it says whether sources changed since).";

/// Every revision the server speaks.
fn supported() -> Vec<&'static str> {
    MODERN_VERSIONS
        .iter()
        .chain(LEGACY_VERSIONS)
        .copied()
        .collect()
}

type Out = Arc<Mutex<Box<dyn Write + Send>>>;

fn send(out: &Out, message: &Value) {
    if let Ok(mut w) = out.lock() {
        let _ = writeln!(w, "{}", jsonrpc::line(message));
        let _ = w.flush();
    }
}

struct Server {
    ctx: Arc<Context>,
    out: Out,
    inflight: Arc<Mutex<HashMap<String, Arc<Cancel>>>>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

fn capabilities() -> Value {
    json!({"tools": {"listChanged": false}})
}

fn server_info() -> Value {
    json!({"name": "sail", "title": "sail (S.A.I.L.)", "version": env!("CARGO_PKG_VERSION")})
}

impl Server {
    fn notification(&self, method: &str, params: &Map<String, Value>) {
        if method == "notifications/cancelled"
            && let Some(id) = params.get("requestId")
            && let Ok(inflight) = self.inflight.lock()
            && let Some(cancel) = inflight.get(&id.to_string())
        {
            cancel.cancel();
        }
        // `notifications/initialized` and anything else need no action.
    }

    fn request(&mut self, id: &Value, method: &str, params: &Map<String, Value>) {
        if let Some(requested) = params.get("_meta").and_then(|m| m.get(VERSION_KEY)) {
            let known = requested.as_str().is_some_and(|v| supported().contains(&v));
            if !known {
                send(
                    &self.out,
                    &jsonrpc::error(
                        id,
                        jsonrpc::UNSUPPORTED_PROTOCOL_VERSION,
                        "Unsupported protocol version",
                        Some(json!({"supported": supported(), "requested": requested})),
                    ),
                );
                return;
            }
        }
        let result = match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str);
                let version = asked
                    .filter(|v| LEGACY_VERSIONS.contains(v))
                    .unwrap_or(LEGACY_VERSIONS[0]);
                json!({
                    "protocolVersion": version,
                    "capabilities": capabilities(),
                    "serverInfo": server_info(),
                    "instructions": INSTRUCTIONS,
                })
            }
            "server/discover" => json!({
                "resultType": "complete",
                "supportedVersions": supported(),
                "capabilities": capabilities(),
                "_meta": {"io.modelcontextprotocol/serverInfo": server_info()},
                "instructions": INSTRUCTIONS,
            }),
            "ping" => json!({"resultType": "complete"}),
            "tools/list" => json!({"resultType": "complete", "tools": tools::definitions()}),
            "tools/call" => return self.call(id, params),
            _ => {
                send(
                    &self.out,
                    &jsonrpc::error(
                        id,
                        jsonrpc::METHOD_NOT_FOUND,
                        &format!("Method not found: {method}"),
                        None,
                    ),
                );
                return;
            }
        };
        send(&self.out, &jsonrpc::ok(id, &result));
    }

    fn call(&mut self, id: &Value, params: &Map<String, Value>) {
        let invalid = |message: &str| jsonrpc::error(id, jsonrpc::INVALID_PARAMS, message, None);
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            send(&self.out, &invalid("tools/call needs a string `name`"));
            return;
        };
        let args = match params.get("arguments") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(a)) => a.clone(),
            Some(_) => {
                send(&self.out, &invalid("`arguments` must be an object"));
                return;
            }
        };
        let known = tools::definitions()
            .as_array()
            .is_some_and(|t| t.iter().any(|t| t["name"] == name));
        if !known {
            send(&self.out, &invalid(&format!("Unknown tool: {name}")));
            return;
        }
        let key = id.to_string();
        let cancel = Arc::new(Cancel::default());
        if let Ok(mut inflight) = self.inflight.lock() {
            inflight.insert(key.clone(), Arc::clone(&cancel));
        }
        let (ctx, out, inflight) = (
            Arc::clone(&self.ctx),
            Arc::clone(&self.out),
            Arc::clone(&self.inflight),
        );
        let (id, name) = (id.clone(), name.to_owned());
        self.workers.push(std::thread::spawn(move || {
            let result = tools::call(&ctx, &name, &args, &cancel);
            if let Ok(mut inflight) = inflight.lock() {
                inflight.remove(&key);
            }
            if cancel.is_cancelled() {
                return;
            }
            let message = match result {
                Some(r) => jsonrpc::ok(&id, &r.to_json()),
                None => jsonrpc::error(
                    &id,
                    jsonrpc::INVALID_PARAMS,
                    &format!("Unknown tool: {name}"),
                    None,
                ),
            };
            send(&out, &message);
        }));
        self.workers.retain(|w| !w.is_finished());
    }
}

/// Serves MCP on `input`/`output` until `input` ends, then waits for the
/// tool calls in flight and returns.
///
/// # Errors
///
/// Only an error reading `input`.
pub fn serve(
    config: Config,
    mut input: impl BufRead,
    output: impl Write + Send + 'static,
) -> std::io::Result<()> {
    let mut server = Server {
        ctx: Arc::new(Context::new(config)),
        out: Arc::new(Mutex::new(Box::new(output))),
        inflight: Arc::new(Mutex::new(HashMap::new())),
        workers: Vec::new(),
    };
    let mut line = Vec::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_ascii();
        if trimmed.is_empty() {
            continue;
        }
        match jsonrpc::parse(trimmed) {
            Err(response) => send(&server.out, &response),
            Ok(Message::Response) => {}
            Ok(Message::Notification { method, params }) => server.notification(&method, &params),
            Ok(Message::Request { id, method, params }) => server.request(&id, &method, &params),
        }
    }
    for worker in server.workers.drain(..) {
        let _ = worker.join();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer the test can read back.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .map_err(|_| std::io::Error::other("poisoned"))?
                .extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn exchange(input: &str) -> Vec<Value> {
        let out = Shared::default();
        let config = Config {
            sail: "/nonexistent/sail".into(),
            store_dir: None,
            cwd: std::env::temp_dir(),
        };
        assert!(serve(config, input.as_bytes(), out.clone()).is_ok());
        let bytes = out.0.lock().map(|b| b.clone()).unwrap_or_default();
        String::from_utf8_lossy(&bytes)
            .lines()
            .map(|l| serde_json::from_str(l).unwrap_or_default())
            .collect()
    }

    fn by_id(responses: &[Value], id: i64) -> Value {
        responses
            .iter()
            .find(|r| r["id"] == json!(id))
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn legacy_handshake_and_tools_list() {
        let r = exchange(concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":4,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#,
            "\n",
        ));
        assert_eq!(r.len(), 4, "{r:?}");
        let init = by_id(&r, 1);
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(init["result"]["serverInfo"]["name"], "sail");
        assert!(init["result"]["capabilities"]["tools"].is_object());
        let list = by_id(&r, 2);
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert_eq!(
            names,
            ["verify", "counterexample", "cached_result", "fragility"]
        );
        assert!(by_id(&r, 3)["result"].is_object());
        assert_eq!(
            by_id(&r, 4)["result"]["protocolVersion"],
            LEGACY_VERSIONS[0]
        );
    }

    #[test]
    fn modern_requests_and_version_errors() {
        let r = exchange(concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"1900-01-01"}}}"#,
            "\n",
        ));
        let discover = by_id(&r, 1);
        assert_eq!(discover["result"]["supportedVersions"][0], "2026-07-28");
        let e = by_id(&r, 2);
        assert_eq!(e["error"]["code"], jsonrpc::UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(e["error"]["data"]["requested"], "1900-01-01");
    }

    #[test]
    fn protocol_errors() {
        let r = exchange(concat!(
            "not json\n",
            r#"{"jsonrpc":"2.0","id":1,"method":"nope"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nope"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"verify","arguments":[]}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"unknown/notification"}"#,
            "\n",
        ));
        assert_eq!(r.len(), 5, "{r:?}");
        assert!(
            r.iter()
                .any(|m| m["id"].is_null() && m["error"]["code"] == jsonrpc::PARSE_ERROR)
        );
        assert_eq!(by_id(&r, 1)["error"]["code"], jsonrpc::METHOD_NOT_FOUND);
        for id in [2, 3, 4] {
            assert_eq!(
                by_id(&r, id)["error"]["code"],
                jsonrpc::INVALID_PARAMS,
                "{id}"
            );
        }
    }

    #[test]
    fn tool_argument_errors_are_tool_results() {
        let r = exchange(concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"verify","arguments":{"modul":"x"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"counterexample","arguments":{}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"cached_result","arguments":{"project_dir":"/nonexistent/sail-project"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"fragility"}}"#,
            "\n",
        ));
        for id in 1..=4 {
            let result = &by_id(&r, id)["result"];
            assert_eq!(result["isError"], true, "{id}: {result}");
            assert_eq!(result["resultType"], "complete");
            assert!(result["content"][0]["text"].is_string());
        }
        assert!(
            by_id(&r, 1)["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("unknown argument `modul`")
        );
        assert_eq!(
            by_id(&r, 4)["result"]["structuredContent"]["available"],
            false
        );
    }

    #[test]
    fn lookups_without_runs_say_so() {
        let dir = std::env::temp_dir().join(format!("sail-mcp-empty-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&dir).is_ok());
        let req = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"cached_result","arguments":{"project_dir": dir}}});
        let r = exchange(&format!("{req}\n"));
        assert_eq!(
            by_id(&r, 1)["result"]["structuredContent"]["error"],
            "no_results"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
