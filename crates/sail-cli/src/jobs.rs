//! Local verification jobs: submission, inspection, cancellation and diagnostics.

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use sail_api::jobs::{Client, Request, Response, Status, Submission};

pub(crate) fn environment() -> Result<BTreeMap<String, String>, String> {
    std::env::vars_os()
        .filter(|(key, _)| {
            ![
                "SAIL_JOB_EVENTS",
                "SAIL_MANAGED_FORCE",
                "SAIL_JOB_GUARD_PARENT",
            ]
            .iter()
            .any(|value| key == value)
        })
        .map(|(key, value)| {
            let key = key
                .into_string()
                .map_err(|_| "SAIL0812: managed jobs require UTF-8 environment names".to_owned())?;
            let value = value
                .into_string()
                .map_err(|_| format!("SAIL0812: managed jobs require a UTF-8 value for {key}"))?;
            Ok((key, value))
        })
        .collect()
}

/// Connect to the local job endpoint, automatically starting the daemon.
///
/// # Errors
/// Returns setup or bounded startup errors, without changing verification inputs.
pub(crate) fn connect_or_start() -> Result<Client, String> {
    let cache = sail_verus::cache_dir().map_err(|error| error.to_string())?;
    let socket = std::env::var_os("SAIL_DAEMON_SOCKET")
        .filter(|value| !value.is_empty())
        .map_or_else(|| crate::default_socket(&cache), PathBuf::from);
    let jobs_socket = sail_api::jobs::socket_path(&socket);
    let client = Client::new(&jobs_socket);
    if client.call(&Request::List { project_dir: None }).is_ok() {
        return Ok(client);
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let command = if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
        "_serve-jobs"
    } else {
        "serve"
    };
    Command::new(executable)
        .arg(command)
        .args(["--socket"])
        .arg(&socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|error| format!("SAIL0810: cannot start job daemon: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if client.call(&Request::List { project_dir: None }).is_ok() {
            return Ok(client);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "SAIL0810: job endpoint {} did not start",
        jobs_socket.display()
    ))
}

pub(crate) fn submit(client: &Client, args: &[String]) -> Result<Status, String> {
    let project_dir = std::env::current_dir().map_err(|error| error.to_string())?;
    let args = strip_output(args)?;
    let input_revision = revision(&project_dir, &args).map(|snapshot| snapshot.revision);
    let submission = Submission {
        project_dir,
        args,
        force: std::env::var("SAIL_MANAGED_FORCE").is_ok_and(|value| value == "1"),
        environment: environment()?,
        input_revision,
    };
    match client
        .call(&Request::Submit { submission })
        .map_err(|error| error.to_string())?
    {
        Response::Status(status) => Ok(status),
        _ => Err("SAIL0802: unexpected submit response".to_owned()),
    }
}

pub(crate) fn revision(
    project: &std::path::Path,
    args: &[String],
) -> Option<sail_verus::managed::InputSnapshot> {
    let cargo = std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from);
    sail_verus::managed::snapshot(project, args, &cargo).ok()
}

fn strip_output(args: &[String]) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut iterator = args.iter();
    while let Some(arg) = iterator.next() {
        if arg == "--" {
            result.push(arg.clone());
            result.extend(iterator.cloned());
            break;
        }
        match arg.as_str() {
            "--background" | "--incremental" | "--output=json" | "--json" => {}
            "--output" => {
                if iterator.next().map(String::as_str) != Some("json") {
                    return Err("SAIL0811: background output must be json".to_owned());
                }
            }
            _ => result.push(arg.clone()),
        }
    }
    Ok(result)
}

/// Submits verification and immediately returns its durable job identity.
pub(crate) fn background(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let json = args
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "--output" || arg == "--output=json" || arg == "--json");
    match connect_or_start().and_then(|client| submit(&client, args)) {
        Ok(status) => {
            let text = if json {
                serde_json::to_string(&status).unwrap_or_default()
            } else {
                status_line(&status)
            };
            let _ = writeln!(out, "{text}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            let _ = writeln!(err, "error[SAIL0810]: {message}");
            ExitCode::from(2)
        }
    }
}

