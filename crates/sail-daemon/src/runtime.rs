//! Separately versioned owner-private runtime endpoint in the existing daemon.

use sail_api::runtime::{
    Error,
    transport::{
        self, Deadline, IO_TIMEOUT, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request, Response,
    },
};
use sail_runtime::{Cancellation, Runtime};
#[cfg(test)]
mod tests;
use std::{
    collections::BTreeMap,
    io::{self, Read as _},
    os::unix::{
        fs::{MetadataExt as _, PermissionsExt as _},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// Explicit bounds and installed-tool configuration for the shared-daemon endpoint.
#[derive(Clone, Debug)]
pub struct Config {
    /// Exact same runtime configuration used by the embedded path.
    pub runtime: sail_runtime::Config,
    /// Maximum simultaneously retained client frames and connection workers.
    pub max_clients: usize,
    /// Maximum active executions; control requests retain independent capacity.
    pub max_active: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            runtime: sail_runtime::Config::default(),
            max_clients: 8,
            max_active: 2,
        }
    }
}

struct State {
    runtime: Runtime,
    active: Mutex<BTreeMap<String, Cancellation>>,
    activity: Arc<AtomicU64>,
    clients: AtomicUsize,
    stop: AtomicBool,
    max_clients: usize,
    max_active: usize,
}

/// Bound runtime listener, ready to be started beside cache and project endpoints.
pub struct Server {
    socket: PathBuf,
    listener: UnixListener,
    state: Arc<State>,
    stamp: (u64, u64),
}
/// Shutdown and idle-activity handle for the runtime service.
pub struct Handle {
    socket: PathBuf,
    state: Arc<State>,
    thread: Option<JoinHandle<()>>,
    stamp: (u64, u64),
}

fn error(kind: &str, message: impl Into<String>) -> Error {
    Error {
        code: "SAIL6201".into(),
        kind: kind.into(),
        message: message.into(),
    }
}
fn failure(kind: &str, message: impl Into<String>) -> Response {
    Response::Error {
        error: error(kind, message),
    }
}
fn poison<T>(_: std::sync::PoisonError<T>) -> io::Error {
    io::Error::other("SAIL6201: runtime state lock failed")
}

/// Bind an independently versioned endpoint without changing cache RPC or Cargo jobs.
///
/// # Errors
/// Refuses invalid bounds, unowned paths, live endpoints and socket failures.
pub fn bind(socket: &Path, config: &Config) -> io::Result<Server> {
    if !(4..=32).contains(&config.max_clients)
        || config.max_active == 0
        || config.max_active > config.max_clients - 2
    {
        return Err(io::Error::other(
            "SAIL6201: invalid runtime client/execution bounds",
        ));
    }
    let socket = std::path::absolute(socket)?;
    transport::private_parent(&socket, true)?;
    if std::fs::symlink_metadata(&socket).is_ok() {
        transport::validate_socket(&socket)?;
        let probe = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
        match probe.connect_timeout(
            &socket2::SockAddr::unix(&socket)?,
            Duration::from_millis(200),
        ) {
            Ok(()) => {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "SAIL6201: runtime endpoint already running",
                ));
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) =>
            {
                std::fs::remove_file(&socket)?;
            }
            Err(e) => return Err(e),
        }
    }
    let runtime = Runtime::new(config.runtime.clone()).map_err(io::Error::other)?;
    let listener = UnixListener::bind(&socket)?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let meta = std::fs::symlink_metadata(&socket)?;
    Ok(Server {
        socket,
        listener,
        stamp: (meta.dev(), meta.ino()),
        state: Arc::new(State {
            runtime,
            active: Mutex::new(BTreeMap::new()),
            activity: Arc::new(AtomicU64::new(0)),
            clients: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
            max_clients: config.max_clients,
            max_active: config.max_active,
        }),
    })
}

impl Server {
    /// Share the existing daemon idle counter before starting any service thread.
    ///
    /// # Errors
    /// Refuses to replace activity after the service state was shared.
    pub fn with_activity(mut self, activity: Arc<AtomicU64>) -> io::Result<Self> {
        let Some(state) = Arc::get_mut(&mut self.state) else {
            remove(&self.socket, self.stamp);
            return Err(io::Error::other(
                "SAIL6201: runtime activity is already shared",
            ));
        };
        state.activity = activity;
        Ok(self)
    }
    /// Start the endpoint's bounded connection owner thread.
    ///
    /// # Errors
    /// Returns listener thread creation failures and removes this endpoint.
    pub fn spawn(self) -> io::Result<Handle> {
        let state = Arc::clone(&self.state);
        let socket = self.socket.clone();
        let stamp = self.stamp;
        match thread::Builder::new()
            .name("sail-runtime-listener".into())
            .spawn(move || serve(&self.listener, &state))
        {
            Ok(thread) => Ok(Handle {
                socket,
                state: self.state,
                thread: Some(thread),
                stamp,
            }),
            Err(e) => {
                remove(&socket, stamp);
                Err(e)
            }
        }
    }
}

fn remove(socket: &Path, stamp: (u64, u64)) {
    if std::fs::symlink_metadata(socket).is_ok_and(|meta| (meta.dev(), meta.ino()) == stamp) {
        let _ = std::fs::remove_file(socket);
    }
}

