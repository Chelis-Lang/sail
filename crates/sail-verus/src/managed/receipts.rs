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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub version: u32,
    pub package_id: String,
    pub name: String,
    pub kind: Vec<String>,
    pub context: String,
    pub invocation: Invocation,
    pub artifact_hashes: BTreeMap<PathBuf, String>,
}

impl Receipt {
    pub(crate) fn valid(&self, context: &str) -> bool {
        self.version == 1
            && self.context == context
            && self.invocation.success
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
