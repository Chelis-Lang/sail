//! Optional transport initialization owned by daemon setup and bounded lookup helpers.

use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use super::{SharedConfig, TRUST_RELOAD};
use crate::remote::{HttpClient, RemoteConfig};

#[derive(Clone)]
struct Remote {
    loaded: Instant,
    client: Option<HttpClient>,
    endpoint: Option<String>,
}

pub(super) enum Resolution {
    Ready(Option<HttpClient>),
    Initializing,
    TimedOut,
}

#[derive(Default)]
pub(super) struct State {
    current: Mutex<Option<Remote>>,
    descriptor: Mutex<Option<String>>,
    ready: Condvar,
    initializer: Mutex<()>,
    presence: super::presence::State,
    #[cfg(test)]
    pub(super) before_constructor: Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>>,
}

/// One setup constructor. Shutdown abandons its result; it never waits for a builder.
pub(super) struct Setup {
    abandoned: Arc<Mutex<bool>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Setup {
    fn drop(&mut self) {
        if let Ok(mut abandoned) = self.abandoned.lock() {
            *abandoned = true;
        }
        // Joining an unfinished blocking constructor would turn shutdown into
        // an unbounded wait. The shared constructor gate prevents overlap.
        if self.thread.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

fn configuration(root: &Path, config: &SharedConfig) -> Option<RemoteConfig> {
    let explicit = config.store.to_string_lossy();
    let configured = crate::remote::load(root).ok().flatten();
    if explicit.starts_with("http://") || explicit.starts_with("https://") {
        configured
            .filter(|c| c.endpoint.trim_end_matches('/') == explicit.trim_end_matches('/'))
            .or_else(|| {
                std::env::var_os("SAIL_SHARED_TOKEN_FILE").map(|token| RemoteConfig {
                    endpoint: explicit.to_string(),
                    token_file: token.into(),
                    ca_file: std::env::var_os("SAIL_SHARED_CA_FILE").map(Into::into),
                })
            })
    } else if explicit.is_empty() {
        configured
    } else {
        None
    }
}

impl State {
    /// Initialize a configured HTTP transport during daemon setup.
    /// No request or trust validation is performed. Optional presence discovery
    /// waits at most 100 ms; client construction belongs to the setup owner.
    pub(super) fn prepare(self: &Arc<Self>, root: &Path, config: &SharedConfig) -> Option<Setup> {
        let explicit = config.store.to_string_lossy();
        if !explicit.is_empty()
            && !explicit.starts_with("http://")
            && !explicit.starts_with("https://")
        {
            return None;
        }
        if self.configured(root, config) != Some(true) {
            return None;
        }
        let state = Arc::clone(self);
        let root = root.to_path_buf();
        let config = config.clone();
        let abandoned = Arc::new(Mutex::new(false));
        let cancelled = Arc::clone(&abandoned);
        let thread = std::thread::Builder::new()
            .name("sail-shared-setup".to_owned())
            .spawn(move || {
                let _ = state.resolve_inner(&root, &config, || {}, Some(&cancelled));
            })
            .ok()?;
        Some(Setup {
            abandoned,
            thread: Some(thread),
        })
    }

    pub(super) fn configured(&self, root: &Path, config: &SharedConfig) -> Option<bool> {
        self.presence.configured(root, config)
    }

    /// Inspect while the cache retains ownership; a foreground reader must
    /// never own the final transport clone when a helper retires it.
    pub(super) fn info(&self) -> Option<sail_api::protocol::RemoteInfo> {
        let current = self.current.try_lock().ok()?;
        Some(current.as_ref()?.client.as_ref()?.info())
    }

    #[cfg(test)]
    #[allow(
        clippy::unwrap_used,
        reason = "test fixture deliberately holds publication mutex"
    )]
    pub(super) fn current_for_test(&self) -> impl Drop + '_ {
        self.current.lock().unwrap()
    }

    #[cfg(test)]
    pub(super) fn phases(&self) -> Vec<(&'static str, Instant)> {
        self.current
            .lock()
            .ok()
            .and_then(|current| current.as_ref()?.client.as_ref().map(HttpClient::phases))
            .unwrap_or_default()
    }

