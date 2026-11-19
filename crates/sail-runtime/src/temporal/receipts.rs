//! Exact completed check receipts in an isolated, trusted local disk namespace.
use super::{Dispatch, Job, ReplayPolicy, Request, Response, TemporalBackend, Worker};
use crate::Cancellation;
use sail_api::temporal::{MAX_DOCUMENT_BYTES, Operation, Outcome};
use sail_cache::{DiskCache, Meta};
use sail_key::QueryKey;
use std::path::Path;
use std::time::Instant;

const NAMESPACE: &str = "temporal-v1";
const PRODUCER: &str = "authenticated-worker";
const MAX_HEADER: usize = 4096;

pub(super) fn dispatch(
    request: &Request,
    worker: &Worker,
    backend: &dyn TemporalBackend,
    receipt: Option<&Receipt<'_>>,
    cancellation: &Cancellation,
    deadline: Instant,
) -> (Dispatch, Option<String>) {
    let interrupted = || {
        if cancellation.is_cancelled() {
            Some(Dispatch::Cancelled)
        } else if Instant::now() >= deadline {
            Some(Dispatch::TimedOut)
        } else {
            None
        }
    };
    if let Some(stop) = interrupted() {
        return (stop, None);
    }
    if let Some(receipt) = receipt
        && let Some(response) = receipt.read(request)
    {
        return match interrupted() {
            Some(stop) => (stop, None),
            None => (Dispatch::Completed(response), Some(receipt.key())),
        };
    }
    if let Some(stop) = interrupted() {
        return (stop, None);
    }
    let dispatch = backend.run(
        worker,
        request,
        deadline.saturating_duration_since(Instant::now()),
        &cancellation.0,
    );
    if let Dispatch::Completed(response) = &dispatch
        && let Some(receipt) = receipt
    {
        if let Some(stop) = interrupted() {
            return (stop, None);
        }
        // Publication checks interruption immediately before its atomic rename.
        // Once committed, the completed evidence belongs to this successful call.
        if !receipt.store(request, response, || interrupted().is_some())
            && let Some(stop) = interrupted()
        {
            return (stop, None);
        }
    }
    (dispatch, None)
}

pub(super) struct Receipt<'a> {
    cache: DiskCache,
    key: QueryKey,
    policy: ReplayPolicy,
    worker: &'a Worker,
}

impl<'a> Receipt<'a> {
    pub(super) fn for_check(
        directory: Option<&Path>,
        job: &Job,
        request_hash: &str,
        backend_version: &str,
        worker: &'a Worker,
        policy: Option<ReplayPolicy>,
    ) -> Option<Self> {
        if !matches!(job.request.operation, Operation::Check { .. }) {
            return None;
        }
        let policy = policy?;
        let directory = directory?;
        let context = context(
            &worker.sha256,
            backend_version,
            &super::platform(),
            policy.identity,
        )?;
        let key = sail_api::temporal::check_receipt_key(&context, request_hash, job.wall_time_ms);
        Some(Self {
            cache: DiskCache::new(directory.join(NAMESPACE)),
            key,
            policy,
            worker,
        })
    }

    pub(super) fn key(&self) -> String {
        self.key.to_string()
    }

    fn eligible(&self, request: &Request, response: &Response) -> bool {
        matches!(response.outcome, Outcome::Holds | Outcome::Refuted)
            && response.requested_checks.is_some()
            && (self.policy.validate)(request, response).is_ok()
    }

    pub(super) fn read(&self, request: &Request) -> Option<Box<Response>> {
        let bytes = storage::read(&self.cache, &self.key)?;
        let (meta, payload) = sail_cache::parse_entry(&bytes, &self.key()).ok()?;
        if meta.mode != NAMESPACE
            || meta.command != NAMESPACE
            || meta.solver != self.worker.sha256
            || meta.provenance != PRODUCER
            || payload.len() > MAX_DOCUMENT_BYTES
        {
            return None;
        }
        let response: Response = sail_api::temporal::decode(&payload).ok()?;
        self.eligible(request, &response)
            .then(|| Box::new(response))
    }

    fn store(
        &self,
        request: &Request,
        response: &Response,
        interrupted: impl Fn() -> bool,
    ) -> bool {
        if !self.eligible(request, response) {
            return false;
        }
        let Ok(bytes) = serde_json::to_vec(response) else {
            return false;
        };
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return false;
        }
        let meta = Meta {
            entry_version: sail_cache::ENTRY_VERSION,
            mode: NAMESPACE.into(),
            key: self.key(),
            reply_sha256: sail_api::temporal::sha256(&bytes),
            command: NAMESPACE.into(),
            solver: self.worker.sha256.clone(),
            stored_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |age| age.as_secs()),
            provenance: PRODUCER.into(),
            effort: None,
        };
        storage::publish(&self.cache, &self.key, &meta, &bytes, interrupted)
    }
}

