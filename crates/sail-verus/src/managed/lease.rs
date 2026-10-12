//! Optional Linux kernel read leases for expensive runtime digests. A separate
//! trusted Python 3 process owns descriptors and handles SIGIO without unsafe
//! Rust. Unsupported kernels/filesystems, conflicting mappings and helper loss
//! disable reuse. Input digests are collected only after these guards are held.

use super::precise::Observer;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd as _, OwnedFd};
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
    // Membership alone is not authority. The caller must independently read
    // under this owner and still run its entire fresh clean barrier.
    pub(super) fn covers_exact_path(&self, path: &Path) -> bool {
        self.bindings
            .iter()
            .any(|(bound, _, _, _)| bound.as_os_str() == path.as_os_str())
    }
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
            .checked_add(mounts.retained_bytes()?)?
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
        Self::acquire_unbounded(paths, None, false)
    }
    /// The managed run's own owners: as `acquire`, reusing a closure manifest
    /// probed earlier in the same run when it names the same interpreter, and
    /// overlapping the helper's complete closure reads. The helper still
    /// checks its own loaded closure against that list before and after
    /// acquiring leases, so a stale list refuses ownership.
    pub(super) fn acquire_using(paths: &[PathBuf], known: Option<&HelperClosure>) -> Option<Self> {
        Self::acquire_unbounded(paths, known, true)
    }
    fn acquire_unbounded(
        paths: &[PathBuf],
        known: Option<&HelperClosure>,
        parallel: bool,
    ) -> Option<Self> {
        let mut stage = "input-file-limit";
        #[cfg(test)]
        let started = std::time::Instant::now();
        let lease = Self::acquire_at_stage(paths, &mut stage, None, None, known, parallel);
        #[cfg(test)]
        if lease.is_none() {
            eprintln!(
                "lease acquisition unavailable: stage={stage} elapsed={:?}",
                started.elapsed()
            );
        }
        lease
    }
    #[allow(
        dead_code,
        reason = "Bounded daemon construction is integrated separately"
    )]
    pub(super) fn acquire_bounded(
        paths: &[PathBuf],
        cancelled: &dyn Fn() -> bool,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        started: std::time::Instant,
    ) -> Option<Self> {
        Self::acquire_at_stage(
            paths,
            &mut "input-file-limit",
            Some(cancelled),
            Some((stop, started)),
            None,
            false,
        )
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Keeps the original acquisition stages and optional limits in one fail-closed transaction"
    )]
    fn acquire_at_stage(
        paths: &[PathBuf],
        stage: &mut &'static str,
        cancelled: Option<&dyn Fn() -> bool>,
        stop: Option<(
            std::sync::Arc<std::sync::atomic::AtomicBool>,
            std::time::Instant,
        )>,
        known: Option<&HelperClosure>,
        parallel: bool,
    ) -> Option<Self> {
        if paths.len() > 4096 || cancelled.is_some_and(|cancelled| cancelled()) {
            return None;
        }
        *stage = "interpreter-resolution";
        let interpreter = interpreter()?;
        *stage = "helper-runtime-closure";
        // A manifest probed earlier in this run is the same pre-observer probe
        // result; the observer, baseline digests and helper checks below are
        // unchanged and still bind the actual launched closure.
        let closure = match known.filter(|known| known.interpreter == interpreter) {
            Some(known) => known.paths.clone(),
            None => helper_paths_with(&interpreter, cancelled)?,
        };
        *stage = "helper-native-observer";
        let helper_observer = if cancelled.is_some() {
            let (stop, started) = stop?;
            Observer::begin_files_bounded_with(&closure, stop, started)?
        } else {
            Observer::begin(&closure, &[])?
        };
        *stage = "helper-baseline-digests";
        let baseline = helper_digests_with(&closure, cancelled, parallel)?;
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
        let mut binding_bytes = 0usize;
        let bindings: Vec<_> = paths
            .iter()
            .map(|path| {
                let canonical = path.canonicalize().ok()?;
                if let Some(cancelled) = cancelled {
                    binding_bytes = binding_bytes
                        .checked_add(path.as_os_str().len())?
                        .checked_add(canonical.as_os_str().len())?;
                    if cancelled()
                        || binding_bytes > 512 * 1024
                        || path.as_os_str().len() > 4096
                        || canonical.as_os_str().len() > 4096
                    {
                        return None;
                    }
                }
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
        let mut bytes = if cancelled.is_some() {
            #[derive(serde::Serialize)]
            struct Request<'a> {
                files: &'a [(&'a [u8], &'a u64, &'a u64)],
                runtime: &'a [&'a str],
            }
            bounded_encode(&Request {
                files: &identities,
                runtime: &runtime,
            })?
        } else {
            serde_json::to_vec(&serde_json::json!({ "files": identities, "runtime": runtime }))
                .ok()?
        };
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
        if cancelled.is_some_and(|cancelled| cancelled()) {
            return None;
        }
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
        if cancelled.is_some_and(|cancelled| cancelled()) {
            return None;
        }
        socket.write_all(&bytes).ok()?;
        *stage = "helper-readiness-frame";
        #[cfg(test)]
        let fds = lease.readiness(&mut socket)?;
        #[cfg(not(test))]
        let fds = descriptors(&mut socket)?;
        if cancelled.is_some_and(|cancelled| cancelled()) {
            return None;
        }
        let identities: Vec<_> = lease.bindings.iter().map(|(_, _, d, i)| (*d, *i)).collect();
        *stage = "kernel-lease-inventory-bind";
        lease.kernel = if let Some(cancelled) = cancelled {
            super::kernel_leases::Inventory::bind_bounded(
                lease.process.id(),
                &fds,
                &identities,
                cancelled,
            )
        } else {
            super::kernel_leases::Inventory::bind(lease.process.id(), &fds, &identities)
        };
        let validation = lease.validate_initial(&closure, &baseline, stage, cancelled, parallel)?;
        if let Err(reason) = validation {
            *stage = reason;
            #[cfg(test)]
            eprintln!("lease acquisition failed: {reason}");
            return None;
        }
        if cancelled.is_some_and(|cancelled| cancelled()) {
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
        cancelled: Option<&dyn Fn() -> bool>,
        parallel: bool,
    ) -> Option<std::result::Result<(), &'static str>> {
        let validation = if self.kernel.is_none() {
            Err("kernel-lease-inventory-binding-unavailable")
        } else if !self.bindings_current() {
            Err("lease-binding-invalidated")
        } else {
            *stage = "helper-final-digests";
            if helper_digests_with(closure, cancelled, parallel)? != baseline {
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
        if !self.mounts.clean() {
            return false;
        }
        let descriptors = ProcBindings::open();
        #[cfg(test)]
        if binding_batch::enabled()
            && let Some(descriptors) = &descriptors
        {
            return binding_batch::matches(descriptors, &self.bindings);
        }
        self.bindings
            .iter()
            .all(|(path, canonical, device, inode)| {
                current_binding_matches(descriptors.as_ref(), path, canonical, *device, *inode)
            })
    }
}

