//! Coherent proof-time inputs. Linux leases exclude existing writable mappings
//! and new writes through any inode alias; native queues protect names and
//! absent paths throughout the establishing invocation.

use super::InputSnapshot;
#[cfg(target_os = "linux")]
use super::{lease::ReadLease, precise::Observer};

pub(super) struct Guard {
    #[cfg(target_os = "linux")]
    observer: Option<Observer>,
    #[cfg(target_os = "linux")]
    lease: Option<ReadLease>,
}
impl Guard {
    pub(super) fn begin(snapshot: &InputSnapshot) -> Self {
        #[cfg(target_os = "linux")]
        {
            let observer = Observer::begin(&snapshot.roots, &snapshot.excluded);
            let lease = observer
                .as_ref()
                .and_then(|o| ReadLease::acquire(&o.files()));
            Self { observer, lease }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = snapshot;
            Self {}
        }
    }
    pub(super) fn ready(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.observer.as_mut().is_some_and(Observer::clean)
                && self.lease.as_mut().is_some_and(ReadLease::clean)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = self;
            true
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
        drop(guard);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
