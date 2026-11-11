//! Frozen source trees and exclusively owned mutation copies.
//!
//! This is source-copy machinery, not proof admission. The runner must also
//! bind Cargo's external input closure and the actual verifier/solver identity.
//! Source symlinks and special files are refused; no copied path aliases user
//! source. Build output is never shared between copies.

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

const MAX_FILES: usize = 50_000;
const MAX_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone)]
struct File {
    bytes: Vec<u8>,
    executable: bool,
}

/// One immutable workspace tree, with relative paths and original exact bytes.
/// Git metadata named `.git` and directories named `target` are excluded explicitly;
/// admission must refuse a declared source input in an excluded directory.
#[derive(Clone)]
pub struct Snapshot {
    files: BTreeMap<PathBuf, File>,
    digest: String,
}

fn error(message: impl std::fmt::Display) -> Error {
    Error::new("SAIL3171", message.to_string())
}

fn hash(files: &BTreeMap<PathBuf, File>) -> String {
    let mut digest = Sha256::new();
    digest.update(b"sail-campaign-tree-v1\0");
    for (path, file) in files {
        let name = path.as_os_str().as_encoded_bytes();
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name);
        digest.update([u8::from(file.executable)]);
        digest.update((file.bytes.len() as u64).to_le_bytes());
        digest.update(&file.bytes);
    }
    format!("{:x}", digest.finalize())
}

fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn read_file(path: &Path, expected: &std::fs::Metadata, size: usize) -> Result<Vec<u8>> {
    let file = std::fs::File::from(
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .map_err(error)?,
    );
    let actual = file.metadata().map_err(error)?;
    if !actual.is_file()
        || actual.len() != expected.len()
        || actual.permissions().mode() & 0o111 != expected.permissions().mode() & 0o111
    {
        return Err(error("source file changed before being opened"));
    }
    let mut content = Vec::new();
    file.take((size as u64) + 1)
        .read_to_end(&mut content)
        .map_err(error)?;
    if content.len() != size {
        return Err(error("source changed while being captured"));
    }
    Ok(content)
}

