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
) -> (RemoteConfig, std::thread::JoinHandle<()>, PathBuf) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
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
    (config, thread, root)
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
    let (config, thread, root) = fake_response(
        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n",
        Duration::from_millis(300),
    );
    let started = std::time::Instant::now();
    let client = HttpClient::new(&config, Duration::from_millis(50)).unwrap();
    let constructed = started.elapsed();
    let result = client.get(&solver(), &KEY);
    let fetched = started.elapsed();
    assert!(result.is_err());
    // The original temporary client drops before the following elapsed check.
    // Keep that runtime join inside the measured scope when naming the client.
    drop(client);
    let elapsed = started.elapsed();
    eprintln!(
        "cold stalled response: constructor={constructed:?}, request={:?}, drop={:?}, total={elapsed:?}, error={:?}; original request=50ms server=300ms total-bound=250ms",
        fetched.saturating_sub(constructed),
        elapsed.saturating_sub(fetched),
        result.as_ref().err()
    );
    assert!(elapsed < Duration::from_millis(250));
    thread.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tls_requires_trusted_ca_and_matching_hostname() {
    use rustls::pki_types::pem::PemObject as _;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let certificate = rustls::pki_types::CertificateDer::from_pem_slice(include_bytes!(
        "fixtures/http-local.pem"
    ))
    .unwrap();
    let key =
        rustls::pki_types::PrivateKeyDer::from_pem_slice(include_bytes!("fixtures/http-local.key"))
            .unwrap();
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
    std::fs::write(&ca, include_bytes!("fixtures/http-ca.pem")).unwrap();
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
