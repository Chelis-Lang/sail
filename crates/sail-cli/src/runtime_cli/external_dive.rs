//! Bound the external check independently of the checker's own deadline.
pub(super) const MAX_BYTES: u64 = 16 << 20;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Refuses an external Dive whose `--version` names a different release than
/// the D.I.V.E. Sail links: its reports are read under that release's schema.
pub(super) fn require_release(dive: &Path, code: u8, output: &[u8]) -> Result<(), String> {
    let expected = sail_dive::DIVE_VERSION;
    let text = String::from_utf8_lossy(output);
    let found = text
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("dive "))
        .and_then(|rest| rest.split_whitespace().next())
        .filter(|_| code == 0);
    if found == Some(expected) {
        return Ok(());
    }
    let found = found.map_or_else(
        || "an unrecognized version".to_owned(),
        |v| format!("dive {v}"),
    );
    Err(format!(
        "error[SAIL6103]: {} reports {found}, but this sail reads dive {expected} reports; \
         install the matching release with `cargo install dive-cli --locked --version {expected}` \
         or from https://github.com/Chelis-Lang/dive/releases/tag/v{expected}",
        dive.display()
    ))
}

pub(super) fn cancellation() -> Result<std::sync::Arc<AtomicBool>, String> {
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    for signal in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(signal, std::sync::Arc::clone(&cancel))
            .map_err(|e| format!("error[SAIL6102]: cannot install cancellation handler: {e}"))?;
    }
    Ok(cancel)
}

pub(super) fn stop(cancel: &AtomicBool, session: &str) -> (u8, Vec<u8>) {
    let reason = if cancel.load(Ordering::Relaxed) {
        "cancelled"
    } else {
        "time limit"
    };
    (
        4,
        format!(r#"{{"verdict":"stopped","reason":"{session} session {reason}"}}"#).into_bytes(),
    )
}

fn terminate(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = i32::try_from(child.id())
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    {
        use rustix::process::{Signal, kill_process_group};
        // Let Dive reap its nested symbolic worker even if the supervisor was
        // stopped. The process group is still owned until we reap its leader.
        let _ = kill_process_group(pid, Signal::INT);
        let _ = kill_process_group(pid, Signal::CONT);
        // Python may wait 250 ms inside Popen.wait before propagating
        // KeyboardInterrupt. Allow the supervisor to finish its group cleanup.
        thread::sleep(Duration::from_millis(400));
        let _ = kill_process_group(pid, Signal::KILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub(super) fn run(
    mut command: Command,
    started: Instant,
    budget: Duration,
    cancel: &AtomicBool,
    session: &'static str,
) -> Result<(u8, Vec<u8>), String> {
    let failure = |message: String| format!("error[SAIL6102]: {session} check: {message}");
    if cancel.load(Ordering::Relaxed) || started.elapsed() >= budget {
        return Ok(stop(cancel, session));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("error[SAIL6102]: {session} check: {e}"))?;
    let pipe = child
        .stdout
        .take()
        .ok_or_else(|| failure("missing Dive output pipe".into()))?;
    let read = match thread::Builder::new()
        .name(format!("sail-{session}-report"))
        .spawn(move || {
            let mut bytes = Vec::new();
            pipe.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
            Ok::<_, std::io::Error>(bytes)
        }) {
        Ok(reader) => reader,
        Err(error) => {
            terminate(&mut child);
            return Err(failure(error.to_string()));
        }
    };
    let mut reader = Some(read);
    let mut output = None;
    loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() >= budget {
            terminate(&mut child);
            return Ok(stop(cancel, session));
        }
        if reader.as_ref().is_some_and(thread::JoinHandle::is_finished)
            && let Some(done) = reader.take()
        {
            match done.join() {
                Ok(Ok(bytes)) if bytes.len() as u64 <= MAX_BYTES => output = Some(bytes),
                _ => {
                    terminate(&mut child);
                    return Err(failure(
                        "unreadable Dive report or report larger than 16 MiB".into(),
                    ));
                }
            }
        }
        if output.is_some() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status
                        .code()
                        .and_then(|n| u8::try_from(n).ok())
                        .ok_or_else(|| failure(format!("Dive exited with {status}")))?;
                    return Ok((code, output.take().unwrap_or_default()));
                }
                Ok(None) => (),
                Err(error) => {
                    terminate(&mut child);
                    return Err(failure(error.to_string()));
                }
            }
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::require_release;
    use std::path::Path;

    #[test]
    fn only_the_linked_dive_release_is_admitted() {
        let dive = Path::new("/opt/dive");
        let linked = format!("dive {} (v0.1.0-12-gabc)\n", sail_dive::DIVE_VERSION);
        assert!(require_release(dive, 0, linked.as_bytes()).is_ok());
        for (code, output) in [
            (0, &b"dive 9.9.9 (v9.9.9)\n"[..]),
            (0, b"TLC2 Version 2.19\n"),
            (0, b""),
            (2, linked.as_bytes()),
        ] {
            let error = require_release(dive, code, output)
                .err()
                .unwrap_or_default();
            assert!(
                error.starts_with("error[SAIL6103]: /opt/dive reports "),
                "{error}"
            );
            assert!(
                error.contains("cargo install dive-cli --locked --version"),
                "{error}"
            );
        }
        let error = require_release(dive, 0, b"dive 9.9.9 (v9.9.9)\n")
            .err()
            .unwrap_or_default();
        assert!(error.contains("reports dive 9.9.9, but"), "{error}");
    }
}
