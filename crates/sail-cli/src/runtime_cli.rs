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
    if request.is_some() || options.action == "prepare-temporal" {
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
    if options.action == "prepare-temporal" {
        return prepare_temporal(&cache, request.as_ref(), extensions, &cancellation, out);
    }
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

// The installer has one background check per runtime cache. Foreground execution
// never takes this lock and always follows its usual admission and receipt checks.
fn prepare_temporal(
    cache: &Path,
    first_job: Option<&Request>,
    extensions: &crate::Extensions,
    cancellation: &Cancellation,
    out: &mut dyn Write,
) -> Result<u8, String> {
    let probe;
    let request = if let Some(request) = first_job {
        if !matches!(&request.input, sail_api::runtime::Input::Temporal { job }
            if matches!(job.request.operation, sail_api::temporal::Operation::Check { .. }))
        {
            return Err(
                "error[SAIL6101]: temporal preparation requires a temporal check job".into(),
            );
        }
        request
    } else {
        probe = decode_request(include_bytes!("temporal_warmup.json").as_slice())?;
        &probe
    };
    let mut config = configuration(cache.to_owned(), extensions)?;
    if first_job.is_none() {
        // Always exercise the worker. A synthetic installation probe has no
        // caller evidence to publish and should leave no receipt.
        config.cache_directory = None;
        config.temporal_cache_directory = None;
    }
    let mut directories = std::fs::DirBuilder::new();
    directories.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        directories.mode(0o700);
    }
    directories
        .create(cache)
        .map_err(|e| format!("error[SAIL6102]: cannot open preparation cache: {e}"))?;
    let ownership = std::fs::File::open(cache)
        .map_err(|e| format!("error[SAIL6102]: cannot open preparation cache: {e}"))?;
    ownership.try_lock().map_err(|e| {
        format!("error[SAIL6102]: temporal preparation busy or cache cannot be locked: {e}")
    })?;
    // A just-promoted worker is too new for the authenticated image cache.
    // Only the synthetic probe waits. A supplied check starts immediately.
    #[cfg(unix)]
    if first_job.is_none()
        && let Some(worker) = &config.temporal_worker
        && let Ok(metadata) = std::fs::metadata(&worker.executable)
    {
        use sail_api::stamp::{SETTLED, Stamp};
        use std::time::{Duration, Instant, SystemTime};
        let stamp = Stamp::of(&metadata);
        let start = Instant::now();
        while !stamp.settled(SystemTime::now())
            && start.elapsed() < SETTLED
            && !cancellation.is_cancelled()
        {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    let response = Runtime::new(config)
        .and_then(|runtime| runtime.execute(request, cancellation))
        .map_err(|e| e.to_string())?;
    result(out, &response)
}

// Resolve existing components before normalizing "..". An uncreated suffix
// can contain a parent component followed by an existing symlink. Remember
// whether any resolved component traversed the unsigned solver result tree.
fn resolve_cache_path(path: &Path, boundary: Option<&Path>) -> std::io::Result<(PathBuf, bool)> {
    use std::path::Component;
    let mut resolved = PathBuf::new();
    let mut crossed = false;
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                resolved.push(name);
                crossed |= boundary.is_some_and(|root| resolved.starts_with(root));
                match std::fs::symlink_metadata(&resolved) {
                    Ok(metadata) if metadata.is_symlink() => resolved = resolved.canonicalize()?,
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
            other => resolved.push(other.as_os_str()),
        }
        crossed |= boundary.is_some_and(|root| resolved.starts_with(root));
    }
    Ok((resolved, crossed))
}

// Parent permissions and descriptor-relative ownership checks still apply to
// the chosen store. This mapping keeps explicitly selected solver cache roots
// isolated and prevents unsigned solver exports from transporting native proofs.
pub(crate) fn temporal_cache_override(global: &Path, selected: &Path) -> Option<PathBuf> {
    let runtime = global.join("runtime");
    let resolved = resolve_cache_path(&global.join("results"), None).and_then(|(results, _)| {
        resolve_cache_path(selected, Some(&results)).map(|(path, crossed)| (results, path, crossed))
    });
    let scope = match resolved {
        Ok((_, path, false)) => {
            return (std::path::absolute(selected).ok().as_ref() != Some(&path)).then_some(path);
        }
        Ok((results, path, true)) => {
            if path == results || path == results.join("runtime") {
                return Some(runtime);
            }
            path
        }
        // A dangling alias or permission failure cannot justify trusting the
        // selected tree. Derive an isolated location from the caller's scope.
        Err(_) => std::path::absolute(selected).unwrap_or_else(|_| selected.to_owned()),
    };
    // A direct child of the configured root keeps the final directory under
    // the receipt store's O_NOFOLLOW check even if a sibling runtime is a symlink.
    let digest = sail_api::temporal::sha256(scope.as_os_str().as_encoded_bytes());
    Some(global.join(format!("runtime-scope-{digest}")))
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
    // An embedder can omit its temporal backend.
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
        // Development opt-in; default capabilities stay unchanged until qualification.
        cvc5: std::env::var_os("SAIL_RUNTIME_CVC5").map(std::path::PathBuf::from),
        temporal_cache_directory: temporal_cache_override(&root, &cache),
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
    "error[SAIL6101]: usage: sail runtime capabilities [--cache DIR | --socket CACHE_SOCKET] | sail runtime execute --file FILE|- [--cache DIR | --socket CACHE_SOCKET] | sail runtime prepare-temporal [--file FILE|-] [--cache DIR]".into()
}

struct Options<'a> {
    action: &'a str,
    file: Option<PathBuf>,
    cache: Option<PathBuf>,
    socket: Option<PathBuf>,
}

fn options(args: &[String]) -> Result<Options<'_>, String> {
    let action = args.first().map(String::as_str).ok_or_else(usage)?;
    if !matches!(action, "capabilities" | "execute" | "prepare-temporal") {
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
            "--file" if action != "capabilities" && result.file.is_none() => {
                result.file = Some(value.into());
            }
            "--cache" if result.cache.is_none() => result.cache = Some(value.into()),
            "--socket" if action != "prepare-temporal" && result.socket.is_none() => {
                result.socket = Some(value.into());
            }
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
    fn preparation_requires_a_check_and_respects_cache_ownership() -> Result<(), String> {
        let cache = std::env::temp_dir().join(format!("sail-temporal-prep-{}", std::process::id()));
        let prepare = |first: Option<&Request>| {
            prepare_temporal(
                &cache,
                first,
                &crate::Extensions::default(),
                &Cancellation::default(),
                &mut Vec::new(),
            )
        };
        let mut request = decode_request(include_bytes!("temporal_warmup.json").as_slice())?;
        let sail_api::runtime::Input::Temporal { job } = &mut request.input else {
            return Err("expected a temporal worker warmup".into());
        };
        job.request.operation = sail_api::temporal::Operation::Simulate {
            traces: 1,
            depth: 1,
        };
        assert!(prepare(Some(&request)).is_err_and(|e| e.contains("requires a temporal check")));
        // An embedder without a temporal backend reports Unavailable.
        assert_eq!(prepare(None)?, 1);
        let lock = std::fs::File::open(&cache).map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|e| e.to_string())?;
        assert!(prepare(None).is_err_and(|e| e.contains("busy")));
        drop(lock);
        std::fs::remove_dir(cache).map_err(|e| e.to_string())?;
        Ok(())
    }

    #[test]
    fn admission_rejects_unknown_options_and_malformed_documents() {
        for args in [
            vec![],
            vec!["execute"],
            vec!["prepare-temporal", "--socket", "sock"],
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
