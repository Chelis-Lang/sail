//! Optional, bounded handoff to daemon-owned preparation.

use std::io;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use sail_api::jobs::{Request, Response};
use sail_api::preparation::{Origin, Outcome, Submission, VERSION};
use sail_api::project::{Currentness, ProjectResult};
use sail_verus::managed::{Config, noop};

const HANDOFF_WAIT: Duration = Duration::from_millis(25);
const CAPTURE_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Default)]
struct Capture {
    bytes: Vec<u8>,
    overflow: bool,
}

impl io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let remaining = CAPTURE_LIMIT.saturating_sub(self.bytes.len());
        self.bytes
            .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
        self.overflow |= bytes.len() > remaining;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn enabled() -> bool {
    std::env::var_os("SAIL_BACKGROUND_PREPARE").is_none_or(|value| value == "1")
}
fn connect(config: &Config) -> Option<UnixStream> {
    let socket = sail_api::jobs::socket_path(config.socket.as_ref()?);
    connection(&socket)
}
fn connection(socket: &Path) -> Option<UnixStream> {
    let stream = UnixStream::connect(socket).ok()?;
    stream.set_read_timeout(Some(HANDOFF_WAIT)).ok()?;
    stream.set_write_timeout(Some(HANDOFF_WAIT)).ok()?;
    Some(stream)
}

/// Retaining this connection keeps maintenance below the foreground call.
pub(crate) fn foreground(config: &Config) -> Option<UnixStream> {
    let mut stream = connect(config)?;
    sail_api::jobs::write_frame(
        &mut stream,
        &Request::Foreground {
            version: VERSION,
            project_dir: config.workspace.clone(),
            args: config.args.clone(),
        },
    )
    .ok()?;
    match sail_api::jobs::read_frame::<Response>(&mut stream).ok()? {
        Response::Preparation(status) if status.version == VERSION => Some(stream),
        _ => None,
    }
}
fn handoff(config: &Config, request: Submission) {
    let Some(mut stream) = connect(config) else {
        return;
    };
    let _ = sail_api::jobs::write_frame(&mut stream, &Request::Prepare(request));
    // Handoff never waits for preparation or result-currentness computation.
}

fn verification(config: &Config) -> Option<sail_api::jobs::Submission> {
    Some(sail_api::jobs::Submission {
        mode: sail_api::jobs::Mode::Stock,
        project_dir: config.workspace.clone(),
        args: config.args.clone(),
        force: false,
        environment: crate::jobs::environment().ok()?,
        input_revision: None,
    })
}
fn eligible(config: &Config, result: &ProjectResult) -> bool {
    let exact_scope = sail_verus::managed::partial(&config.args);
    !config.force
        && result.exit_code == Some(0)
        && result.currentness == Currentness::Current
        && result.engine.is_none()
        && result.no_reuse.is_none()
        && result.targets.iter().any(|target| target.requested)
        && result
            .targets
            .iter()
            .filter(|target| target.requested)
            .all(|target| target.complete || exact_scope)
}

pub(crate) fn after_verification(config: &Config, json: bool, result: &ProjectResult) {
    if !enabled() || !eligible(config, result) || result.execution.verifier_invocations == 0 {
        return;
    }
    let (Some(key), Some(verification)) = (noop::key(config, json), verification(config)) else {
        return;
    };
    handoff(
        config,
        Submission {
            version: VERSION,
            verification,
            json,
            colour: noop::caller_colour(&config.args, json),
            origin: Some(Origin {
                key,
                context: result.context_sha256.clone(),
                revision: result.input_revision.clone(),
            }),
        },
    );
}

fn new_submission(directory: &Path) -> Option<Submission> {
    let directory = directory.canonicalize().ok()?;
    let mut environment = crate::jobs::environment().ok()?;
    environment.insert("PWD".into(), directory.display().to_string());
    Some(Submission {
        version: VERSION,
        verification: sail_api::jobs::Submission {
            mode: sail_api::jobs::Mode::Stock,
            project_dir: directory,
            args: Vec::new(),
            force: false,
            environment,
            input_revision: None,
        },
        origin: None,
        json: false,
        colour: noop::caller_colour(&[], false),
    })
}

