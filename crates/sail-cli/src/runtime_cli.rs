//! Versioned raw-query runtime, separate from Cargo verification commands.
use sail_api::runtime::{Answer, Outcome, Output, Request};
use sail_runtime::{Cancellation, Config, Runtime};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Bound the JSON envelope as well as the decoded script lengths.
const MAX_DOCUMENT_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) fn run(
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
    extensions: &crate::Extensions,
) -> ExitCode {
    match command(args, out, extensions) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            let _ = writeln!(err, "{message}");
            ExitCode::from(2)
        }
    }
}

fn command(
    args: &[String],
    out: &mut dyn Write,
    extensions: &crate::Extensions,
) -> Result<u8, String> {
    let options = options(args)?;
    let request = options.file.as_deref().map(read_request).transpose()?;
    let cancellation = Cancellation::default();
    if request.is_some() {
        let signal = cancellation.clone();
        ctrlc::set_handler(move || signal.cancel())
            .map_err(|e| format!("error[SAIL6102]: cannot install cancellation handler: {e}"))?;
    }
    #[cfg(unix)]
    if let Some(socket) = &options.socket {
        let client = sail_api::runtime::transport::Client::new(
            sail_api::runtime::transport::socket_path(socket),
        );
        if options.action == "capabilities" {
            write_json(out, &client.capabilities().map_err(|e| e.to_string())?)?;
            return Ok(0);
        }
        let response = client
            .execute_cancellable(request.as_ref().ok_or_else(usage)?, || {
                cancellation.is_cancelled()
            })
            .map_err(|e| e.to_string())?;
        return result(out, &response);
    }
    #[cfg(not(unix))]
    if options.socket.is_some() {
        return Err("error[SAIL6201]: runtime sockets require Unix".into());
    }
    let cache = options.cache.unwrap_or(
        sail_verus::cache_dir()
            .map_err(|e| e.to_string())?
            .join("runtime"),
    );
    let runtime = Runtime::new(configuration(cache, extensions)?).map_err(|e| e.to_string())?;
    if options.action == "capabilities" {
        write_json(out, &runtime.capabilities())?;
        return Ok(0);
    }
    let response = runtime
        .execute(request.as_ref().ok_or_else(usage)?, &cancellation)
        .map_err(|e| e.to_string())?;
    result(out, &response)
}

/// Resolve only the existing bundle path; the executor authenticates solver bytes.
/// No Verus process or whole-engine validation runs on daemon startup.
pub(crate) fn configuration(
    cache: PathBuf,
    extensions: &crate::Extensions,
) -> Result<Config, String> {
    let root = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let pins = sail_verus::toolchain::Pins::embedded().map_err(|e| e.to_string())?;
    let build = sail_verus::engine::resolve_build(&root, &pins);
    let z3 = sail_verus::source_build::checkout_dir(&build).join("source/target-verus/release/z3");
    // A missing, unreadable or malformed worker installation only makes
    // temporal jobs unavailable; it never stops the service or `sail verify`.
    // Without a linked backend no worker installation is read (ADR 0083).
    let (temporal_worker, temporal_unavailable) = match std::env::current_exe() {
        _ if extensions.temporal.is_none() => (None, None),
        Ok(sail) => sail_runtime::temporal::locate(&sail),
        Err(error) => (
            None,
            Some(sail_api::runtime::Error {
                code: "SAIL6102".into(),
                kind: "unavailable".into(),
                message: format!("native temporal worker unavailable: {error}"),
            }),
        ),
    };
    Ok(Config {
        temporal_backend: extensions.temporal.clone(),
        temporal_worker,
        temporal_unavailable,
        z3: Some(z3),
        cache_directory: Some(cache),
        workers: 2,
    })
}

#[cfg(unix)]
pub(crate) fn start_service(
    cache_socket: &Path,
    cache: PathBuf,
    activity: std::sync::Arc<std::sync::atomic::AtomicU64>,
    extensions: &crate::Extensions,
) -> Result<sail_daemon::runtime::Handle, String> {
    let runtime = configuration(cache, extensions)?;
    sail_daemon::runtime::bind(
        &sail_api::runtime::transport::socket_path(cache_socket),
        &sail_daemon::runtime::Config {
            runtime,
            ..sail_daemon::runtime::Config::default()
        },
    )
    .and_then(|server| server.with_activity(activity))
    .and_then(sail_daemon::runtime::Server::spawn)
    .map_err(|error| error.to_string())
}