impl Handle {
    /// Active executions for the existing daemon idle guard.
    #[must_use]
    pub fn activity(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.state.activity)
    }
    /// Cancel all active backend work, join all workers and remove the owned endpoint.
    pub fn shutdown(&mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
        if let Ok(active) = self.state.active.lock() {
            for token in active.values() {
                token.cancel();
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        remove(&self.socket, self.stamp);
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct ClientGuard(Arc<State>);
impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.0.clients.fetch_sub(1, Ordering::SeqCst);
    }
}

fn serve(listener: &UnixListener, state: &Arc<State>) {
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !state.stop.load(Ordering::SeqCst) {
        let mut i = 0;
        while i < workers.len() {
            if workers[i].is_finished() {
                let _ = workers.swap_remove(i).join();
            } else {
                i += 1;
            }
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if state
                    .clients
                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                        (count < state.max_clients).then_some(count + 1)
                    })
                    .is_err()
                {
                    let _ = transport::write_frame(
                        &mut Deadline::new(&stream, Duration::from_millis(50)),
                        &failure("busy", "runtime connection limit reached"),
                        MAX_RESPONSE_BYTES,
                    );
                    continue;
                }
                let client = ClientGuard(Arc::clone(state));
                if let Ok(worker) = thread::Builder::new()
                    .name("sail-runtime-connection".into())
                    .spawn(move || {
                        let guard = client;
                        let _ = stream.set_nonblocking(false);
                        connection(&stream, &guard.0);
                    })
                {
                    workers.push(worker);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
    if let Ok(active) = state.active.lock() {
        for token in active.values() {
            token.cancel();
        }
    }
    for worker in workers {
        let _ = worker.join();
    }
}

fn connection(stream: &UnixStream, state: &Arc<State>) {
    let request = transport::read_frame(&mut Deadline::new(stream, IO_TIMEOUT), MAX_REQUEST_BYTES);
    let (response, _registration) = match request {
        Ok(request) => dispatch(stream, state, request),
        Err(e) => (failure("invalid_frame", e.to_string()), None),
    };
    if let Err(e) = transport::write_frame(
        &mut Deadline::new(stream, IO_TIMEOUT),
        &response,
        MAX_RESPONSE_BYTES,
    ) {
        // Serialization bound failures happen before the frame header is sent.
        if e.kind() == io::ErrorKind::InvalidData {
            let _ = transport::write_frame(
                &mut Deadline::new(stream, IO_TIMEOUT),
                &failure("response_limit", e.to_string()),
                MAX_RESPONSE_BYTES,
            );
        }
    }
}

struct Registration {
    state: Arc<State>,
    id: String,
}
impl Registration {
    fn new(state: &Arc<State>, id: &str, token: Cancellation) -> io::Result<Self> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(io::Error::other("SAIL6201: invalid transport identity"));
        }
        let mut active = state.active.lock().map_err(poison)?;
        if state.stop.load(Ordering::SeqCst) {
            return Err(io::Error::other("SAIL6201: runtime is shutting down"));
        }
        if active.contains_key(id) {
            return Err(io::Error::other(
                "SAIL6201: duplicate active transport identity",
            ));
        }
        if active.len() >= state.max_active {
            return Err(io::Error::other("SAIL6201: active execution limit reached"));
        }
        active.insert(id.into(), token);
        state.activity.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            state: Arc::clone(state),
            id: id.into(),
        })
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        if let Ok(mut active) = self.state.active.lock() {
            active.remove(&self.id);
        }
        self.state.activity.fetch_sub(1, Ordering::SeqCst);
    }
}

fn dispatch(
    stream: &UnixStream,
    state: &Arc<State>,
    request: Request,
) -> (Response, Option<Registration>) {
    match request {
        Request::Capabilities => (
            Response::Capabilities {
                capabilities: state.runtime.capabilities(),
            },
            None,
        ),
        Request::Cancel { id } => {
            let active = match state.active.lock() {
                Ok(active) => active,
                Err(e) => return (failure("infrastructure", e.to_string()), None),
            };
            let found = active.get(&id).is_some_and(|token| {
                token.cancel();
                true
            });
            (Response::Cancelled { id, found }, None)
        }
        Request::Execute { id, request } => {
            let token = Cancellation::default();
            let registration = match Registration::new(state, &id, token.clone()) {
                Ok(registration) => registration,
                Err(e) => return (failure("busy", e.to_string()), None),
            };
            let response = (|| {
                let done = Arc::new(AtomicBool::new(false));
                let monitor = match stream.try_clone() {
                    Ok(stream) => stream,
                    Err(e) => return failure("infrastructure", e.to_string()),
                };
                let finished = Arc::clone(&done);
                let cancel = token.clone();
                let monitor = thread::Builder::new()
                    .name("sail-runtime-disconnect".into())
                    .spawn(move || {
                        let mut monitor = monitor;
                        if monitor.set_nonblocking(true).is_err() {
                            cancel.cancel();
                            return;
                        }
                        let mut byte = [0_u8; 1];
                        while !finished.load(Ordering::SeqCst) {
                            match monitor.read(&mut byte) {
                                Err(e)
                                    if matches!(
                                        e.kind(),
                                        io::ErrorKind::WouldBlock
                                            | io::ErrorKind::TimedOut
                                            | io::ErrorKind::Interrupted
                                    ) =>
                                {
                                    thread::sleep(Duration::from_millis(2));
                                }
                                _ => {
                                    cancel.cancel();
                                    break;
                                }
                            }
                        }
                    });
                let monitor = match monitor {
                    Ok(monitor) => monitor,
                    Err(e) => return failure("infrastructure", e.to_string()),
                };
                let result = state.runtime.execute(&request, &token);
                done.store(true, Ordering::SeqCst);
                if monitor.join().is_err() {
                    return failure("infrastructure", "disconnect monitor failed");
                }
                match result {
                    Ok(response) => Response::Completed { id, response },
                    Err(error) => Response::Error { error },
                }
            })();
            (response, Some(registration))
        }
    }
}
