//! CLI proof events, explicit focused repair validation and evidence operations.

mod arguments;
mod evidence;

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use sail_api::jobs::{Client, Mode, Request, Response};
use sail_api::proof::Page;

pub(crate) use evidence::run as evidence;

fn error(message: &str, err: &mut dyn Write) -> ExitCode {
    let _ = writeln!(err, "error[SAIL0906]: {message}");
    ExitCode::from(2)
}

pub(super) fn call(client: &Client, request: &Request) -> Result<Response, String> {
    let response = client.call(request).map_err(|error| error.to_string())?;
    if let Response::Error(error) = response {
        Err(error)
    } else {
        Ok(response)
    }
}

/// Checks that the engine is installed and valid before a job is submitted,
/// so a missing engine reports `SAIL3120` instead of leaving an
/// infrastructure-error job behind. The worker validates it again. With an
/// explicitly selected service (`SAIL_DAEMON_SOCKET`), that service owns the
/// worker, so the check is left to it; the job's error then carries the
/// worker's `SAIL3120`.
fn engine_available() -> Result<(), sail_verus::Error> {
    if std::env::var_os("SAIL_DAEMON_SOCKET").is_some_and(|socket| !socket.is_empty()) {
        return Ok(());
    }
    let cache = sail_verus::cache_dir()?;
    let pins = sail_verus::toolchain::Pins::embedded()?;
    sail_verus::engine::status(&cache, &pins).map(|_| ())
}

/// Parses, checks the engine and submits one repair job; on failure the
/// error is already written and the exit code is returned.
fn submit(
    args: &[String],
    err: &mut dyn Write,
) -> Result<(Client, sail_api::jobs::Status, bool, bool), ExitCode> {
    let (selection, args, background, json) =
        arguments::repair(args).map_err(|message| error(&message, err))?;
    if let Err(engine) = engine_available() {
        let _ = writeln!(err, "{engine}");
        return Err(ExitCode::from(2));
    }
    let client = crate::jobs::connect_or_start().map_err(|message| error(&message, err))?;
    let status = crate::jobs::submit_mode(&client, args, Mode::Repair(selection))
        .map_err(|message| error(&message, err))?;
    Ok((client, status, background, json))
}

pub(crate) fn repair(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let (client, status, background, json) = match submit(args, err) {
        Ok(result) => result,
        Err(code) => return code,
    };
    if background {
        let text = if json {
            serde_json::to_string(&status).unwrap_or_default()
        } else {
            format!("{} queued repair validation", status.id)
        };
        let _ = writeln!(out, "{text}");
        return ExitCode::SUCCESS;
    }
    let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal = std::sync::Arc::clone(&stopped);
    if let Err(message) =
        ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::SeqCst))
    {
        cancel_owned(&client, &status.id);
        return error(
            &format!("cannot supervise repair cancellation: {message}"),
            err,
        );
    }
    let _ = writeln!(err, "Repair validation job {}", status.id);
    loop {
        if stopped.load(std::sync::atomic::Ordering::SeqCst) {
            cancel_owned(&client, &status.id);
            return ExitCode::from(130);
        }
        match call(
            &client,
            &Request::Result {
                id: status.id.clone(),
            },
        ) {
            Ok(Response::Result(document)) if document.job.state.terminal() => {
                let Some(mut repair) = document.repair else {
                    return error(
                        document
                            .job
                            .error
                            .as_deref()
                            .unwrap_or("repair evidence unavailable"),
                        err,
                    );
                };
                // A job result carries the full phase once, as `result`; the
                // standalone repair document carries it as `batch.full`.
                repair.batch.full = document
                    .result
                    .and_then(|result| serde_json::from_value(result).ok());
                if json {
                    let _ = writeln!(
                        out,
                        "{}",
                        serde_json::to_string(&repair).unwrap_or_default()
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "{}: repair {}; full evidence {}; {} compiler invocations; batched={}",
                        status.id,
                        if repair.accepted {
                            "accepted"
                        } else {
                            "not accepted"
                        },
                        repair.batch.full.as_ref().map_or_else(
                            || "unavailable".to_owned(),
                            |full| format!("{:?}", full.currentness)
                        ),
                        repair.batch.compiler_invocations,
                        repair.batch.batched
                    );
                }
                if let Some(error) = &repair.batch.selection_error {
                    let _ = writeln!(err, "error[{}]: {}", error.code, error.message);
                }
                if let Some(fault) = &repair.batch.engine_fault {
                    // ADR 0027: the full report is an independent stock run.
                    let _ = writeln!(err, "error[{}]: {}", fault.code, fault.message);
                }
                return ExitCode::from(u8::from(!repair.accepted));
            }
            Ok(Response::Result(_)) => std::thread::sleep(Duration::from_millis(100)),
            Ok(_) => {
                cancel_owned(&client, &status.id);
                return error("unexpected repair response", err);
            }
            Err(message) => {
                cancel_owned(&client, &status.id);
                return error(&message, err);
            }
        }
    }
}