    pub(super) fn endpoint(&self) -> Option<String> {
        self.descriptor.try_lock().ok()?.clone()
    }

    /// Only call from a bounded helper: client construction itself has no deadline.
    pub(super) fn resolve(
        &self,
        root: &Path,
        config: &SharedConfig,
        changed_configuration: impl FnOnce(),
    ) -> Resolution {
        self.resolve_inner(root, config, changed_configuration, None)
    }

    /// Only lookup helpers wait for another constructor, within their existing
    /// lookup budget. The mutex predicate prevents lost publication wakeups.
    pub(super) fn wait_ready(&self, deadline: Instant) -> Resolution {
        let Ok(current) = self.current.lock() else {
            return Resolution::Initializing;
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Ok((current, _)) = self
            .ready
            .wait_timeout_while(current, remaining, |current| {
                current
                    .as_ref()
                    .is_none_or(|remote| remote.loaded.elapsed() >= TRUST_RELOAD)
            })
        else {
            return Resolution::Initializing;
        };
        match current.as_ref() {
            Some(remote) if remote.loaded.elapsed() < TRUST_RELOAD => {
                Resolution::Ready(remote.client.clone())
            }
            _ => Resolution::TimedOut,
        }
    }

    fn resolve_inner(
        &self,
        root: &Path,
        config: &SharedConfig,
        changed_configuration: impl FnOnce(),
        abandoned: Option<&Mutex<bool>>,
    ) -> Resolution {
        if let Ok(current) = self.current.lock()
            && let Some(remote) = current.as_ref()
            && remote.loaded.elapsed() < TRUST_RELOAD
        {
            return Resolution::Ready(remote.client.clone());
        }
        let Ok(_initializing) = self.initializer.try_lock() else {
            return Resolution::Initializing;
        };
        if abandoned.is_some_and(|cancelled| cancelled.lock().map_or(true, |value| *value)) {
            return Resolution::Initializing;
        }
        let previous = match self.current.lock() {
            Ok(current) => current.clone(),
            Err(_) => return Resolution::Initializing,
        };
        if let Some(remote) = &previous
            && remote.loaded.elapsed() < TRUST_RELOAD
        {
            return Resolution::Ready(remote.client.clone());
        }
        let configured = configuration(root, config);
        let endpoint = configured
            .as_ref()
            .and_then(|c| crate::remote::endpoint(&c.endpoint).ok())
            .map(|url| url.to_string());
        // Display metadata is published separately before any token, CA or
        // client construction. It is never signing or serving authority.
        if let Ok(mut descriptor) = self.descriptor.lock() {
            descriptor.clone_from(&endpoint);
        }
        #[cfg(test)]
        if configured.is_some()
            && let Some(pause) = self.before_constructor.lock().ok().and_then(|v| v.clone())
        {
            pause();
        }
        let mut client = configured.and_then(|c| {
            HttpClient::refresh(
                &c,
                config.timeout,
                previous.as_ref().and_then(|remote| remote.client.as_ref()),
            )
            .ok()
        });
        if let (Some(new), Some(old)) = (&mut client, &previous)
            && old.endpoint == endpoint
            && let Some(previous) = &old.client
        {
            new.retain_metrics(previous);
        }
        let changed = previous.as_ref().is_none_or(|old| {
            old.endpoint != endpoint
                || match (&old.client, &client) {
                    (Some(old), Some(new)) => !old.same_configuration(new),
                    (None, None) => false,
                    _ => true,
                }
        });
        // Hold the setup cancellation lock through publication: after its
        // owner shuts down, a completed constructor cannot install a client.
        let cancellation = match abandoned.map(Mutex::lock) {
            Some(Ok(guard)) if !*guard => Some(guard),
            Some(_) => return Resolution::Initializing,
            None => None,
        };
        let replaced = if let Ok(mut current) = self.current.lock() {
            // Invalidate old-configuration suppression before publishing the
            // replacement. A waiting helper must not pair the new transport
            // with an old negative lookup or backoff decision.
            if changed {
                changed_configuration();
            }
            let replaced = current.replace(Remote {
                loaded: Instant::now(),
                client: client.clone(),
                endpoint,
            });
            self.ready.notify_all();
            replaced
        } else {
            return Resolution::Initializing;
        };
        drop(cancellation);
        // The last reqwest client joins its runtime when dropped. Neither that
        // join nor its construction may hold the foreground snapshot mutex.
        drop(replaced);
        Resolution::Ready(client)
    }

    #[cfg(test)]
    #[allow(clippy::unwrap_used, reason = "test-only fixture expiration")]
    pub(super) fn expire(&self) {
        self.current.lock().unwrap().as_mut().unwrap().loaded =
            Instant::now().checked_sub(TRUST_RELOAD).unwrap();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test setup and assertions")]
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    struct Fixture {
        root: std::path::PathBuf,
        tier: super::super::SharedTier,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            use std::os::unix::fs::PermissionsExt as _;
            let root = std::env::temp_dir()
                .join(format!("sail-http-setup-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let token = root.join("read.token");
            std::fs::write(&token, "0123456789abcdef0123456789abcdef").unwrap();
            std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
            crate::remote::connect(
                &root,
                &RemoteConfig {
                    endpoint: "http://127.0.0.1:1".to_owned(),
                    token_file: token,
                    ca_file: None,
                },
            )
            .unwrap();
            Self {
                // These tests exercise constructor ownership and reloads.
                // Optional automatic discovery may correctly decline after
                // 100 ms on a loaded host, leaving no setup owner to await.
                // Presence freshness and its bound have separate controls.
                tier: super::super::SharedTier::auto(
                    Some(SharedConfig {
                        store: "http://127.0.0.1:1".into(),
                        trust_dir: root.join("trust"),
                        timeout: super::super::DEFAULT_SHARED_TIMEOUT,
                        backoff: super::super::DEFAULT_SHARED_BACKOFF,
                    }),
                    &root,
                ),
                root,
            }
        }

        fn wait_finished(&self) {
            let started = Instant::now();
            loop {
                let finished = self
                    .tier
                    .setup
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .thread
                    .as_ref()
                    .unwrap()
                    .is_finished();
                if finished {
                    return;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(2),
                    "setup owner must finish"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn setup_fixture_does_not_depend_on_optional_presence_discovery() {
        let fixture = Fixture::new("explicit-setup");
        let count = Arc::new(AtomicUsize::new(0));
        let calls = Arc::clone(&count);
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        *fixture.tier.remote.presence.before_read.lock().unwrap() = Some(Arc::new(move || {
            calls.fetch_add(1, Ordering::Relaxed);
            let _ = release_rx.lock().unwrap().recv();
        }));
        fixture.tier.prepare();
        let owns_setup = fixture.tier.setup.lock().unwrap().is_some();
        // Release an old fixture's discovery worker before the assertion.
        let _ = release_tx.send(());
        assert!(
            owns_setup,
            "constructor tests require an explicit setup owner"
        );
        fixture.wait_finished();
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn daemon_setup_constructs_transport_without_network_or_trust() {
        let fixture = Fixture::new("no-request");
        fixture.tier.prepare();
        fixture.wait_finished();
        let info = fixture.tier.remote.info().unwrap();
        assert_eq!(info.requests, 0);
        assert_eq!(info.last_status, None);
        assert!(fixture.tier.trusted().0.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn blocked_configuration_file_read_never_runs_on_setup_caller() {
        let root = std::env::temp_dir().join(format!("sail-setup-fifo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let fifo = root.join(crate::remote::CONFIG_FILE);
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let state = Arc::new(State::default());
        let caller_state = Arc::clone(&state);
        let caller_root = root.clone();
        let config = SharedConfig {
            store: "http://127.0.0.1:1".into(),
            trust_dir: root.join("trust"),
            timeout: Duration::from_millis(250),
            backoff: Duration::from_secs(30),
        };
        let (returned_tx, returned_rx) = std::sync::mpsc::channel();
        let caller = std::thread::spawn(move || {
            let started = Instant::now();
            let owner = caller_state.prepare(&caller_root, &config);
            let _ = returned_tx.send((owner, started.elapsed()));
        });
        let received = returned_rx.recv_timeout(Duration::from_millis(250));
        // Linux FIFO read+write open does not wait for a peer. Release an old
        // foreground reader (negative control) or the new helper before asserting.
        let release = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&fifo)
            .unwrap();
        let opened_deadline = Instant::now() + Duration::from_secs(2);
        let reader_opened = loop {
            let descriptors = std::fs::read_dir("/proc/self/fd")
                .unwrap()
                .flatten()
                .filter(|entry| std::fs::read_link(entry.path()).ok().as_ref() == Some(&fifo))
                .count();
            if descriptors >= 2 {
                break true;
            }
            if Instant::now() >= opened_deadline {
                break false;
            }
            std::thread::yield_now();
        };
        drop(release);
        caller.join().unwrap();
        assert!(
            reader_opened,
            "owned configuration reader must be released before assertions"
        );
        let (owner, elapsed) = received.unwrap();
        assert!(owner.is_some());
        assert!(
            elapsed < Duration::from_millis(250),
            "setup waited {elapsed:?}"
        );
        drop(owner);
        let deadline = Instant::now() + Duration::from_secs(2);
        while state.initializer.try_lock().is_err() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unconfigured_and_directory_stores_have_no_setup_owner() {
        let state = Arc::new(State::default());
        let root = std::env::temp_dir().join(format!("sail-no-http-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut config = SharedConfig {
            store: std::path::PathBuf::new(),
            trust_dir: root.join("trust"),
            timeout: Duration::from_millis(250),
            backoff: Duration::from_secs(30),
        };
        assert!(state.prepare(&root, &config).is_none());
        config.store = root.join("shared-directory");
        assert!(state.prepare(&root, &config).is_none());
        assert!(state.current.lock().unwrap().is_none());
    }

    #[test]
    fn paused_setup_has_one_owner_and_shutdown_discards_its_client() {
        let fixture = Fixture::new("abandon");
        let count = Arc::new(AtomicUsize::new(0));
        let calls = Arc::clone(&count);
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        *fixture.tier.remote.before_constructor.lock().unwrap() = Some(Arc::new(move || {
            calls.fetch_add(1, Ordering::Relaxed);
            entered_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        }));
        fixture.tier.prepare();
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        for _ in 0..4 {
            fixture.tier.prepare();
            assert!(matches!(
                fixture
                    .tier
                    .remote
                    .resolve(&fixture.root, &fixture.tier.config, || {}),
                Resolution::Initializing
            ));
        }
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert!(fixture.tier.remote.info().is_none());
        drop(fixture.tier.setup.lock().unwrap().take());
        assert!(started.elapsed() < Duration::from_millis(250));
        release_tx.send(()).unwrap();
        let started = Instant::now();
        while fixture.tier.remote.initializer.try_lock().is_err() {
            assert!(started.elapsed() < Duration::from_secs(2));
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(fixture.tier.remote.current.lock().unwrap().is_none());
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn failed_setup_retains_reload_cadence_without_constructor_retries() {
        let fixture = Fixture::new("invalid-token");
        std::fs::write(fixture.root.join("read.token"), "bad").unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let calls = Arc::clone(&count);
        *fixture.tier.remote.before_constructor.lock().unwrap() = Some(Arc::new(move || {
            calls.fetch_add(1, Ordering::Relaxed);
        }));
        fixture.tier.prepare();
        fixture.wait_finished();
        for _ in 0..4 {
            fixture.tier.prepare();
            assert!(matches!(
                fixture
                    .tier
                    .remote
                    .resolve(&fixture.root, &fixture.tier.config, || {}),
                Resolution::Ready(None)
            ));
        }
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert!(fixture.tier.remote.info().is_none());
        fixture.tier.remote.expire();
        assert!(matches!(
            fixture
                .tier
                .remote
                .resolve(&fixture.root, &fixture.tier.config, || {}),
            Resolution::Ready(None)
        ));
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }
}
