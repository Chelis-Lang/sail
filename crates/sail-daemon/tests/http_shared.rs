//! Authentication, immutable publication and untrusted-reader checks for HTTP sharing.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test setup and assertions"
)]

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sail_cache::shared::{SecretKey, Solver, TrustedKeys, sign_entry, verify_entry};
use sail_daemon::http_server::{self, ServerConfig};
use sail_daemon::remote::{HttpClient, Published, RemoteConfig};
use sail_key::QueryKey;

static COUNTER: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    address: std::net::SocketAddr,
    read: RemoteConfig,
    write: RemoteConfig,
    stop: sail_daemon::http_server::Handle,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "sail-http-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let server = http_server::bind(&ServerConfig::local(&root)).unwrap();
        let address = server.local_addr().unwrap();
        let endpoint = format!("http://{address}/");
        let read = RemoteConfig {
            endpoint: endpoint.clone(),
            token_file: server.read_token_file.clone(),
            ca_file: None,
        };
        let write = RemoteConfig {
            endpoint,
            token_file: server.write_token_file.clone(),
            ca_file: None,
        };
        let (stop, thread) = server.spawn().unwrap();
        Self {
            root,
            address,
            read,
            write,
            stop,
            thread: Some(thread),
        }
    }
    fn client(config: &RemoteConfig) -> HttpClient {
        HttpClient::new(config, Duration::from_secs(2)).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.stop();
        if let Some(thread) = self.thread.take() {
            assert!(thread.join().unwrap().is_ok());
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn solver() -> Solver {
    Solver {
        name: "z3".to_owned(),
        version: "4.16.0".to_owned(),
        binary_sha256: "ab".repeat(32),
        platform: "linux-x86_64".to_owned(),
    }
}
const KEY: QueryKey = QueryKey([7; 32]);

#[test]
fn signed_publication_is_immutable_and_reader_auth_does_not_grant_signing_trust() {
    let fixture = Fixture::new();
    let writer = Fixture::client(&fixture.write);
    let reader = Fixture::client(&fixture.read);
    let secret = SecretKey::from_seed([1; 32]);
    let data = sign_entry(&secret, &KEY, b"unsat\n", "check-sat", &solver(), None, 1);
    assert_eq!(reader.get(&solver(), &KEY).unwrap(), None);
    assert_eq!(
        writer.put(&solver(), &KEY, data.clone(), &secret.public()),
        Ok(Published::Created)
    );
    assert_eq!(
        writer.put(&solver(), &KEY, data.clone(), &secret.public()),
        Ok(Published::Present)
    );
    let conflicting = sign_entry(&secret, &KEY, b"sat\n", "check-sat", &solver(), None, 2);
    assert_eq!(
        writer.put(&solver(), &KEY, conflicting, &secret.public()),
        Ok(Published::Conflict)
    );
    assert!(
        reader
            .put(&solver(), &KEY, data.clone(), &secret.public())
            .is_err()
    );
    let fetched = reader.get(&solver(), &KEY).unwrap().unwrap();
    assert_eq!(fetched, data);
    assert!(verify_entry(&fetched, &KEY, &TrustedKeys::default(), &solver()).is_err());
    let mut trust = TrustedKeys::default();
    trust.insert(secret.public());
    assert_eq!(
        verify_entry(&fetched, &KEY, &trust, &solver())
            .unwrap()
            .reply,
        b"unsat\n"
    );
}

#[test]
fn forged_misfiled_wrong_shape_and_exhausted_entries_are_refused() {
    let fixture = Fixture::new();
    let writer = Fixture::client(&fixture.write);
    let secret = SecretKey::from_seed([2; 32]);
    let valid = sign_entry(&secret, &KEY, b"unsat\n", "check-sat", &solver(), None, 1);
    let mut forged = valid.clone();
    *forged.last_mut().unwrap() = b'x';
    assert!(
        writer
            .put(&solver(), &KEY, forged, &secret.public())
            .is_err()
    );
    assert!(
        writer
            .put(&solver(), &QueryKey([8; 32]), valid, &secret.public())
            .is_err()
    );
    let wrong_shape = sign_entry(
        &secret,
        &KEY,
        b"(error \"x\")\n",
        "check-sat",
        &solver(),
        None,
        1,
    );
    assert!(
        writer
            .put(&solver(), &KEY, wrong_shape, &secret.public())
            .is_err()
    );
    let exhausted = sail_cache::Effort {
        limit: Some(100),
        used: Some(100),
        reason: Some("canceled".to_owned()),
    };
    let unknown = sign_entry(
        &secret,
        &KEY,
        b"unknown\n",
        "check-sat",
        &solver(),
        Some(&exhausted),
        1,
    );
    assert!(
        writer
            .put(&solver(), &KEY, unknown, &secret.public())
            .is_err()
    );
    assert_eq!(
        Fixture::client(&fixture.read).get(&solver(), &KEY).unwrap(),
        None
    );
}

#[test]
fn interrupted_upload_never_creates_a_visible_entry_and_oversize_is_rejected_early() {
    let fixture = Fixture::new();
    let token = sail_daemon::remote::read_token(&fixture.write.token_file).unwrap();
    let path = HttpClient::entry_path(&solver(), &KEY).unwrap();
    let mut stream = TcpStream::connect(fixture.address).unwrap();
    write!(stream, "PUT /{path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Length: 1000\r\nsail-public-key: {}\r\n\r\npartial", SecretKey::from_seed([3;32]).public().to_text().trim()).unwrap();
    drop(stream);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        Fixture::client(&fixture.read).get(&solver(), &KEY).unwrap(),
        None
    );
    let mut stream = TcpStream::connect(fixture.address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(stream, "PUT /{path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Length: 67108865\r\nsail-public-key: {}\r\n\r\n", SecretKey::from_seed([3;32]).public().to_text().trim()).unwrap();
    let mut text = String::new();
    stream.read_to_string(&mut text).unwrap();
    assert!(text.starts_with("HTTP/1.1 413"));
}

#[test]
fn endpoint_policy_and_private_settings_do_not_expose_credentials() {
    for endpoint in [
        "http://localhost",
        "http://example.com",
        "https://user:secret@example.com",
        "https://example.com?secret=x",
        "https://example.com#secret",
        "file:///tmp/x",
    ] {
        assert!(
            sail_daemon::remote::endpoint(endpoint).is_err(),
            "{endpoint}"
        );
    }
    assert!(sail_daemon::remote::endpoint("http://127.0.0.1:1234").is_ok());
    assert!(sail_daemon::remote::endpoint("http://[::1]:1234").is_ok());
    let fixture = Fixture::new();
    let root = fixture.root.join("reader-cache");
    sail_daemon::remote::connect(&root, &fixture.read).unwrap();
    assert_eq!(
        sail_daemon::remote::load(&root).unwrap(),
        Some(fixture.read.clone())
    );
    let saved = std::fs::read_to_string(root.join(sail_daemon::remote::CONFIG_FILE)).unwrap();
    assert!(!saved.contains(&sail_daemon::remote::read_token(&fixture.read.token_file).unwrap()));
    assert!(!format!("{:?}", Fixture::client(&fixture.read)).contains("127.0.0.1"));
    sail_daemon::remote::disconnect(&root).unwrap();
    assert_eq!(sail_daemon::remote::load(&root).unwrap(), None);
}

fn fake_response(
    text: impl AsRef<[u8]> + Send + 'static,
    delay: Duration,
) -> (RemoteConfig, FakeServer, PathBuf) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let root = std::env::temp_dir().join(format!(
        "sail-http-fake-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let token_file = root.join("token");
    std::fs::write(&token_file, "0123456789abcdef0123456789abcdef").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let config = RemoteConfig {
        endpoint: format!("http://{}", listener.local_addr().unwrap()),
        token_file,
        ca_file: None,
    };
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut input = [0u8; 4096];
        let _ = stream.read(&mut input);
        std::thread::sleep(delay);
        let _ = stream.write_all(text.as_ref());
    });
    (config, FakeServer { address, thread }, root)
}

/// The thread of a fake server that answers one request.
struct FakeServer {
    address: std::net::SocketAddr,
    thread: std::thread::JoinHandle<()>,
}
impl FakeServer {
    /// Waits for the server to finish, once the client has returned.
    ///
    /// A client that failed before it connected would otherwise leave the
    /// server waiting to accept, and this join waiting with it, for ever.
    /// [`end_accept`] ends that wait, so the test reaches its assertion on the
    /// client's result.
    fn join(self) -> std::thread::Result<()> {
        end_accept(self.address);
        self.thread.join()
    }
}

/// Ends a fake server's wait for a client that never connected.
///
/// The connection made here is closed at once, so a server that accepts it
/// reads an empty request. It queues behind any connection the client did
/// make, and a server accepts only one, so it never displaces a real request.
fn end_accept(address: std::net::SocketAddr) {
    drop(TcpStream::connect(address));
}

/// How long a held fake server waits before it gives up and completes its
/// response unreleased. No passing run waits for it. It is the bound on how
/// long a client may take to give up: one that takes longer, or never times
/// out at all, is answered by the watchdog, which the test then observes as a
/// response that was no longer withheld, and fails. The bound is set far above
/// any scheduling delay, so host load cannot reach it; only a client whose
/// timeout does not work can.
const HELD_SERVER_WATCHDOG: Duration = Duration::from_secs(120);

/// A fake server whose response stalls until the test releases it.
///
/// It accepts one request, writes `before`, and writes `after` only once
/// [`HeldResponse::release`] is called. The test therefore fixes the order of
/// events: whatever the client did before the release, it did without the
/// withheld bytes, however slowly the host scheduled either side.
///
/// The server stops withholding only after its receive returns, and that
/// happens on exactly three events: the send in `release`, the sender's drop
/// (which `release` owns, so no earlier), or [`HELD_SERVER_WATCHDOG`]. A test
/// that reads [`HeldResponse::resumed`] as false after the client returned and
/// before it calls `release` has therefore shown both that the client returned
/// without the withheld bytes and that it did so within the watchdog's bound.
struct HeldResponse {
    config: RemoteConfig,
    root: PathBuf,
    address: std::net::SocketAddr,
    release: std::sync::mpsc::Sender<()>,
    resumed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: std::thread::JoinHandle<Vec<u8>>,
}
impl HeldResponse {
    fn new(before: &'static [u8], after: &'static [u8]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let root = std::env::temp_dir().join(format!(
            "sail-http-held-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let token_file = root.join("token");
        std::fs::write(&token_file, "0123456789abcdef0123456789abcdef").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let address = listener.local_addr().unwrap();
        let config = RemoteConfig {
            endpoint: format!("http://{address}/"),
            token_file,
            ca_file: None,
        };
        let (release, held) = std::sync::mpsc::channel::<()>();
        let resumed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread = std::thread::spawn({
            let resumed = std::sync::Arc::clone(&resumed);
            move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 4096];
                let length = stream.read(&mut request).unwrap_or(0);
                let _ = stream.write_all(before);
                // Ends on a send, a dropped sender (a failed test) or the watchdog.
                let _ = held.recv_timeout(HELD_SERVER_WATCHDOG);
                resumed.store(true, Ordering::SeqCst);
                let _ = stream.write_all(after);
                request[..length].to_vec()
            }
        });
        Self {
            config,
            root,
            address,
            release,
            resumed,
            thread,
        }
    }
    /// Whether the server has stopped withholding the rest of its response.
    fn resumed(&self) -> bool {
        self.resumed.load(Ordering::SeqCst)
    }
    /// Lets the server finish, and returns the request bytes it had read.
    fn release(self) -> Vec<u8> {
        let _ = self.release.send(());
        end_accept(self.address);
        let request = self.thread.join().unwrap();
        let _ = std::fs::remove_dir_all(self.root);
        request
    }
}

#[test]
fn response_without_length_keeps_exact_bytes_across_initial_buffer_boundary() {
    let payload: Vec<_> = (0..13_000)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect();
    let mut wire = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
    wire.extend_from_slice(&payload);
    let (config, thread, root) = fake_response(wire, Duration::ZERO);
    let result = Fixture::client(&config).get(&solver(), &KEY).unwrap();
    assert_eq!(result.as_deref(), Some(payload.as_slice()));
    thread.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn known_length_zero_small_boundary_and_large_keep_complete_exact_body() {
    for length in [0, 5, 8192, 8193] {
        let payload: Vec<_> = (0..length)
            .map(|index| u8::try_from(index % 251).unwrap())
            .collect();
        let mut wire =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n")
                .into_bytes();
        wire.extend_from_slice(&payload);
        let (config, thread, root) = fake_response(wire, Duration::ZERO);
        let client = Fixture::client(&config);
        let result = client.get(&solver(), &KEY);
        thread.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
        assert_eq!(result.unwrap().as_deref(), Some(payload.as_slice()));
        assert_eq!(client.info().last_status.as_deref(), Some("ok"));
    }
}

#[test]
fn small_known_length_truncation_keeps_interrupted_response_classification() {
    let (config, thread, root) = fake_response(
        b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nshort",
        Duration::ZERO,
    );
    let client = Fixture::client(&config);
    let result = client.get(&solver(), &KEY);
    thread.join().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap_err(), "SAIL0650: shared response interrupted");
    assert_eq!(
        client.info().last_status.as_deref(),
        Some("interrupted-response")
    );
    assert_eq!(client.info().unavailable, 1);
}

#[test]
fn known_length_missing_final_byte_still_obeys_original_request_timeout() {
    let (config, unused, root) = fake_response(
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
        Duration::ZERO,
    );
    let setup = Fixture::client(&config).get(&solver(), &KEY);
    unused.join().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(setup.unwrap(), None);
    // The final body byte is withheld until the client has returned, so the
    // client can only have stopped on its own request timeout. That timeout
    // is the fixture's ordinary one: it must outlast the response headers'
    // arrival, as every other exchange in this file assumes, and a shorter
    // one would let a loaded host turn this case into a plain request timeout.
    let server = HeldResponse::new(
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nabc",
        b"d",
    );
    let client = Fixture::client(&server.config);
    let result = client.get(&solver(), &KEY);
    let withheld = !server.resumed();
    let request = server.release();
    assert!(
        withheld,
        "the client returned only after the final byte was released: {result:?}"
    );
    assert!(request.starts_with(b"GET /v2/"), "request: {request:?}");
    assert_eq!(result.unwrap_err(), "SAIL0650: shared response interrupted");
    assert_eq!(
        client.info().last_status.as_deref(),
        Some("interrupted-response")
    );
    assert_eq!(client.info().unavailable, 1);
}

#[test]
fn ambiguous_or_invalid_length_remains_unavailable_and_chunked_stays_streamed() {
    for wire in [
        &b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nContent-Length: 4\r\nConnection: close\r\n\r\nbody"[..],
        &b"HTTP/1.1 200 OK\r\nContent-Length: invalid\r\nConnection: close\r\n\r\nbody"[..],
    ] {
        let (config, thread, root) = fake_response(wire, Duration::ZERO);
        let client = Fixture::client(&config);
        let result = client.get(&solver(), &KEY);
        thread.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
        assert!(result.is_err());
        assert_eq!(client.info().unavailable, 1);
    }
    let (config, thread, root) = fake_response(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n",
        Duration::ZERO,
    );
    let result = Fixture::client(&config).get(&solver(), &KEY);
    thread.join().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap(), Some(b"abcde".to_vec()));
}

