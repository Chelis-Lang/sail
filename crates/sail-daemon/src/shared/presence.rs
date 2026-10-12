//! Bounded optional configuration discovery, without foreground filesystem calls.

use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::SharedConfig;

const STATUS_WAIT: Duration = Duration::from_millis(100);

struct Reply {
    started: Instant,
    value: Mutex<Option<bool>>,
    ready: Condvar,
}

struct Probe {
    reply: Arc<Reply>,
    worker: JoinHandle<()>,
}

#[derive(Default)]
pub(super) struct State {
    current: Mutex<Option<Probe>>,
    #[cfg(test)]
    pub(super) before_read: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl State {
    /// A completed earlier probe is never reused as current presence authority.
    /// One blocked filesystem call occupies the sole slot; other calls decline.
    pub(super) fn configured(&self, root: &Path, config: &SharedConfig) -> Option<bool> {
        if !config.store.as_os_str().is_empty() {
            return Some(true);
        }
        let entered = Instant::now();
        let wait = config.timeout.min(STATUS_WAIT);
        let reply = {
            let mut current = self.current.try_lock().ok()?;
            if let Some(probe) = &*current {
                // Publication can precede thread teardown. A completed I/O
                // answer is not fresh for this caller, but finish retiring its
                // sole owner within this caller's SAME bound before probing.
                let io_complete = probe.reply.value.try_lock().ok()?.is_some();
                if io_complete {
                    while !probe.worker.is_finished() {
                        if entered.elapsed() >= wait {
                            return None;
                        }
                        std::thread::yield_now();
                    }
                }
            }
            if current
                .as_ref()
                .is_some_and(|probe| probe.worker.is_finished())
                && let Some(probe) = current.take()
            {
                let _ = probe.worker.join();
            }
            if let Some(probe) = &*current {
                // Only a probe begun after THIS caller entered can establish
                // its fresh presence. Busy older owners are never invalidated.
                if probe.reply.started < entered {
                    return None;
                }
                Arc::clone(&probe.reply)
            } else {
                let path = root.join(crate::remote::CONFIG_FILE);
                let reply = Arc::new(Reply {
                    started: Instant::now(),
                    value: Mutex::new(None),
                    ready: Condvar::new(),
                });
                let publication = Arc::clone(&reply);
                #[cfg(test)]
                let before_read = self.before_read.try_lock().ok()?.clone();
                let worker = std::thread::Builder::new()
                    .name("sail-shared-presence".to_owned())
                    .spawn(move || {
                        #[cfg(test)]
                        if let Some(before_read) = before_read {
                            before_read();
                        }
                        let present = path.is_file();
                        if let Ok(mut value) = publication.value.lock() {
                            *value = Some(present);
                            publication.ready.notify_all();
                        }
                    })
                    .ok()?;
                *current = Some(Probe {
                    reply: Arc::clone(&reply),
                    worker,
                });
                reply
            }
        };
        let value = reply.value.try_lock().ok()?;
        let remaining = wait.saturating_sub(entered.elapsed());
        let (value, _) = reply
            .ready
            .wait_timeout_while(value, remaining, |value| value.is_none())
            .ok()?;
        if entered.elapsed() >= wait {
            return None;
        }
        *value
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if let Ok(current) = self.current.get_mut()
            && current
                .as_ref()
                .is_some_and(|probe| probe.worker.is_finished())
            && let Some(probe) = current.take()
        {
            let _ = probe.worker.join();
        }
        // An unfinished kernel call cannot be joined within a foreground bound.
        // It owns only its path and fixed reply, never a client or this State.
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test assertions")]
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn config() -> SharedConfig {
        SharedConfig {
            store: std::path::PathBuf::new(),
            trust_dir: std::path::PathBuf::new(),
            timeout: Duration::from_millis(250),
            backoff: Duration::from_secs(30),
        }
    }

    #[test]
    fn paused_presence_has_one_owner_and_never_returns_earlier_result() {
        let root = std::env::temp_dir().join(format!("sail-presence-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(crate::remote::CONFIG_FILE), b"{}").unwrap();
        let state = State::default();
        let count = Arc::new(AtomicUsize::new(0));
        let calls = Arc::clone(&count);
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        *state.before_read.lock().unwrap() = Some(Arc::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            let _ = entered_tx.send(());
            let _ = release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(2));
        }));
        let started = Instant::now();
        assert_eq!(state.configured(&root, &config()), None);
        assert!(started.elapsed() < Duration::from_millis(250));
        entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(state.configured(&root, &config()), None);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        // The already-started probe sees the old file. Its completed answer is
        // ineligible for a later caller, which must observe the removed file.
        let _ = release_tx.send(());
        let deadline = Instant::now() + Duration::from_secs(2);
        while !state
            .current
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .worker
            .is_finished()
        {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        std::fs::remove_file(root.join(crate::remote::CONFIG_FILE)).unwrap();
        *state.before_read.lock().unwrap() = None;
        assert_eq!(state.configured(&root, &config()), Some(false));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_configuration_does_not_start_discovery_and_busy_mutex_declines() {
        let state = State::default();
        let mut config = config();
        config.store = "/sail-nonexistent-explicit-store".into();
        let held = state.current.lock().unwrap();
        assert_eq!(
            state.configured(Path::new("/not/read"), &config),
            Some(true)
        );
        config.store.clear();
        assert_eq!(state.configured(Path::new("/not/read"), &config), None);
        drop(held);
        assert!(state.current.lock().unwrap().is_none());
    }
}