// This foreground command owns precisely this submitted/coalesced request.
// Best-effort cancellation uses the same bounded RPC; an unavailable endpoint
// must not replace the original polling error with its cleanup error.
fn cancel_owned(client: &Client, id: &str) {
    let _ = call(
        client,
        &Request::Cancel {
            id: id.to_owned(),
            superseded: false,
        },
    );
}

pub(crate) fn worker(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    if args.first().map(String::as_str) != Some("--selection-json") {
        return error("private repair worker requires a selection", err);
    }
    let Some(selection) = args
        .get(1)
        .and_then(|value| serde_json::from_str::<sail_api::repair::Selection>(value).ok())
    else {
        return error("invalid repair selection", err);
    };
    if let Err(message) = arguments::validate(&selection) {
        return error(&message, err);
    }
    if std::env::var_os("SAIL_JOB_GUARD_PARENT").is_some_and(|v| v == "1") {
        crate::portfolio::guard_parent();
    }
    let arguments = match arguments::worker_args(&selection, args.get(2..).unwrap_or_default()) {
        Ok(arguments) => arguments,
        Err(message) => return error(&message, err),
    };
    let config = match crate::managed_cli::config(arguments, true) {
        Ok(config) => config,
        Err(message) => return error(&message, err),
    };
    match sail_verus::engine::repair(&config, &selection.function, err) {
        Ok(batch) => {
            let code = batch
                .full
                .as_ref()
                .and_then(|full| full.exit_code)
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(2);
            let document = sail_api::repair::ResultDocument::from(batch);
            let _ = writeln!(
                out,
                "{}",
                serde_json::to_string(&document).unwrap_or_default()
            );
            ExitCode::from(code)
        }
        // Engine errors carry their own stable code (for example SAIL3120).
        Err(engine) => {
            let _ = writeln!(err, "{engine}");
            ExitCode::from(2)
        }
    }
}

pub(crate) fn events(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let mut run = || -> Result<(), String> {
        let (id, mut cursor, follow, jsonl) = arguments::events(args)?;
        let client = crate::jobs::connect_or_start()?;
        loop {
            let response = call(
                &client,
                &Request::ProofEvents {
                    id: id.clone(),
                    cursor,
                    limit: 64,
                },
            )?;
            let Response::ProofEvents(page) = response else {
                return Err("unexpected proof stream response".into());
            };
            write_page(&page, jsonl, out)?;
            out.flush().map_err(|error| error.to_string())?;
            let advanced = !page.events.is_empty();
            cursor = page.next;
            if !follow || (page.terminal && !advanced) {
                break;
            }
            if !advanced {
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        Ok(())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => error(&message, err),
    }
}

fn write_page(page: &Page, jsonl: bool, out: &mut dyn Write) -> Result<(), String> {
    if jsonl {
        for event in &page.events {
            writeln!(
                out,
                "{}",
                serde_json::to_string(event).map_err(|error| error.to_string())?
            )
            .map_err(|error| error.to_string())?;
        }
        // Cursor/control records remain explicit even if a page has no observations.
        writeln!(out, "{}", serde_json::json!({"kind":"cursor","version":page.version,"next":page.next,"gap":page.gap,"terminal":page.terminal})).map_err(|error| error.to_string())
    } else {
        writeln!(
            out,
            "{}",
            serde_json::to_string(page).map_err(|error| error.to_string())?
        )
        .map_err(|error| error.to_string())
    }
}
