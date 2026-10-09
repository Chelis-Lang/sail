//! Optional authenticated shared-cache server. It transports signed SMT entries;
//! authentication never confers signing trust on readers.

use std::convert::Infallible;
use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use sail_cache::shared::{
    Header, MAX_ENTRY_BYTES, PublicKey, SharedStore, TrustedKeys, verify_entry,
};
use sail_key::QueryKey;

use crate::remote::read_token;

const CONNECTIONS: usize = 8;
const BODY_TIMEOUT: Duration = Duration::from_secs(30);
const HEADER_TIMEOUT: Duration = Duration::from_secs(5);
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
type Reply = Response<Full<Bytes>>;
struct Upload(PathBuf);
impl Drop for Upload {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Server settings. Remote traffic must arrive through a TLS reverse proxy.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Entry store directory.
    pub store: PathBuf,
    /// Loopback bind by default; port zero chooses an available port.
    pub listen: SocketAddr,
    /// Explicitly permit a private backend bind behind a TLS reverse proxy.
    pub behind_tls_proxy: bool,
}

impl ServerConfig {
    /// A local server on an automatically allocated loopback port.
    #[must_use]
    pub fn local(store: impl Into<PathBuf>) -> Self {
        Self {
            store: store.into(),
            listen: SocketAddr::from(([127, 0, 0, 1], 0)),
            behind_tls_proxy: false,
        }
    }
}

/// Bound shared server and generated credential paths.
pub struct Server {
    listener: TcpListener,
    state: Arc<State>,
    /// Read-only token file (owner-only).
    pub read_token_file: PathBuf,
    /// Read/write token file (owner-only).
    pub write_token_file: PathBuf,
}

struct State {
    root: PathBuf,
    read: String,
    write: String,
}

/// Cooperative server shutdown handle.
#[derive(Clone)]
pub struct Handle(Arc<AtomicBool>);

impl Handle {
    /// Stops acceptance and closes current connections.
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn private_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn credential(path: &Path) -> Result<String, String> {
    if !path.exists() {
        let mut random = [0u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|_| "SAIL0651: cannot obtain credential randomness".to_owned())?;
        let token = sail_key::sha256(&random)
            .iter()
            .fold(String::new(), |mut text, b| {
                use std::fmt::Write as _;
                let _ = write!(text, "{b:02x}");
                text
            });
        random.fill(0);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options
            .open(path)
            .and_then(|mut f| f.write_all(token.as_bytes()))
        {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err("SAIL0651: cannot create private credential file".to_owned()),
        }
    }
    read_token(path)
}

/// Binds a bounded HTTP server and creates private reader/writer credentials.
///
/// # Errors
/// The bind policy, filesystem or credential setup fails.
pub fn bind(config: &ServerConfig) -> Result<Server, String> {
    if !config.listen.ip().is_loopback() && !config.behind_tls_proxy {
        return Err("SAIL0651: a non-loopback bind requires --behind-tls-proxy".to_owned());
    }
    private_dir(&config.store).map_err(|_| "SAIL0651: cannot create shared store".to_owned())?;
    let metadata = config.store.join(".sail-server");
    private_dir(&metadata)
        .map_err(|_| "SAIL0651: cannot create private server metadata".to_owned())?;
    let read_token_file = metadata.join("read.token");
    let write_token_file = metadata.join("write.token");
    let state = Arc::new(State {
        root: config.store.clone(),
        read: credential(&read_token_file)?,
        write: credential(&write_token_file)?,
    });
    let listener = TcpListener::bind(config.listen)
        .map_err(|_| "SAIL0651: cannot bind shared server".to_owned())?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "SAIL0651: cannot configure shared server socket".to_owned())?;
    Ok(Server {
        listener,
        state,
        read_token_file,
        write_token_file,
    })
}