pub(crate) fn new_project(directory: &Path) {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};
    if !enabled() {
        return;
    }
    let Some(request) = new_submission(directory) else {
        return;
    };
    let Ok(cache) = sail_verus::cache_dir() else {
        return;
    };
    let explicit = std::env::var_os("SAIL_DAEMON_SOCKET").map(std::path::PathBuf::from);
    let socket = sail_api::jobs::cache_socket(
        &cache,
        explicit.as_deref(),
        &request.verification.project_dir,
    );
    if let Some(mut stream) = connection(&sail_api::jobs::socket_path(&socket)) {
        let _ = sail_api::jobs::write_frame(&mut stream, &Request::Prepare(request));
        return;
    }
    // The daemon itself owns initial preparation. Project creation never waits
    // for daemon startup, toolchain probing, Cargo, downloads or verification.
    // An explicitly selected unavailable service is never replaced.
    if explicit.is_some() {
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let _ = Command::new(executable)
        .arg("_serve-prepare-new")
        .arg(&request.verification.project_dir)
        .arg(if request.colour { "1" } else { "0" })
        .args(["--socket"])
        .arg(socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
}

pub(crate) fn serve_new(
    args: &[String],
    err: &mut dyn io::Write,
    extensions: &crate::Extensions,
) -> ExitCode {
    let [directory, colour, args @ ..] = args else {
        return ExitCode::from(2);
    };
    let colour = match colour.as_str() {
        "0" => false,
        "1" => true,
        _ => return ExitCode::from(2),
    };
    let Some(mut request) = new_submission(Path::new(directory)) else {
        return ExitCode::from(2);
    };
    request.colour = colour;
    crate::serve_prepared(args, err, extensions, Some(request))
}

/// A private child runs ordinary verification and records only its real output.
pub(crate) fn worker(
    args: &[String],
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> ExitCode {
    let outcome = worker_result(args);
    match outcome {
        Ok(outcome) => {
            if serde_json::to_writer(out, &outcome).is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            let _ = writeln!(err, "error[SAIL0806]: preparation declined: {error}");
            ExitCode::from(1)
        }
    }
}
fn worker_result(args: &[String]) -> Result<Outcome, String> {
    crate::portfolio::guard_parent();
    let path = args.first().ok_or("missing private preparation request")?;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let task: Submission = sail_api::jobs::read_frame(&mut file).map_err(|e| e.to_string())?;
    if task.version != VERSION
        || task.verification.force
        || task.verification.mode != sail_api::jobs::Mode::Stock
    {
        return Err("unsupported preparation request".into());
    }
    let mut config = crate::managed_cli::config(task.verification.args.clone(), task.json)?;
    config.invocation_colour = Some(task.colour);
    config.force = false;
    let key = noop::key(&config, task.json).ok_or("invocation cannot be recorded")?;
    if task.origin.as_ref().is_some_and(|origin| origin.key != key) {
        return Err("invocation changed".into());
    }
    let (mut stdout, mut stderr) = (Capture::default(), Capture::default());
    let shown: Option<&mut dyn io::Write> = if task.json { None } else { Some(&mut stdout) };
    let (result, pending) = sail_verus::managed::run_for_replay(&config, shown, &mut stderr, &key)
        .map_err(|error| error.to_string())?;
    if stdout.overflow || stderr.overflow {
        return Err("output exceeds preparation capture limit".into());
    }
    if !eligible(&config, &result) {
        return Err("complete Current proof unavailable".into());
    }
    let origin = Origin {
        key,
        context: result.context_sha256.clone(),
        revision: result.input_revision.clone(),
    };
    if let Some(expected) = &task.origin
        && *expected != origin
    {
        return Err("source or verification context changed".into());
    }
    let can_record =
        task.origin.is_some() && result.execution.verifier_invocations == 0 && pending.is_some();
    let ready =
        can_record && pending.is_some_and(|pending| pending.commit(stdout.bytes, stderr.bytes));
    Ok(Outcome {
        ready,
        origin: task.origin.is_none().then_some(origin),
    })
}

#[cfg(test)]
mod tests {
    use super::{CAPTURE_LIMIT, Capture};
    use std::io::Write as _;

    #[test]
    fn oversized_capture_drains_output_but_cannot_be_recorded() -> std::io::Result<()> {
        let mut capture = Capture::default();
        capture.write_all(&vec![b'a'; CAPTURE_LIMIT])?;
        assert!(!capture.overflow);
        capture.write_all(b"tail")?;
        assert!(capture.overflow);
        assert_eq!(capture.bytes.len(), CAPTURE_LIMIT);
        Ok(())
    }
}