// Each component is separately framed by JSON, including paths/bytes in the request digest.
// The verified worker hash also binds its compiled-in standard modules and their admission.
pub(super) fn context(worker: &str, backend: &str, platform: &str, policy: &str) -> Option<String> {
    let bytes = serde_json::to_vec(&(
        NAMESPACE,
        sail_api::VERSION,
        worker,
        backend,
        platform,
        policy,
    ))
    .ok()?;
    Some(sail_api::temporal::sha256(&bytes))
}

#[cfg(unix)]
mod storage {
    use super::{DiskCache, MAX_DOCUMENT_BYTES, MAX_HEADER, Meta, QueryKey};
    use rustix::fs::{
        AtFlags, FileType, Mode, OFlags, mkdirat, open, openat, renameat, statat, unlinkat,
    };
    use rustix::io::Errno;
    use std::fs::File;
    use std::io::{Read as _, Write as _};
    use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
    use std::sync::atomic::{AtomicU64, Ordering};

    const DIRECTORY: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    fn owned_directory(file: File) -> Option<File> {
        let meta = file.metadata().ok()?;
        (meta.is_dir()
            && meta.uid() == rustix::process::geteuid().as_raw()
            && meta.mode() & 0o022 == 0)
            .then_some(file)
    }

    // Anchor each access to an open, verified directory. A rename of its path
    // cannot redirect subsequent reads or publications into another directory.
    fn shard(cache: &DiskCache, key: &QueryKey, create: bool) -> Option<File> {
        let base = cache.root().parent()?;
        if create {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(base)
                .ok()?;
        }
        let mut dir = owned_directory(File::from(open(base, DIRECTORY, Mode::empty()).ok()?))?;
        let path = cache.entry_path(key);
        for part in path.parent()?.strip_prefix(base).ok()?.components() {
            let name = part.as_os_str();
            if create {
                match mkdirat(&dir, name, Mode::RWXU) {
                    Ok(()) | Err(Errno::EXIST) => {}
                    Err(_) => return None,
                }
            }
            dir = owned_directory(File::from(
                openat(&dir, name, DIRECTORY, Mode::empty()).ok()?,
            ))?;
        }
        Some(dir)
    }

    pub(super) fn read(cache: &DiskCache, key: &QueryKey) -> Option<Vec<u8>> {
        let dir = shard(cache, key, false)?;
        let file = File::from(
            openat(
                &dir,
                key.to_string(),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .ok()?,
        );
        let metadata = file.metadata().ok()?;
        let bound = u64::try_from(MAX_DOCUMENT_BYTES + MAX_HEADER).ok()?;
        if !metadata.is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o022 != 0
            || metadata.len() > bound
        {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(bound + 1).read_to_end(&mut bytes).ok()?;
        (bytes.len() <= MAX_DOCUMENT_BYTES + MAX_HEADER).then_some(bytes)
    }

    pub(super) fn publish(
        cache: &DiskCache,
        key: &QueryKey,
        meta: &Meta,
        bytes: &[u8],
        interrupted: impl Fn() -> bool,
    ) -> bool {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let Some(dir) = shard(cache, key, true) else {
            return false;
        };
        match statat(&dir, &*meta.key, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => {}
            Err(Errno::NOENT) => {}
            _ => return false,
        }
        // Existing readers retain the old inode. Concurrent complete writers
        // with the same content key can safely replace one another, including
        // a damaged receipt, without an unlink window.
        let temp = format!(
            ".tmp-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            meta.key
        );
        let Ok(fd) = openat(
            &dir,
            temp.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
        ) else {
            return false;
        };
        let mut file = File::from(fd);
        let write = serde_json::to_writer(&mut file, meta)
            .map_err(std::io::Error::other)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.write_all(bytes));
        drop(file);
        if write.is_err()
            || interrupted()
            || renameat(&dir, temp.as_str(), &dir, meta.key.as_str()).is_err()
        {
            let _ = unlinkat(&dir, temp.as_str(), AtFlags::empty());
            return false;
        }
        true
    }
}

#[cfg(not(unix))]
mod storage {
    use super::{DiskCache, Meta, QueryKey};
    pub(super) fn read(_: &DiskCache, _: &QueryKey) -> Option<Vec<u8>> {
        None
    }
    pub(super) fn publish(
        _: &DiskCache,
        _: &QueryKey,
        _: &Meta,
        _: &[u8],
        _: impl Fn() -> bool,
    ) -> bool {
        false
    }
}
