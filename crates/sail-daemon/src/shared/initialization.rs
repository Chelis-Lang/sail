//! Optional transport initialization owned by bounded shared-lookup helpers.

use std::path::Path;
use std::sync::Mutex;
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
}

#[derive(Default)]
pub(super) struct State {
    current: Mutex<Option<Remote>>,
    initializer: Mutex<()>,
    #[cfg(test)]
    pub(super) before_constructor: Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>>,
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
    /// Inspect while the cache retains ownership; a foreground reader must
    /// never own the final transport clone when a helper retires it.
    pub(super) fn info(&self) -> Option<sail_api::protocol::RemoteInfo> {
        let current = self.current.lock().ok()?;
        Some(current.as_ref()?.client.as_ref()?.info())
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
        self.current.lock().ok()?.as_ref()?.endpoint.clone()
    }

    /// Only call from a bounded helper: client construction itself has no deadline.
    pub(super) fn resolve(
        &self,
        root: &Path,
        config: &SharedConfig,
        changed_configuration: impl FnOnce(),
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
        #[cfg(test)]
        if configured.is_some()
            && let Some(pause) = self.before_constructor.lock().ok().and_then(|v| v.clone())
        {
            pause();
        }
        let mut client = configured.and_then(|c| HttpClient::new(&c, config.timeout).ok());
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
        let replaced = if let Ok(mut current) = self.current.lock() {
            current.replace(Remote {
                loaded: Instant::now(),
                client: client.clone(),
                endpoint,
            })
        } else {
            return Resolution::Initializing;
        };
        // The last reqwest client joins its runtime when dropped. Neither that
        // join nor its construction may hold the foreground snapshot mutex.
        if changed {
            changed_configuration();
        }
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
