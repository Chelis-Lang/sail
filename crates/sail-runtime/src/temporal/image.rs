//! Verified private worker images, reused under a settled complete stat
//! stamp (ADR 0082, ADR 0173).
//!
//! Every native job runs a private copy of the installed worker whose bytes
//! were hashed against the installation manifest, so a later change to the
//! installed file cannot change what runs. Hashing and copying the 76 MB
//! worker cost about 100 ms per job (#765). This module keeps one verified
//! copy per installed worker in a per-user private directory and reuses it
//! while both files keep their recorded complete stamps:
//!
//! - The installed worker's stamp (device, inode, size, mode, mtime and
//!   ctime with nanoseconds) is recorded only when it was settled
//!   ([`Stamp::settled`]) before the bytes were read and is unchanged after
//!   they were hashed. Any later write, replacement, `chmod` or `utimes`
//!   advances ctime, so the next job misses and verifies the bytes again.
//! - The private copy is mode `0500` in a mode `0700` directory owned by
//!   this user. Its own complete stamp, owner and link count must still
//!   equal the ones recorded right after it was written; writing it needs a
//!   `chmod` first, which changes ctime.
//!
//! Anything unexpected (another owner, a symbolic link, wider permissions,
//! an unreadable or foreign record, a different source path or digest)
//! misses, and the job uses a fresh verified copy of its own exactly as
//! before. A miss never fails a job that would otherwise run.
use sail_api::stamp::Stamp;
use serde::{Deserialize, Serialize};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

/// Largest worker executable admitted, as before caching.
const MAX_WORKER_BYTES: u64 = 512 * 1024 * 1024;
const RECORD: &str = "record.json";
const RECORD_LIMIT: u64 = 4096;
const EXECUTABLE: &str = "sail-runtime-worker";
/// Interrupted installs older than this are removed by the next install.
const ABANDONED: Duration = Duration::from_secs(3600);

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    source: PathBuf,
    sha256: String,
    source_stamp: Stamp,
    image_stamp: Stamp,
}

/// A verified private copy of the worker, ready to execute.
#[derive(Debug)]
pub struct Image {
    executable: PathBuf,
    /// A per-job copy is removed with its directory on drop; a reused
    /// copy stays for later jobs.
    owned: Option<PathBuf>,
}

impl Image {
    /// The verified private executable.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Whether this is a reused copy rather than one written for this job.
    #[must_use]
    pub fn reused(&self) -> bool {
        self.owned.is_none()
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if let Some(directory) = &self.owned {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

fn stamp(path: &Path) -> Option<Stamp> {
    std::fs::metadata(path).ok().map(|m| Stamp::of(&m))
}

#[cfg(unix)]
fn private_directory(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::symlink_metadata(path).is_ok_and(|m| {
        m.file_type().is_dir()
            && m.uid() == rustix::process::getuid().as_raw()
            && m.mode() & 0o777 == 0o700
    })
}

#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new().mode(0o700).create(path)
}

/// The per-user image directory, created on first use. `None` when it is
/// not a private directory of this user.
#[cfg(unix)]
fn root() -> Option<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "sail-runtime-images-{}",
        rustix::process::getuid().as_raw()
    ));
    match create_private(&root) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return None,
    }
    private_directory(&root).then_some(root)
}

fn entry(root: &Path, source: &Path, sha256: &str) -> PathBuf {
    let mut key = sha256.as_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(source.as_os_str().as_encoded_bytes());
    root.join(crate::hash(&key))
}

