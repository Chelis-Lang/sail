//! Content validation for Cargo's declared build-script inputs.
//!
//! Cargo checksum freshness covers Rust source; build-script fingerprints still
//! use timestamps. These receipts additionally hash declared paths and environment.

use super::files::digest;
#[cfg(test)]
use super::files::json;
use super::input::snapshot_paths;
use super::receipts::read;
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[path = "build_inputs_family.rs"]
pub(super) mod family;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildInput {
    package_id: String,
    package_name: String,
    manifest: PathBuf,
    paths: Vec<PathBuf>,
    env: BTreeMap<String, Option<String>>,
    digest: String,
    covered: bool,
    #[serde(default)]
    output: Option<(PathBuf, String)>,
}

fn current(record: &BuildInput, environment: Option<&BTreeMap<String, String>>) -> (String, bool) {
    let (files, complete) = snapshot_paths(&record.paths);
    let env: BTreeMap<String, Option<String>> = record
        .env
        .keys()
        .map(|k| (k.clone(), effective_value(k, environment)))
        .collect();
    (
        digest(&serde_json::to_vec(&(files, env)).unwrap_or_default()),
        complete,
    )
}

fn effective_value(key: &str, environment: Option<&BTreeMap<String, String>>) -> Option<String> {
    // The pinned Cargo-Verus sets this value unconditionally before Cargo.
    // invoke() also binds it explicitly, without changing process environment.
    // Old records hashed from the parent value fail the new content comparison.
    if key == "RUSTC_BOOTSTRAP" {
        Some("1".to_owned())
    } else {
        environment.map_or_else(|| std::env::var(key).ok(), |e| e.get(key).cloned())
    }
}

#[cfg(target_os = "linux")]
pub(super) fn observation_paths(path: &Path) -> Vec<PathBuf> {
    read::<Vec<BuildInput>>(path)
        .unwrap_or_default()
        .into_iter()
        .flat_map(|record| record.paths)
        .collect()
}

pub(super) fn stale(path: &Path, data: &Value) -> (Vec<String>, Vec<PathBuf>, bool) {
    stale_environment(path, data, None)
}

