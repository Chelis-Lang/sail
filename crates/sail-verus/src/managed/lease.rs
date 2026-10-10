//! Optional Linux kernel read leases for expensive runtime digests. A separate
//! trusted Python 3 process owns descriptors and handles SIGIO without unsafe
//! Rust. Unsupported kernels/filesystems, conflicting mappings and helper loss
//! disable reuse. Source/build/config contents are always hashed separately.

use super::precise::Observer;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub(super) struct ReadLease {
    process: Child,
    socket: UnixStream,
    bindings: Vec<(PathBuf, PathBuf, u64, u64)>,
    mounts: String,
    interpreter: PathBuf,
    helper_observer: Observer,
    failure: Option<&'static str>,
}
impl ReadLease {
    #[cfg(test)]
    pub(super) fn stop_owner_for_test(&mut self) -> std::io::Result<()> {
        self.process.kill()?;
        self.process.wait().map(|_| ())
    }
    pub(super) fn acquire(paths: &[PathBuf]) -> Option<Self> {
        if paths.len() > 4096 {
            return None;
        }
        let interpreter = interpreter()?;
        let closure = helper_paths(&interpreter)?;
        let helper_observer = Observer::begin(&closure, &[])?;
        let baseline = helper_digests(&closure)?;
        let mut paths = paths.to_vec();
        paths.extend(closure.iter().filter(|p| p.is_file()).cloned());
        paths.sort();
        paths.dedup();
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
        // JSON PathBuf serialization panics inside json! on non-UTF-8 names.
        // The private Linux helper opens exact native bytes, with no lossy path
        // conversion. Its imported-runtime report remains fallible UTF-8 JSON.
        let identities: Vec<_> = bindings
            .iter()
            .map(|(_, p, d, i)| (p.as_os_str().as_encoded_bytes(), d, i))
            .collect();
        let runtime: Vec<_> = closure.iter().map(|p| p.to_str()).collect::<Option<_>>()?;
        let mut bytes = serde_json::to_vec(&serde_json::json!({
            "files": identities,
            "runtime": runtime,
        }))
        .ok()?;
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
        let helper = format!(
            "{}\n{}",
            include_str!("helper_runtime.py"),
            include_str!("lease.py")
        );
        let process = Command::new(&interpreter)
            .args(["-I", "-u", "-c", &helper])
            .stdin(Stdio::from(child_input))
            .stdout(Stdio::from(child_output))
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut lease = Self {
            process,
            socket: parent_socket,
            bindings,
            mounts,
            interpreter,
            helper_observer,
            failure: None,
        };
        socket.write_all(&bytes).ok()?;
        let mut reply = [0; 3];
        socket.read_exact(&mut reply).ok()?;
        if reply != *b"ok\n"
            || !lease.bindings_current()
            || helper_digests(&closure)? != baseline
            || !lease.helper_observer.clean()
        {
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
        // A timed-out request can leave an old acknowledgement buffered. Never
        // let a later check certify a previously invalidated owner generation.
        if self.failure.is_some() {
            return false;
        }
        if !self.bindings_current() {
            self.failure.get_or_insert("lease-binding-invalidated");
            return false;
        }
        if interpreter().as_ref() != Some(&self.interpreter) {
            self.failure.get_or_insert("helper-interpreter-changed");
            return false;
        }
        if !self.helper_observer.clean() {
            self.failure.get_or_insert("helper-observation-invalidated");
            return false;
        }
        let mut reply = [0; 3];
        if self.socket.write_all(b"check\n").is_err() {
            self.failure.get_or_insert("lease-owner-unavailable");
            return false;
        }
        if let Err(error) = self.socket.read_exact(&mut reply) {
            let reason = if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) {
                "lease-check-read-timeout"
            } else {
                "lease-owner-unavailable"
            };
            self.failure.get_or_insert(reason);
            return false;
        }
        if reply != *b"ok\n" {
            self.failure.get_or_insert("lease-protocol-unavailable");
            return false;
        }
        true
    }
    #[cfg(test)]
    pub(super) fn stop_owner_for_test(&mut self) -> std::io::Result<()> {
        self.process.kill()?;
        self.process.wait().map(|_| ())
    }
    pub(super) fn failure_reason(&self) -> &'static str {
        self.failure
            .unwrap_or("kernel-lease-validation-unavailable")
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
// PATH is selected once for launch; subsequent barriers also check its exact
// resolution. No interpreter substitution is hidden by an unchanged PATH value.
fn interpreter() -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join("python3"))
        .find(|path| path.is_file())
        .and_then(|path| std::path::absolute(path).ok())
}
fn helper_digests(paths: &[PathBuf]) -> Option<Vec<Option<String>>> {
    paths
        .iter()
        .map(|path| {
            if path.is_file() {
                super::files::file_digest(path).ok().map(Some)
            } else if !path.exists() {
                Some(None)
            } else {
                None
            }
        })
        .collect()
}
pub(super) fn helper_identity() -> Option<Vec<(String, String)>> {
    let paths = helper_paths(&interpreter()?)?;
    paths
        .into_iter()
        .filter(|p| p.is_file())
        .map(|path| {
            Some((
                path.to_str()?.to_owned(),
                super::files::file_digest(&path).ok()?,
            ))
        })
        .collect()
}
fn helper_paths(interpreter: &Path) -> Option<Vec<PathBuf>> {
    // This cold-only process is bounded independently of any job RPC. The
    // installed interpreter/loader bootstrap is the explicit trusted OS boundary.
    let (mut socket, child_socket) = UnixStream::pair().ok()?;
    socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let output: OwnedFd = child_socket.into();
    let probe = format!(
        "{}\nprint(json.dumps(support_paths()), flush=True)",
        include_str!("helper_runtime.py")
    );
    let mut child = Command::new(interpreter)
        .args(["-I", "-u", "-c", &probe])
        .stdin(Stdio::null())
        .stdout(Stdio::from(output))
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut bytes = Vec::new();
    let read = std::io::Read::by_ref(&mut socket)
        .take(1_048_577)
        .read_to_end(&mut bytes);
    let _ = child.kill();
    let _ = child.wait();
    if read.is_err() || bytes.len() > 1_048_576 {
        return None;
    }
    let mut paths: Vec<PathBuf> = serde_json::from_slice(&bytes).ok()?;
    if paths.len() > 1024 || paths.iter().any(|p| !p.is_absolute()) {
        return None;
    }
    paths.push(interpreter.to_owned());
    paths.sort();
    paths.dedup();
    Some(paths)
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
    fn replacing_selected_interpreter_under_unchanged_path_invalidates_projection() {
        let root = fixture().with_extension("python-path");
        assert!(std::fs::create_dir(&root).is_ok());
        let actual = interpreter().unwrap_or_else(|| unreachable!("Python fixture"));
        assert!(std::os::unix::fs::symlink(actual, root.join("python3")).is_ok());
        let mut paths = vec![root.clone()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let path = std::env::join_paths(paths).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let output = Command::new(std::env::current_exe().unwrap_or_default())
            .args([
                "--exact",
                "managed::lease::tests::selected_interpreter_child",
                "--nocapture",
            ])
            .env("PATH", path)
            .env("SAIL_LEASE_INTERPRETER_CHILD", "1")
            .output()
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
    #[test]
    fn selected_interpreter_child() {
        // The outer test changes only its child's environment: Rust 2024 forbids
        // mutating this test runner's shared process environment without unsafe.
        if std::env::var_os("SAIL_LEASE_INTERPRETER_CHILD").is_none() {
            return;
        }
        let file = fixture();
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(lease.clean());
        let selected = interpreter().unwrap_or_default();
        let replacement = selected.with_extension("new");
        assert!(std::os::unix::fs::symlink("/bin/true", &replacement).is_ok());
        assert!(std::fs::rename(replacement, selected).is_ok());
        assert!(!lease.clean());
        assert!(std::fs::remove_file(file).is_ok());
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
    fn delayed_owner(
        lease: &mut ReadLease,
        file: &Path,
    ) -> std::io::BufReader<std::process::ChildStderr> {
        let (socket, child_socket) =
            UnixStream::pair().unwrap_or_else(|e| unreachable!("fixture:{e}"));
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        socket
            .set_write_timeout(Some(Duration::from_millis(50)))
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let input: OwnedFd = child_socket
            .try_clone()
            .unwrap_or_else(|e| unreachable!("fixture:{e}"))
            .into();
        let output: OwnedFd = child_socket.into();
        // Widen the legal interval between closing lease descriptors on SIGIO
        // and process exit. A delayed first acknowledgement remains queued;
        // the replacement owner never acknowledges the second request.
        let helper = r"import fcntl,os,signal,sys,time
fd=os.open(sys.argv[1],os.O_RDONLY)
def released(_signal,_frame):
 os.close(fd)
 print('released',file=sys.stderr,flush=True)
signal.signal(signal.SIGIO,released)
fcntl.fcntl(fd,fcntl.F_SETLEASE,fcntl.F_RDLCK)
print('ready',file=sys.stderr,flush=True)
assert sys.stdin.buffer.readline()==b'check\n'
time.sleep(0.15)
sys.stdout.buffer.write(b'ok\n');sys.stdout.buffer.flush()
print('buffered',file=sys.stderr,flush=True)
sys.stdin.buffer.readline()
time.sleep(10)
";
        lease.process = Command::new("python3")
            .args(["-I", "-u", "-c", helper])
            .arg(file)
            .stdin(Stdio::from(input))
            .stdout(Stdio::from(output))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        lease.socket = socket;
        std::io::BufReader::new(
            lease
                .process
                .stderr
                .take()
                .unwrap_or_else(|| unreachable!("fixture stderr")),
        )
    }
    #[test]
    fn delayed_acknowledgement_after_mapping_break_cannot_recertify_owner() {
        use std::io::BufRead as _;
        let file = fixture();
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(lease.process.kill().is_ok());
        assert!(lease.process.wait().is_ok());
        let mut events = delayed_owner(&mut lease, &file);
        let mut line = String::new();
        assert!(events.read_line(&mut line).is_ok());
        assert_eq!(line.trim(), "ready");
        assert!(!lease.clean());
        assert_eq!(lease.failure_reason(), "lease-check-read-timeout");
        line.clear();
        assert!(events.read_line(&mut line).is_ok());
        assert_eq!(line.trim(), "buffered");
        let mut mapping = Command::new("python3").args(["-I","-u","-c",
            "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); m[:]=b'fail'; m.flush(); m[:]=b'good'; m.flush(); print('mapped',flush=True); sys.stdin.readline(); m.close()"])
            .arg(&file).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let mut ready = [0; 7];
        assert!(
            mapping
                .stdout
                .as_mut()
                .is_some_and(|o| o.read_exact(&mut ready).is_ok())
        );
        assert_eq!(&ready, b"mapped\n");
        line.clear();
        assert!(events.read_line(&mut line).is_ok());
        assert_eq!(line.trim(), "released");
        assert!(ReadLease::acquire(std::slice::from_ref(&file)).is_none());
        assert!(
            !lease.clean(),
            "late acknowledgement cannot certify a broken owner"
        );
        assert_eq!(lease.failure_reason(), "lease-check-read-timeout");
        assert!(
            mapping
                .stdin
                .as_mut()
                .is_some_and(|i| i.write_all(b"done\n").is_ok())
        );
        assert!(mapping.wait().is_ok_and(|s| s.success()));
        drop(lease);
        assert!(std::fs::remove_file(file).is_ok());
    }
    #[test]
    fn paused_owner_reports_bounded_timeout_without_weakening_validation() {
        let file = fixture();
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(lease.clean());
        assert!(
            Command::new("kill")
                .args(["-s", "STOP", "--", &lease.process.id().to_string()])
                .status()
                .is_ok_and(|s| s.success())
        );
        assert!(!lease.clean());
        assert_eq!(lease.failure_reason(), "lease-check-read-timeout");
        assert!(lease.process.kill().is_ok());
        assert!(lease.process.wait().is_ok());
        drop(lease);
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
        assert_eq!(lease.failure_reason(), "lease-owner-unavailable");
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        let replacement = file.with_extension("new");
        assert!(std::fs::write(&replacement, b"fail").is_ok());
        assert!(std::fs::rename(replacement, &file).is_ok());
        assert!(!lease.clean());
        assert_eq!(lease.failure_reason(), "lease-binding-invalidated");
        assert!(std::fs::remove_file(file).is_ok());
    }
}
