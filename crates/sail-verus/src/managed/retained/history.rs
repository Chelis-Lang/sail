//! Independent copies of original selected histories, never live observations.
//!
//! The arena and adopting run own separate immutable copies. Their filesystem
//! protection and actual child/image/pipe attribution remain native contracts.

use super::super::{
    dependency_snapshot::{Content, read_content},
    files::{digest, json},
    receipts::{Invocation, RECEIPT_VERSION, Receipt},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

const FILE_LIMIT: u64 = 256 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 1024 * 1024 * 1024;
const ENTRY_LIMIT: usize = 65_536;
const DEPTH_LIMIT: usize = 16;
const MANIFEST_LIMIT: u64 = 8 * 1024 * 1024;

/// Content-bound original producer, including its original receipt and commit.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct History {
    version: u32,
    receipt: Receipt,
    files: BTreeMap<PathBuf, Content>,
}

fn key_valid(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn contained(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn scan(directory: &Path) -> Option<BTreeMap<PathBuf, Content>> {
    fn visit(
        root: &Path,
        at: &Path,
        depth: usize,
        files: &mut BTreeMap<PathBuf, Content>,
        total: &mut u64,
    ) -> Option<()> {
        if depth > DEPTH_LIMIT || !std::fs::symlink_metadata(at).ok()?.file_type().is_dir() {
            return None;
        }
        for entry in std::fs::read_dir(at).ok()? {
            let entry = entry.ok()?;
            let path = entry.path();
            let kind = entry.file_type().ok()?;
            if kind.is_dir() {
                visit(root, &path, depth + 1, files, total)?;
            } else if kind.is_file() {
                if files.len() >= ENTRY_LIMIT {
                    return None;
                }
                let body = read_content(&path, FILE_LIMIT)?;
                *total = total.checked_add(body.size)?;
                if *total > TOTAL_LIMIT {
                    return None;
                }
                let relative = path.strip_prefix(root).ok()?;
                if !contained(relative) {
                    return None;
                }
                files.insert(relative.to_owned(), body);
            } else {
                return None;
            }
        }
        Some(())
    }
    let mut files = BTreeMap::new();
    visit(directory, directory, 0, &mut files, &mut 0)?;
    Some(files)
}

fn same_receipt(a: &Receipt, b: &Receipt) -> bool {
    serde_json::to_vec(a)
        .ok()
        .zip(serde_json::to_vec(b).ok())
        .is_some_and(|(a, b)| a == b)
}

impl History {
    /// Capture only an independently admissible original local producer.
    pub(super) fn capture(receipt: &Receipt) -> Option<Self> {
        if receipt.producer_archive.is_some()
            || receipt.invocation.retained_from.is_some()
            || receipt.invocation.retained_history.is_some()
            || receipt.invocation.dependency_build.is_some()
            || !receipt.valid(&receipt.context)
        {
            return None;
        }
        let source = receipt.committed.parent()?;
        let selected = receipt.invocation.selected_attempt.as_ref()?;
        let attempts = receipt
            .invocation
            .selected_directory(source)?
            .parent()?
            .to_owned();
        let mut files = scan(&attempts)?
            .into_iter()
            .map(|(path, content)| (Path::new("attempts").join(path), content))
            .collect::<BTreeMap<_, _>>();
        files.insert(
            PathBuf::from("committed.json"),
            read_content(&receipt.committed, MANIFEST_LIMIT)?,
        );
        let history = Self {
            version: 1,
            receipt: receipt.clone(),
            files,
        };
        let copied = source
            .join("sidecars")
            .join(receipt.invocation.identity.key())
            .join("attempts");
        if !selected.archive_matches(
            &copied,
            &receipt.invocation.identity,
            receipt.invocation.establishing_run(),
            receipt.invocation.report.exit_code,
        ) {
            return None;
        }
        history.intact_metadata().then_some(history)
    }

    pub(super) fn key(&self) -> Option<String> {
        Some(digest(&serde_json::to_vec(self).ok()?))
    }

    pub(super) fn incoming(&self) -> Option<(u64, u64)> {
        Some((
            self.files
                .values()
                .try_fold(MANIFEST_LIMIT, |sum, body| sum.checked_add(body.size))?,
            u64::try_from(self.files.len())
                .ok()?
                .checked_mul(2)?
                .checked_add(8)?,
        ))
    }

    pub(super) fn matches_receipt(&self, receipt: &Receipt) -> bool {
        self.intact_metadata() && same_receipt(&self.receipt, receipt)
    }

    pub(super) fn matches_streams(&self, stdout: &Content, stderr: &Content) -> bool {
        let Some(selected) = &self.receipt.invocation.selected_attempt else {
            return false;
        };
        let directory = Path::new("attempts").join(selected.slot().to_string());
        self.files.get(&directory.join("verus.stdout")) == Some(stdout)
            && self.files.get(&directory.join("verus.stderr")) == Some(stderr)
    }

    fn intact_metadata(&self) -> bool {
        self.version == 1
            && self.receipt.version == RECEIPT_VERSION
            && self.receipt.clone().seal().checksum == self.receipt.checksum
            && self.receipt.producer_archive.is_none()
            && self.receipt.invocation.retained_from.is_none()
            && self.receipt.invocation.retained_history.is_none()
            && self.receipt.invocation.dependency_build.is_none()
            && self.receipt.invocation.selected_attempt.is_some()
            && self.receipt.invocation.successful_for(None)
            && !self.receipt.artifact_hashes.is_empty()
            && !self.receipt.invocation.run_id.is_empty()
            && self.files.contains_key(Path::new("committed.json"))
            && self.files.len() <= ENTRY_LIMIT
            && self.files.iter().all(|(path, body)| {
                contained(path)
                    && key_valid(&body.sha256)
                    && body.size <= FILE_LIMIT
                    && (path == Path::new("committed.json") || path.starts_with("attempts"))
            })
            && self
                .files
                .values()
                .try_fold(0u64, |sum, body| sum.checked_add(body.size))
                .is_some_and(|size| size <= TOTAL_LIMIT)
    }

    fn intact_at(&self, directory: &Path) -> bool {
        let check = || -> Option<bool> {
            if !self.intact_metadata() || scan(&directory.join("files"))? != self.files {
                return Some(false);
            }
            let commit: serde_json::Value = serde_json::from_slice(
                &std::fs::read(directory.join("files/committed.json")).ok()?,
            )
            .ok()?;
            let invocation = &self.receipt.invocation;
            Some(
                commit["context"].as_str() == Some(self.receipt.context.as_str())
                    && commit["run_id"].as_str() == Some(invocation.run_id.as_str())
                    && commit["receipt_checksums"]
                        .as_array()?
                        .iter()
                        .any(|value| value.as_str() == Some(self.receipt.checksum.as_str()))
                    && invocation.selected_attempt.as_ref()?.archive_matches(
                        &directory.join("files/attempts"),
                        &invocation.identity,
                        &invocation.run_id,
                        invocation.report.exit_code,
                    ),
            )
        };
        check() == Some(true)
    }

    /// Copy into a fresh directory, then validate every copied byte and origin.
    fn copy_from(&self, from: impl Fn(&Path) -> Option<PathBuf>, to: &Path) -> Option<()> {
        std::fs::create_dir(to).ok()?;
        let outcome = (|| {
            for (relative, content) in &self.files {
                let destination = to.join("files").join(relative);
                std::fs::create_dir_all(destination.parent()?).ok()?;
                super::archive::fresh_copy(&from(relative)?, &destination, content, false)?;
            }
            json(&to.join("manifest.json"), self).ok()?;
            self.intact_at(to).then_some(())
        })();
        if outcome.is_none() {
            let _ = std::fs::remove_dir_all(to);
        }
        outcome
    }

    pub(super) fn publish(&self, arena: &Path) -> Option<()> {
        let directory = arena.join("histories");
        std::fs::create_dir_all(&directory).ok()?;
        let destination = directory.join(self.key()?);
        if destination.exists() {
            return self.intact_at(&destination).then_some(());
        }
        if !self.receipt.valid(&self.receipt.context) {
            return None;
        }
        let source = self.receipt.committed.parent()?;
        self.copy_from(
            |relative| {
                if relative == Path::new("committed.json") {
                    Some(self.receipt.committed.clone())
                } else {
                    Some(
                        source
                            .join("sidecars")
                            .join(self.receipt.invocation.identity.key())
                            .join(relative),
                    )
                }
            },
            &destination,
        )?;
        self.receipt.valid(&self.receipt.context).then_some(())
    }

    pub(super) fn available(&self, arena: &Path) -> bool {
        self.key()
            .is_some_and(|key| self.intact_at(&arena.join("histories").join(key)))
    }

    /// The passed run directory comes from the current native runtime, not the
    /// archived record. This creates a separate archive, never a live attempt.
    pub(super) fn adopt(&self, arena: &Path, run: &Path) -> Option<String> {
        let key = self.key()?;
        let source = arena.join("histories").join(&key);
        if !self.intact_at(&source) {
            return None;
        }
        let root = run.join("retained-histories");
        std::fs::create_dir_all(&root).ok()?;
        let destination = root.join(&key);
        if !destination.exists() {
            self.copy_from(
                |relative| Some(source.join("files").join(relative)),
                &destination,
            )?;
        }
        self.intact_at(&destination).then_some(key)
    }
}

/// Validate an archived selection in the independent current run's namespace.
/// An arbitrary absolute evidence path is never accepted from a result record.
pub(in crate::managed) fn selected_directory(
    invocation: &Invocation,
    run: &Path,
) -> Option<PathBuf> {
    let key = invocation.retained_history.as_deref()?;
    if !key_valid(key)
        || invocation.dependency_build.is_some()
        || invocation.run_id.as_bytes() != run.file_name()?.as_encoded_bytes()
    {
        return None;
    }
    let original_run = invocation.retained_from.as_deref()?;
    let directory = run.join("retained-histories").join(key);
    let bytes = super::archive::bytes(&directory.join("manifest.json"), MANIFEST_LIMIT)?;
    let history: History = serde_json::from_slice(&bytes).ok()?;
    let original = &history.receipt.invocation;
    if history.key()?.as_str() != key
        || original.run_id != original_run
        || original.identity != invocation.identity
        || original.selected_attempt != invocation.selected_attempt
        || original.success != invocation.success
        || original.dependencies != invocation.dependencies
        || serde_json::to_vec(&original.report).ok()?
            != serde_json::to_vec(&invocation.report).ok()?
        || !history.intact_at(&directory)
    {
        return None;
    }
    let slot = original.selected_attempt.as_ref()?.slot();
    Some(directory.join("files/attempts").join(slot.to_string()))
}