/// Runs `sail jobs list|status|result|cancel|logs ID [--follow] [--output json]`.
pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let mut run = || -> Result<(), String> {
        let (json, follow, args) = job_arguments(args)?;
        let client = connect_or_start()?;
        let id = args.get(1).cloned().unwrap_or_default();
        if args.len() > 2 {
            return Err("SAIL0811: unexpected job arguments".to_owned());
        }
        let request = match args.first().map(String::as_str) {
            Some("list") if args.len() == 1 && !follow => Request::List { project_dir: None },
            Some("status") if !id.is_empty() && !follow => Request::Status { id },
            Some("result") if !id.is_empty() && !follow => Request::Result { id },
            Some("cancel") if !id.is_empty() && !follow => Request::Cancel { id, superseded: false },
            Some("logs") if !id.is_empty() && !json => return logs(&client, &id, follow, out),
            _ => return Err("SAIL0811: usage: sail jobs list|status|result|cancel|logs ID [--follow] [--output json]".to_owned()),
        };
        let response = client.call(&request).map_err(|error| error.to_string())?;
        let text = if json {
            serde_json::to_string_pretty(&response).map_err(|error| error.to_string())?
        } else {
            response_text(&response)
        };
        writeln!(out, "{text}").map_err(|error| error.to_string())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(err, "error[SAIL0811]: {message}");
            ExitCode::from(2)
        }
    }
}
fn job_arguments(args: &[String]) -> Result<(bool, bool, Vec<String>), String> {
    let mut json = false;
    let mut follow = false;
    let mut rest = Vec::new();
    let mut iterator = args.iter();
    while let Some(arg) = iterator.next() {
        match arg.as_str() {
            "--output" => {
                if iterator.next().map(String::as_str) != Some("json") {
                    return Err("SAIL0811: expected --output json".to_owned());
                }
                json = true;
            }
            "--output=json" | "--json" => json = true,
            "--follow" => follow = true,
            _ if arg.starts_with('-') => return Err(format!("SAIL0811: unknown job option {arg}")),
            _ => rest.push(arg.clone()),
        }
    }
    Ok((json, follow, rest))
}
fn status_line(status: &Status) -> String {
    format!(
        "{} {:?} ({:?}) {} {:?}",
        status.id,
        status.state,
        status.currentness,
        status.project_dir.display(),
        status.args
    )
}
fn response_text(response: &Response) -> String {
    match response {
        Response::Status(status) => status_line(status),
        Response::List(jobs) => {
            if jobs.is_empty() {
                "No retained verification jobs.".to_owned()
            } else {
                jobs.iter().map(status_line).collect::<Vec<_>>().join("\n")
            }
        }
        Response::Result(document) => {
            let mut text = status_line(&document.job);
            if let Some(result) = &document.result
                && let Ok(result) =
                    serde_json::from_value::<sail_api::project::ProjectResult>(result.clone())
            {
                use std::fmt::Write as _;
                for target in result.targets.iter().filter(|target| target.requested) {
                    let _ = write!(
                        text,
                        "\n  {}: {} verified, {} failed, {} unknown; {} established by {}",
                        target.target,
                        target.report.summary.verified,
                        target.report.summary.failed,
                        target.report.summary.unknown,
                        if target.reused {
                            "reused"
                        } else {
                            "verified now"
                        },
                        target.establishing_run
                    );
                }
            }
            if let Some(error) = &document.job.error {
                text.push('\n');
                text.push_str(error);
            }
            text
        }
        Response::Chunk(chunk) => chunk.text.clone(),
        Response::Error(error) => error.clone(),
    }
}

fn logs(client: &Client, id: &str, follow: bool, out: &mut dyn Write) -> Result<(), String> {
    let mut offset = 0;
    loop {
        match client
            .call(&Request::Logs {
                id: id.to_owned(),
                offset,
            })
            .map_err(|error| error.to_string())?
        {
            Response::Chunk(chunk) => {
                write!(out, "{}", chunk.text).map_err(|error| error.to_string())?;
                out.flush().map_err(|error| error.to_string())?;
                let empty = chunk.next == offset;
                offset = chunk.next;
                if !follow || (chunk.complete && empty) {
                    return Ok(());
                }
                if empty {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            _ => return Err("SAIL0802: unexpected log response".to_owned()),
        }
    }
}
