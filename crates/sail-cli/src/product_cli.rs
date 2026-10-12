//! CLI proof events and evidence operations (explain, baselines, compare).

mod arguments;
mod evidence;

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use sail_api::jobs::{Client, Request, Response};
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