pub(super) fn stale_environment(
    path: &Path,
    data: &Value,
    environment: Option<&BTreeMap<String, String>>,
) -> (Vec<String>, Vec<PathBuf>, bool) {
    let previous = read::<Vec<BuildInput>>(path).unwrap_or_default();
    let mut clean = BTreeSet::new();
    let mut extras = Vec::new();
    let mut covered = true;
    for record in &previous {
        let (hash, complete) = current(record, environment);
        if !record.covered || !complete || hash != record.digest {
            clean.insert(record.package_name.clone());
        }
        extras.extend(record.paths.iter().cloned());
        covered &= record.covered && complete;
    }
    // A shared compiler target can contain an older script output even when
    // this proof context has no observation. Registry scripts can consume
    // external inputs too: clean every unobserved script before learning its
    // declared inputs beside Cargo's output.
    for package in data["packages"].as_array().into_iter().flatten() {
        if package["targets"].as_array().is_some_and(|targets| {
            targets.iter().any(|t| {
                t["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == "custom-build"))
            })
        }) && !previous
            .iter()
            .any(|r| r.package_id == package["id"].as_str().unwrap_or_default())
        {
            clean.insert(package["name"].as_str().unwrap_or_default().to_owned());
            covered = false;
        }
    }
    (clean.into_iter().collect(), extras, covered)
}

pub(super) fn update(path: &Path, artifacts: &Path, data: &Value) -> Result<()> {
    let mut previous = read::<Vec<BuildInput>>(path).unwrap_or_default();
    for value in std::fs::read_to_string(artifacts)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        if value["reason"] != "build-script-executed" {
            continue;
        }
        let Some(package) = data["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["id"] == value["package_id"])
        else {
            continue;
        };
        // Registry scripts are immutable under Cargo.lock, but declared external
        // inputs are still inspected when Cargo exposes their build output.
        let manifest = PathBuf::from(package["manifest_path"].as_str().unwrap_or_default());
        let package_dir = manifest.parent().unwrap_or(Path::new("."));
        let output = Path::new(value["out_dir"].as_str().unwrap_or_default())
            .parent()
            .unwrap_or(Path::new("."))
            .join("output");
        let text = std::fs::read_to_string(&output).unwrap_or_default();
        let mut paths = Vec::new();
        let mut env = BTreeMap::new();
        for line in text.lines() {
            let line = line
                .strip_prefix("cargo::")
                .or_else(|| line.strip_prefix("cargo:"))
                .unwrap_or(line);
            if let Some(p) = line.strip_prefix("rerun-if-changed=") {
                paths.push(package_dir.join(p));
            }
            if let Some(key) = line.strip_prefix("rerun-if-env-changed=") {
                env.insert(key.to_owned(), effective_value(key, None));
            }
        }
        // With no declaration Cargo watches the entire package. Hash that same
        // input contract and refresh on content changes, independent of mtime.
        if paths.is_empty() && env.is_empty() {
            paths.push(package_dir.to_owned());
        }
        let mut record = BuildInput {
            package_id: package["id"].as_str().unwrap_or_default().to_owned(),
            package_name: package["name"].as_str().unwrap_or_default().to_owned(),
            manifest,
            paths,
            env,
            digest: String::new(),
            covered: true,
            output: Some((output, digest(text.as_bytes()))),
        };
        let (hash, complete) = current(&record, None);
        record.digest = hash;
        record.covered = complete;
        // Cargo can emit separate host/target units for one package. Keep
        // every distinct output contract; the last unit cannot certify others.
        previous.retain(|r| {
            r.package_id != record.package_id
                || r.output.as_ref().is_some_and(|(path, _)| {
                    record
                        .output
                        .as_ref()
                        .is_none_or(|(actual, _)| path != actual)
                })
        });
        previous.push(record);
    }
    super::files::json_metadata(path, &previous)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_focus_context_cleans_registry_scripts_before_learning_external_inputs()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-build-input-focus-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let payload = root.join("external.txt");
        std::fs::write(&payload, "good")?;
        let mut record = BuildInput {
            package_id: "registry-pkg".to_owned(),
            package_name: "registry-pkg".to_owned(),
            manifest: root.join("Cargo.toml"),
            paths: vec![payload.clone()],
            env: BTreeMap::new(),
            digest: String::new(),
            covered: true,
            output: None,
        };
        record.digest = current(&record, None).0;
        let selected_state = root.join("selected.json");
        let missing_state = root.join("missing.json");
        json(&selected_state, &vec![record])?;
        let data = serde_json::json!({"packages":[{
            "id":"registry-pkg", "name":"registry-pkg",
            "source":"registry+https://example.invalid/index",
            "targets":[{"kind":["custom-build"]}]
        }]});
        assert_eq!(stale(&selected_state, &data).0, Vec::<String>::new());
        let file = std::fs::File::open(&payload)?;
        let mtime = file.metadata()?.modified()?;
        std::fs::write(&payload, "fail")?;
        file.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
        assert_eq!(file.metadata()?.modified()?, mtime);
        assert_eq!(stale(&selected_state, &data).0, vec!["registry-pkg"]);
        let (clean, _, covered) = stale(&missing_state, &data);
        assert_eq!(clean, vec!["registry-pkg"]);
        assert!(!covered);
        assert!(!missing_state.exists());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn declared_external_build_input_refresh_ignores_restored_timestamps() {
        let root =
            std::env::temp_dir().join(format!("sail-build-input-test-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let payload = root.join("payload.txt");
        assert!(std::fs::write(&payload, "good").is_ok());
        let mut record = BuildInput {
            package_id: "pkg".to_owned(),
            package_name: "pkg".to_owned(),
            manifest: root.join("Cargo.toml"),
            paths: vec![payload.clone()],
            env: BTreeMap::new(),
            digest: String::new(),
            covered: true,
            output: None,
        };
        record.digest = current(&record, None).0;
        let state = root.join("state.json");
        assert!(json(&state, &vec![record]).is_ok());
        let data = serde_json::json!({"packages":[]});
        assert_eq!(stale(&state, &data).0, Vec::<String>::new());
        let file = std::fs::File::open(&payload).unwrap_or_else(|e| unreachable!("fixture:{e}"));
        let mtime = file
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!(std::fs::write(&payload, "fail").is_ok());
        assert!(
            file.set_times(std::fs::FileTimes::new().set_modified(mtime))
                .is_ok()
        );
        assert_eq!(stale(&state, &data).0, vec!["pkg"]);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
