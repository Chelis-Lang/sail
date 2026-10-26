//! Persisted driver observations and successful proof receipts.

use super::files::{digest, digest_all_tagged, file_digest, json, read_outcome_keys_match};
use crate::{Error, Result};
use sail_acceptance::inputs::{InputFact, InputValue, compare_readback};
use sail_api::output::Report;
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
    /// Per-run content-restoration plan admitted by the managed caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained: Option<super::retained::Plan>,
    /// Whether the driver asks Verus for attribution logs ([`super::Config`]'s
    /// `attribution_logs`; always for an interactive-engine run).
    #[serde(default)]
    pub attribution_logs: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TargetIdentity {
    pub manifest: PathBuf,
    pub name: String,
    pub crate_type: String,
    pub rustc_args_sha256: String,
    /// Cargo's exact binary target name, before Rust crate-name normalization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_name: Option<String>,
}

impl TargetIdentity {
    /// Raw launcher fields, without treating Rust name normalization as Cargo identity.
    pub(crate) fn compiler_unit(&self) -> sail_acceptance::authority::CompilerUnit {
        sail_acceptance::authority::CompilerUnit {
            manifest: self.manifest.as_os_str().as_encoded_bytes().to_vec(),
            name: self.name.as_bytes().to_vec(),
            crate_type: self.crate_type.as_bytes().to_vec(),
            cargo_name: self
                .cargo_name
                .as_ref()
                .map(|name| name.as_bytes().to_vec()),
        }
    }

    pub(crate) fn key(&self) -> String {
        digest(&serde_json::to_vec(self).unwrap_or_default())
    }

    /// Cargo-facing target name; legacy unambiguous identities retain their name.
    pub(crate) fn target_name(&self) -> &str {
        self.cargo_name.as_deref().unwrap_or(&self.name)
    }

    /// Resolve one metadata target, refusing legacy or malformed ambiguous names.
    pub(crate) fn cargo_target<'a>(
        &self,
        package: &'a serde_json::Value,
    ) -> Option<&'a serde_json::Value> {
        if package["manifest_path"].as_str().map(Path::new) != Some(self.manifest.as_path()) {
            return None;
        }
        let unit = self.compiler_unit();
        let mut matches = package["targets"].as_array()?.iter().filter(|target| {
            let Some(name) = target["name"].as_str() else {
                return false;
            };
            let Some(kinds) = target["kind"].as_array() else {
                return false;
            };
            let Some(kinds): Option<Vec<Vec<u8>>> = kinds
                .iter()
                .map(|kind| kind.as_str().map(|kind| kind.as_bytes().to_vec()))
                .collect()
            else {
                return false;
            };
            sail_acceptance::authority::compiler_unit_matches_target(&unit, name.as_bytes(), &kinds)
        });
        let target = matches.next()?;
        matches.next().is_none().then_some(target)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Invocation {
    /// Original proof run for the named same-workspace content experiment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_from: Option<String>,
    /// Separate archived original history copied into this run, never a live seal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_history: Option<String>,
    /// Exact immutable producer namespace. Legacy records cannot select a live attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_attempt: Option<crate::adapter::attempts::SelectedAttempt>,
    pub identity: TargetIdentity,
    pub run_id: String,
    pub report: Report,
    pub success: bool,
    pub dependencies: BTreeMap<PathBuf, String>,
    /// Original proof provenance for compiler-boundary dependency replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_build: Option<sail_api::project::DependencyBuildOrigin>,
}

impl Invocation {
    pub(crate) fn selected_directory(&self, run: &Path) -> Option<PathBuf> {
        if self.retained_history.is_some() {
            return super::retained::history::selected_directory(self, run);
        }
        if self.dependency_build.is_some() || self.retained_from.is_some() {
            return None;
        }
        self.selected_attempt
            .as_ref()?
            .directory(run, &self.identity, self.report.exit_code)
    }
    pub(crate) fn producer_available(&self, run: &Path) -> bool {
        match (&self.selected_attempt, &self.dependency_build) {
            (Some(_), None) => self.selected_directory(run).is_some(),
            (None, Some(_)) => self.retained_from.is_none() && self.retained_history.is_none(),
            _ => false,
        }
    }
    /// Whole proofs retain their success authority. A partial report has a
    /// separate authority: exact recorded arguments, successful verifier output
    /// and no source-engine fallback. It never turns `success` into true.
    pub(crate) fn successful_for(&self, partial_args: Option<&[String]>) -> bool {
        if self.selected_attempt.is_some() == self.dependency_build.is_some()
            || self.retained_from.is_some() != self.retained_history.is_some()
            || self.dependency_build.is_some() && self.retained_from.is_some()
        {
            return false;
        }
        match partial_args {
            None => self.success && complete(&self.report),
            Some(args) => {
                super::runner::focused(args)
                    && self.report.command == args
                    && self.report.engine.is_none()
                    && complete(&self.report)
            }
        }
    }