fn read_record(directory: &Path) -> Option<Record> {
    let mut bytes = Vec::new();
    std::fs::File::open(directory.join(RECORD))
        .and_then(|f| f.take(RECORD_LIMIT + 1).read_to_end(&mut bytes))
        .ok()?;
    if u64::try_from(bytes.len()).ok()? > RECORD_LIMIT {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// The reusable verified copy of `source` with digest `sha256`, if both
/// files still carry their recorded stamps.
#[cfg(unix)]
fn reusable(source: &Path, sha256: &str) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt as _;
    if !source.is_absolute() {
        return None;
    }
    let directory = entry(&root()?, source, sha256);
    if !private_directory(&directory) {
        return None;
    }
    let record = read_record(&directory)?;
    if record.version != 1
        || record.source != source
        || record.sha256 != sha256
        || stamp(source)? != record.source_stamp
    {
        return None;
    }
    let executable = directory.join(EXECUTABLE);
    let image = std::fs::symlink_metadata(&executable).ok()?;
    (image.file_type().is_file()
        && image.uid() == rustix::process::getuid().as_raw()
        && image.nlink() == 1
        && image.mode() & 0o777 == 0o500
        && Stamp::of(&image) == record.image_stamp)
        .then_some(executable)
}

#[cfg(not(unix))]
fn reusable(_: &Path, _: &str) -> Option<PathBuf> {
    None
}

/// Whether `source` with digest `sha256` was verified at its current
/// settled stamp. Only a hit is proof; a miss means "hash it".
#[must_use]
pub(crate) fn verified(source: &Path, sha256: &str) -> bool {
    reusable(source, sha256).is_some()
}

/// Read `source` within the admission bound and check its digest.
fn read_verified(source: &Path, sha256: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(source)
        .and_then(|file| file.take(MAX_WORKER_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_WORKER_BYTES {
        return Err("worker executable exceeds its 512 MiB admission bound".into());
    }
    if crate::hash(&bytes) != sha256 {
        return Err("worker executable differs from its pinned identity".into());
    }
    Ok(bytes)
}

/// Writes `bytes` as a mode `0500` executable in a fresh private directory.
fn write_image(directory: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    #[cfg(unix)]
    create_private(directory).map_err(|e| e.to_string())?;
    #[cfg(not(unix))]
    std::fs::create_dir(directory).map_err(|e| e.to_string())?;
    let executable = directory.join(EXECUTABLE);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o500);
    }
    options
        .open(&executable)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| e.to_string())?;
    Ok(executable)
}

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// A fresh verified copy of `source` for one job, as before caching.
///
/// # Errors
/// An invalid digest, unreadable, oversized or different worker, or an
/// unwritable temporary directory.
pub fn fresh(source: &Path, sha256: &str) -> Result<Image, String> {
    check_digest(sha256)?;
    let bytes = read_verified(source, sha256)?;
    per_job(&bytes)
}

fn per_job(bytes: &[u8]) -> Result<Image, String> {
    let directory = std::env::temp_dir().join(unique("sail-runtime-image"));
    let image = Image {
        executable: directory.join(EXECUTABLE),
        owned: Some(directory.clone()),
    };
    write_image(&directory, bytes)?;
    Ok(image)
}

fn check_digest(sha256: &str) -> Result<(), String> {
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("worker requires a pinned lowercase SHA-256".into());
    }
    Ok(())
}

/// The verified private copy of `source` for one job: a reused one when
/// both stamps still match, otherwise a fresh one, recorded for reuse when
/// the installed worker was settled and unchanged while it was hashed.
///
/// # Errors
/// As [`fresh`]. Failing to record a reusable copy is not an error.
pub fn image(source: &Path, sha256: &str) -> Result<Image, String> {
    check_digest(sha256)?;
    if let Some(executable) = reusable(source, sha256) {
        return Ok(Image {
            executable,
            owned: None,
        });
    }
    let start = SystemTime::now();
    let before = source.is_absolute().then(|| stamp(source)).flatten();
    let bytes = read_verified(source, sha256)?;
    if let Some(before) = before
        && before.settled(start)
        && stamp(source) == Some(before)
        && let Some(image) = install(source, sha256, before, &bytes)
    {
        return Ok(image);
    }
    per_job(&bytes)
}

/// Publishes a verified copy for reuse. `None` leaves the job to use a
/// per-job copy.
#[cfg(unix)]
fn install(source: &Path, sha256: &str, source_stamp: Stamp, bytes: &[u8]) -> Option<Image> {
    let root = root()?;
    sweep(&root);
    let staged = root.join(unique(".new"));
    let written = write_image(&staged, bytes);
    let Ok(executable) = written else {
        let _ = std::fs::remove_dir_all(&staged);
        return None;
    };
    let record = std::fs::symlink_metadata(&executable)
        .ok()
        .map(|m| Record {
            version: 1,
            source: source.to_owned(),
            sha256: sha256.to_owned(),
            source_stamp,
            image_stamp: Stamp::of(&m),
        })
        .and_then(|r| serde_json::to_vec(&r).ok());
    let published = record.is_some_and(|record| {
        let mut options = std::fs::OpenOptions::new();
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.write(true).create_new(true).mode(0o600);
        }
        options
            .open(staged.join(RECORD))
            .and_then(|mut f| f.write_all(&record))
            .is_ok()
    }) && {
        let target = entry(&root, source, sha256);
        // Retire a stale entry first; a job that already chose it and
        // finds it gone retries with a fresh copy.
        let retired = root.join(unique(".old"));
        if std::fs::rename(&target, &retired).is_ok() {
            let _ = std::fs::remove_dir_all(&retired);
        }
        std::fs::rename(&staged, &target).is_ok()
    };
    if published {
        // Hand out only what a later lookup would accept.
        return reusable(source, sha256).map(|executable| Image {
            executable,
            owned: None,
        });
    }
    // Another process published first, or a write failed: this job keeps
    // its staged verified copy, removed when it finishes.
    Some(Image {
        executable,
        owned: Some(staged),
    })
}