impl Server {
    /// Actual listening address.
    ///
    /// # Errors
    /// The listener's address is unavailable.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Starts the server on a dedicated thread; the normal local CLI never calls this.
    ///
    /// # Errors
    /// The worker thread cannot be created.
    pub fn spawn(self) -> std::io::Result<(Handle, std::thread::JoinHandle<Result<(), String>>)> {
        let stop = Handle(Arc::new(AtomicBool::new(false)));
        let child_stop = stop.clone();
        let thread = std::thread::Builder::new()
            .name("sail-shared-http".to_owned())
            .spawn(move || self.serve(&child_stop))?;
        Ok((stop, thread))
    }

    fn serve(self, stop: &Handle) -> Result<(), String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|_| "SAIL0651: cannot initialize shared server".to_owned())?;
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(self.listener)
                .map_err(|_| "SAIL0651: cannot start shared server".to_owned())?;
            let capacity = Arc::new(tokio::sync::Semaphore::new(CONNECTIONS));
            while !stop.0.load(Ordering::SeqCst) {
                let accepted =
                    tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;
                let Ok(Ok((stream, _))) = accepted else {
                    continue;
                };
                let Ok(permit) = Arc::clone(&capacity).try_acquire_owned() else {
                    continue;
                };
                let state = Arc::clone(&self.state);
                let shutdown = stop.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    let service = service_fn(move |request| route(request, Arc::clone(&state)));
                    let mut builder = http1::Builder::new();
                    builder
                        .timer(TokioTimer::new())
                        .header_read_timeout(HEADER_TIMEOUT)
                        .max_buf_size(16 * 1024)
                        .max_headers(64)
                        .keep_alive(false);
                    let connection = builder.serve_connection(TokioIo::new(stream), service);
                    tokio::pin!(connection);
                    loop {
                        if tokio::time::timeout(Duration::from_millis(100), &mut connection)
                            .await
                            .is_ok()
                            || shutdown.0.load(Ordering::SeqCst)
                        {
                            break;
                        }
                    }
                });
            }
            Ok(())
        })
    }
}

fn response(status: StatusCode) -> Reply {
    let mut reply = Response::new(Full::new(Bytes::new()));
    *reply.status_mut() = status;
    reply
}

fn authorized(request: &Request<Incoming>, state: &State) -> bool {
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    let Some(token) = token else {
        return false;
    };
    let equals = |expected: &str| {
        token.len() == expected.len()
            && token
                .bytes()
                .zip(expected.bytes())
                .fold(0u8, |acc, (a, b)| acc | a ^ b)
                == 0
    };
    equals(&state.write) || request.method() == Method::GET && equals(&state.read)
}

