//! Actual endpoint/client tests with separate cancellation and malformed frames.
use super::{Config, Registration, bind};
use sail_api::runtime::transport::{
    self, Client, Deadline, IO_TIMEOUT, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request, Response,
};
use sail_api::runtime::{self as api, Input, Limits, Outcome, Output, Query};
use std::{
    io,
    os::unix::{fs::PermissionsExt as _, net::UnixStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Directory(PathBuf);
impl Directory {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = PathBuf::from("/tmp").join(format!(
            "sail-rt-service-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn socket(&self) -> PathBuf {
        transport::socket_path(&self.0.join("cache.sock"))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn request(script: &str) -> api::Request {
    api::Request {
        version: api::VERSION,
        input: Input::Smt {
            queries: vec![Query {
                obligation_id: "obligation".into(),
                requirement_ids: vec!["REQ-1".into()],
                backend: "z3".into(),
                script: script.into(),
                limits: Limits::default(),
            }],
        },
    }
}

fn call(socket: &std::path::Path, request: &Request) -> io::Result<Response> {
    let stream = UnixStream::connect(socket)?;
    let mut deadline = Deadline::new(&stream, IO_TIMEOUT);
    transport::write_frame(&mut deadline, request, MAX_REQUEST_BYTES)?;
    transport::read_frame(&mut deadline, MAX_RESPONSE_BYTES)
}

#[test]
fn endpoint_serves_capabilities_and_unavailable_without_a_false_answer() -> TestResult {
    let directory = Directory::new()?;
    let socket = directory.socket();
    let mut handle = bind(&socket, &Config::default())?.spawn()?;
    let client = Client::new(&socket);
    assert!(!client.capabilities()?.backends[0].installed);
    let Output::Smt { results } = client.execute(&request("(check-sat)"))?.output;
    assert_eq!(results[0].outcome, Outcome::Unavailable);
    assert_eq!(results[0].checks, []);
    assert!(
        client
            .execute(&request("(echo \"unsat\") (check-sat)"))
            .is_err()
    );
    let finished = Instant::now();
    while handle.activity().load(Ordering::SeqCst) != 0
        && finished.elapsed() < Duration::from_secs(1)
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(handle.activity().load(Ordering::SeqCst), 0);
    handle.shutdown();
    assert!(!socket.exists());
    assert!(client.execute(&request("(check-sat)")).is_err());
    Ok(())
}

#[test]
fn control_requests_remain_responsive_at_the_execution_cap_and_duplicates_are_refused() -> TestResult
{
    let directory = Directory::new()?;
    let socket = directory.socket();
    let server = bind(
        &socket,
        &Config {
            max_active: 1,
            ..Config::default()
        },
    )?;
    let state = Arc::clone(&server.state);
    let token = sail_runtime::Cancellation::default();
    let registration = Registration::new(&state, "held-id", token.clone())?;
    let mut handle = server.spawn()?;
    let client = Client::new(&socket);
    let started = Instant::now();
    assert!(client.capabilities().is_ok());
    let duplicate = call(
        &socket,
        &Request::Execute {
            id: "held-id".into(),
            request: request("(check-sat)"),
        },
    )?;
    assert!(matches!(duplicate, Response::Error { error } if error.message.contains("duplicate")));
    let busy = call(
        &socket,
        &Request::Execute {
            id: "other-id".into(),
            request: request("(check-sat)"),
        },
    )?;
    assert!(matches!(busy, Response::Error { error } if error.message.contains("execution limit")));
    assert!(client.cancel("held-id")?);
    assert!(token.is_cancelled());
    assert!(started.elapsed() < Duration::from_secs(3));
    drop(registration);
    assert_eq!(handle.activity().load(Ordering::SeqCst), 0);
    assert!(!client.cancel("held-id")?);
    handle.shutdown();
    Ok(())
}

#[test]
fn malformed_frames_and_wrong_permissions_fail_closed_and_do_not_poison_the_service() -> TestResult
{
    use std::io::Write as _;
    let directory = Directory::new()?;
    let socket = directory.socket();
    let mut handle = bind(&socket, &Config::default())?.spawn()?;
    let mut stream = UnixStream::connect(&socket)?;
    stream.write_all(&u32::MAX.to_be_bytes())?;
    let response: Response =
        transport::read_frame(&mut Deadline::new(&stream, IO_TIMEOUT), MAX_RESPONSE_BYTES)?;
    assert!(matches!(response, Response::Error { .. }));
    let client = Client::new(&socket);
    assert!(client.capabilities().is_ok());
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o666))?;
    assert!(client.capabilities().is_err());
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    let parent = socket.parent().ok_or("parent missing")?;
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o755))?;
    assert!(client.capabilities().is_err());
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    assert!(client.capabilities().is_ok());
    handle.shutdown();
    Ok(())
}

#[test]
fn symlink_parent_is_never_used_and_live_endpoint_is_never_replaced() -> TestResult {
    let directory = Directory::new()?;
    let socket = directory.socket();
    let mut handle = bind(&socket, &Config::default())?.spawn()?;
    assert!(bind(&socket, &Config::default()).is_err());
    assert!(Client::new(&socket).capabilities().is_ok());
    let link = directory.0.join("alias");
    std::os::unix::fs::symlink(socket.parent().ok_or("parent")?, &link)?;
    assert!(bind(&link.join("new.sock"), &Config::default()).is_err());
    handle.shutdown();
    Ok(())
}

#[test]
fn malformed_service_response_and_wrong_execution_identity_never_become_a_verdict() -> TestResult {
    use std::os::unix::net::UnixListener;
    let directory = Directory::new()?;
    let socket = directory.socket();
    transport::private_parent(&socket, true)?;
    let listener = UnixListener::bind(&socket)?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    let fake = std::thread::spawn(move || -> io::Result<()> {
        let (stream, _) = listener.accept()?;
        let mut deadline = Deadline::new(&stream, IO_TIMEOUT);
        let request: Request = transport::read_frame(&mut deadline, MAX_REQUEST_BYTES)?;
        assert!(matches!(request, Request::Capabilities));
        let runtime = sail_runtime::Runtime::new(sail_runtime::Config::default())
            .map_err(io::Error::other)?;
        transport::write_frame(
            &mut deadline,
            &Response::Capabilities {
                capabilities: runtime.capabilities(),
            },
            MAX_RESPONSE_BYTES,
        )?;
        let (stream, _) = listener.accept()?;
        let mut deadline = Deadline::new(&stream, IO_TIMEOUT);
        let request: Request = transport::read_frame(&mut deadline, MAX_REQUEST_BYTES)?;
        let Request::Execute { request, .. } = request else {
            return Err(io::Error::other("expected execute"));
        };
        let response = runtime
            .execute(&request, &sail_runtime::Cancellation::default())
            .map_err(io::Error::other)?;
        transport::write_frame(
            &mut deadline,
            &Response::Completed {
                id: "wrong-id".into(),
                response,
            },
            MAX_RESPONSE_BYTES,
        )
    });
    assert!(
        Client::new(&socket)
            .execute(&request("(check-sat)"))
            .is_err()
    );
    fake.join().map_err(|_| "fake server panicked")??;
    Ok(())
}

fn pinned() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(std::env::var("SAIL_TEST_Z3")?))
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn actual_pinned_service_matches_embedding_and_reuses_the_same_cache() -> TestResult {
    let directory = Directory::new()?;
    let config = sail_runtime::Config {
        z3: Some(pinned()?),
        cache_directory: Some(directory.0.join("records")),
        ..sail_runtime::Config::default()
    };
    let embedded = sail_runtime::Runtime::new(config.clone())?;
    let query = request(
        "(set-option :produce-models true) (declare-const x Int) (assert (> x 2)) (check-sat) (get-value (x))",
    );
    let expected = embedded.execute(&query, &sail_runtime::Cancellation::default())?;
    let socket = directory.socket();
    let mut handle = bind(
        &socket,
        &Config {
            runtime: config,
            ..Config::default()
        },
    )?
    .spawn()?;
    let response = Client::new(&socket).execute(&query)?;
    let Output::Smt { results } = &response.output;
    let Output::Smt { results: original } = &expected.output;
    assert_eq!(results[0].outcome, Outcome::Completed);
    assert_eq!(results[0].stdout, original[0].stdout);
    assert_eq!(results[0].checks, original[0].checks);
    assert!(results[0].cache_hit);
    assert_eq!(results[0].requirement_ids, ["REQ-1"]);
    handle.shutdown();
    Ok(())
}