#[cfg(not(unix))]
fn install(_: &Path, _: &str, _: Stamp, _: &[u8]) -> Option<Image> {
    None
}

/// Removes interrupted installs and entries whose installed worker changed.
#[cfg(unix)]
fn sweep(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(".new-") || name.starts_with(".old-") {
            let old = std::fs::symlink_metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age >= ABANDONED);
            if old {
                let _ = std::fs::remove_dir_all(&path);
            }
        } else if private_directory(&path)
            && read_record(&path).is_some_and(|r| stamp(&r.source) != Some(r.source_stamp))
        {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A settled installed worker: written, then its ctime left to age.
    fn installed(
        name: &str,
        bytes: &[u8],
    ) -> Result<(PathBuf, String), Box<dyn std::error::Error>> {
        let directory =
            std::env::temp_dir().join(format!("sail-image-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory)?;
        let source = directory.join("sail-runtime-worker");
        std::fs::write(&source, bytes)?;
        std::thread::sleep(sail_api::stamp::SETTLED + Duration::from_millis(100));
        Ok((source, crate::hash(bytes)))
    }

    /// Removes the test's installed worker and its published entry.
    fn remove(source: &Path, sha: &str) -> TestResult {
        if let Some(root) = root() {
            let _ = std::fs::remove_dir_all(entry(&root, source, sha));
        }
        std::fs::remove_dir_all(source.parent().ok_or("no parent")?)?;
        Ok(())
    }

    #[test]
    fn a_settled_worker_is_reused_until_either_file_changes() -> TestResult {
        let (source, sha) = installed("reuse", b"worker bytes")?;
        let first = image(&source, &sha)?;
        assert!(first.reused(), "settled worker was not recorded");
        assert_eq!(std::fs::read(first.executable())?, b"worker bytes");
        assert!(verified(&source, &sha));
        let second = image(&source, &sha)?;
        assert!(second.reused());
        assert_eq!(first.executable(), second.executable());
        drop((first, second));
        // The reused copy stays after its jobs end.
        assert!(verified(&source, &sha));

        // Same bytes rewritten in place: ctime moves, so verify again.
        std::fs::write(&source, b"worker bytes")?;
        assert!(!verified(&source, &sha));
        let rewritten = image(&source, &sha)?;
        assert!(!rewritten.reused(), "an unsettled rewrite was trusted");
        let directory = rewritten
            .executable()
            .parent()
            .ok_or("no parent")?
            .to_owned();
        drop(rewritten);
        assert!(!directory.exists(), "per-job copy left behind");

        // Different bytes are refused, reused or not.
        std::fs::write(&source, b"other bytes")?;
        assert!(image(&source, &sha).is_err());
        assert!(!verified(&source, &sha));
        remove(&source, &sha)?;
        Ok(())
    }

    #[test]
    fn a_tampered_private_copy_is_never_reused() -> TestResult {
        let (source, sha) = installed("tamper", b"tamper worker")?;
        let first = image(&source, &sha)?;
        assert!(first.reused());
        let copy = first.executable().to_owned();
        drop(first);
        std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o700))?;
        std::fs::write(&copy, b"tamper WORKER")?;
        assert!(!verified(&source, &sha));
        // The next job verifies the installed bytes again and republishes.
        std::thread::sleep(Duration::from_millis(10));
        let next = image(&source, &sha)?;
        assert_eq!(std::fs::read(next.executable())?, b"tamper worker");
        drop(next);
        remove(&source, &sha)?;
        Ok(())
    }

    #[test]
    fn fresh_images_are_private_and_removed() -> TestResult {
        let (source, sha) = installed("fresh", b"fresh worker")?;
        let image = fresh(&source, &sha)?;
        assert!(!image.reused());
        let metadata = std::fs::symlink_metadata(image.executable())?;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o500);
        let directory = image.executable().parent().ok_or("no parent")?.to_owned();
        assert!(private_directory(&directory));
        drop(image);
        assert!(!directory.exists());
        assert!(fresh(&source, "A".repeat(64).as_str()).is_err());
        remove(&source, &sha)?;
        Ok(())
    }
}
