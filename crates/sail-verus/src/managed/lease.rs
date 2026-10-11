//! Optional Linux kernel read leases for expensive runtime digests. A separate
//! trusted Python 3 process owns descriptors and handles SIGIO without unsafe
//! Rust. Unsupported kernels/filesystems, conflicting mappings and helper loss
//! disable reuse. Input digests are collected only after these guards are held.

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
    kernel: Option<super::kernel_leases::Inventory>,
    bindings: Vec<(PathBuf, PathBuf, u64, u64)>,
    mounts: super::kernel_mounts::Guard,
    interpreter: PathBuf,
    helper_observer: Observer,
    failure: Option<&'static str>,
}
impl ReadLease {
    /// Requested Rust storage; native descriptors and helper RSS are separate.
    #[allow(
        dead_code,
        reason = "Used by forthcoming bounded daemon sysroot admission"
    )]
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let Self {
            process: _,
            socket: _,
            kernel,
            bindings,
            mounts,
            interpreter,
            helper_observer,
            failure: _,
        } = self;
        let used = std::mem::size_of::<Self>()
            .checked_add(super::retained::vector(bindings)?)?
            .checked_add(mounts.capacity())?
            .checked_add(interpreter.capacity())?
            .checked_add(helper_observer.retained_bytes()?)?
            .checked_add(
                kernel
                    .as_ref()
                    .map_or(Some(0), super::kernel_leases::Inventory::retained_bytes)?,
            )?;
        bindings
            .iter()
            .try_fold(used, |used, (path, canonical, _, _)| {
                used.checked_add(path.capacity())?
                    .checked_add(canonical.capacity())
            })
    }

    #[cfg(test)]
    pub(super) fn stop_owner_for_test(&mut self) -> std::io::Result<()> {
        self.process.kill()?;
        self.process.wait().map(|_| ())
    }
    pub(super) fn acquire(paths: &[PathBuf]) -> Option<Self> {
        let mut stage = "input-file-limit";
        #[cfg(test)]
        let started = std::time::Instant::now();
        let lease = Self::acquire_at_stage(paths, &mut stage);
        #[cfg(test)]
        if lease.is_none() {
            eprintln!(
                "lease acquisition unavailable: stage={stage} elapsed={:?}",
                started.elapsed()
            );
        }
        lease
    }
    fn acquire_at_stage(paths: &[PathBuf], stage: &mut &'static str) -> Option<Self> {
        if paths.len() > 4096 {
            return None;
        }
        *stage = "interpreter-resolution";
        let interpreter = interpreter()?;
        *stage = "helper-runtime-closure";
        let closure = helper_paths(&interpreter)?;
        *stage = "helper-native-observer";
        let helper_observer = Observer::begin(&closure, &[])?;
        *stage = "helper-baseline-digests";
        let baseline = helper_digests(&closure)?;
        let mut paths = paths.to_vec();
        paths.extend(closure.iter().filter(|p| p.is_file()).cloned());
        paths.sort();
        paths.dedup();
        *stage = "combined-file-limit";
        if paths.len() > 4096 {
            return None;
        }
        *stage = "mount-inventory";
        let mounts = super::kernel_mounts::Guard::begin()?;
        *stage = "input-path-bindings";
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
        *stage = "helper-path-encoding";
        let runtime: Vec<_> = closure.iter().map(|p| p.to_str()).collect::<Option<_>>()?;
        *stage = "helper-request-encoding";
        let mut bytes = serde_json::to_vec(&serde_json::json!({
            "files": identities,
            "runtime": runtime,
        }))
        .ok()?;
        bytes.push(b'\n');
        if bytes.len() > 1_048_576 {
            return None;
        }
        *stage = "helper-socket-setup";
        let (mut socket, child_socket) = UnixStream::pair().ok()?;
        socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .ok()?;
        let child_input: OwnedFd = child_socket.try_clone().ok()?.into();
        let child_output: OwnedFd = child_socket.into();
        let parent_socket = socket.try_clone().ok()?;
        *stage = "helper-process-spawn";
        let process = spawn_helper(&interpreter, child_input, child_output)?;
        let mut lease = Self {
            process,
            socket: parent_socket,
            kernel: None,
            bindings,
            mounts,
            interpreter,
            helper_observer,
            failure: None,
        };
        *stage = "helper-request-write";
        socket.write_all(&bytes).ok()?;
        *stage = "helper-readiness-frame";
        #[cfg(test)]
        let fds = lease.readiness(&mut socket)?;
        #[cfg(not(test))]
        let fds = descriptors(&mut socket)?;
        let identities: Vec<_> = lease.bindings.iter().map(|(_, _, d, i)| (*d, *i)).collect();
        *stage = "kernel-lease-inventory-bind";
        lease.kernel = super::kernel_leases::Inventory::bind(lease.process.id(), &fds, &identities);
        let validation = lease.validate_initial(&closure, &baseline, stage)?;
        if let Err(reason) = validation {
            *stage = reason;
            #[cfg(test)]
            eprintln!("lease acquisition failed: {reason}");
            return None;
        }
        Some(lease)
    }
    #[cfg(test)]
    fn readiness(&mut self, socket: &mut UnixStream) -> Option<Vec<u32>> {
        let fds = descriptors(socket);
        #[cfg(test)]
        if fds.is_none() {
            eprintln!(
                "lease readiness unavailable: child_status={:?}",
                self.process.try_wait()
            );
        }
        fds
    }
    fn validate_initial(
        &mut self,
        closure: &[PathBuf],
        baseline: &[Option<String>],
        stage: &mut &'static str,
    ) -> Option<std::result::Result<(), &'static str>> {
        let validation = if self.kernel.is_none() {
            Err("kernel-lease-inventory-binding-unavailable")
        } else if !self.bindings_current() {
            Err("lease-binding-invalidated")
        } else {
            *stage = "helper-final-digests";
            if helper_digests(closure)? != baseline {
                Err("helper-byte-identity-invalidated")
            } else if !self.helper_observer.clean() {
                Err("helper-observation-invalidated")
            } else {
                self.kernel
                    .as_ref()
                    .ok_or("kernel-lease-state-unavailable")
                    .and_then(super::kernel_leases::Inventory::check)
            }
        };
        Some(validation)
    }
    pub(super) fn clean(&mut self) -> bool {
        // No acknowledgement or later successful sweep can recertify an owner
        // generation that has already failed a kernel or namespace barrier.
        if self.failure.is_some() {
            return false;
        }
        #[cfg(test)]
        let started = std::time::Instant::now();
        let bindings_current = self.bindings_current();
        #[cfg(test)]
        trace_clean("bindings", started);
        if !bindings_current {
            self.failure.get_or_insert("lease-binding-invalidated");
            #[cfg(test)]
            eprintln!("lease barrier failed: lease-binding-invalidated");
            return false;
        }
        let interpreter_current = interpreter().as_ref() == Some(&self.interpreter);
        #[cfg(test)]
        trace_clean("interpreter", started);
        if !interpreter_current {
            self.failure.get_or_insert("helper-interpreter-changed");
            return false;
        }
        let helper_clean = self.helper_observer.clean();
        #[cfg(test)]
        trace_clean("helper-native", started);
        if !helper_clean {
            self.failure.get_or_insert("helper-observation-invalidated");
            #[cfg(test)]
            eprintln!("lease barrier failed: helper-observation-invalidated");
            return false;
        }
        let kernel = self
            .kernel
            .as_ref()
            .ok_or("kernel-lease-state-unavailable")
            .and_then(super::kernel_leases::Inventory::check);
        #[cfg(test)]
        trace_clean("kernel-inventory", started);
        if let Err(reason) = kernel {
            self.failure.get_or_insert(reason);
            #[cfg(test)]
            eprintln!("lease barrier failed: {reason}");
            return false;
        }
        true
    }
    pub(super) fn failure_reason(&self) -> &'static str {
        self.failure
            .unwrap_or("kernel-lease-validation-unavailable")
    }
    fn bindings_current(&mut self) -> bool {
        self.mounts.clean()
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
#[cfg(test)]
fn trace_clean(stage: &'static str, started: std::time::Instant) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("SAIL_FRESHNESS_TRACE").is_some_and(|v| v == "1")) {
        eprintln!(
            "sail-lease thread={:?} elapsed_us={} stage={stage}",
            std::thread::current().id(),
            started.elapsed().as_micros()
        );
    }
}
fn spawn_helper(interpreter: &Path, input: OwnedFd, output: OwnedFd) -> Option<Child> {
    let helper = format!(
        "{}\n{}",
        include_str!("helper_runtime.py"),
        include_str!("lease.py")
    );
    Command::new(interpreter)
        .args(["-I", "-u", "-c", &helper])
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output))
        .stderr(Stdio::null())
        .spawn()
        .ok()
}
fn descriptors(socket: &mut UnixStream) -> Option<Vec<u32>> {
    let mut reply = Vec::new();
    let read = std::io::BufRead::read_until(
        &mut std::io::BufReader::new(std::io::Read::by_ref(socket).take(65_537)),
        b'\n',
        &mut reply,
    );
    #[cfg(test)]
    if let Err(error) = &read {
        eprintln!(
            "lease readiness read failed: kind={:?} errno={:?} bytes={}",
            error.kind(),
            error.raw_os_error(),
            reply.len()
        );
    }
    read.ok()?;
    if reply.len() > 65_536 || !reply.ends_with(b"\n") {
        #[cfg(test)]
        eprintln!(
            "lease readiness frame invalid: bytes={} terminated={}",
            reply.len(),
            reply.ends_with(b"\n")
        );
        return None;
    }
    let decoded = serde_json::from_slice(&reply);
    #[cfg(test)]
    if decoded.is_err() {
        eprintln!("lease readiness inventory invalid: bytes={}", reply.len());
    }
    decoded.ok()
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
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
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
    #[test]
    fn sleeping_owner_with_kernel_confirmed_leases_needs_no_scheduled_acknowledgement() {
        let file = fixture();
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        // The ready helper waits for EOF and deliberately implements no check
        // acknowledgement. These barriers must use its actual kernel leases.
        for _ in 0..5 {
            assert!(lease.clean());
        }
        assert!(std::fs::write(&file, b"fail").is_ok());
        assert!(!lease.clean());
        assert!(!lease.clean());
        drop(lease);
        assert!(std::fs::remove_file(file).is_ok());
    }
    #[test]
    fn paused_reader_keeps_unbroken_generation_but_rejects_writer_and_mapping_breaks() {
        for change in [0, 1, 2] {
            let file = fixture();
            let alias = file.with_extension("reader-pause");
            assert!(std::fs::hard_link(&file, &alias).is_ok());
            let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
                .unwrap_or_else(|| unreachable!("fixture"));
            let checked = lease
                .kernel
                .as_ref()
                .unwrap_or_else(|| unreachable!("kernel fixture"))
                .check_after_pause_for_test(|| {
                    // The kernel rows have already been read. Retain them
                    // across a pause longer than the removed timing heuristic.
                    std::thread::sleep(Duration::from_millis(100));
                    if change == 1 {
                        assert!(std::fs::write(&alias, b"fail").is_ok());
                    } else if change == 2 {
                        assert!(Command::new("python3").args(["-I", "-c",
                            "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); original=m[:]; m[:]=b'fail'; m.flush(); m[:]=original; m.flush(); m.close()"])
                            .arg(&alias).status().is_ok_and(|s|s.success()));
                        assert_eq!(std::fs::read(&file).unwrap_or_default(), b"good");
                    }
                });
            assert_eq!(checked.is_ok(), change == 0, "change={change}");
            assert_eq!(lease.clean(), change == 0, "change={change}");
            drop(lease);
            assert!(std::fs::remove_file(alias).is_ok());
            assert!(std::fs::remove_file(file).is_ok());
        }
    }
    #[test]
    fn stale_acknowledgement_cannot_certify_a_broken_writable_mapping() {
        let file = fixture();
        let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(lease.clean());
        let (socket, mut stale) =
            UnixStream::pair().unwrap_or_else(|e| unreachable!("fixture:{e}"));
        // Preserve the old late-ack attack: bytes remain available, but the
        // actual owner generation loses its leases through a writable mapping.
        assert!(stale.write_all(b"ok\n").is_ok());
        let original = std::mem::replace(&mut lease.socket, socket);
        let mut mapping = Command::new("python3").args(["-I", "-u", "-c",
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
        assert!(!lease.clean());
        assert!(ReadLease::acquire(std::slice::from_ref(&file)).is_none());
        assert!(
            mapping
                .stdin
                .as_mut()
                .is_some_and(|i| i.write_all(b"done\n").is_ok())
        );
        assert!(mapping.wait().is_ok_and(|s| s.success()));
        assert!(
            !lease.clean(),
            "buffered acknowledgement cannot revive the owner"
        );
        drop(original);
        drop(lease);
        assert!(std::fs::remove_file(file).is_ok());
    }
    #[test]
    fn malformed_and_truncated_readiness_cannot_authorize_inventory() {
        for bytes in [
            &b"ok\n"[..],
            &b"[3,\"wrong\"]\n"[..],
            &b"[3]"[..],
            &b"[-1]\n"[..],
        ] {
            let (mut socket, mut owner) =
                UnixStream::pair().unwrap_or_else(|e| unreachable!("fixture:{e}"));
            assert!(owner.write_all(bytes).is_ok());
            drop(owner);
            assert!(descriptors(&mut socket).is_none());
        }
        let (mut socket, mut owner) =
            UnixStream::pair().unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(owner.write_all(&vec![b' '; 65_537]).is_ok());
        drop(owner);
        assert!(descriptors(&mut socket).is_none());
    }
    #[test]
    fn readiness_inventory_cannot_bind_duplicate_or_wrong_descriptors() {
        let file = fixture();
        let lease = ReadLease::acquire(std::slice::from_ref(&file))
            .unwrap_or_else(|| unreachable!("fixture"));
        let pid = lease.process.id();
        let metadata = std::fs::metadata(&file).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let identity = (metadata.dev(), metadata.ino());
        assert!(
            super::super::kernel_leases::Inventory::bind(pid, &[0, 0], &[identity, identity])
                .is_none()
        );
        assert!(super::super::kernel_leases::Inventory::bind(pid, &[0], &[identity]).is_none());
        assert!(super::super::kernel_leases::Inventory::bind(pid, &[], &[identity]).is_none());
        drop(lease);
        assert!(std::fs::remove_file(file).is_ok());
    }
    #[test]
    fn distinct_hardlink_descriptors_are_counted_without_missing_any_lease() {
        let file = fixture();
        let alias = file.with_extension("lease-alias");
        assert!(std::fs::hard_link(&file, &alias).is_ok());
        let mut lease = ReadLease::acquire(&[file.clone(), alias.clone()])
            .unwrap_or_else(|| unreachable!("fixture"));
        assert!(lease.clean());
        assert!(std::fs::write(&alias, b"fail").is_ok());
        assert!(!lease.clean());
        drop(lease);
        assert!(std::fs::remove_file(alias).is_ok());
        assert!(std::fs::remove_file(file).is_ok());
    }
    #[test]
    fn paused_owner_and_lease_break_remain_sticky_without_scheduling_dependency() {
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
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline
            && !std::fs::read_to_string(format!("/proc/{}/stat", lease.process.id())).is_ok_and(
                |text| {
                    text.rsplit_once(')')
                        .is_some_and(|(_, fields)| fields.trim_start().starts_with('T'))
                },
            )
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            std::fs::read_to_string(format!("/proc/{}/stat", lease.process.id())).is_ok_and(
                |text| text
                    .rsplit_once(')')
                    .is_some_and(|(_, fields)| fields.trim_start().starts_with('T'))
            )
        );
        assert!(!lease.clean());
        assert_eq!(lease.failure_reason(), "kernel-lease-owner-inactive");
        assert!(
            Command::new("kill")
                .args(["-s", "CONT", "--", &lease.process.id().to_string()])
                .status()
                .is_ok_and(|s| s.success())
        );
        assert!(
            !lease.clean(),
            "a resumed owner cannot revive an invalidated generation"
        );
        assert!(lease.process.kill().is_ok());
        assert!(lease.process.wait().is_ok());
        drop(lease);
        assert!(std::fs::remove_file(file).is_ok());
    }
    #[test]
    fn omitted_global_rows_require_live_exact_descriptor_leases_before_failure() {
        for mapping in [false, true] {
            let file = fixture();
            let mut lease = ReadLease::acquire(std::slice::from_ref(&file))
                .unwrap_or_else(|| unreachable!("fixture"));
            assert!(lease.clean());
            assert!(
                lease
                    .kernel
                    .as_ref()
                    .is_some_and(|kernel| kernel.check_omitted_global_for_test().is_ok())
            );
            if mapping {
                assert!(Command::new("python3").args(["-I", "-c",
                    "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); original=m[:]; m[:]=b'fail'; m.flush(); m[:]=original; m.flush(); m.close()"])
                    .arg(&file).status().is_ok_and(|s|s.success()));
                assert_eq!(std::fs::read(&file).unwrap_or_default(), b"good");
            } else {
                assert!(std::fs::write(&file, b"fail").is_ok());
            }
            assert!(
                lease
                    .kernel
                    .as_ref()
                    .is_some_and(|kernel| kernel.check_omitted_global_for_test().is_err())
            );
            assert!(
                !lease.clean(),
                "independent kernel validation cannot revive a break"
            );
            drop(lease);
            assert!(std::fs::remove_file(file).is_ok());
        }
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
        assert_eq!(lease.failure_reason(), "kernel-lease-owner-inactive");
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
