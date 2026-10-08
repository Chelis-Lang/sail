//! Drives `sail mcp` through one `verify` tool call (used by `ci/run.sh e2e`).
//!
//! ```text
//! check_mcp <sail> <project-dir> <sail.json> <out.json>
//! ```
//!
//! It starts `<sail> mcp`, sends `initialize`, `notifications/initialized`,
//! `tools/list` and `tools/call verify {"project_dir": <project-dir>}`, then
//! closes stdin. It checks that:
//! - the handshake succeeds and the four tools are listed;
//! - the call succeeds and returns a schema-v1 report;
//! - every function's verdict and tier equal those in `<sail.json>` (the
//!   stdout of `sail verify --output json` on the same project), with the
//!   same set of functions.
//!
//! The returned report is written to `<out.json>`, for `check_output`.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Command, ExitCode, Stdio};

use sail_api::output::Report;
use serde_json::{Value, json};

fn verdicts(r: &Report) -> BTreeMap<String, String> {
    r.crates
        .iter()
        .flat_map(|c| &c.functions)
        .map(|f| (f.function.clone(), format!("{:?}/{:?}", f.verdict, f.tier)))
        .collect()
}

fn exchange(sail: &str, project: &str) -> Result<Vec<Value>, String> {
    let mut child = Command::new(sail)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {sail} mcp: {e}"))?;
    let messages = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "check_mcp", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "verify", "arguments": {"project_dir": project}}}),
    ];
    {
        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        for m in &messages {
            writeln!(stdin, "{m}").map_err(|e| format!("write: {e}"))?;
        }
        // Dropping stdin ends the session once the call has been answered.
    }
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut out = Vec::new();
    for line in BufReader::new(stdout).lines() {
        let line = line.map_err(|e| format!("read: {e}"))?;
        out.push(serde_json::from_str(&line).map_err(|e| format!("not JSON: {e}: {line}"))?);
    }
    let status = child.wait().map_err(|e| format!("wait: {e}"))?;
    if !status.success() {
        return Err(format!("sail mcp exited with {status}"));
    }
    Ok(out)
}

fn check(args: &[String]) -> Result<String, String> {
    let [sail, project, expected, out] = args else {
        return Err("usage: check_mcp <sail> <project-dir> <sail.json> <out.json>".to_owned());
    };
    let responses = exchange(sail, project)?;
    let response = |id: i64| {
        responses
            .iter()
            .find(|r| r["id"] == json!(id))
            .cloned()
            .ok_or(format!("no response {id}"))
    };
    if response(1)?["result"]["serverInfo"]["name"] != "sail" {
        return Err(format!("bad initialize response: {}", response(1)?));
    }
    let tools = response(2)?;
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["name"].as_str())
        .collect();
    if names != ["verify", "counterexample", "cached_result", "fragility"] {
        return Err(format!("unexpected tools: {names:?}"));
    }
    let call = response(3)?;
    let result = &call["result"];
    if result["isError"] != false {
        return Err(format!("verify failed: {call}"));
    }
    let doc = result["structuredContent"]["report"].clone();
    let report: Report =
        serde_json::from_value(doc.clone()).map_err(|e| format!("not a v1 report: {e}"))?;
    let want: Report =
        serde_json::from_slice(&std::fs::read(expected).map_err(|e| format!("{expected}: {e}"))?)
            .map_err(|e| format!("{expected}: {e}"))?;
    let (got, want) = (verdicts(&report), verdicts(&want));
    if got.is_empty() || got != want {
        return Err(format!(
            "verdicts differ from sail verify --output json:\n  mcp:  {got:?}\n  sail: {want:?}"
        ));
    }
    std::fs::write(
        out,
        serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n",
    )
    .map_err(|e| format!("{out}: {e}"))?;
    let summary = result["content"][0]["text"].as_str().unwrap_or_default();
    Ok(format!(
        "check_mcp: {} functions agree with sail verify --output json\n{summary}",
        got.len()
    ))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match check(&args) {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("check_mcp: {e}");
            ExitCode::FAILURE
        }
    }
}
