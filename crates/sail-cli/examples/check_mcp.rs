//! Drives `sail mcp` through one `verify` tool call (used by `ci/run.sh e2e`).
//!
//! ```text
//! check_mcp <sail> <project-dir> <sail.json> <out.json>
//! ```
//!
//! It starts `<sail> mcp`, sends `initialize`, `notifications/initialized`,
//! `tools/list` and `tools/call verify {"project_dir": <project-dir>}`, then
//! closes stdin. It checks that:
//! - the handshake succeeds and `tools/list` is exactly this build's fifteen
//!   compatibility, job and proof tools, with their key input contracts;
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

const TOOL_NAMES: [&str; 15] = [
    "verify",
    "counterexample",
    "cached_result",
    "fragility",
    "start_verification",
    "verification_status",
    "verification_result",
    "cancel_verification",
    "verification_events",
    "explain_failure",
    "compare_verification",
    "save_baseline",
    "list_baselines",
    "get_baseline",
    "start_repair_validation",
];

/// The installed `sail mcp` must list exactly the tools this build defines,
/// and those keep the contract properties agents rely on.
fn check_tools(value: &Value) -> Result<(), String> {
    let tools = value
        .as_array()
        .ok_or("tools/list did not return an array")?;
    let names = tools
        .iter()
        .map(|tool| tool["name"].as_str().ok_or("tool has no string name"))
        .collect::<Result<Vec<_>, _>>()?;
    if names != TOOL_NAMES {
        return Err(format!("unexpected tools: {names:?}"));
    }
    if *value != sail_mcp::tools::definitions() {
        return Err("tools/list differs from this build's tool definitions".to_owned());
    }
    let schema = |index: usize| &tools[index]["inputSchema"];
    let required = [
        (
            9,
            json!(["job_id", "package_id", "target", "kind", "function"]),
        ),
        (11, json!(["job_id", "name"])),
        (14, json!(["package", "kind", "function", "engine"])),
    ];
    for (index, fields) in required {
        if schema(index)["required"] != fields {
            return Err(format!("{} must require {fields}", TOOL_NAMES[index]));
        }
    }
    if schema(8)["properties"]["after"]["minimum"] != json!(0) {
        return Err("verification_events cursors must be non-negative".to_owned());
    }
    if schema(4)["properties"]["engine"]["enum"] != json!(["stock", "interactive"]) {
        return Err(
            "start_verification must declare stock and explicit interactive engine modes"
                .to_owned(),
        );
    }
    Ok(())
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
    check_tools(&response(2)?["result"]["tools"])?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_tools_match_the_public_contract() {
        assert!(check_tools(&sail_mcp::tools::definitions()).is_ok());
    }

    #[test]
    fn extra_missing_nameless_or_changed_tools_are_rejected() {
        let original = sail_mcp::tools::definitions();
        let mut extra = original.clone();
        if let Some(tools) = extra.as_array_mut() {
            tools.push(json!({"name":"unexpected_tool"}));
        }
        assert!(check_tools(&extra).is_err());
        let mut missing = original.clone();
        if let Some(tools) = missing.as_array_mut() {
            tools.pop();
        }
        assert!(check_tools(&missing).is_err());
        let mut nameless = original.clone();
        nameless[8]["name"] = Value::Null;
        assert!(check_tools(&nameless).is_err());
        let mut changed = original;
        changed[12]["inputSchema"]["properties"]["project_dir"] = json!({"type":"string"});
        assert!(check_tools(&changed).is_err());
    }
}
