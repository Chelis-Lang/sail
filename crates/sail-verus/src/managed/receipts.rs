//! Persisted driver observations and successful proof receipts.

use super::files::{digest, digest_all, json};
use crate::{Error, Result};
use sail_api::output::{Report, VerdictName};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Runtime {
    pub run_id: String,
    pub args: Vec<String>,
    pub z3: PathBuf,
    pub z3_sha256: String,
    pub daemon: bool,
    pub sidecars: PathBuf,
    pub invocations: PathBuf,
    pub original_wrapper: PathBuf,
    pub receipts: PathBuf,
    pub context_sha256: String,
    pub input_revision: String,
    pub packages: BTreeMap<PathBuf, String>,
    pub target_kinds: BTreeMap<PathBuf, BTreeMap<String, Vec<String>>>,
    pub engine: Option<crate::engine::Request>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TargetIdentity {
    pub manifest: PathBuf,
    pub name: String,
    pub crate_type: String,
    pub rustc_args_sha256: String,
}

impl TargetIdentity {
    pub(crate) fn key(&self) -> String {
        digest(&serde_json::to_vec(self).unwrap_or_default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Invocation {
    pub identity: TargetIdentity,
    pub run_id: String,
    pub report: Report,
    pub success: bool,
    pub dependencies: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub version: u32,
    pub checksum: String,
    pub package_id: String,
    pub name: String,
    pub kind: Vec<String>,
    pub context: String,
    pub input_revision: String,
    pub invocation: Invocation,
    pub artifact_hashes: BTreeMap<PathBuf, String>,
    #[serde(default)]
    pub committed: PathBuf,
}

impl Receipt {
    pub(crate) fn seal(mut self) -> Self {
        self.checksum = String::new();
        self.checksum = digest(&serde_json::to_vec(&self).unwrap_or_default());
        self
    }
    pub(crate) fn valid(&self, context: &str) -> bool {
        let checksum = self.clone().seal().checksum;
        self.version == 2
            && read::<Commit>(&self.committed).is_some_and(|commit| {
                commit.context == self.context
                    && commit.run_id == self.invocation.run_id
                    && commit.receipt_checksums.contains(&self.checksum)
            })
            && self.checksum == checksum
            && self.context == context
            && self.invocation.success
            && digests_match(&self.invocation.dependencies)
            && complete(&self.invocation.report)
            && !self.artifact_hashes.is_empty()
            && digests_match(&self.artifact_hashes)
    }
}

/// Written last: an interrupted publication can only leave unusable receipts.
#[derive(Serialize, Deserialize)]
struct Commit {
    context: String,
    run_id: String,
    receipt_checksums: BTreeSet<String>,
}

/// Copies a run's staged receipts, then writes the one commit marker that
/// binds their checksums; until the marker exists no copied receipt is valid.
pub(super) fn publish(run: &Path, destination: &Path, context: &str) -> Result<()> {
    let mut commit = Commit {
        context: context.to_owned(),
        run_id: run
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        receipt_checksums: BTreeSet::new(),
    };
    for entry in std::fs::read_dir(run.join("pending-receipts"))
        .map_err(|e| Error::new("SAIL3107", e.to_string()))?
    {
        let entry = entry.map_err(|e| Error::new("SAIL3107", e.to_string()))?;
        let receipt: Receipt = read(&entry.path())
            .ok_or_else(|| Error::new("SAIL3107", "staged proof receipt is unreadable"))?;
        if receipt.context != context
            || receipt.invocation.run_id != commit.run_id
            || receipt.committed != run.join("committed.json")
            || receipt.clone().seal().checksum != receipt.checksum
        {
            return Err(Error::new(
                "SAIL3107",
                "staged proof receipt binding failed",
            ));
        }
        json(&destination.join(entry.file_name()), &receipt)?;
        commit.receipt_checksums.insert(receipt.checksum);
    }
    #[cfg(test)]
    if let Some(path) = std::env::var_os("SAIL_TEST_MANAGED_RECEIPT_CHECKPOINT") {
        std::fs::write(PathBuf::from(path).join("ready"), [])
            .map_err(|e| Error::new("SAIL3107", e.to_string()))?;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    json(&run.join("committed.json"), &commit)
}

/// A fresh verifier must retire the prior proof before rewriting its artifact.
pub(crate) fn retire(runtime: &Runtime, identity: &TargetIdentity) -> Result<()> {
    for entry in
        std::fs::read_dir(&runtime.receipts).map_err(|e| Error::new("SAIL3111", e.to_string()))?
    {
        let entry = entry.map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        if read::<Receipt>(&entry.path()).is_some_and(|receipt| {
            receipt.invocation.identity.manifest == identity.manifest
                && receipt.invocation.identity.name == identity.name
                && receipt.invocation.identity.crate_type == identity.crate_type
        }) {
            std::fs::remove_file(entry.path())
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        }
    }
    Ok(())
}

pub(crate) fn complete(report: &Report) -> bool {
    report.exit_code == Some(0)
        && report.error.is_none()
        && report.invariant_violations().is_empty()
        && !report.crates.is_empty()
        && report.crates.iter().all(|c| {
            c.verus.errors == Some(0)
                && c.verus.success != Some(false)
                && c.verus.encountered_vir_error != Some(true)
                && c.functions
                    .iter()
                    .all(|f| f.verdict == VerdictName::Verified)
        })
}

pub(super) fn artifact_hashes(paths: &[PathBuf]) -> Result<BTreeMap<PathBuf, String>> {
    // Complete reads of every artifact overlap; the first failure in path order
    // is still the reported error.
    let files: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
    paths
        .iter()
        .zip(digest_all(&files))
        .map(|(p, digest)| Ok((p.clone(), digest?)))
        .collect()
}

/// Every listed file still has exactly the recorded digest; reads overlap.
fn digests_match(expected: &BTreeMap<PathBuf, String>) -> bool {
    let files: Vec<&Path> = expected.keys().map(PathBuf::as_path).collect();
    expected
        .values()
        .zip(digest_all(&files))
        .all(|(hash, digest)| digest.as_ref() == Ok(hash))
}

pub(crate) fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_api::output::CrateReport;
    fn fixture(path: &Path, committed: PathBuf) -> Receipt {
        assert!(std::fs::write(path, "artifact").is_ok());
        let mut report = crate::report::setup_failure(&[], "SAIL3100", "fixture");
        report.error = None;
        report.exit_code = Some(0);
        report.crates.push(CrateReport {
            name: Some("input".to_owned()),
            verus: sail_api::output::VerusResults {
                verified: Some(0),
                errors: Some(0),
                success: Some(true),
                encountered_vir_error: Some(false),
            },
            functions: Vec::new(),
        });
        Receipt {
            version: 2,
            committed,
            checksum: String::new(),
            package_id: "input".to_owned(),
            name: "input".to_owned(),
            kind: vec!["lib".to_owned()],
            context: "ctx".to_owned(),
            input_revision: "inputs".to_owned(),
            invocation: Invocation {
                identity: TargetIdentity {
                    manifest: PathBuf::from("Cargo.toml"),
                    name: "input".to_owned(),
                    crate_type: "lib".to_owned(),
                    rustc_args_sha256: "args".to_owned(),
                },
                run_id: "run".to_owned(),
                report,
                success: true,
                dependencies: BTreeMap::new(),
            },
            artifact_hashes: artifact_hashes(&[path.to_owned()]).unwrap_or_default(),
        }
        .seal()
    }

    #[test]
    fn receipt_rejects_corruption_changed_artifact_and_wrong_context() {
        let path = std::env::temp_dir().join(format!("sail-receipt-test-{}", std::process::id()));
        let receipt = fixture(&path, path.with_extension("committed.json"));
        assert!(!receipt.valid("ctx"));
        assert!(
            json(
                &receipt.committed,
                &Commit {
                    context: "ctx".to_owned(),
                    run_id: "run".to_owned(),
                    receipt_checksums: BTreeSet::from([receipt.checksum.clone()]),
                }
            )
            .is_ok()
        );
        assert!(receipt.valid("ctx"));
        assert!(!receipt.valid("different"));
        let mut corrupt = receipt.clone();
        corrupt.invocation.report.mode = "other".to_owned();
        assert!(!corrupt.valid("ctx"));
        assert!(std::fs::write(&path, "tampered").is_ok());
        assert!(!receipt.valid("ctx"));
        assert!(std::fs::remove_file(&receipt.committed).is_ok());
        assert!(std::fs::remove_file(path).is_ok());
    }
    #[test]
    fn interrupted_publisher_helper() {
        let Some(path) = std::env::var_os("SAIL_TEST_MANAGED_RECEIPT_CHECKPOINT") else {
            return;
        };
        let root = PathBuf::from(path);
        let run = root.join("run");
        assert!(std::fs::create_dir_all(run.join("pending-receipts")).is_ok());
        assert!(std::fs::create_dir_all(root.join("receipts")).is_ok());
        let receipt = fixture(&root.join("artifact"), run.join("committed.json"));
        assert!(json(&run.join("pending-receipts/fixture.json"), &receipt).is_ok());
        assert!(publish(&run, &root.join("receipts"), "ctx").is_ok());
    }

    #[test]
    fn killing_publisher_before_commit_never_authorizes_partial_publication() {
        let root = std::env::temp_dir().join(format!("sail-receipt-kill-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let executable = std::env::current_exe().unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let mut child = std::process::Command::new(executable)
            .args([
                "--exact",
                "managed::receipts::tests::interrupted_publisher_helper",
                "--nocapture",
            ])
            .env("SAIL_TEST_MANAGED_RECEIPT_CHECKPOINT", &root)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !root.join("ready").is_file() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let ready = root.join("ready").is_file();
        assert!(child.kill().is_ok());
        assert!(child.wait().is_ok());
        assert!(ready);
        let receipt = read::<Receipt>(&root.join("receipts/fixture.json"))
            .unwrap_or_else(|| unreachable!("partial receipt was published"));
        assert!(!root.join("run/committed.json").exists());
        assert!(!receipt.valid("ctx"));
        assert!(publish(&root.join("run"), &root.join("receipts"), "ctx").is_ok());
        assert!(receipt.valid("ctx"));
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
