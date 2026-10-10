//! Offline replay of complete captured frames through the actual jobs client.
#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use sail_api::jobs::{Client, MAX_FRAME, Request, Response, read_frame, write_frame};
use serde_json::{Value, json};

#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [path] = args.as_slice() else {
        return Err("usage: result_client_profile CAPTURED_FRAME".into());
    };
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_FRAME as u64 + 5)
        .read_to_end(&mut bytes)?;
    let mut reader = std::io::Cursor::new(&bytes);
    let expected: Response = read_frame(&mut reader)?;
    if reader.position() != bytes.len() as u64 {
        return Err("extra bytes after captured frame".into());
    }
    let expected = serde_json::to_value(expected)?;
    let bytes = Arc::new(bytes);
    let mut results = serde_json::Map::new();
    for threads in [1, 4] {
        for case in [Case::Generic, Case::ColdClient, Case::WarmClient] {
            results.insert(
                format!("{}_{}", case.name(), threads),
                measure(case, threads, &bytes, &expected)?,
            );
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "kind":"offline_actual_client_full_frame_replay", "live_acceptance":false,
            "frame_bytes":bytes.len(), "frame_sha256":sail_key::sha256(&bytes).iter().fold(String::new(), |mut digest, byte| {let _=write!(digest,"{byte:02x}");digest}),
            "complete_graph_equality_checked":true, "samples_per_case":8,
            "every_call_receives_complete_fresh_frame":true,"results":results
        }))?
    );
    Ok(())
}
#[cfg(not(unix))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("Unix socket replay requires Unix".into())
}

struct StopOnExit(Arc<AtomicBool>);
impl Drop for StopOnExit {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
struct OwnedSocketRoot(std::path::PathBuf);
impl Drop for OwnedSocketRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[cfg(unix)]
fn accept_owned(
    listener: &std::os::unix::net::UnixListener,
    stop: &AtomicBool,
) -> io::Result<std::os::unix::net::UnixStream> {
    let started = Instant::now();
    loop {
        if stop.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "profile cancelled",
            ));
        }
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error),
        }
    }
}

#[derive(Clone, Copy)]
enum Case {
    Generic,
    ColdClient,
    WarmClient,
}
impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Generic => "generic_frame",
            Self::ColdClient => "cold_client",
            Self::WarmClient => "warm_exact_frame_client",
        }
    }
}
#[cfg(unix)]
fn measure(
    case: Case,
    threads: usize,
    bytes: &Arc<Vec<u8>>,
    expected: &Value,
) -> io::Result<Value> {
    use std::os::unix::net::{UnixListener, UnixStream};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let root = std::path::Path::new("/tmp").join(format!(
        "scp-{}-{}-{threads}-{}",
        std::process::id(),
        case.name(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root)?;
    let owned_root = OwnedSocketRoot(root.clone());
    let socket = root.join("rpc.sock");
    let listener = UnixListener::bind(&socket)?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let client = Client::with_result_decoding_cache(&socket);
    let warmup = matches!(case, Case::WarmClient);
    let request = Request::Result {
        id: "exact-captured-job".into(),
    };
    let measured = std::thread::scope(|scope| -> io::Result<Value> {
        let server = scope.spawn(|| -> io::Result<()> {
            for _ in 0..(8 + usize::from(warmup)) {
                let mut stream = accept_owned(&listener, &stop)?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                let _: Request = read_frame(&mut stream)?;
                stream.write_all(bytes)?;
            }
            Ok(())
        });
        let _stop_on_exit = StopOnExit(Arc::clone(&stop));
        if warmup {
            validate(client.call(&request)?, expected)?;
        }
        let mut samples = Vec::new();
        for _ in 0..(8 / threads) {
            let barrier = Barrier::new(threads);
            let wave = std::thread::scope(|inner| -> io::Result<Vec<(f64, Response)>> {
                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        inner.spawn(|| -> io::Result<_> {
                            barrier.wait();
                            let started = Instant::now();
                            let reply = match case {
                                Case::WarmClient => client.call(&request)?,
                                Case::ColdClient => {
                                    Client::with_result_decoding_cache(&socket).call(&request)?
                                }
                                Case::Generic => {
                                    let mut stream = UnixStream::connect(&socket)?;
                                    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                                    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                                    write_frame(&mut stream, &request)?;
                                    read_frame(&mut stream)?
                                }
                            };
                            Ok((started.elapsed().as_secs_f64() * 1000.0, reply))
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|handle| {
                        handle
                            .join()
                            .map_err(|_| io::Error::other("profile reader failed"))?
                    })
                    .collect()
            })?;
            // Full graph validation and destruction are outside each timed wave.
            for (elapsed, reply) in wave {
                validate(reply, expected)?;
                samples.push(elapsed);
            }
        }
        server
            .join()
            .map_err(|_| io::Error::other("profile server failed"))??;
        samples.sort_by(f64::total_cmp);
        Ok(
            json!({"threads":threads,"samples_ms":samples,"median_ms":samples[3].midpoint(samples[4]),"p95_ms":samples[7]}),
        )
    });
    drop(owned_root);
    measured
}
fn validate(reply: Response, expected: &Value) -> io::Result<()> {
    if serde_json::to_value(reply).map_err(io::Error::other)? != *expected {
        return Err(io::Error::other("entire decoded response graph changed"));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn malformed_warmup_cancels_owned_server_instead_of_waiting_for_unused_connections()
    -> io::Result<()> {
        let mut bytes = Vec::new();
        let payload = b"{not-json}";
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .map_err(io::Error::other)?
                .to_be_bytes(),
        );
        bytes.extend_from_slice(payload);
        let started = Instant::now();
        assert!(measure(Case::WarmClient, 1, &Arc::new(bytes), &Value::Null).is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        Ok(())
    }
    #[test]
    fn graph_validation_failure_signals_owned_server_and_returns_original_error() -> io::Result<()>
    {
        let mut bytes = Vec::new();
        write_frame(
            &mut bytes,
            &Response::Capabilities {
                proof_version: 1,
                repair: true,
            },
        )?;
        let started = Instant::now();
        let error = measure(Case::WarmClient, 1, &Arc::new(bytes), &Value::Null)
            .err()
            .ok_or_else(|| io::Error::other("expected graph refusal"))?;
        assert!(
            error
                .to_string()
                .contains("entire decoded response graph changed")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        Ok(())
    }
}