    /// Original proof provenance; the local invocation run remains separately sealed.
    pub(crate) fn establishing_run(&self) -> &str {
        self.retained_from.as_deref().unwrap_or_else(|| {
            self.dependency_build
                .as_ref()
                .map_or(self.run_id.as_str(), |origin| {
                    origin.establishing_run.as_str()
                })
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub version: u32,
    /// Exact ordered invocation arguments for a partial report; never a full proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_args: Option<Vec<String>>,
    pub checksum: String,
    /// Dependency-store archive key, assigned only by an admitted store transfer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_archive: Option<String>,
    pub package_id: String,
    pub name: String,
    pub kind: Vec<String>,
    pub context: String,
    pub input_revision: String,
    pub invocation: Invocation,
    pub artifact_hashes: BTreeMap<PathBuf, String>,
    #[serde(default)]
    pub committed: PathBuf,
    /// The value, or absence, of every variable a unit of the establishing
    /// run declared (dep-info `# env-dep:`, build-script
    /// `rerun-if-env-changed`); reuse requires each to be unchanged.
    #[serde(default)]
    pub declared_environment: BTreeMap<String, Option<Vec<u8>>>,
}

/// Version seven binds the original selected process history and archive origin.
// Older receipts may lack this producer binding or contain incomplete/mapped
// environment declarations. Preserve their reports, but refuse proof reuse.
pub(crate) const RECEIPT_VERSION: u32 = 7;

impl Receipt {
    pub(crate) fn seal(mut self) -> Self {
        self.checksum = String::new();
        self.checksum = digest(&serde_json::to_vec(&self).unwrap_or_default());
        self
    }
    pub(super) fn producer_history_intact(&self) -> bool {
        if let Some(key) = &self.producer_archive {
            return super::store::retained_producer_intact(self, key);
        }
        self.committed
            .parent()
            .is_some_and(|run| self.invocation.producer_available(run))
    }
    pub(crate) fn valid(&self, context: &str) -> bool {
        self.valid_in(context, &PhaseDigests::default())
    }

    /// [`Receipt::valid`], reading each file at most once in `digests`' phase.
    pub(super) fn valid_in(&self, context: &str, digests: &PhaseDigests) -> bool {
        self.valid_scoped_in(context, digests, None)
    }

    /// A partial report is usable only by the exact ordered partial invocation.
    /// Ordinary proof consumers always call `valid_in`, which refuses it.
    pub(super) fn valid_scoped_in(
        &self,
        context: &str,
        digests: &PhaseDigests,
        partial_args: Option<&[String]>,
    ) -> bool {
        let checksum = self.clone().seal().checksum;
        self.partial_args.as_deref() == partial_args
            && partial_args.is_none_or(super::runner::focused)
            && self.version == RECEIPT_VERSION
            && self.producer_history_intact()
            && super::environment::unchanged(&self.declared_environment)
            && read::<Commit>(&self.committed).is_some_and(|commit| {
                commit.context == self.context
                    && commit.run_id == self.invocation.run_id
                    && commit.receipt_checksums.contains(&self.checksum)
            })
            && self.checksum == checksum
            && self.context == context
            && self.invocation.successful_for(partial_args)
            && !self.artifact_hashes.is_empty()
            && {
                // Both sets are read together, as one overlapping batch.
                digests.prefetch(
                    self.invocation
                        .dependencies
                        .keys()
                        .chain(self.artifact_hashes.keys())
                        .map(PathBuf::as_path),
                );
                digests.matches(&self.invocation.dependencies)
                    && digests.matches(&self.artifact_hashes)
            }
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

/// Adopts a receipt from the dependency store (ADR 0045) into this proof
/// context after a Current run reused it, so later runs of this workspace
/// reuse the dependency through their own receipts. The receipt keeps the
/// establishing run (its provenance) and is bound to this context and input
/// revision by its own commit marker, written last as in [`publish`].
pub(super) fn adopt(
    run: &Path,
    destination: &Path,
    file_name: &std::ffi::OsStr,
    context: &str,
    input_revision: String,
    mut receipt: Receipt,
) -> Result<()> {
    let adopted = run.join("adopted");
    std::fs::create_dir_all(&adopted).map_err(|e| Error::new("SAIL3107", e.to_string()))?;
    context.clone_into(&mut receipt.context);
    receipt.input_revision = input_revision;
    receipt.committed = adopted.join(file_name);
    let receipt = receipt.seal();
    let commit = Commit {
        context: context.to_owned(),
        run_id: receipt.invocation.run_id.clone(),
        receipt_checksums: BTreeSet::from([receipt.checksum.clone()]),
    };
    json(&destination.join(file_name), &receipt)?;
    json(&receipt.committed, &commit)
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
                && (identity.cargo_name.is_none()
                    || receipt.invocation.identity.cargo_name.is_none()
                    || receipt.invocation.identity.cargo_name == identity.cargo_name)
        }) {
            std::fs::remove_file(entry.path())
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        }
    }
    Ok(())
}

pub(crate) fn complete(report: &Report) -> bool {
    sail_api::acceptance::complete_report(
        report,
        sail_acceptance::authority::CompletionPolicy::Receipt,
    )
    .is_some()
}

#[cfg(test)]
pub(super) fn artifact_hashes(paths: &[PathBuf]) -> Result<BTreeMap<PathBuf, String>> {
    PhaseDigests::default().hashes(paths)
}

/// Complete content digests read at most once per path within one phase:
/// the receipt checks after one invocation. A dependency's artifacts are both
/// its own receipt's artifacts and every dependent's inputs (vstd's `.vir` is
/// about 40 MB), and the checks of several receipts overlap their reads.
/// Every digest is still a complete read made in this phase, after the
/// invocation; nothing is carried over from an earlier phase or run.
#[derive(Default)]
pub(super) struct PhaseDigests(std::cell::RefCell<BTreeMap<PathBuf, Result<String>>>);

impl PhaseDigests {
    /// Reads every path not yet read in this phase, concurrently.
    pub(super) fn prefetch<'a>(&self, paths: impl IntoIterator<Item = &'a Path>) {
        let pending: Vec<&Path> = {
            let read = self.0.borrow();
            let mut pending: Vec<&Path> = paths
                .into_iter()
                .filter(|path| !read.contains_key(*path))
                .collect();
            pending.sort_unstable();
            pending.dedup();
            pending
        };
        self.retain_reads(&pending, digest_all_tagged(&pending));
    }

    fn retain_reads(&self, pending: &[&Path], digests: Vec<(&Path, Result<String>)>) {
        if !read_outcome_keys_match(pending, &digests) {
            return;
        }
        self.0.borrow_mut().extend(
            digests
                .into_iter()
                .map(|(path, digest)| (path.to_owned(), digest)),
        );
    }

    /// Every listed file has exactly the recorded digest.
    pub(super) fn matches(&self, expected: &BTreeMap<PathBuf, String>) -> bool {
        self.prefetch(expected.keys().map(PathBuf::as_path));
        let read = self.0.borrow();
        expected
            .iter()
            .all(|(path, hash)| read.get(path).is_some_and(|d| d.as_ref() == Ok(hash)))
    }

    /// As [`artifact_hashes`]: the first failure in path order is the error.
    pub(super) fn hashes(&self, paths: &[PathBuf]) -> Result<BTreeMap<PathBuf, String>> {
        self.prefetch(paths.iter().map(PathBuf::as_path));
        let read = self.0.borrow();
        paths
            .iter()
            .map(|path| {
                let digest = read.get(path).cloned().unwrap_or_else(|| file_digest(path));
                Ok((path.clone(), digest?))
            })
            .collect()
    }
}

/// Every listed file still has exactly the recorded digest; reads overlap.
pub(super) fn digests_match(expected: &BTreeMap<PathBuf, String>) -> bool {
    let files: Vec<&Path> = expected.keys().map(PathBuf::as_path).collect();
    digests_match_readback(expected, &digest_all_tagged(&files))
}

fn digests_match_readback(
    expected: &BTreeMap<PathBuf, String>,
    reads: &[(&Path, Result<String>)],
) -> bool {
    if expected.is_empty() && reads.is_empty() {
        return true;
    }
    let required: Vec<_> = expected
        .iter()
        .map(|(path, hash)| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: InputValue::Present(hash.as_bytes()),
        })
        .collect();
    let observed: Vec<_> = reads
        .iter()
        .map(|(path, digest)| InputFact {
            namespace: 0,
            name: path.as_os_str().as_encoded_bytes(),
            value: digest.as_ref().map_or(InputValue::Unreadable, |hash| {
                InputValue::Present(hash.as_bytes())
            }),
        })
        .collect();
    let (before_order, before_inverse) = super::input::proposed_order(&required);
    let (after_order, after_inverse) = super::input::proposed_order(&observed);
    compare_readback(
        &required,
        &observed,
        &before_order,
        &before_inverse,
        &after_order,
        &after_inverse,
    )
    .is_equal()
}

