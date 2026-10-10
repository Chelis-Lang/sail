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
                .map_err(|error| {
                    io::Error::other(format!("blocked_write={blocked_write}: readiness: {error}"))
                })?;
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

#[test]
fn completed_cached_reply_is_refused_when_cancelled_at_final_admission() -> io::Result<()> {
    let expected: Response = serde_json::from_value(serde_json::json!({
        "response":"result", "value":{
            "job":{"version":1,"id":"owned","project_dir":"/project","args":[],
            "state":"completed","submitted_at_ms":1,"started_at_ms":2,
            "finished_at_ms":3,"exit_code":0,"input_revision":"revision",
            "currentness":"current","error":null},
            "result":{"proof":"complete-model"}}}))
    .map_err(io::Error::other)?;
    let reply = expected.clone();
    let server = Server::start(move |mut stream, _| {
        let _: Request = read_frame(&mut stream)?;
        write_frame(&mut stream, &reply)
    })?;
    let client = Client::new(&server.socket);
    let mut seeded = Vec::new();
    write_frame(&mut seeded, &expected)?;
    client
        .decoded
        .as_ref()
        .ok_or_else(|| io::Error::other("cache absent"))?
        .read(&mut std::io::Cursor::new(&seeded))?;
    client
        .decoded
        .as_ref()
        .ok_or_else(|| io::Error::other("cache absent"))?
        .read(&mut std::io::Cursor::new(&seeded))?;
    let caller = std::thread::current().id();
    let caller_checks = AtomicUsize::new(0);
    let error = client
        .call_cancellable(&request(), || {
            // The caller's third check occurs after receiving the full fresh frame
            // and joining its monitor. This models cancellation at final admission.
            std::thread::current().id() == caller
                && caller_checks.fetch_add(1, Ordering::Relaxed) == 2
        })
        .err()
        .ok_or_else(|| io::Error::other("late cached success admitted"))?;
    assert_eq!(caller_checks.load(Ordering::Relaxed), 3);
    assert_eq!(error.io_error().kind(), io::ErrorKind::Interrupted);
    assert!(!error.is_pre_rpc_connect_failure());
    Ok(())
}

#[test]
#[allow(
    clippy::panic,
    reason = "Exercises an external SDK cancellation callback unwind."
)]
fn callback_unwind_breaks_blocked_read_and_joins_monitor() -> io::Result<()> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let server = Server::start(move |mut stream, _| {
        let _: Request = read_frame(&mut stream)?;
        ready_tx.send(()).map_err(io::Error::other)?;
        let mut byte = [0];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "failed monitor did not close socket"
        );
        Ok(())
    })?;
    let caller = std::thread::current().id();
    let ready_rx = Mutex::new(ready_rx);
    let monitor_failed = AtomicBool::new(false);
    let error = Client::new(&server.socket)
        .call_cancellable(&request(), || {
            if std::thread::current().id() != caller {
                // Wait for actual request bytes; this is an after-send failure.
                // try_recv stays prompt while the server reads the request.
                if ready_rx
                    .lock()
                    .is_ok_and(|receiver| receiver.try_recv().is_ok())
                {
                    monitor_failed.store(true, Ordering::Release);
                    panic!("external callback failure");
                }
            }
            false
        })
        .err()
        .ok_or_else(|| io::Error::other("failed callback admitted success"))?;
    assert!(monitor_failed.load(Ordering::Acquire));
    assert!(!error.is_pre_rpc_connect_failure());
    assert!(error.io_error().to_string().contains("SAIL0806"));
    Ok(())
}

#[test]
fn controlled_send_buffer_proves_blocked_write_cancellation() -> io::Result<()> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = Server::start(move |mut stream, _| {
        let mut length = [0; 4];
        stream.read_exact(&mut length)?;
        ready_tx.send(()).map_err(io::Error::other)?;
        let _ = release_rx.recv_timeout(TRANSPORT);
        Ok(())
    })?;
    let stream = UnixStream::connect(&server.socket)?;
    let socket = socket2::SockRef::from(&stream);
    socket.set_send_buffer_size(4096)?;
    let capacity = socket.send_buffer_size()?;
    let payload = capacity
        .checked_mul(8)
        .ok_or_else(|| io::Error::other("buffer overflow"))?;
    assert!(payload > capacity && payload < super::super::MAX_FRAME / 2);
    let request = Request::Result {
        id: "x".repeat(payload),
    };
    let client = Client::new(&server.socket);
    let cancelled = AtomicBool::new(false);
    let error = std::thread::scope(|scope| -> io::Result<_> {
        let call = scope.spawn(|| {
            client.exchange_cancellable(
                stream,
                &request,
                &|| cancelled.load(Ordering::Acquire),
                Instant::now(),
            )
        });
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(io::Error::other)?;
        // The peer consumed only the header. The larger-than-SO_SNDBUF body
        // cannot finish writing until it drains data; the call is still owned.
        assert!(!call.is_finished());
        cancelled.store(true, Ordering::Release);
        let result = call.join().map_err(|_| io::Error::other("caller failed"))?;
        release_tx.send(()).map_err(io::Error::other)?;
        result
            .err()
            .ok_or_else(|| io::Error::other("blocked write admitted success"))
    })?;
    assert_eq!(error.io_error().kind(), io::ErrorKind::Interrupted);
    assert!(!error.is_pre_rpc_connect_failure());
    Ok(())
}

#[test]
fn full_unix_backlog_refuses_before_rpc_without_blocking_connect() -> io::Result<()> {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let root = PathBuf::from(format!(
        "/tmp/sail-backlog-{}-{}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root)?;
    let socket = root.join("rpc.sock");
    let _owner = Server {
        root,
        socket: socket.clone(),
        thread: None,
    };
    let address = socket2::SockAddr::unix(&socket)?;
    let listener = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
    listener.bind(&address)?;
    listener.listen(1)?;
    let mut held = Vec::new();
    let mut saturated = false;
    for _ in 0..8 {
        let connecting = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
        connecting.set_nonblocking(true)?;
        match connecting.connect(&address) {
            Ok(()) => held.push(connecting),
            // Linux reports EAGAIN for a full Unix backlog; macOS reports
            // ECONNREFUSED. The owned listener and filled peers stay alive.
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::ConnectionRefused
                ) =>
            {
                saturated = true;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    assert!(saturated && !held.is_empty(), "owned backlog not saturated");
    let error = Client::new(&socket)
        .call_cancellable(&request(), || false)
        .err()
        .ok_or_else(|| io::Error::other("full backlog admitted a reply"))?;
    assert!(error.is_pre_rpc_connect_failure(), "{error}");
    // Only the already-filled connections exist, and none carries RPC bytes.
    for _ in &held {
        let (peer, _) = listener.accept()?;
        peer.set_nonblocking(true)?;
        let descriptor: std::os::fd::OwnedFd = peer.into();
        let mut peer = UnixStream::from(descriptor);
        let mut byte = [0];
        assert_eq!(
            peer.read(&mut byte).err().map(|error| error.kind()),
            Some(io::ErrorKind::WouldBlock)
        );
    }
    Ok(())
}
