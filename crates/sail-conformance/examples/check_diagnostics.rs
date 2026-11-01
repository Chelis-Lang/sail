//! Expose ADR 0042's existing diagnostic splitter to benchmark tooling.
//!
//! Reads combined command output from stdin and emits complete messages and
//! ordered remainder as JSON. The benchmark supplies its existing arm-root
//! substitutions; this helper performs no normalization of message bytes.
#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::process::ExitCode;

fn split_input() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .lock()
        .read_to_string(&mut input)
        .map_err(|error| error.to_string())?;
    let split = sail_conformance::diagnostics::split(&input);
    let output = serde_json::json!({
        "kind": "sail-diagnostic-split",
        "version": 1,
        "messages": split.messages,
        "rest": split.rest,
    });
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &output).map_err(|error| error.to_string())?;
    stdout.write_all(b"\n").map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    match split_input() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("check_diagnostics: {error}");
            ExitCode::from(2)
        }
    }
}