fn hard_request() -> api::Request {
    let mut request = request(
        "(declare-fun f (Int) Int) (assert (forall ((x Int)) (> (f x) 0))) (assert (forall ((x Int)) (> (f x) (f (+ x 1))))) (check-sat)",
    );
    let Input::Smt { queries } = &mut request.input;
    queries[0].limits.resource_limit = 1_000_000_000;
    request
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn actual_client_cancellation_and_disconnect_leave_no_active_execution() -> TestResult {
    let directory = Directory::new()?;
    let socket = directory.socket();
    let config = Config {
        runtime: sail_runtime::Config {
            z3: Some(pinned()?),
            ..sail_runtime::Config::default()
        },
        ..Config::default()
    };
    let mut handle = bind(&socket, &config)?.spawn()?;
    let activity = handle.activity();
    let response = Client::new(&socket)
        .execute_cancellable(&hard_request(), || activity.load(Ordering::SeqCst) != 0)?;
    let Output::Smt { results } = response.output;
    assert_eq!(results[0].outcome, Outcome::Cancelled);
    assert_eq!(results[0].checks, []);
    let stream = UnixStream::connect(&socket)?;
    transport::write_frame(
        &mut Deadline::new(&stream, IO_TIMEOUT),
        &Request::Execute {
            id: "disconnect-id".into(),
            request: hard_request(),
        },
        MAX_REQUEST_BYTES,
    )?;
    let started = Instant::now();
    while activity.load(Ordering::SeqCst) == 0 && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(activity.load(Ordering::SeqCst), 1);
    drop(stream);
    while activity.load(Ordering::SeqCst) != 0 && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(activity.load(Ordering::SeqCst), 0);
    assert!(Client::new(&socket).capabilities().is_ok());
    handle.shutdown();
    Ok(())
}

#[test]
fn existing_daemon_activity_counter_is_shared_without_overwriting_other_jobs() -> TestResult {
    let directory = Directory::new()?;
    let activity = Arc::new(AtomicU64::new(7));
    let server =
        bind(&directory.socket(), &Config::default())?.with_activity(Arc::clone(&activity))?;
    let state = Arc::clone(&server.state);
    let token = sail_runtime::Cancellation::default();
    let registration = Registration::new(&state, "activity", token.clone())?;
    assert_eq!(activity.load(Ordering::SeqCst), 8);
    let mut handle = server.spawn()?;
    handle.shutdown();
    assert!(token.is_cancelled());
    drop(registration);
    assert_eq!(activity.load(Ordering::SeqCst), 7);
    Ok(())
}

#[test]
fn execution_registration_covers_blocked_response_delivery() -> TestResult {
    let directory = Directory::new()?;
    let service = bind(&directory.socket(), &Config::default())?;
    let state = Arc::clone(&service.state);
    let activity = Arc::clone(&state.activity);
    let (server, client) = UnixStream::pair()?;
    socket2::SockRef::from(&server).set_send_buffer_size(1024)?;
    let worker = std::thread::spawn(move || super::connection(&server, &state));
    let mut payload = request("(check-sat)");
    let Input::Smt { queries } = &mut payload.input;
    queries[0].requirement_ids = (0..1024)
        .map(|index| format!("{index:04}-{}", "a".repeat(1000)))
        .collect();
    transport::write_frame(
        &mut Deadline::new(&client, IO_TIMEOUT),
        &Request::Execute {
            id: "blocked-response".into(),
            request: payload,
        },
        MAX_REQUEST_BYTES,
    )?;
    let started = Instant::now();
    while activity.load(Ordering::SeqCst) == 0 && started.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(activity.load(Ordering::SeqCst), 1);
    drop(client);
    worker.join().map_err(|_| "connection worker panicked")?;
    assert_eq!(activity.load(Ordering::SeqCst), 0);
    Ok(())
}
