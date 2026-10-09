//! Persisted driver observations and successful proof receipts.

use super::files::{digest, file_digest};
use crate::Result;
use sail_api::output::{Report, VerdictName};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
}

impl Receipt {
    pub(crate) fn seal(mut self) -> Self {
        self.checksum = String::new();
        self.checksum = digest(&serde_json::to_vec(&self).unwrap_or_default());
        self
    }
    pub(crate) fn valid(&self, context: &str) -> bool {
        let checksum = self.clone().seal().checksum;
        self.version == 1
            && self.checksum == checksum
            && self.context == context
            && self.invocation.success
            && self
                .invocation
                .dependencies
                .iter()
                .all(|(path, hash)| file_digest(path).as_ref() == Ok(hash))
            && complete(&self.invocation.report)
            && !self.artifact_hashes.is_empty()
            && self
                .artifact_hashes
                .iter()
                .all(|(path, hash)| file_digest(path).as_ref() == Ok(hash))
    }
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
    paths
        .iter()
        .map(|p| Ok((p.clone(), file_digest(p)?)))
        .collect()
}

pub(crate) fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_api::output::CrateReport;
    #[test]
    fn receipt_rejects_corruption_changed_artifact_and_wrong_context() {
        let path = std::env::temp_dir().join(format!("sail-receipt-test-{}", std::process::id()));
        assert!(std::fs::write(&path, "artifact").is_ok());
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
        let receipt = Receipt {
            version: 1,
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
            artifact_hashes: artifact_hashes(std::slice::from_ref(&path)).unwrap_or_default(),
        }
        .seal();
        assert!(receipt.valid("ctx"));
        assert!(!receipt.valid("different"));
        let mut corrupt = receipt.clone();
        corrupt.invocation.report.mode = "other".to_owned();
        assert!(!corrupt.valid("ctx"));
        assert!(std::fs::write(&path, "tampered").is_ok());
        assert!(!receipt.valid("ctx"));
        assert!(std::fs::remove_file(path).is_ok());
    }
}