fn result(out: &mut dyn Write, response: &sail_api::runtime::Response) -> Result<u8, String> {
    let complete = match &response.output {
        Output::Temporal { result } => {
            result.outcome == Outcome::Completed
                && result.response.as_ref().is_some_and(|response| {
                    matches!(
                        response.outcome,
                        sail_api::temporal::Outcome::Holds
                            | sail_api::temporal::Outcome::Refuted
                            | sail_api::temporal::Outcome::Sampled
                            | sail_api::temporal::Outcome::Valid
                            | sail_api::temporal::Outcome::Invalid
                    )
                })
        }
        Output::Smt { results } => results.iter().all(|result| {
            result.outcome == Outcome::Completed
                && result
                    .checks
                    .iter()
                    .all(|check| check.answer != Answer::Unknown)
        }),
    };
    write_json(out, response)?;
    Ok(u8::from(!complete))
}

fn usage() -> String {
    "error[SAIL6101]: usage: sail runtime capabilities [--cache DIR | --socket CACHE_SOCKET] | sail runtime execute --file FILE|- [--cache DIR | --socket CACHE_SOCKET]".into()
}

struct Options<'a> {
    action: &'a str,
    file: Option<PathBuf>,
    cache: Option<PathBuf>,
    socket: Option<PathBuf>,
}

fn options(args: &[String]) -> Result<Options<'_>, String> {
    let action = args.first().map(String::as_str).ok_or_else(usage)?;
    if !matches!(action, "capabilities" | "execute") {
        return Err(usage());
    }
    let mut result = Options {
        action,
        file: None,
        cache: None,
        socket: None,
    };
    let mut options = args[1..].iter();
    while let Some(option) = options.next() {
        let value = options.next().ok_or_else(usage)?;
        match option.as_str() {
            "--file" if action == "execute" && result.file.is_none() => {
                result.file = Some(value.into());
            }
            "--cache" if result.cache.is_none() => result.cache = Some(value.into()),
            "--socket" if result.socket.is_none() => result.socket = Some(value.into()),
            _ => return Err(usage()),
        }
    }
    if (action == "execute" && result.file.is_none())
        || (result.socket.is_some() && result.cache.is_some())
    {
        return Err(usage());
    }
    Ok(result)
}

fn read_request(path: &Path) -> Result<Request, String> {
    let input: Box<dyn Read> = if path == Path::new("-") {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path).map_err(|e| {
            format!(
                "error[SAIL6101]: cannot read runtime request {}: {e}",
                path.display()
            )
        })?)
    };
    decode_request(input)
}

fn decode_request(input: impl Read) -> Result<Request, String> {
    let mut bytes = Vec::new();
    input
        .take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("error[SAIL6101]: cannot read runtime request: {e}"))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("error[SAIL6101]: runtime JSON exceeds 128 MiB".into());
    }
    sail_api::temporal::decode(&bytes)
        .map_err(|e| format!("error[SAIL6101]: invalid runtime JSON: {e}"))
}

fn write_json(out: &mut dyn Write, value: &impl serde::Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *out, value)
        .and_then(|()| writeln!(out).map_err(serde_json::Error::io))
        .map_err(|e| format!("error[SAIL6102]: cannot write runtime response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_rejects_unknown_options_and_malformed_documents() {
        for args in [
            vec![],
            vec!["execute"],
            vec!["capabilities", "--file", "x"],
            vec!["execute", "--file", "x", "--file", "y"],
            vec!["execute", "--file", "x", "--cache"],
        ] {
            assert!(options(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert!(decode_request(&b"{}"[..]).is_err());
        assert!(
            decode_request(
                &b"{\"version\":1,\"input\":{\"kind\":\"smt\",\"queries\":[]},\"extra\":true}"[..]
            )
            .is_err()
        );
    }
}