// Candidate remains test-only until complete-operation measurements justify
// selection. Production barriers still perform every original absolute lookup.
#[cfg(test)]
#[path = "lease_binding_batch.rs"]
mod binding_batch;

/// Per-barrier descriptor lookup, with no retained identity or freshness memo.
struct ProcBindings {
    directory: OwnedFd,
}
impl ProcBindings {
    fn open() -> Option<Self> {
        use rustix::fs::{Mode, OFlags, PROC_SUPER_MAGIC, fstatfs, open};
        let directory = open(
            "/proc/self/fd",
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()?;
        (fstatfs(&directory).ok()?.f_type == PROC_SUPER_MAGIC).then_some(Self { directory })
    }
    fn matches(&self, path: &Path, canonical: &Path, device: u64, inode: u64) -> bool {
        use rustix::fs::{Mode, OFlags, open};
        let Ok(file) = open(path, OFlags::PATH | OFlags::CLOEXEC, Mode::empty()) else {
            return false;
        };
        self.descriptor_matches(&file, canonical, device, inode)
    }
    fn descriptor_matches(
        &self,
        file: &OwnedFd,
        canonical: &Path,
        device: u64,
        inode: u64,
    ) -> bool {
        use rustix::fs::{fstat, readlinkat_raw};
        let Ok(metadata) = fstat(file) else {
            return false;
        };
        if metadata.st_dev != device || metadata.st_ino != inode {
            return false;
        }
        // The opened descriptor pins this exact lookup. procfs supplies its
        // kernel pathname, including symlink traversal and the opened hardlink.
        // A full buffer may be truncated: use the unchanged original predicate
        // instead of accepting a prefix. No proc lookup failure certifies input.
        let mut bytes = [0u8; 4097];
        let Ok(count) = readlinkat_raw(
            &self.directory,
            file.as_raw_fd().to_string(),
            bytes.as_mut_slice(),
        ) else {
            return false;
        };
        let Some(actual) = bytes.get(..count).filter(|_| count < bytes.len()) else {
            return false;
        };
        // d_path appends this suffix to unlinked files. It can look like a live
        // filename that literally ends in those bytes. Refuse
        // the shortcut for both cases; the original predicate resolves them.
        !actual.ends_with(b" (deleted)") && actual == canonical.as_os_str().as_encoded_bytes()
    }
}
fn current_binding_matches(
    descriptors: Option<&ProcBindings>,
    path: &Path,
    canonical: &Path,
    device: u64,
    inode: u64,
) -> bool {
    descriptors.is_some_and(|owner| owner.matches(path, canonical, device, inode))
        || original_binding_matches(path, canonical, device, inode)
}
fn original_binding_matches(path: &Path, canonical: &Path, device: u64, inode: u64) -> bool {
    path.canonicalize().as_deref().ok() == Some(canonical)
        && std::fs::metadata(path).is_ok_and(|m| m.dev() == device && m.ino() == inode)
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
fn helper_digests_with(
    paths: &[PathBuf],
    cancelled: Option<&dyn Fn() -> bool>,
    parallel: bool,
) -> Option<Vec<Option<String>>> {
    // Only the managed run's own owners overlap these reads; daemon freshness
    // and bounded construction keep the original sequential reads.
    if parallel && cancelled.is_none() {
        return helper_digests(paths);
    }
    let mut read = 0u64;
    paths
        .iter()
        .map(|path| {
            if path.is_file() {
                if let Some(cancelled) = cancelled {
                    bounded_digest(path, &mut read, cancelled).map(Some)
                } else {
                    super::files::file_digest(path).ok().map(Some)
                }
            } else if !path.exists() {
                Some(None)
            } else {
                None
            }
        })
        .collect()
}
/// The unbounded classification of `helper_digests_with`: each path is a
/// regular file read completely, absent, or refused. Classification happens
/// once per path and the complete reads overlap.
fn helper_digests(paths: &[PathBuf]) -> Option<Vec<Option<String>>> {
    let present: Vec<bool> = paths
        .iter()
        .map(|path| {
            if path.is_file() {
                Some(true)
            } else if !path.exists() {
                Some(false)
            } else {
                None
            }
        })
        .collect::<Option<_>>()?;
    let files: Vec<&Path> = paths
        .iter()
        .zip(&present)
        .filter(|(_, present)| **present)
        .map(|(path, _)| path.as_path())
        .collect();
    let mut digests = super::files::digest_all(&files).into_iter();
    present
        .into_iter()
        .map(|present| {
            if present {
                digests.next()?.ok().map(Some)
            } else {
                Some(None)
            }
        })
        .collect()
}
fn bounded_encode(value: &impl serde::Serialize) -> Option<Vec<u8>> {
    struct Writer(Vec<u8>);
    impl Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self
                .0
                .len()
                .checked_add(bytes.len())
                .is_none_or(|size| size > 1_048_576)
            {
                return Err(std::io::Error::other(
                    "bounded helper request exceeds limit",
                ));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer(Vec::new());
    serde_json::to_writer(&mut writer, value).ok()?;
    Some(writer.0)
}
fn bounded_digest(path: &Path, read: &mut u64, cancelled: &dyn Fn() -> bool) -> Option<String> {
    use sha2::{Digest as _, Sha256};
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    let mut file = std::fs::File::from(descriptor);
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 65536].into_boxed_slice();
    loop {
        if cancelled() {
            return None;
        }
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        *read = read.checked_add(u64::try_from(count).ok()?)?;
        if *read > 128 * 1024 * 1024 {
            return None;
        }
        hasher.update(&buffer[..count]);
    }
    Some(format!("{:x}", hasher.finalize()))
}
/// One managed run's probe of the helper interpreter's loaded closure. It is
/// never retained across runs and grants no authority by itself.
#[derive(Clone)]
pub(super) struct HelperClosure {
    interpreter: PathBuf,
    paths: Vec<PathBuf>,
}

pub(super) fn helper_closure() -> Option<HelperClosure> {
    let interpreter = interpreter()?;
    let paths = helper_paths(&interpreter)?;
    Some(HelperClosure { interpreter, paths })
}

pub(super) fn helper_identity(closure: &HelperClosure) -> Option<Vec<(String, String)>> {
    let paths = &closure.paths;
    let files: Vec<&Path> = paths
        .iter()
        .filter(|p| p.is_file())
        .map(PathBuf::as_path)
        .collect();
    // Complete reads of every closure file overlap; any unreadable or
    // non-UTF-8 member still withholds the whole optional identity.
    files
        .iter()
        .zip(super::files::digest_all(&files))
        .map(|(path, digest)| Some((path.to_str()?.to_owned(), digest.ok()?)))
        .collect()
}
fn helper_paths(interpreter: &Path) -> Option<Vec<PathBuf>> {
    helper_paths_with(interpreter, None)
}
fn helper_paths_with(
    interpreter: &Path,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Option<Vec<PathBuf>> {
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
    let read = if let Some(cancelled) = cancelled {
        bounded_helper_output(&mut socket, &mut bytes, cancelled)
    } else {
        std::io::Read::by_ref(&mut socket)
            .take(1_048_577)
            .read_to_end(&mut bytes)
            .map(|_| ())
    };
    let _ = child.kill();
    let _ = child.wait();
    if read.is_err() || bytes.len() > 1_048_576 {
        return None;
    }
    let mut paths: Vec<PathBuf> = if cancelled.is_some() {
        #[derive(serde::Deserialize)]
        struct Paths(#[serde(deserialize_with = "bounded_paths")] Vec<PathBuf>);
        serde_json::from_slice::<Paths>(&bytes).ok()?.0
    } else {
        serde_json::from_slice(&bytes).ok()?
    };
    if paths.len() > 1024 || paths.iter().any(|p| !p.is_absolute()) {
        return None;
    }
    paths.push(interpreter.to_owned());
    paths.sort();
    paths.dedup();
    Some(paths)
}
fn bounded_helper_output(
    socket: &mut UnixStream,
    bytes: &mut Vec<u8>,
    cancelled: &dyn Fn() -> bool,
) -> std::io::Result<()> {
    socket.set_nonblocking(true)?;
    let started = std::time::Instant::now();
    let mut chunk = [0u8; 8192];
    loop {
        if cancelled() || started.elapsed() >= Duration::from_secs(2) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "bounded helper observation cancelled or expired",
            ));
        }
        match socket.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(count) => {
                if bytes.len().checked_add(count).is_none_or(|n| n > 1_048_576) {
                    return Err(std::io::Error::other("bounded helper output exceeds limit"));
                }
                bytes.extend_from_slice(&chunk[..count]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}
fn bounded_paths<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<PathBuf>, D::Error> {
    struct Paths;
    impl<'de> serde::de::Visitor<'de> for Paths {
        type Value = Vec<PathBuf>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("at most1024 helper paths")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while values.len() < 1024 {
                let Some(path) = sequence.next_element::<PathBuf>()? else {
                    return Ok(values);
                };
                if !path.is_absolute() || path.as_os_str().len() > 4096 {
                    return Err(serde::de::Error::custom("unsupported bounded helper path"));
                }
                values.push(path);
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom(
                    "bounded helper path count exceeded",
                ));
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Paths)
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
    fn descriptor_bindings_preserve_exact_native_path_predicate() -> std::io::Result<()> {
        use std::os::unix::ffi::OsStringExt as _;
        use std::os::unix::fs::symlink;
        let root = fixture().with_extension("descriptor-matrix");
        std::fs::create_dir_all(root.join("nested/inside"))?;
        let file = root.join("nested/input");
        std::fs::write(&file, b"same inode, distinct spellings")?;
        std::fs::hard_link(&file, root.join("hardlink"))?;
        symlink("nested/input", root.join("alias"))?;
        symlink("nested", root.join("directory"))?;
        symlink("missing", root.join("dangling"))?;
        symlink("loop", root.join("loop"))?;
        let binary_name = root.join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
        std::fs::write(&binary_name, b"native bytes")?;
        let deleted_name = root.join("literal (deleted)");
        std::fs::write(&deleted_name, b"literal suffix")?;
        let paths = [
            file,
            root.join("hardlink"),
            root.join("alias"),
            root.join("directory/input"),
            root.join("nested/inside/.././input"),
            binary_name,
            deleted_name,
        ];
        let owner = ProcBindings::open()
            .ok_or_else(|| std::io::Error::other("real proc descriptor directory unavailable"))?;
        for expected in &paths {
            let canonical = expected.canonicalize()?;
            let metadata = std::fs::metadata(expected)?;
            for path in paths.iter().chain([
                &root.join("missing"),
                &root.join("dangling"),
                &root.join("loop"),
            ]) {
                let old =
                    original_binding_matches(path, &canonical, metadata.dev(), metadata.ino());
                let fast = owner.matches(path, &canonical, metadata.dev(), metadata.ino());
                assert!(!fast || old, "shortcut admitted a different native binding");
                assert_eq!(
                    current_binding_matches(
                        Some(&owner),
                        path,
                        &canonical,
                        metadata.dev(),
                        metadata.ino()
                    ),
                    old
                );
                assert_eq!(
                    current_binding_matches(None, path, &canonical, metadata.dev(), metadata.ino()),
                    old
                );
                assert!(!owner.matches(path, &canonical, metadata.dev(), metadata.ino() ^ 1));
            }
        }
        // The shortcut may not interpret the deleted marker as a live filename.
        let literal = paths
            .last()
            .ok_or_else(|| std::io::Error::other("fixture"))?;
        let metadata = std::fs::metadata(literal)?;
        assert!(!owner.matches(
            literal,
            &literal.canonicalize()?,
            metadata.dev(),
            metadata.ino()
        ));
        assert!(original_binding_matches(
            literal,
            &literal.canonicalize()?,
            metadata.dev(),
            metadata.ino()
        ));
        std::fs::remove_dir_all(root)
    }
    #[test]
    fn unlinked_descriptor_cannot_spoof_a_literal_deleted_hardlink() -> std::io::Result<()> {
        use rustix::fs::{Mode, OFlags, open};
        let file = fixture();
        let literal = PathBuf::from(format!("{} (deleted)", file.display()));
        std::fs::hard_link(&file, &literal)?;
        let metadata = std::fs::metadata(&file)?;
        let canonical = literal.canonicalize()?;
        let owner =
            ProcBindings::open().ok_or_else(|| std::io::Error::other("proc unavailable"))?;
        let opened = open(&file, OFlags::PATH | OFlags::CLOEXEC, Mode::empty())?;
        std::fs::remove_file(&file)?;
        assert!(!original_binding_matches(
            &file,
            &canonical,
            metadata.dev(),
            metadata.ino()
        ));
        assert!(!owner.descriptor_matches(&opened, &canonical, metadata.dev(), metadata.ino()));
        std::fs::remove_file(literal)
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

#[cfg(test)]
mod bounded_controls {
    use super::*;
    #[test]
    fn bounded_helper_encoding_preserves_exact_original_json_and_refuses_growth() {
        #[derive(serde::Serialize)]
        struct Request<'a> {
            files: &'a [(&'a [u8], &'a u64, &'a u64)],
            runtime: &'a [&'a str],
        }
        let device = 12;
        let inode = 34;
        let files = [(b"/tmp/exact".as_slice(), &device, &inode)];
        let runtime = ["/usr/bin/python3", "/usr/lib/python3"];
        let typed = bounded_encode(&Request {
            files: &files,
            runtime: &runtime,
        })
        .unwrap_or_else(|| unreachable!("bounded fixture"));
        let old = serde_json::to_vec(&serde_json::json!({"files": files, "runtime": runtime}))
            .unwrap_or_else(|_| unreachable!("original fixture"));
        assert_eq!(
            typed, old,
            "typed serialization must preserve original helper bytes"
        );
        assert!(bounded_encode(&"x".repeat(1024 * 1024)).is_none());
    }
    #[test]
    fn helper_observation_shutdown_uses_shared_stop_without_renewing_clock() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let (mut reader, writer) =
            UnixStream::pair().unwrap_or_else(|_| unreachable!("socket fixture"));
        let stop = Arc::new(AtomicBool::new(false));
        let owner = Arc::clone(&stop);
        let (started, ready) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            assert!(started.send(()).is_ok());
            bounded_helper_output(&mut reader, &mut Vec::new(), &|| {
                owner.load(Ordering::Acquire)
            })
        });
        assert!(ready.recv_timeout(Duration::from_secs(2)).is_ok());
        let clock = std::time::Instant::now();
        stop.store(true, Ordering::Release);
        assert!(worker.join().is_ok_and(|result| result.is_err()));
        assert!(clock.elapsed() < Duration::from_millis(250));
        drop(writer);
    }
}
