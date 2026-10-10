//! Owned real-socket cancellation and complete-frame controls.
use super::*;
use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

struct Server {
    root: PathBuf,
    socket: PathBuf,
    thread: Option<std::thread::JoinHandle<io::Result<()>>>,
}
impl Server {
    fn start(
        handler: impl FnOnce(UnixStream, PathBuf) -> io::Result<()> + Send + 'static,
    ) -> io::Result<Self> {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from(format!(
            "/tmp/sail-cancel-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        let socket = root.join("rpc.sock");
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        let path = socket.clone();
        let thread = std::thread::spawn(move || {
            let started = Instant::now();
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == io::ErrorKind::WouldBlock
                            && started.elapsed() < Duration::from_secs(2) =>
                    {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => return Err(error),
                }
            };
            // Accepted sockets inherit the nonblocking listener flag on macOS.
            // This fixture exercises blocking transport, like the real daemon.
            stream.set_nonblocking(false)?;
            stream.set_read_timeout(Some(TRANSPORT))?;
            stream.set_write_timeout(Some(TRANSPORT))?;
            handler(stream, path)
        });
        Ok(Self {
            root,
            socket,
            thread: Some(thread),
        })
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn request() -> Request {
    Request::Result {
        id: "owned-job".into(),
    }
}
fn response() -> Response {
    Response::Capabilities {
        proof_version: crate::proof::VERSION,
        repair: true,
    }
}

#[test]
fn only_initial_connect_failures_admit_startup_fallback() -> io::Result<()> {
    let client = Client::new(format!(
        "/tmp/sail-never-created-{}.sock",
        std::process::id()
    ));
    let error = client
        .call_cancellable(&request(), || false)
        .err()
        .ok_or_else(|| io::Error::other("connect succeeded"))?;
    assert!(error.is_pre_rpc_connect_failure());
    assert_eq!(error.io_error().kind(), io::ErrorKind::NotFound);
    let cancelled = client
        .call_cancellable(&request(), || true)
        .err()
        .ok_or_else(|| io::Error::other("cancel ignored"))?;
    assert!(!cancelled.is_pre_rpc_connect_failure());
    assert_eq!(cancelled.io_error().kind(), io::ErrorKind::Interrupted);
    let server = Server::start(|stream, _| {
        let _ = stream.shutdown(Shutdown::Both);
        Ok(())
    })?;
    let error = Client::new(&server.socket)
        .call_cancellable(&request(), || false)
        .err()
        .ok_or_else(|| io::Error::other("closed peer succeeded"))?;
    assert!(!error.is_pre_rpc_connect_failure());
    Ok(())
}

#[test]
fn negotiated_rpc_cannot_retry_after_second_connect_failure() -> io::Result<()> {
    let server = Server::start(|mut stream, path| {
        let received: Request = read_frame(&mut stream)?;
        assert!(matches!(received, Request::Capabilities));
        std::fs::remove_file(path)?;
        write_frame(&mut stream, &response())
    })?;
    let submit = Request::Submit {
        submission: super::super::Submission {
            mode: Mode::Interactive,
            project_dir: "/project".into(),
            args: Vec::new(),
            force: false,
            environment: BTreeMap::new(),
            input_revision: None,
        },
    };
    let error = Client::new(&server.socket)
        .call_cancellable(&submit, || false)
        .err()
        .ok_or_else(|| io::Error::other("second connect succeeded"))?;
    assert!(!error.is_pre_rpc_connect_failure());
    assert_eq!(error.io_error().kind(), io::ErrorKind::NotFound);
    Ok(())
}

#[test]
fn blocked_read_and_write_cancel_and_leave_no_polling_monitor() -> io::Result<()> {
    for blocked_write in [false, true] {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = Server::start(move |mut stream, _| {
            if blocked_write {
                let mut length = [0; 4];
                stream.read_exact(&mut length)?;
            } else {
                let _: Request = read_frame(&mut stream)?;
            }
            ready_tx.send(()).map_err(io::Error::other)?;
            let _ = release_rx.recv_timeout(TRANSPORT);
            Ok(())
        })?;
        let client = Client::with_result_decoding_cache(&server.socket);
        let cancelled = AtomicBool::new(false);
        let polls = AtomicUsize::new(0);
        let request = if blocked_write {
            Request::Result {
                id: "x".repeat(8 * 1024 * 1024),
            }
        } else {
            request()
        };
        let result = std::thread::scope(|scope| -> io::Result<_> {
            let call = scope.spawn(|| {
                client.call_cancellable(&request, || {
                    polls.fetch_add(1, Ordering::Relaxed);
                    cancelled.load(Ordering::Acquire)
                })
            });
            ready_rx
                .recv_timeout(Duration::from_secs(2))
                .map_err(io::Error::other)?;
            cancelled.store(true, Ordering::Release);
            let result = call.join().map_err(|_| io::Error::other("caller failed"))?;
            release_tx.send(()).map_err(io::Error::other)?;
            Ok(result)
        })?;
        let error = result
            .err()
            .ok_or_else(|| io::Error::other("cancel ignored"))?;
        assert_eq!(error.io_error().kind(), io::ErrorKind::Interrupted);
        assert!(!error.is_pre_rpc_connect_failure());
        let finished = polls.load(Ordering::Acquire);
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(polls.load(Ordering::Acquire), finished);
    }
    Ok(())
}

#[test]
fn slow_fragmented_reply_obeys_whole_transport_deadline_and_joins_owner() -> io::Result<()> {
    let mut frame = Vec::new();
    write_frame(&mut frame, &response())?;
    let length = frame.len();
    let sent = Arc::new(AtomicUsize::new(0));
    let produced = Arc::clone(&sent);
    let server = Server::start(move |mut stream, _| {
        let _: Request = read_frame(&mut stream)?;
        for byte in frame {
            if stream.write_all(&[byte]).is_err() {
                break;
            }
            produced.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(80));
        }
        Ok(())
    })?;
    let started = Instant::now();
    let error = Client::new(&server.socket)
        .call_cancellable(&request(), || false)
        .err()
        .ok_or_else(|| io::Error::other("dribbled response succeeded"))?;
    assert_eq!(error.io_error().kind(), io::ErrorKind::TimedOut);
    assert!(!error.is_pre_rpc_connect_failure());
    assert!(started.elapsed() >= TRANSPORT);
    drop(server);
    assert!(sent.load(Ordering::Acquire) < length);
    Ok(())
}

#[test]
fn cached_frame_does_not_complete_a_cancelled_rpc_without_fresh_response() -> io::Result<()> {
    let expected:Response=serde_json::from_value(serde_json::json!({"response":"result","value":{"job":{"version":1,"id":"owned","project_dir":"/project","args":[],"state":"completed","submitted_at_ms":1,"started_at_ms":2,"finished_at_ms":3,"exit_code":0,"input_revision":"revision","currentness":"current","error":null},"result":{"proof":"complete-model"}}})).map_err(io::Error::other)?;
    let mut frame = Vec::new();
    write_frame(&mut frame, &expected)?;
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = Server::start(move |mut stream, _| {
        let _: Request = read_frame(&mut stream)?;
        ready_tx.send(()).map_err(io::Error::other)?;
        let _ = release_rx.recv_timeout(TRANSPORT);
        let _ = stream.write_all(&frame);
        Ok(())
    })?;
    let client = Client::with_result_decoding_cache(&server.socket);
    let mut seeded = Vec::new();
    write_frame(&mut seeded, &expected)?;
    client
        .decoded
        .as_ref()
        .ok_or_else(|| io::Error::other("cache absent"))?
        .read(&mut std::io::Cursor::new(seeded))?;
    let cancelled = AtomicBool::new(false);
    let error = std::thread::scope(|scope| -> io::Result<_> {
        let call = scope
            .spawn(|| client.call_cancellable(&request(), || cancelled.load(Ordering::Acquire)));
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(io::Error::other)?;
        assert!(!call.is_finished());
        cancelled.store(true, Ordering::Release);
        let result = call.join().map_err(|_| io::Error::other("caller failed"))?;
        release_tx.send(()).map_err(io::Error::other)?;
        result
            .err()
            .ok_or_else(|| io::Error::other("late cached reply returned"))
    })?;
    assert_eq!(error.io_error().kind(), io::ErrorKind::Interrupted);
    Ok(())
}

#[test]
fn complete_normal_reply_preserves_graph_and_wakes_owned_monitor() -> io::Result<()> {
    let server = Server::start(|mut stream, _| {
        let _: Request = read_frame(&mut stream)?;
        write_frame(&mut stream, &response())
    })?;
    let polls = AtomicUsize::new(0);
    let actual = Client::with_result_decoding_cache(&server.socket)
        .call_cancellable(&request(), || {
            polls.fetch_add(1, Ordering::Relaxed);
            false
        })
        .map_err(io::Error::other)?;
    assert_eq!(
        serde_json::to_value(actual).map_err(io::Error::other)?,
        serde_json::to_value(response()).map_err(io::Error::other)?
    );
    let finished = polls.load(Ordering::Acquire);
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(polls.load(Ordering::Acquire), finished);
    Ok(())
}
