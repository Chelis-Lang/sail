//! Optional authenticated HTTP shared-store transport and private reader configuration.
//! No client or runtime is constructed until a remote store is configured.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::{StatusCode, Url};
use sail_cache::shared::{MAX_ENTRY_BYTES, PublicKey, Solver};
use sail_key::QueryKey;
use serde::{Deserialize, Serialize};

/// Private configuration filename beneath the local cache root.
pub const CONFIG_FILE: &str = "sail-remote.json";

/// Reader settings. Credentials are referenced by file, never stored inline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteConfig {
    /// HTTPS endpoint, or literal loopback HTTP endpoint for local use.
    pub endpoint: String,
    /// Owner-only bearer credential file.
    pub token_file: PathBuf,
    /// Optional PEM certificate authority for private deployments.
    pub ca_file: Option<PathBuf>,
}

fn failure(message: &str) -> String {
    format!("SAIL0650: {message}")
}

const INITIAL_BODY_BYTES: usize = 8 * 1024;

fn read_body(mut response: impl std::io::Read, length: Option<u64>) -> std::io::Result<Vec<u8>> {
    // Only the pinned transport's exact body size hint enters here. Its HTTP1
    // decoder enforces this message boundary; early EOF remains an error.
    // The fast path uses the same fixed allocation as the streaming path and
    // skips the additional blocking response Read used to observe EOF.
    let mut bytes = Vec::with_capacity(INITIAL_BODY_BYTES);
    if let Some(length) = length.filter(|length| *length <= INITIAL_BODY_BYTES as u64) {
        let length = usize::try_from(length).map_err(std::io::Error::other)?;
        bytes.resize(length, 0);
        response.read_exact(&mut bytes)?;
    } else {
        response.take(MAX_ENTRY_BYTES + 1).read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

/// Validates an endpoint without accepting URL credentials, queries or fragments.
/// HTTP is permitted only for literal loopback IP addresses.
///
/// # Errors
/// The endpoint is malformed or does not satisfy the transport policy.
pub fn endpoint(text: &str) -> Result<Url, String> {
    let mut url = Url::parse(text).map_err(|_| failure("invalid shared endpoint"))?;
    let loopback = url.host_str().is_some_and(|h| {
        h.trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
    });
    if !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(failure(
            "use HTTPS (HTTP only on literal loopback), without URL credentials, query or fragment",
        ));
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

/// Reads a bounded owner-only credential file, refusing symlinks and blank tokens.
///
/// # Errors
/// The file is unsafe, unreadable or malformed.
pub fn read_token(path: &Path) -> Result<String, String> {
    let meta =
        std::fs::symlink_metadata(path).map_err(|_| failure("cannot read credential file"))?;
    if !meta.is_file() || meta.len() > 4096 {
        return Err(failure(
            "credential must be a regular file of at most 4096 bytes",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(failure("credential file must be private (chmod 600)"));
        }
    }
    let text = std::fs::read_to_string(path).map_err(|_| failure("cannot read credential file"))?;
    let token = text.trim();
    if token.len() < 16
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(failure(
            "credential must contain at least 16 token characters",
        ));
    }
    Ok(token.to_owned())
}

/// Loads optional remote reader settings. Invalid settings fail safe.
///
/// # Errors
/// A present configuration is invalid or unreadable.
pub fn load(root: &Path) -> Result<Option<RemoteConfig>, String> {
    let path = root.join(CONFIG_FILE);
    let data = match std::fs::read(&path) {
        Ok(d) if d.len() <= 16 * 1024 => d,
        Ok(_) => return Err(failure("remote configuration is too large")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(failure("cannot read remote configuration")),
    };
    let config: RemoteConfig =
        serde_json::from_slice(&data).map_err(|_| failure("invalid remote configuration"))?;
    endpoint(&config.endpoint)?;
    Ok(Some(config))
}

/// Saves settings atomically, validating credentials and TLS setup first.
///
/// # Errors
/// Validation or private configuration persistence fails.
pub fn connect(root: &Path, config: &RemoteConfig) -> Result<(), String> {
    HttpClient::new(config, Duration::from_millis(250))?;
    std::fs::create_dir_all(root).map_err(|_| failure("cannot create cache root"))?;
    let path = root.join(CONFIG_FILE);
    let tmp = root.join(format!(".sail-remote-{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let data = serde_json::to_vec(config).map_err(|_| failure("cannot encode remote settings"))?;
    let result = options
        .open(&tmp)
        .and_then(|mut f| f.write_all(&data))
        .and_then(|()| std::fs::rename(&tmp, &path));
    let _ = std::fs::remove_file(tmp);
    result.map_err(|_| failure("cannot save remote settings"))
}

/// Removes remote settings, leaving local entries and signing trust intact.
///
/// # Errors
/// A present configuration cannot be removed.
pub fn disconnect(root: &Path) -> Result<(), String> {
    match std::fs::remove_file(root.join(CONFIG_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(failure("cannot remove remote configuration")),
    }
}

/// A pooled HTTP client. Debug deliberately excludes endpoint and credentials.
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base: Url,
    token: String,
    metrics: Arc<Metrics>,
    ca_hash: Option<[u8; 32]>,
    timeout: Duration,
}

#[derive(Default)]
struct Metrics {
    requests: AtomicU64,
    auth: AtomicU64,
    unavailable: AtomicU64,
    last: Mutex<Option<String>>,
    #[cfg(test)]
    phases: Mutex<Vec<(&'static str, std::time::Instant)>>,
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HttpClient(<private configuration>)")
    }
}

/// An authenticated publication's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Published {
    /// New entry committed.
    Created,
    /// Identical entry already present.
    Present,
    /// Another valid publication owns this path.
    Conflict,
}

impl HttpClient {
    /// Constructs a TLS-validating client with one whole-request deadline.
    ///
    /// # Errors
    /// The endpoint, credential, CA or HTTP client is invalid.
    pub fn new(config: &RemoteConfig, timeout: Duration) -> Result<Self, String> {
        Self::refresh(config, timeout, None)
    }

    /// Revalidate private configuration before reusing an identical loopback
    /// HTTP transport. HTTPS reconstructs so platform root reload is unchanged.
    pub(crate) fn refresh(
        config: &RemoteConfig,
        timeout: Duration,
        previous: Option<&Self>,
    ) -> Result<Self, String> {
        #[cfg(test)]
        let began = std::time::Instant::now();
        let base = endpoint(&config.endpoint)?;
        let token = read_token(&config.token_file)?;
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            // Concurrent readers may race installation; either winner installs the same provider.
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        let mut builder = Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy();
        let mut ca_hash = None;
        if let Some(path) = &config.ca_file {
            let pem = std::fs::read(path).map_err(|_| failure("cannot read private CA file"))?;
            if pem.len() > 1024 * 1024 {
                return Err(failure("private CA file is too large"));
            }
            // The rustls `from_pem` constructor only stores bytes; a PEM with
            // no certificates can otherwise silently add no private roots.
            let certificates = reqwest::Certificate::from_pem_bundle(&pem)
                .map_err(|_| failure("invalid private CA certificate"))?;
            if certificates.is_empty() {
                return Err(failure("private CA file contains no certificates"));
            }
            ca_hash = Some(sail_key::sha256(&pem));
            for certificate in certificates {
                builder = builder.add_root_certificate(certificate);
            }
        }
        if base.scheme() == "http"
            && let Some(previous) = previous
            && previous.base == base
            && previous.token == token
            && previous.ca_hash == ca_hash
            && previous.timeout == timeout
        {
            #[cfg(test)]
            previous.phase("client-configuration-reused");
            return Ok(previous.clone());
        }
        let client = builder
            .build()
            .map_err(|_| failure("cannot initialize HTTP transport"))?;
        let metrics = Arc::new(Metrics::default());
        #[cfg(test)]
        if let Ok(mut records) = metrics.phases.lock() {
            records.push(("client-construction-start", began));
            records.push(("client-construction-end", std::time::Instant::now()));
        }
        Ok(Self {
            client,
            base,
            token,
            metrics,
            ca_hash,
            timeout,
        })
    }

    #[cfg(test)]
    fn phase(&self, name: &'static str) {
        if let Ok(mut records) = self.metrics.phases.lock() {
            records.push((name, std::time::Instant::now()));
        }
    }

    #[cfg(test)]
    pub(crate) fn phases(&self) -> Vec<(&'static str, std::time::Instant)> {
        self.metrics
            .phases
            .lock()
            .map(|records| records.clone())
            .unwrap_or_default()
    }

    pub(crate) fn same_configuration(&self, other: &Self) -> bool {
        self.base == other.base
            && self.token == other.token
            && self.ca_hash == other.ca_hash
            && self.timeout == other.timeout
    }

    pub(crate) fn retain_metrics(&mut self, previous: &Self) {
        self.metrics = Arc::clone(&previous.metrics);
    }

    /// Sanitized transport state, suitable for status output.
    #[must_use]
    pub fn info(&self) -> sail_api::protocol::RemoteInfo {
        sail_api::protocol::RemoteInfo {
            endpoint: self.base.to_string(),
            last_status: self.metrics.last.try_lock().ok().and_then(|v| v.clone()),
            requests: self.metrics.requests.load(Ordering::Relaxed),
            auth_refused: self.metrics.auth.load(Ordering::Relaxed),
            unavailable: self.metrics.unavailable.load(Ordering::Relaxed),
            coalesced: 0,
        }
    }

    fn record(&self, status: &str, unavailable: bool) {
        if let Ok(mut current) = self.metrics.last.lock() {
            *current = Some(status.to_owned());
        }
        if unavailable {
            self.metrics.unavailable.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Relative format-v2 path, using only validated solver identity fields.
    #[must_use]
    pub fn entry_path(solver: &Solver, key: &QueryKey) -> Option<String> {
        if !solver.well_formed() {
            return None;
        }
        let key = key.to_string();
        Some(format!(
            "v2/{}/{}/{}/{}",
            solver.platform,
            solver.binary_sha256,
            key.get(..2)?,
            key
        ))
    }

    /// Fetches bounded bytes. Authentication, HTTP and transport failures are unavailable stores.
    ///
    /// # Errors
    /// The store is unavailable or the response exceeds the limit.
    pub fn get(&self, solver: &Solver, key: &QueryKey) -> Result<Option<Vec<u8>>, String> {
        #[cfg(test)]
        self.phase("request-start");
        let path =
            Self::entry_path(solver, key).ok_or_else(|| failure("invalid solver identity"))?;
        let url = self
            .base
            .join(&path)
            .map_err(|_| failure("invalid entry path"))?;
        self.metrics.requests.fetch_add(1, Ordering::Relaxed);
        let request = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .header("Accept-Encoding", "identity");
        #[cfg(test)]
        self.phase("request-built");
        let sent = request.send();
        #[cfg(test)]
        self.phase("request-send-returned");
        let response = sent.map_err(|e| {
            self.record(
                if e.is_timeout() {
                    "timeout"
                } else {
                    "unavailable"
                },
                true,
            );
            failure("shared store unavailable")
        })?;
        if response.status() == StatusCode::NOT_FOUND {
            self.record("not-found", false);
            return Ok(None);
        }
        if response.status() != StatusCode::OK
            || response.headers().contains_key("content-encoding")
        {
            let status = response.status();
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                self.metrics.auth.fetch_add(1, Ordering::Relaxed);
            }
            self.record(&format!("http-{}", status.as_u16()), true);
            return Err(failure("shared store rejected lookup"));
        }
        let length = response.content_length();
        if length.is_some_and(|n| n > MAX_ENTRY_BYTES) {
            self.record("oversized-response", true);
            return Err(failure("shared response exceeds entry limit"));
        }
        let bytes = read_body(response, length).map_err(|_| {
            self.record("interrupted-response", true);
            failure("shared response interrupted")
        })?;
        if bytes.len() as u64 > MAX_ENTRY_BYTES {
            self.record("oversized-response", true);
            return Err(failure("shared response exceeds entry limit"));
        }
        self.record("ok", false);
        #[cfg(test)]
        self.phase("response-body-completed");
        Ok(Some(bytes))
    }

    /// Uploads a signed entry. The supplied public key checks publication integrity,
    /// and does not grant trust to any reader.
    ///
    /// # Errors
    /// Publication fails, or the store refuses the upload.
    pub fn put(
        &self,
        solver: &Solver,
        key: &QueryKey,
        data: Vec<u8>,
        public: &PublicKey,
    ) -> Result<Published, String> {
        if data.len() as u64 > MAX_ENTRY_BYTES {
            return Err(failure("publication exceeds entry limit"));
        }
        let path =
            Self::entry_path(solver, key).ok_or_else(|| failure("invalid solver identity"))?;
        let url = self
            .base
            .join(&path)
            .map_err(|_| failure("invalid entry path"))?;
        let response = self
            .client
            .put(url)
            .bearer_auth(&self.token)
            .header("sail-public-key", public.to_text().trim())
            .header("Content-Type", "application/octet-stream")
            .body(data)
            .send()
            .map_err(|_| failure("shared publication unavailable"))?;
        match response.status() {
            StatusCode::CREATED => Ok(Published::Created),
            StatusCode::NO_CONTENT => Ok(Published::Present),
            StatusCode::CONFLICT => Ok(Published::Conflict),
            _ => Err(failure("shared publication refused")),
        }
    }
}

#[cfg(all(test, unix))]
mod refresh_tests {
    #![allow(clippy::unwrap_used, reason = "test setup and assertions")]

    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct CountedRead<'a> {
        remaining: &'a [u8],
        calls: usize,
        eof: usize,
    }
    impl std::io::Read for CountedRead<'_> {
        fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
            self.calls += 1;
            let read = self.remaining.read(destination)?;
            self.eof += usize::from(read == 0);
            Ok(read)
        }
    }

    #[test]
    fn small_exact_body_avoids_eof_read_with_only_existing_fixed_capacity() {
        for length in [0, 5, INITIAL_BODY_BYTES] {
            let expected = vec![7; length];
            let mut input = CountedRead {
                remaining: &expected,
                calls: 0,
                eof: 0,
            };
            let result = read_body(&mut input, Some(length as u64)).unwrap();
            assert_eq!(result, expected);
            assert_eq!(result.capacity(), INITIAL_BODY_BYTES);
            assert_eq!(input.calls, usize::from(length != 0));
            assert_eq!(input.eof, 0);
        }
        let mut short = CountedRead {
            remaining: b"short",
            calls: 0,
            eof: 0,
        };
        assert_eq!(
            read_body(&mut short, Some(6)).unwrap_err().kind(),
            std::io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn large_and_unknown_length_keep_original_stream_and_eof() {
        for length in [None, Some(INITIAL_BODY_BYTES as u64 + 1)] {
            let expected = vec![11; INITIAL_BODY_BYTES + 1];
            let mut input = CountedRead {
                remaining: &expected,
                calls: 0,
                eof: 0,
            };
            let result = read_body(&mut input, length).unwrap();
            assert_eq!(result, expected);
            assert_eq!(input.eof, 1);
            assert!(input.calls >= 2);
        }
    }

    struct Fixture {
        root: PathBuf,
        config: RemoteConfig,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "sail-http-refresh-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            let token = root.join("read.token");
            std::fs::write(&token, "0123456789abcdef0123456789abcdef").unwrap();
            std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
            Self {
                config: RemoteConfig {
                    endpoint: "http://127.0.0.1:9/".to_owned(),
                    token_file: token,
                    ca_file: None,
                },
                root,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn unchanged_loopback_configuration_reuses_transport_after_revalidation() {
        let fixture = Fixture::new();
        let timeout = Duration::from_millis(250);
        let first = HttpClient::new(&fixture.config, timeout).unwrap();
        let second = HttpClient::refresh(&fixture.config, timeout, Some(&first)).unwrap();
        assert!(Arc::ptr_eq(&first.metrics, &second.metrics));
        assert!(
            second
                .phases()
                .iter()
                .any(|(name, _)| *name == "client-configuration-reused")
        );
        assert_eq!(second.info().requests, 0, "refresh must not send a request");
        std::fs::set_permissions(
            &fixture.config.token_file,
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(HttpClient::refresh(&fixture.config, timeout, Some(&first)).is_err());
        std::fs::remove_file(&fixture.config.token_file).unwrap();
        assert!(HttpClient::refresh(&fixture.config, timeout, Some(&first)).is_err());
    }

    #[test]
    fn changed_credentials_endpoint_or_timeout_replace_transport() {
        let mut fixture = Fixture::new();
        let timeout = Duration::from_millis(250);
        let first = HttpClient::new(&fixture.config, timeout).unwrap();
        let changed_timeout =
            HttpClient::refresh(&fixture.config, Duration::from_millis(50), Some(&first)).unwrap();
        assert!(!first.same_configuration(&changed_timeout));
        assert!(!Arc::ptr_eq(&first.metrics, &changed_timeout.metrics));
        std::fs::write(
            &fixture.config.token_file,
            "fedcba9876543210fedcba9876543210",
        )
        .unwrap();
        let changed_token = HttpClient::refresh(&fixture.config, timeout, Some(&first)).unwrap();
        assert!(!first.same_configuration(&changed_token));
        assert!(!Arc::ptr_eq(&first.metrics, &changed_token.metrics));
        fixture.config.endpoint = "http://127.0.0.1:10/".to_owned();
        let changed_endpoint =
            HttpClient::refresh(&fixture.config, timeout, Some(&changed_token)).unwrap();
        assert!(!changed_token.same_configuration(&changed_endpoint));
        assert!(!Arc::ptr_eq(
            &changed_token.metrics,
            &changed_endpoint.metrics
        ));
    }

    #[test]
    fn complete_ca_bytes_and_validity_are_checked_before_loopback_reuse() {
        let mut fixture = Fixture::new();
        let ca = fixture.root.join("ca.pem");
        let pem = include_bytes!("../tests/fixtures/http-ca.pem");
        std::fs::write(&ca, pem).unwrap();
        fixture.config.ca_file = Some(ca.clone());
        let timeout = Duration::from_millis(250);
        let first = HttpClient::new(&fixture.config, timeout).unwrap();
        let same = HttpClient::refresh(&fixture.config, timeout, Some(&first)).unwrap();
        assert!(Arc::ptr_eq(&first.metrics, &same.metrics));
        let mut changed = pem.to_vec();
        changed.push(b'\n');
        std::fs::write(&ca, changed).unwrap();
        let changed_ca = HttpClient::refresh(&fixture.config, timeout, Some(&first)).unwrap();
        assert!(!first.same_configuration(&changed_ca));
        assert!(!Arc::ptr_eq(&first.metrics, &changed_ca.metrics));
        std::fs::write(&ca, b"invalid PEM").unwrap();
        assert!(HttpClient::refresh(&fixture.config, timeout, Some(&first)).is_err());
    }

    #[test]
    fn https_refresh_preserves_platform_root_reconstruction() {
        let mut fixture = Fixture::new();
        fixture.config.endpoint = "https://example.invalid/".to_owned();
        let timeout = Duration::from_millis(250);
        let first = HttpClient::new(&fixture.config, timeout).unwrap();
        let second = HttpClient::refresh(&fixture.config, timeout, Some(&first)).unwrap();
        assert!(first.same_configuration(&second));
        assert!(!Arc::ptr_eq(&first.metrics, &second.metrics));
        assert_eq!(second.info().requests, 0);
    }
}