fn collect(root: &Path) -> Result<BTreeMap<PathBuf, File>> {
    let mut files = BTreeMap::new();
    let mut todo = vec![PathBuf::new()];
    let mut bytes = 0usize;
    while let Some(directory) = todo.pop() {
        let entries = std::fs::read_dir(root.join(&directory)).map_err(error)?;
        for entry in entries {
            let entry = entry.map_err(error)?;
            if entry.file_name() == ".git" {
                continue;
            }
            let path = directory.join(entry.file_name());
            let metadata = std::fs::symlink_metadata(entry.path()).map_err(error)?;
            if metadata.is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
                return Err(error(format!(
                    "source link or special file: {}",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                if entry.file_name() != "target" {
                    todo.push(path);
                }
                continue;
            }
            let size = usize::try_from(metadata.len()).map_err(error)?;
            bytes = bytes
                .checked_add(size)
                .ok_or_else(|| error("source size overflow"))?;
            if files.len() >= MAX_FILES || bytes > MAX_BYTES {
                return Err(error("source tree exceeds 50000 files or 512 MiB"));
            }
            let content = read_file(&entry.path(), &metadata, size)?;
            files.insert(
                path,
                File {
                    bytes: content,
                    executable: metadata.permissions().mode() & 0o111 != 0,
                },
            );
        }
    }
    Ok(files)
}

impl Snapshot {
    /// Reads the entire admitted workspace twice and requires identical bytes,
    /// paths and executable bits. This does not establish external-input coverage.
    ///
    /// # Errors
    /// `SAIL3171` for unreadable, changing, linked, special or oversized input.
    pub fn capture(root: &Path) -> Result<Self> {
        let root = root.canonicalize().map_err(error)?;
        let files = collect(&root)?;
        let digest = hash(&files);
        if hash(&collect(&root)?) != digest {
            return Err(error("source tree changed while being captured"));
        }
        Ok(Self { files, digest })
    }

    /// Exact tree identity, independent of the copy's absolute path.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Exact captured content at a normal relative path.
    #[must_use]
    pub fn bytes(&self, file: &Path) -> Option<&[u8]> {
        self.files.get(file).map(|file| file.bytes.as_slice())
    }

    /// Every frozen path in deterministic order.
    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.files.keys().map(PathBuf::as_path)
    }

    /// Applies exactly one range to an immutable twin and recomputes its identity.
    ///
    /// # Errors
    /// `SAIL3171` for an absent/non-relative path, stale original bytes or invalid
    /// UTF-8 boundary or a patched tree exceeding 512 MiB. The caller must bind
    /// the resulting digest to its mutation.
    pub fn patched(
        &self,
        file: &Path,
        start: usize,
        end: usize,
        before: &str,
        after: &str,
    ) -> Result<Self> {
        if !relative(file) {
            return Err(error("mutation path must be a normal relative file"));
        }
        let original = self
            .files
            .get(file)
            .ok_or_else(|| error("mutation file was not captured"))?;
        let text = std::str::from_utf8(&original.bytes).map_err(error)?;
        if text.get(start..end) != Some(before) || before == after {
            return Err(error("mutation's exact original range does not match"));
        }
        let Some(prefix) = text.get(..start) else {
            return Err(error("invalid mutation start"));
        };
        let Some(suffix) = text.get(end..) else {
            return Err(error("invalid mutation end"));
        };
        let total = self.files.values().try_fold(0usize, |total, file| {
            total
                .checked_add(file.bytes.len())
                .ok_or_else(|| error("source size overflow"))
        })?;
        let patched_total = total
            .checked_sub(before.len())
            .and_then(|total| total.checked_add(after.len()));
        if patched_total.is_none_or(|total| total > MAX_BYTES) {
            return Err(error("patched source tree exceeds 512 MiB"));
        }
        let mut files = self.files.clone();
        files.insert(
            file.to_path_buf(),
            File {
                bytes: format!("{prefix}{after}{suffix}").into_bytes(),
                executable: original.executable,
            },
        );
        let digest = hash(&files);
        Ok(Self { files, digest })
    }

    /// Creates a new private copy; it never writes through an existing root.
    /// The parent must be the daemon's private, exclusively owned campaign dir.
    ///
    /// # Errors
    /// `SAIL3171` for an existing destination or any filesystem failure.
    pub fn create_copy(&self, destination: &Path) -> Result<()> {
        std::fs::create_dir(destination).map_err(error)?;
        std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o700))
            .map_err(error)?;
        for (relative, file) in &self.files {
            let path = destination.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(error)?;
            }
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(error)?;
            output.write_all(&file.bytes).map_err(error)?;
            let mode = if file.executable { 0o700 } else { 0o600 };
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                .map_err(error)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Root(PathBuf);
    impl Root {
        fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
            let root = std::env::temp_dir().join(format!(
                "sail-campaign-tree-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root)?;
            Ok(Self(root))
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn copies_are_independent_and_patches_bind_all_original_bytes() -> TestResult {
        let root = Root::new()?;
        let source = root.0.join("source");
        std::fs::create_dir(&source)?;
        std::fs::write(source.join("lib.rs"), "fn f() { let x = 1; }\n")?;
        std::fs::write(source.join("Cargo.toml"), "[package]\nname='t'\n")?;
        let baseline = Snapshot::capture(&source)?;
        let original = baseline.bytes(Path::new("lib.rs")).ok_or("missing file")?;
        let text = std::str::from_utf8(original)?;
        let start = text.find('1').ok_or("missing literal")?;
        let mutant = baseline.patched(Path::new("lib.rs"), start, start + 1, "1", "2")?;
        assert_ne!(baseline.digest(), mutant.digest());
        baseline.create_copy(&root.0.join("baseline"))?;
        mutant.create_copy(&root.0.join("mutant"))?;
        assert_eq!(
            Snapshot::capture(&root.0.join("baseline"))?.digest(),
            baseline.digest()
        );
        assert_eq!(
            Snapshot::capture(&root.0.join("mutant"))?.digest(),
            mutant.digest()
        );
        std::fs::write(root.0.join("mutant/lib.rs"), "changed")?;
        assert_eq!(std::fs::read(source.join("lib.rs"))?, original);
        assert_eq!(std::fs::read(root.0.join("baseline/lib.rs"))?, original);
        assert_eq!(baseline.digest(), Snapshot::capture(&source)?.digest());
        assert!(
            baseline
                .patched(Path::new("lib.rs"), start, start + 1, "0", "2")
                .is_err()
        );
        assert!(
            baseline
                .patched(Path::new("../source/lib.rs"), start, start + 1, "1", "2")
                .is_err()
        );
        assert!(
            baseline
                .patched(Path::new("lib.rs"), start, start + 1, "1", "1")
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn source_links_and_preexisting_destinations_are_refused() -> TestResult {
        let root = Root::new()?;
        std::fs::write(root.0.join("original"), "keep")?;
        let source = root.0.join("source");
        std::fs::create_dir(&source)?;
        std::os::unix::fs::symlink(root.0.join("original"), source.join("linked"))?;
        assert!(Snapshot::capture(&source).is_err());
        std::fs::remove_file(source.join("linked"))?;
        std::fs::write(source.join("lib.rs"), "fn f() {}")?;
        let snapshot = Snapshot::capture(&source)?;
        let alias = root.0.join("alias");
        std::os::unix::fs::symlink(&source, &alias)?;
        assert!(snapshot.create_copy(&alias).is_err());
        assert!(snapshot.create_copy(&source).is_err());
        assert_eq!(std::fs::read_to_string(root.0.join("original"))?, "keep");
        Ok(())
    }

    #[test]
    fn source_change_permissions_and_ambiguous_file_boundaries_change_identity() -> TestResult {
        let root = Root::new()?;
        std::fs::write(root.0.join("a"), "bc")?;
        let first = Snapshot::capture(&root.0)?;
        std::fs::remove_file(root.0.join("a"))?;
        std::fs::write(root.0.join("ab"), "c")?;
        let second = Snapshot::capture(&root.0)?;
        assert_ne!(first.digest(), second.digest());
        std::fs::set_permissions(root.0.join("ab"), std::fs::Permissions::from_mode(0o700))?;
        let executable = Snapshot::capture(&root.0)?;
        assert_ne!(second.digest(), executable.digest());
        std::fs::write(root.0.join("ab"), "d")?;
        assert_ne!(executable.digest(), Snapshot::capture(&root.0)?.digest());
        Ok(())
    }

    #[test]
    fn patched_trees_cannot_grow_past_the_source_budget() -> TestResult {
        let root = Root::new()?;
        std::fs::write(root.0.join("lib.rs"), "1")?;
        std::fs::write(root.0.join("other"), "keep")?;
        let baseline = Snapshot::capture(&root.0)?;
        let oversized = "0".repeat(MAX_BYTES + 1);
        let error = baseline
            .patched(Path::new("lib.rs"), 0, 1, "1", &oversized)
            .err()
            .ok_or("oversized mutation was admitted")?;
        assert_eq!(error.code, "SAIL3171");
        assert_eq!(baseline.digest(), Snapshot::capture(&root.0)?.digest());
        assert_eq!(std::fs::read(root.0.join("lib.rs"))?, b"1");
        Ok(())
    }
}