#[test]
fn client_refuses_redirects_encodings_oversize_and_stalled_responses() {
    for response in [
        &b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\n\r\n"[..],
        &b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 3\r\n\r\nbad"[..],
        &b"HTTP/1.1 200 OK\r\nContent-Length: 67108865\r\n\r\n"[..],
        &b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n"[..],
    ] {
        let (config, thread, root) = fake_response(response, Duration::ZERO);
        assert!(
            HttpClient::new(&config, Duration::from_millis(250))
                .unwrap()
                .get(&solver(), &KEY)
                .is_err()
        );
        thread.join().unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
    // The whole response is withheld until the client has returned and been
    // dropped, so neither the request nor the client's shutdown can have
    // waited for the server. The earlier form of this case bounded their
    // elapsed time at 250 ms against a 300 ms server, which a loaded host
    // exceeds in the constructor alone. The bound kept here is causal: the
    // client must return and shut down before the server resumes, and until
    // the release below only the server's watchdog can resume it. A client
    // whose timeout fires late beyond that bound, or not at all, fails the
    // `withheld` assertion; the exact timeout status rules out any other
    // reason for returning early.
    let server = HeldResponse::new(b"", b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
    let client = HttpClient::new(&server.config, Duration::from_millis(50)).unwrap();
    let result = client.get(&solver(), &KEY);
    let info = client.info();
    drop(client);
    let withheld = !server.resumed();
    let request = server.release();
    assert!(
        withheld,
        "the client returned only after the response was released: {result:?}"
    );
    // A request abandoned before it was sent leaves nothing to read.
    assert!(
        request.is_empty() || request.starts_with(b"GET /v2/"),
        "request: {request:?}"
    );
    assert_eq!(result.unwrap_err(), "SAIL0650: shared store unavailable");
    assert_eq!(info.last_status.as_deref(), Some("timeout"));
    assert_eq!(info.unavailable, 1);
}

/// A fresh test CA and a server certificate for 127.0.0.1 that it signs:
/// (CA PEM, server certificate, server key). Generated for each run, so no
/// private key is committed and nothing expires.
fn local_tls() -> (
    String,
    rustls::pki_types::CertificateDer<'static>,
    rustls::pki_types::PrivateKeyDer<'static>,
) {
    use rcgen::{
        BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer,
        KeyPair, KeyUsagePurpose,
    };
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "sail-http-test-ca");
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = Issuer::new(ca_params, ca_key);
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(vec!["127.0.0.1".to_owned()]).unwrap();
    params
        .distinguished_name
        .push(DnType::CommonName, "sail-http-test");
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let certificate = params.signed_by(&key, &issuer).unwrap();
    (
        ca.pem(),
        certificate.der().clone(),
        rustls::pki_types::PrivateKeyDer::Pkcs8(key.serialize_der().into()),
    )
}

#[test]
fn tls_requires_trusted_ca_and_matching_hostname() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (ca_pem, certificate, key) = local_tls();
    let tls = std::sync::Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], key)
            .unwrap(),
    );
    let root = std::env::temp_dir().join(format!(
        "sail-tls-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let ca = root.join("ca.pem");
    std::fs::write(&ca, ca_pem).unwrap();
    let token_file = root.join("read.token");
    std::fs::write(&token_file, "0123456789abcdef0123456789abcdef").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    for (trusted, host, succeeds) in [
        (false, "127.0.0.1", false),
        (true, "127.0.0.1", true),
        (true, "localhost", false),
    ] {
        let listener = TcpListener::bind("0.0.0.0:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let server_tls = std::sync::Arc::clone(&tls);
        let thread = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                if started.elapsed() > Duration::from_secs(3) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let connection = rustls::ServerConnection::new(server_tls).unwrap();
            let mut stream = rustls::StreamOwned::new(connection, stream);
            let mut input = [0u8; 4096];
            if stream.read(&mut input).is_ok() {
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                stream.conn.send_close_notify();
                let _ = stream.flush();
            }
        });
        let config = RemoteConfig {
            endpoint: format!("https://{host}:{port}"),
            token_file: token_file.clone(),
            ca_file: trusted.then(|| ca.clone()),
        };
        let result = HttpClient::new(&config, Duration::from_secs(2))
            .unwrap()
            .get(&solver(), &KEY);
        assert_eq!(
            result.is_ok(),
            succeeds,
            "trusted {trusted}, hostname {host}: {result:?}"
        );
        thread.join().unwrap();
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn malformed_paths_and_concurrent_publications_cannot_replace_entries() {
    let fixture = Fixture::new();
    let token = sail_daemon::remote::read_token(&fixture.write.token_file).unwrap();
    for path in [
        "/../.sail-server/read.token",
        "/v2/%2e%2e/xx/07/0707",
        "/v2/linux/abcd/00/00",
        "/.sail-server/read.token",
    ] {
        let mut stream = TcpStream::connect(fixture.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 404"), "{path}: {response}");
        assert!(!response.contains(&token));
    }
    let secret = SecretKey::from_seed([5; 32]);
    let data = sign_entry(&secret, &KEY, b"unsat\n", "check-sat", &solver(), None, 1);
    let client = Fixture::client(&fixture.write);
    let public = secret.public();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let workers = (0..4)
        .map(|_| {
            let client = client.clone();
            let data = data.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                client.put(&solver(), &KEY, data, &public)
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|w| w.join().unwrap().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        results.iter().filter(|r| **r == Published::Created).count(),
        1
    );
    assert_eq!(
        results.iter().filter(|r| **r == Published::Present).count(),
        3
    );
    assert_eq!(
        Fixture::client(&fixture.read).get(&solver(), &KEY).unwrap(),
        Some(data)
    );
}

#[test]
fn chunked_bodies_are_read_exactly_and_abrupt_termination_is_unavailable() {
    let (config,thread,root) = fake_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n",Duration::ZERO);
    assert_eq!(
        Fixture::client(&config).get(&solver(), &KEY).unwrap(),
        Some(b"abcde".to_vec())
    );
    thread.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
    let (config, thread, root) = fake_response(
        b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\npartial",
        Duration::ZERO,
    );
    assert!(Fixture::client(&config).get(&solver(), &KEY).is_err());
    thread.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn auth_refusals_have_separate_transport_counters() {
    let (config, thread, root) = fake_response(
        b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n",
        Duration::ZERO,
    );
    let client = Fixture::client(&config);
    assert!(client.get(&solver(), &KEY).is_err());
    let info = client.info();
    assert_eq!(info.requests, 1);
    assert_eq!(info.auth_refused, 1);
    assert_eq!(info.unavailable, 1);
    assert_eq!(info.last_status.as_deref(), Some("http-401"));
    thread.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rejected_explicit_urls_never_leak_in_daemon_logs_or_status() {
    let root = PathBuf::from(format!("/tmp/sail-http-redact-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let config = sail_daemon::Config {
        log: Some(root.join("daemon.log")),
        shared: Some(sail_daemon::SharedConfig {
            store: "https://username:supersecret@example.com/?token=secretquery".into(),
            trust_dir: root.join("trust"),
            timeout: Duration::from_millis(250),
            backoff: Duration::from_secs(30),
        }),
        ..sail_daemon::Config::default()
    };
    let socket = root.join("d.sock");
    let server = sail_daemon::bind(
        &socket,
        &root,
        std::sync::Arc::new(sail_daemon::Stats::default()),
        &config,
    )
    .unwrap();
    let stop = server.handle();
    let thread = std::thread::spawn(move || server.run());
    let mut client = sail_api::client::Client::connect(&socket).unwrap();
    let info = serde_json::to_string(&client.info().unwrap()).unwrap();
    drop(client);
    stop.shutdown();
    thread.join().unwrap().unwrap();
    let log = std::fs::read_to_string(root.join("daemon.log")).unwrap();
    for text in [&info, &log] {
        assert!(!text.contains("supersecret"));
        assert!(!text.contains("secretquery"));
        assert!(text.contains("invalid HTTP endpoint"));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn existing_daemon_picks_up_new_remote_configuration_without_restart() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    let root = PathBuf::from(format!("/tmp/sail-http-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let socket = root.join("d.sock");
    let cache = root.join("cache");
    let secret = SecretKey::from_seed([72; 32]);
    sail_cache::shared::add_trusted(&cache, &secret.public()).unwrap();
    let fake = root.join("z3");
    std::fs::write(&fake, "#!/bin/sh\necho 'Z3 version 4.16.0 - 64 bit'\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let server = sail_daemon::bind(
        &socket,
        &cache,
        std::sync::Arc::new(sail_daemon::Stats::default()),
        &sail_daemon::Config::default(),
    )
    .unwrap();
    let stop = server.handle();
    let thread = std::thread::spawn(move || server.run());
    let mut client = sail_api::client::Client::connect(&socket).unwrap();
    let identity = client.hello(fake.to_str().unwrap()).unwrap();
    let identity = Solver {
        name: identity.name,
        version: identity.version,
        binary_sha256: identity.binary_sha256,
        platform: identity.platform,
    };
    assert!(client.info().unwrap().shared.is_none());
    assert_eq!(client.get(&KEY.0).unwrap(), None);
    let data = sign_entry(&secret, &KEY, b"unsat\n", "check-sat", &identity, None, 1);
    assert_eq!(
        Fixture::client(&fixture.write).put(&identity, &KEY, data, &secret.public()),
        Ok(Published::Created)
    );
    sail_daemon::remote::connect(&cache, &fixture.read).unwrap();
    let started = std::time::Instant::now();
    loop {
        if let Some(reply) = client.get(&KEY.0).unwrap() {
            assert_eq!(reply, b"unsat\n");
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(7),
            "daemon did not reload configuration"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let info = client.info().unwrap().shared.unwrap().remote.unwrap();
    assert_eq!(info.last_status.as_deref(), Some("ok"));
    assert_eq!(info.requests, 1);
    drop(client);
    stop.shutdown();
    thread.join().unwrap().unwrap();
    let _ = std::fs::remove_dir_all(root);
}
