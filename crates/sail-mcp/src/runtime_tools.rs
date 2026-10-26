//! Runtime tools use the same independently versioned local service as Rust clients.
use crate::run::Cancel;
use crate::tools::{Context, ToolResult};
use sail_api::runtime::{
    Answer, Outcome, Output, Request,
    transport::{self, Client},
};
use serde_json::{Map, Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({"name":"runtime_capabilities","title":"Inspect verification runtime","description":"Experimental: integration tool. Inspect actual implemented and installed SMT and native temporal backends through Sail's shared daemon. Readiness is not theorem evidence.","inputSchema":{"type":"object","additionalProperties":false,"properties":{}},"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
        json!({"name":"runtime_execute","title":"Execute verification runtime request","description":"Experimental: integration tool. Execute one versioned SMT batch or native temporal job through Sail's shared daemon. Preserve exact backend evidence, caller identities and failure outcomes; satisfiability does not prove Rust code.","inputSchema":{"type":"object","additionalProperties":false,"required":["request"],"properties":{"request":{"type":"object","description":"Version-one runtime request: version=1, input={kind:smt, queries:[{obligation_id,requirement_ids,backend:z3,script,limits?}]}. Limits and transcript admission are enforced before dispatch."}}},"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
    ]
}

pub(super) fn call(
    ctx: &Context,
    name: &str,
    args: &Map<String, Value>,
    cancel: &Cancel,
) -> ToolResult {
    let request = match name {
        "runtime_capabilities" if args.is_empty() => None,
        "runtime_execute" if args.len() == 1 && args.contains_key("request") => {
            match serde_json::from_value::<Request>(args["request"].clone()) {
                Ok(request) => Some(request),
                Err(error) => return ToolResult::invalid(&error.to_string()),
            }
        }
        _ => {
            return ToolResult::invalid(
                "expected no arguments for runtime_capabilities or one versioned request for runtime_execute",
            );
        }
    };
    match run(ctx, request.as_ref(), cancel) {
        Ok(result) => result,
        Err(error) => ToolResult::error(
            format!("Runtime request failed: {error}"),
            json!({"error":error}),
        ),
    }
}

fn endpoint(ctx: &Context) -> Result<PathBuf, String> {
    let cache = sail_api::jobs::cache_directory().map_err(|e| e.to_string())?;
    let explicit = std::env::var_os("SAIL_DAEMON_SOCKET")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    Ok(sail_api::jobs::cache_socket(
        &cache,
        explicit.as_deref(),
        &ctx.config.cwd,
    ))
}

fn connect(ctx: &Context, cancel: &Cancel) -> Result<Client, String> {
    use std::os::unix::{net::UnixStream, process::CommandExt as _};
    let cache_socket = endpoint(ctx)?;
    let client = Client::new(transport::socket_path(&cache_socket));
    if client.capabilities().is_ok() {
        return Ok(client);
    }
    // Existing older daemons must be restarted, never bypassed by another backend.
    if UnixStream::connect(&cache_socket).is_ok() {
        return Err(
            "SAIL6201: the running daemon has no compatible runtime endpoint; restart sail serve"
                .into(),
        );
    }
    let mut child = Command::new(&ctx.config.sail)
        .arg("serve")
        .arg("--socket")
        .arg(&cache_socket)
        .current_dir(&ctx.config.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("SAIL6201: cannot start Sail daemon: {e}"))?;
    // The shared service intentionally outlives any one call; always reap its exit.
    let (finished, exited) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("sail-runtime-daemon".into())
        .spawn(move || {
            let _ = finished.send(child.wait());
        })
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if cancel.is_cancelled() {
            return Err("SAIL6201: runtime call cancelled during startup".into());
        }
        if client.capabilities().is_ok() {
            return Ok(client);
        }
        if exited.try_recv().is_ok() {
            // Another client may have won the startup race.
            return client
                .capabilities()
                .map(|_| client)
                .map_err(|e| e.to_string());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err("SAIL6201: runtime daemon did not become ready; run sail serve to inspect startup".into())
}

fn run(ctx: &Context, request: Option<&Request>, cancel: &Cancel) -> Result<ToolResult, String> {
    if cancel.is_cancelled() {
        return Err("SAIL6201: runtime call cancelled before dispatch".into());
    }
    let client = connect(ctx, cancel)?;
    let Some(request) = request else {
        let capabilities = client.capabilities().map_err(|e| e.to_string())?;
        return Ok(ToolResult::ok(
            "Runtime capabilities, including actual installation readiness.".into(),
            serde_json::to_value(capabilities).map_err(|e| e.to_string())?,
        ));
    };
    let response = client
        .execute_cancellable(request, || cancel.is_cancelled())
        .map_err(|e| e.to_string())?;
    let (text, is_error) = match &response.output {
        Output::Smt { results } => {
            let decided = results
                .iter()
                .filter(|r| {
                    r.outcome == Outcome::Completed
                        && r.checks.iter().all(|c| c.answer != Answer::Unknown)
                })
                .count();
            (
                format!(
                    "{decided}/{} SMT transcripts completed with decided answers; each answer concerns its exact formula.",
                    results.len()
                ),
                decided != results.len(),
            )
        }
        Output::Temporal { result } => {
            let native = result.response.as_ref().map(|r| r.outcome);
            let complete = result.outcome == Outcome::Completed
                && matches!(
                    native,
                    Some(
                        sail_api::temporal::Outcome::Holds
                            | sail_api::temporal::Outcome::Refuted
                            | sail_api::temporal::Outcome::Sampled
                            | sail_api::temporal::Outcome::Valid
                            | sail_api::temporal::Outcome::Invalid
                    )
                );
            (
                format!(
                    "Native temporal execution {:?}; original evidence and bounded scope are retained.",
                    result.outcome
                ),
                !complete,
            )
        }
    };
    let value = serde_json::to_value(response).map_err(|e| e.to_string())?;
    Ok(if is_error {
        ToolResult::error(text, value)
    } else {
        ToolResult::ok(text, value)
    })
}