pub(crate) fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_api::output::CrateReport;

    #[test]
    fn malformed_cargo_kind_fields_are_not_silently_projected_away() {
        let unit = TargetIdentity {
            manifest: "Cargo.toml".into(),
            name: "demo_name".into(),
            crate_type: "rlib".into(),
            rustc_args_sha256: "arguments".into(),
            cargo_name: None,
        };
        let mut package = serde_json::json!({"manifest_path":"Cargo.toml",
            "targets":[{"name":"demo-name","kind":["lib"]}]});
        assert!(unit.cargo_target(&package).is_some());
        for kinds in [
            serde_json::json!(["lib", null]),
            serde_json::json!(["lib", {}]),
            serde_json::json!("lib"),
        ] {
            package["targets"][0]["kind"] = kinds;
            assert!(unit.cargo_target(&package).is_none());
        }
        package["targets"][0]["kind"] = serde_json::json!(["bin"]);
        assert!(unit.cargo_target(&package).is_none());
    }

    #[test]
    fn artifact_read_results_keep_actual_keys_when_reordered()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-tagged-artifacts-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let a = root.join("a.vir");
        let b = root.join("b.vir");
        let extra = root.join("extra.vir");
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "second")?;
        std::fs::write(&extra, "extra")?;
        let paths = [a.as_path(), b.as_path()];
        let expected =
            BTreeMap::from([(a.clone(), file_digest(&a)?), (b.clone(), file_digest(&b)?)]);
        let mut reads = digest_all_tagged(&paths);
        reads.reverse();
        assert!(digests_match_readback(&expected, &reads));
        let phase = PhaseDigests::default();
        phase.retain_reads(&paths, reads);
        assert!(phase.matches(&expected));
        for invalid in [
            vec![(a.as_path(), file_digest(&a))],
            vec![
                (a.as_path(), file_digest(&a)),
                (a.as_path(), file_digest(&a)),
            ],
            vec![
                (a.as_path(), file_digest(&a)),
                (b.as_path(), file_digest(&b)),
                (extra.as_path(), file_digest(&extra)),
            ],
        ] {
            assert!(!digests_match_readback(&expected, &invalid));
            let phase = PhaseDigests::default();
            phase.retain_reads(&paths, invalid);
            assert!(phase.0.borrow().is_empty());
        }
        std::fs::write(&a, "second")?;
        std::fs::write(&b, "first")?;
        let mut swapped = digest_all_tagged(&paths);
        swapped.reverse();
        assert!(!digests_match_readback(&expected, &swapped));
        let phase = PhaseDigests::default();
        phase.retain_reads(&paths, swapped);
        assert!(!phase.matches(&expected));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

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
            version: RECEIPT_VERSION,
            partial_args: None,
            producer_archive: None,
            checksum: String::new(),
            package_id: "input".to_owned(),
            name: "input".to_owned(),
            kind: vec!["lib".to_owned()],
            context: "ctx".to_owned(),
            input_revision: "inputs".to_owned(),
            invocation: Invocation {
                retained_from: None,
                retained_history: None,
                selected_attempt: crate::adapter::attempts::fixture_history(
                    committed.parent().unwrap_or(Path::new("")),
                    "run",
                    &TargetIdentity {
                        manifest: "Cargo.toml".into(),
                        name: "input".into(),
                        crate_type: "lib".into(),
                        rustc_args_sha256: "args".into(),
                        cargo_name: None,
                    },
                    b"fixture",
                    b"",
                    0,
                ),
                identity: TargetIdentity {
                    manifest: PathBuf::from("Cargo.toml"),
                    name: "input".to_owned(),
                    crate_type: "lib".to_owned(),
                    rustc_args_sha256: "args".to_owned(),
                    cargo_name: None,
                },
                run_id: "run".to_owned(),
                report,
                success: true,
                dependencies: BTreeMap::new(),
                dependency_build: None,
            },
            artifact_hashes: artifact_hashes(&[path.to_owned()]).unwrap_or_default(),
            declared_environment: BTreeMap::new(),
            committed,
        }
        .seal()
    }

    #[test]
    fn partial_receipt_cannot_authorize_a_whole_or_different_scope()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-scope-receipt-{}", std::process::id()));
        std::fs::create_dir_all(root.join("run"))?;
        let path = root.join("artifact");
        let args = vec![
            "--".to_owned(),
            "--verify-module".to_owned(),
            "selected".to_owned(),
        ];
        let mut receipt = fixture(&path, root.join("run/committed.json"));
        receipt.partial_args = Some(args.clone());
        receipt.invocation.report.command.clone_from(&args);
        receipt.invocation.success = false;
        let receipt = receipt.seal();
        json(
            &receipt.committed,
            &Commit {
                context: receipt.context.clone(),
                run_id: receipt.invocation.run_id.clone(),
                receipt_checksums: BTreeSet::from([receipt.checksum.clone()]),
            },
        )?;
        let check = |scope: Option<&[String]>| {
            receipt.valid_scoped_in("ctx", &PhaseDigests::default(), scope)
        };
        assert!(check(Some(&args)));
        assert!(!receipt.valid("ctx"));
        assert!(!check(None));
        assert!(!check(Some(&[])));
        let mut changed = args.clone();
        changed[2] = "other".to_owned();
        assert!(!check(Some(&changed)));
        changed = args.clone();
        changed.swap(1, 2);
        assert!(!check(Some(&changed)));
        std::fs::write(&path, "changed artifact")?;
        assert!(!check(Some(&args)));
        std::fs::remove_file(&path)?;
        std::fs::remove_file(&receipt.committed)?;
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn receipt_rejects_corruption_changed_artifact_and_wrong_context() {
        let root = std::env::temp_dir().join(format!("sail-receipt-test-{}", std::process::id()));
        assert!(std::fs::create_dir_all(root.join("run")).is_ok());
        let path = root.join("artifact");
        let receipt = fixture(&path, root.join("run/committed.json"));
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
        // A declared variable whose value differs from the recorded one: the
        // establishing run compiled under another value, so nothing is reused.
        let mut declared = receipt.clone();
        declared.declared_environment.insert(
            "SAIL_TEST_DECLARED_UNSET_L16".to_owned(),
            Some(b"recorded".to_vec()),
        );
        let declared = declared.seal();
        assert!(
            json(
                &declared.committed,
                &Commit {
                    context: "ctx".to_owned(),
                    run_id: "run".to_owned(),
                    receipt_checksums: BTreeSet::from([
                        receipt.checksum.clone(),
                        declared.checksum.clone()
                    ]),
                }
            )
            .is_ok()
        );
        assert!(receipt.valid("ctx"));
        assert!(!declared.valid("ctx"));
        let mut corrupt = receipt.clone();
        corrupt.invocation.report.mode = "other".to_owned();
        assert!(!corrupt.valid("ctx"));
        assert!(std::fs::write(&path, "tampered").is_ok());
        assert!(!receipt.valid("ctx"));
        assert!(std::fs::remove_file(&receipt.committed).is_ok());
        assert!(std::fs::remove_file(path).is_ok());
        assert!(std::fs::remove_dir_all(root).is_ok());
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
