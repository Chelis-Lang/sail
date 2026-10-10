//! Optional Linux kernel read leases for expensive runtime digests. A separate
//! trusted Python 3 process owns descriptors and handles SIGIO without unsafe
//! Rust. Unsupported kernels/filesystems, conflicting mappings and helper loss
//! disable reuse. Source/build/config contents are always hashed separately.

use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub(super) struct ReadLease {
    process: Child,
    socket: UnixStream,
    bindings: Vec<(PathBuf, PathBuf, u64, u64)>,
    mounts: String,
}
impl ReadLease {
    pub(super) fn acquire(paths: &[PathBuf]) -> Option<Self> {
        if paths.len() > 4096 {
            return None;
        }
        let mounts = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
        let bindings: Vec<_> = paths
            .iter()
            .map(|path| {
                let canonical = path.canonicalize().ok()?;
                let metadata = std::fs::metadata(&canonical).ok()?;
                metadata.is_file().then_some((
                    path.clone(),
                    canonical,
                    metadata.dev(),
                    metadata.ino(),
                ))
            })
            .collect::<Option<_>>()?;
        let identities: Vec<_> = bindings.iter().map(|(_, p, d, i)| (p, d, i)).collect();
        let mut bytes = serde_json::to_vec(&identities).ok()?;
        bytes.push(b'\n');
        if bytes.len() > 1_048_576 {
            return None;
        }
        let (mut socket, child_socket) = UnixStream::pair().ok()?;
        socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .ok()?;
        let child_input: OwnedFd = child_socket.try_clone().ok()?.into();
        let child_output: OwnedFd = child_socket.into();
        let parent_socket = socket.try_clone().ok()?;
        let process = Command::new("python3")
            .args(["-I", "-u", "-c", include_str!("lease.py")])
            .stdin(Stdio::from(child_input))
            .stdout(Stdio::from(child_output))
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let lease = Self {
            process,
            socket: parent_socket,
            bindings,
            mounts,
        };
        socket.write_all(&bytes).ok()?;
        let mut reply = [0; 3];
        socket.read_exact(&mut reply).ok()?;
        if reply != *b"ok\n" || !lease.bindings_current() {
            return None;
        }
        lease
            .socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .ok()?;
        lease
            .socket
            .set_write_timeout(Some(Duration::from_millis(50)))
            .ok()?;
        Some(lease)
    }
    pub(super) fn clean(&mut self) -> bool {
        if !self.bindings_current() {
            return false;
        }
        let mut reply = [0; 3];
        self.socket.write_all(b"check\n").is_ok()
            && self.socket.read_exact(&mut reply).is_ok()
            && reply == *b"ok\n"
    }
    fn bindings_current(&self) -> bool {
        std::fs::read_to_string("/proc/self/mountinfo")
            .as_ref()
            .ok()
            == Some(&self.mounts)
            && self
                .bindings
                .iter()
                .all(|(path, canonical, device, inode)| {
                    path.canonicalize().as_ref().ok() == Some(canonical)
                        && std::fs::metadata(path)
                            .is_ok_and(|m| m.dev() == *device && m.ino() == *inode)
                })
    }
}
impl Drop for ReadLease {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    fn fixture() -> PathBuf {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let file = std::env::temp_dir().join(format!(
            "sail-lease-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(std::fs::write(&file, b"good").is_ok());
        file
    }
    #[test]
    fn writer_through_outside_hardlink_breaks_lease_without_waiting_for_kernel_timeout() {
        let file = fixture();
        let alias = file.with_extension("outside");
        assert!(std::fs::hard_link(&file, &alias).is_ok());
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture supports leases"));
        assert!(lease.clean());
        let time = std::time::Instant::now();
        assert!(std::fs::write(&alias, b"fail").is_ok());
        assert!(time.elapsed() < Duration::from_secs(2));
        assert!(!lease.clean());
        assert!(std::fs::remove_file(alias).is_ok());
        assert!(std::fs::remove_file(file).is_ok());
    }
    #[test]
    fn preexisting_writable_mapping_after_writer_close_prevents_lease() {
        let file = fixture();
        let mut child=Command::new("python3").args(["-I","-u","-c",
            "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); print('ready',flush=True); sys.stdin.readline(); m[:]=b'fail'; m.flush(); m.close()"])
            .arg(&file).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap_or_else(|e|unreachable!("fixture:{e}"));
        let mut ready = [0; 6];
        assert!(
            child
                .stdout
                .as_mut()
                .is_some_and(|o| o.read_exact(&mut ready).is_ok())
        );
        assert_eq!(&ready, b"ready\n");
        assert!(ReadLease::acquire(std::slice::from_ref(&file)).is_none());
        assert!(
            child
                .stdin
                .as_mut()
                .is_some_and(|i| i.write_all(b"write\n").is_ok())
        );
        assert!(child.wait().is_ok_and(|s| s.success()));
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("mapping gone"));
        assert!(lease.clean());
        assert!(std::fs::remove_file(file).is_ok());
    }
    #[test]
    fn helper_death_and_replaced_path_never_reuse() {
        let file = fixture();
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(lease.clean());
        assert!(lease.process.kill().is_ok());
        assert!(lease.process.wait().is_ok());
        assert!(!lease.clean());
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        let replacement = file.with_extension("new");
        assert!(std::fs::write(&replacement, b"fail").is_ok());
        assert!(std::fs::rename(replacement, &file).is_ok());
        assert!(!lease.clean());
        assert!(std::fs::remove_file(file).is_ok());
    }
}
