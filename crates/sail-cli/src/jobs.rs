//! Local verification jobs: submission, inspection, cancellation and diagnostics.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use sail_api::jobs::{Client, Request, Response, Status, Submission};

pub(crate) fn environment() -> BTreeMap<String, String> {
    std::env::vars().filter(|(key, _)| !matches!(key.as_str(), "SAIL_JOB_EVENTS" | "SAIL_MANAGED_FORCE"))
        .collect()
}

/// Connect to the local job endpoint, automatically starting the daemon.
///
/// # Errors
/// Returns setup or bounded startup errors, without changing verification inputs.
pub(crate) fn connect_or_start() -> Result<Client, String> {
    let cache = sail_verus::cache_dir().map_err(|error| error.to_string())?;
    let socket = std::env::var_os("SAIL_DAEMON_SOCKET").filter(|value| !value.is_empty())
        .map_or_else(|| crate::default_socket(&cache), PathBuf::from);
    let jobs_socket = sail_api::jobs::socket_path(&socket);
    let client = Client::new(&jobs_socket);
    if client.call(&Request::List { project_dir: None }).is_ok() { return Ok(client); }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    use std::os::unix::process::CommandExt as _;
    Command::new(executable).arg("serve").args(["--socket"]).arg(&socket)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0)
        .spawn().map_err(|error| format!("SAIL0810: cannot start job daemon: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if client.call(&Request::List { project_dir: None }).is_ok() { return Ok(client); }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(format!("SAIL0810: job endpoint {} did not start", jobs_socket.display()))
}

pub(crate) fn submit(client: &Client, args: &[String]) -> Result<Status, String> {
    let project_dir = std::env::current_dir().map_err(|error| error.to_string())?;
    let args = strip_output(args)?;
    let submission = Submission { project_dir, args, force: false, environment: environment(), input_revision: None };
    match client.call(&Request::Submit { submission }).map_err(|error| error.to_string())? {
        Response::Status(status) => Ok(status),
        _ => Err("SAIL0802: unexpected submit response".to_owned()),
    }
}
fn strip_output(args: &[String]) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut iterator = args.iter();
    while let Some(arg) = iterator.next() {
        if arg == "--" { result.push(arg.clone()); result.extend(iterator.cloned()); break; }
        match arg.as_str() {
            "--background" | "--incremental" | "--output=json" => {},
            "--output" => {
                if iterator.next().map(String::as_str) != Some("json") { return Err("SAIL0811: background output must be json".to_owned()); }
            }
            _ => result.push(arg.clone()),
        }
    }
    Ok(result)
}

/// Submits verification and immediately returns its durable job identity.
pub(crate) fn background(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    match connect_or_start().and_then(|client| submit(&client, args)) {
        Ok(status) => { let _ = writeln!(out, "{}", serde_json::to_string(&status).unwrap_or_default()); ExitCode::SUCCESS }
        Err(message) => { let _ = writeln!(err, "error[SAIL0810]: {message}"); ExitCode::from(2) }
    }
}

/// Runs `sail jobs list|status|result|cancel|logs ID [--follow]`.
pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let run = || -> Result<(), String> {
        let client = connect_or_start()?;
        let id = args.get(1).cloned().unwrap_or_default();
        let request = match args.first().map(String::as_str) {
            Some("list") => Request::List { project_dir: None },
            Some("status") if !id.is_empty() => Request::Status { id },
            Some("result") if !id.is_empty() => Request::Result { id },
            Some("cancel") if !id.is_empty() => Request::Cancel { id, superseded: false },
            Some("logs") if !id.is_empty() => return logs(&client, &id, args.iter().any(|arg| arg == "--follow"), out),
            _ => return Err("SAIL0811: usage: sail jobs list|status|result|cancel|logs ID [--follow]".to_owned()),
        };
        let response = client.call(&request).map_err(|error| error.to_string())?;
        writeln!(out, "{}", serde_json::to_string_pretty(&response).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => { let _ = writeln!(err, "error[SAIL0811]: {message}"); ExitCode::from(2) }
    }
}
fn logs(client: &Client, id: &str, follow: bool, out: &mut dyn Write) -> Result<(), String> {
    let mut offset = 0;
    loop {
        match client.call(&Request::Logs { id: id.to_owned(), offset }).map_err(|error| error.to_string())? {
            Response::Chunk(chunk) => {
                write!(out, "{}", chunk.text).map_err(|error| error.to_string())?;
                out.flush().map_err(|error| error.to_string())?;
                let empty = chunk.next == offset;
                offset = chunk.next;
                if !follow || (chunk.complete && empty) { return Ok(()); }
                if empty { std::thread::sleep(Duration::from_millis(100)); }
            }
            _ => return Err("SAIL0802: unexpected log response".to_owned()),
        }
    }
}