fn parse_path(text: &str) -> Option<(String, String, QueryKey)> {
    let parts = text.strip_prefix('/')?.split('/').collect::<Vec<_>>();
    let ["v2", platform, solver, prefix, key] = parts.as_slice() else {
        return None;
    };
    let valid_hex =
        |s: &str| s.len() == 64 && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'));
    if !valid_hex(solver) || !valid_hex(key) || key.get(..2)? != *prefix {
        return None;
    }
    if platform.is_empty()
        || matches!(*platform, "." | "..")
        || !platform
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'+'))
    {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, value) in bytes.iter_mut().enumerate() {
        *value = u8::from_str_radix(key.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some((
        (*platform).to_owned(),
        (*solver).to_owned(),
        QueryKey(bytes),
    ))
}

async fn route(mut request: Request<Incoming>, state: Arc<State>) -> Result<Reply, Infallible> {
    if !authorized(&request, &state) {
        return Ok(response(StatusCode::UNAUTHORIZED));
    }
    if request.uri().query().is_some() {
        return Ok(response(StatusCode::BAD_REQUEST));
    }
    let Some((platform, solver, key)) = parse_path(request.uri().path()) else {
        return Ok(response(StatusCode::NOT_FOUND));
    };
    let path = state
        .root
        .join("v2")
        .join(&platform)
        .join(&solver)
        .join(key.to_string().get(..2).unwrap_or("xx"))
        .join(key.to_string());
    if request.method() == Method::GET {
        let data = tokio::task::spawn_blocking(move || SharedStore::read(&path)).await;
        return Ok(match data {
            Ok(Ok(Some(data))) => Response::new(Full::new(Bytes::from(data))),
            Ok(Ok(None)) => response(StatusCode::NOT_FOUND),
            _ => response(StatusCode::INTERNAL_SERVER_ERROR),
        });
    }
    if request.method() != Method::PUT {
        return Ok(response(StatusCode::METHOD_NOT_ALLOWED));
    }
    if request.headers().contains_key("content-encoding") {
        return Ok(response(StatusCode::UNSUPPORTED_MEDIA_TYPE));
    }
    let public = request
        .headers()
        .get("sail-public-key")
        .and_then(|v| v.to_str().ok())
        .and_then(PublicKey::parse);
    let Some(public) = public else {
        return Ok(response(StatusCode::BAD_REQUEST));
    };
    if request
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .is_some_and(|n| n > MAX_ENTRY_BYTES)
    {
        return Ok(response(StatusCode::PAYLOAD_TOO_LARGE));
    }
    let tmp = state.root.join(".sail-server").join(format!(
        "upload-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _cleanup = Upload(tmp.clone());
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let Ok(mut file) = options.open(&tmp) else {
        return Ok(response(StatusCode::INTERNAL_SERVER_ERROR));
    };
    let body = tokio::time::timeout(BODY_TIMEOUT, async {
        let mut length = 0u64;
        while let Some(frame) = request.body_mut().frame().await {
            let frame = frame.map_err(|_| StatusCode::BAD_REQUEST)?;
            if let Ok(data) = frame.into_data() {
                length = length.saturating_add(data.len() as u64);
                if length > MAX_ENTRY_BYTES {
                    return Err(StatusCode::PAYLOAD_TOO_LARGE);
                }
                file.write_all(&data)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }
        Ok(())
    })
    .await;
    drop(file);
    let result = match body {
        Ok(Ok(())) => tokio::task::spawn_blocking({
            let tmp = tmp.clone();
            move || commit(&tmp, &path, &platform, &solver, &key, &public)
        })
        .await
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Ok(Err(status)) => status,
        Err(_) => StatusCode::REQUEST_TIMEOUT,
    };
    let _ = std::fs::remove_file(tmp);
    Ok(response(result))
}

fn commit(
    tmp: &Path,
    path: &Path,
    platform: &str,
    solver: &str,
    key: &QueryKey,
    public: &PublicKey,
) -> StatusCode {
    let Ok(Some(data)) = SharedStore::read(tmp) else {
        return StatusCode::BAD_REQUEST;
    };
    let header: Option<Header> = data
        .split(|b| *b == b'\n')
        .next()
        .and_then(|line| serde_json::from_slice(line).ok());
    let Some(header) = header else {
        return StatusCode::BAD_REQUEST;
    };
    if header.solver.platform != platform
        || header.solver.binary_sha256 != solver
        || !header.solver.well_formed()
    {
        return StatusCode::BAD_REQUEST;
    }
    let mut trust = TrustedKeys::default();
    trust.insert(*public);
    let Ok(verified) = verify_entry(&data, key, &trust, &header.solver) else {
        return StatusCode::BAD_REQUEST;
    };
    if verified.command.starts_with("check-sat") && sail_api::policy::is_unknown(&verified.reply) {
        let effort = verified.effort.map(|e| sail_api::protocol::Effort {
            limit: e.limit,
            used: e.used,
            reason: e.reason,
        });
        if !sail_api::policy::unknown_servable(effort.as_ref()) {
            return StatusCode::BAD_REQUEST;
        }
    }
    let Some(parent) = path.parent() else {
        return StatusCode::BAD_REQUEST;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    // Hard-link creation is atomic and cannot replace another publication.
    match std::fs::hard_link(tmp, path) {
        Ok(()) => StatusCode::CREATED,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if SharedStore::read(path)
                .ok()
                .flatten()
                .is_some_and(|existing| existing == data)
            {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::CONFLICT
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
