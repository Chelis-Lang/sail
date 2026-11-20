//! Bound the external check independently of the checker's own deadline.
use super::{MAX_BYTES, failure, stopped};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub(super) fn stop(cancel: &AtomicBool) -> (u8, Vec<u8>) {
    if cancel.load(Ordering::Relaxed) {
        (
            4,
            br#"{"verdict":"stopped","reason":"symbolic session cancelled"}"#.to_vec(),
        )
    } else {
        stopped()
    }
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
        thread::sleep(Duration::from_millis(100));
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
) -> Result<(u8, Vec<u8>), String> {
    if cancel.load(Ordering::Relaxed) || started.elapsed() >= budget {
        return Ok(stop(cancel));
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
        .map_err(failure)?;
    let pipe = child
        .stdout
        .take()
        .ok_or_else(|| failure("missing Dive output pipe"))?;
    let read = match thread::Builder::new()
        .name("sail-symbolic-report".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            pipe.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
            Ok::<_, std::io::Error>(bytes)
        }) {
        Ok(reader) => reader,
        Err(error) => {
            terminate(&mut child);
            return Err(failure(error));
        }
    };
    let mut reader = Some(read);
    let mut output = None;
    loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() >= budget {
            terminate(&mut child);
            return Ok(stop(cancel));
        }
        if reader.as_ref().is_some_and(thread::JoinHandle::is_finished)
            && let Some(done) = reader.take()
        {
            match done.join() {
                Ok(Ok(bytes)) if bytes.len() as u64 <= MAX_BYTES => output = Some(bytes),
                _ => {
                    terminate(&mut child);
                    return Err(failure(
                        "unreadable Dive report or report larger than 16 MiB",
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
                    return Err(failure(error));
                }
            }
        }
        thread::sleep(Duration::from_millis(2));
    }
}
