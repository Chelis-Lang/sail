//! Coherent proof-time inputs. Linux leases exclude existing writable mappings
//! and new writes through any inode alias; native queues protect names and
//! absent paths throughout the establishing invocation.

use super::InputSnapshot;
#[cfg(target_os = "linux")]
use super::{lease::ReadLease, precise::Observer};

#[derive(Clone, Copy)]
pub(super) enum Barrier {
    Input,
    PostVerification,
}
impl Barrier {
    fn label(self) -> &'static str {
        match self {
            Self::Input => "input-barrier",
            Self::PostVerification => "post-verification",
        }
    }
}

pub(super) struct Guard {
    #[cfg(target_os = "linux")]
    observer: Option<Observer>,
    #[cfg(target_os = "linux")]
    lease: Option<ReadLease>,
    failure: Option<&'static str>,
    failed_barrier: Option<Barrier>,
}
impl Guard {
    pub(super) fn begin(snapshot: &InputSnapshot) -> Self {
        #[cfg(target_os = "linux")]
        {
            let observer = Observer::begin(&snapshot.roots, &snapshot.excluded);
            let lease = observer
                .as_ref()
                .and_then(|o| ReadLease::acquire(&o.files()));
            let failure = if observer.is_none() {
                Some("native-observer-setup-unavailable")
            } else if lease.is_none() {
                Some("kernel-lease-acquisition-unavailable")
            } else {
                None
            };
            Self {
                observer,
                lease,
                failure,
                failed_barrier: None,
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = snapshot;
            Self {
                failure: None,
                failed_barrier: None,
            }
        }
    }
    pub(super) fn ready_at(&mut self, barrier: Barrier) -> bool {
        let ready = self.ready();
        if !ready {
            self.failed_barrier.get_or_insert(barrier);
        }
        ready
    }
    pub(super) fn ready(&mut self) -> bool {
        if self.failure.is_some() {
            return false;
        }
        #[cfg(target_os = "linux")]
        {
            if !self.observer.as_mut().is_some_and(Observer::clean) {
                self.failure.get_or_insert("native-observation-invalidated");
                return false;
            }
            let Some(lease) = self.lease.as_mut() else {
                self.failure
                    .get_or_insert("kernel-lease-acquisition-unavailable");
                return false;
            };
            if !lease.clean() {
                self.failure.get_or_insert(lease.failure_reason());
                return false;
            }
            true
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = self;
            true
        }
    }
    pub(super) fn failure_reason(&self) -> &'static str {
        self.failure.unwrap_or("coherent-validation-unavailable")
    }
    fn failure_detail(&self) -> Option<&'static str> {
        #[cfg(target_os = "linux")]
        if self.failure == Some("native-observation-invalidated") {
            return self.observer.as_ref().map(Observer::failure_reason);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = self;
        None
    }
    // Only already-recorded static labels: no extra poll, file read or barrier.
    pub(super) fn failure_description(&self) -> String {
        let reason = self.failure_reason();
        match (self.failed_barrier, self.failure_detail()) {
            (Some(barrier), Some(detail)) => {
                format!("{reason}; barrier={}; detail={detail}", barrier.label())
            }
            (Some(barrier), None) => format!("{reason}; barrier={}", barrier.label()),
            (None, Some(detail)) => format!("{reason}; detail={detail}"),
            (None, None) => reason.to_owned(),
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::io::{Read as _, Write as _};
    use std::process::{Command, Stdio};
    fn snapshot(root: &std::path::Path) -> InputSnapshot {
        InputSnapshot {
            workspace: root.to_owned(),
            revision: String::new(),
            roots: vec![root.to_owned(), root.join(".cargo/config.toml")],
            excluded: Vec::new(),
            complete: true,
            observation_paths: BTreeSet::new(),
            content_paths: BTreeSet::new(),
        }
    }
    #[test]
    fn preexisting_mapping_and_transient_writes_cannot_establish_current_proof() {
        let root = std::env::temp_dir().join(format!("sail-proof-lease-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let file = root.join("lib.rs");
        assert!(std::fs::write(&file, b"good").is_ok());
        let mut writer=Command::new("python3").args(["-I","-u","-c",
            "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); print('ready',flush=True); sys.stdin.readline(); m[:]=b'fail'; m.flush(); m[:]=b'good'; m.flush(); m.close()"])
            .arg(&file).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap_or_else(|e|unreachable!("fixture:{e}"));
        let mut ready = [0; 6];
        assert!(
            writer
                .stdout
                .as_mut()
                .is_some_and(|o| o.read_exact(&mut ready).is_ok())
        );
        let mut guard = Guard::begin(&snapshot(&root));
        assert!(!guard.ready());
        assert_eq!(
            guard.failure_reason(),
            "kernel-lease-acquisition-unavailable"
        );
        assert!(
            writer
                .stdin
                .as_mut()
                .is_some_and(|i| i.write_all(b"run\n").is_ok())
        );
        assert!(writer.wait().is_ok_and(|s| s.success()));
        let mut guard = Guard::begin(&snapshot(&root));
        assert!(guard.ready());
        assert!(std::fs::write(&file, b"fail").is_ok());
        assert!(std::fs::write(&file, b"good").is_ok());
        assert!(!guard.ready());
        assert_eq!(guard.failure_reason(), "native-observation-invalidated");
        assert_eq!(
            guard.failure_detail(),
            Some("namespace-tracked-input-event")
        );
        assert!(!guard.ready());
        assert_eq!(guard.failure_reason(), "native-observation-invalidated");
        drop(guard);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[test]
    fn recorded_failure_detail_and_first_barrier_do_not_revalidate_or_revive() {
        let root = std::env::temp_dir().join(format!("sail-proof-detail-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let file = root.join("lib.rs");
        assert!(std::fs::write(&file, b"good").is_ok());
        let mut guard = Guard::begin(&snapshot(&root));
        assert!(guard.ready_at(Barrier::Input));
        assert!(std::fs::write(&file, b"fail").is_ok());
        assert!(std::fs::write(&file, b"good").is_ok());
        assert!(!guard.ready_at(Barrier::PostVerification));
        let description = "native-observation-invalidated; barrier=post-verification; detail=namespace-tracked-input-event";
        assert_eq!(guard.failure_description(), description);
        assert!(!guard.ready_at(Barrier::Input));
        assert_eq!(guard.failure_description(), description);
        drop(guard);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
