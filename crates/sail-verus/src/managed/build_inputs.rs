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

// Cargo metadata retains platform-disabled packages in its resolved graph. The
// empty any() predicate is false on every target; an unexecuted script behind
// ONLY such edges cannot establish an observation. Keep workspace/resolve roots,
// unknown predicates and every ambiguous graph on the original cleaning path.
fn never_scheduled(data: &Value, id: &str) -> bool {
    let check = || -> Option<()> {
        if let Some(selected) = data.get("sail_selected_package_ids") {
            for selected in selected.as_array()? {
                (selected.as_str()? != id).then_some(())?;
            }
        }
        let members = data["workspace_members"].as_array()?;
        for member in members {
            (member.as_str()? != id).then_some(())?;
        }
        let resolve = data["resolve"].as_object()?;
        let root = resolve.get("root")?;
        (root.is_null() || root.as_str()? != id).then_some(())?;
        let nodes = resolve.get("nodes")?.as_array()?;
        (nodes
            .iter()
            .filter(|node| node["id"].as_str() == Some(id))
            .count()
            == 1)
            .then_some(())?;
        let mut incoming = false;
        let mut identities = BTreeSet::new();
        for node in nodes {
            identities.insert(node["id"].as_str()?).then_some(())?;
            for dependency in node["deps"].as_array()? {
                if dependency["pkg"].as_str()? != id {
                    continue;
                }
                let kinds = dependency["dep_kinds"].as_array()?;
                (!kinds.is_empty()).then_some(())?;
                for kind in kinds {
                    let role = kind.get("kind")?;
                    (role.is_null() || role == "dev" || role == "build").then_some(())?;
                    (kind["target"].as_str()? == "cfg(any())").then_some(())?;
                }
                incoming = true;
            }
        }
        incoming.then_some(())
    };
    check().is_some()
}

/// The packages Cargo can schedule for an explicit package selection: the
/// selected packages and everything they reach through any resolved
/// dependency kind. `None` without an explicit, unambiguous selection or a
/// complete, unambiguous resolve graph; then every package is a candidate.
/// The runner refuses Current evidence if Cargo reports a unit outside it.
pub(super) fn selection_closure(data: &Value) -> Option<BTreeSet<String>> {
    let selected = data.get("sail_selected_package_ids")?.as_array()?;
    let nodes = data["resolve"]["nodes"].as_array()?;
    let mut closure = BTreeSet::new();
    let mut pending = Vec::new();
    for id in selected {
        let id = id.as_str()?;
        if closure.insert(id.to_owned()) {
            pending.push(id.to_owned());
        }
    }
    if pending.is_empty() {
        return None;
    }
    while let Some(id) = pending.pop() {
        let mut matches = nodes.iter().filter(|n| n["id"].as_str() == Some(&id));
        let node = matches.next()?;
        matches.next().is_none().then_some(())?;
        for dependency in node["dependencies"].as_array()? {
            let dependency = dependency.as_str()?;
            if closure.insert(dependency.to_owned()) {
                pending.push(dependency.to_owned());
            }
        }
    }
    Some(closure)
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
    // declared inputs beside Cargo's output. A script outside an explicit
    // selection's dependency closure is never scheduled for it, like a
    // platform-disabled one; the runner checks that against Cargo's units.
    let scheduled = selection_closure(data);
    for package in data["packages"].as_array().into_iter().flatten() {
        if scheduled.as_ref().is_some_and(|closure| {
            package["id"]
                .as_str()
                .is_some_and(|id| !closure.contains(id))
        }) {
            continue;
        }
        if package["targets"].as_array().is_some_and(|targets| {
            targets.iter().any(|t| {
                t["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == "custom-build"))
            })
        }) && !previous
            .iter()
            .any(|r| r.package_id == package["id"].as_str().unwrap_or_default())
            && !never_scheduled(data, package["id"].as_str().unwrap_or_default())
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
    fn unselected_workspace_scripts_do_not_require_an_observation() {
        let script = |id: &str| serde_json::json!({"id": id, "name": id, "targets": [{"kind": ["custom-build"]}]});
        let data = serde_json::json!({"workspace_members": ["lib", "bench"],
            "sail_selected_package_ids": ["lib"],
            "packages": [script("lib"), script("syn"), script("bench"), script("wasm")],
            "resolve": {"root": null, "nodes": [
                {"id": "lib", "dependencies": ["syn"]},
                {"id": "syn", "dependencies": []},
                {"id": "bench", "dependencies": ["lib", "wasm"]},
                {"id": "wasm", "dependencies": []}]}});
        assert_eq!(
            selection_closure(&data),
            Some(BTreeSet::from(["lib".to_owned(), "syn".to_owned()]))
        );
        let absent = std::env::temp_dir().join(format!(
            "sail-unselected-script-{}-absent.json",
            std::process::id()
        ));
        assert!(!absent.exists());
        // Scripts in the closure still need observations; the others never run.
        let (clean, _, covered) = stale(&absent, &data);
        assert_eq!(clean, vec!["lib", "syn"]);
        assert!(!covered);
        // Without an explicit, complete selection every script is a candidate.
        let mut variants = Vec::new();
        for selection in [serde_json::json!([]), serde_json::Value::Null] {
            let mut changed = data.clone();
            changed["sail_selected_package_ids"] = selection;
            variants.push(changed);
        }
        let mut missing_node = data.clone();
        missing_node["resolve"]["nodes"] =
            serde_json::json!([{"id": "lib", "dependencies": ["syn"]}]);
        variants.push(missing_node);
        let mut duplicate = data.clone();
        duplicate["resolve"]["nodes"][1]["id"] = serde_json::json!("lib");
        variants.push(duplicate);
        let mut no_edges = data.clone();
        no_edges["resolve"]["nodes"][0] = serde_json::json!({"id": "lib"});
        variants.push(no_edges);
        for changed in variants {
            assert_eq!(selection_closure(&changed), None);
            assert_eq!(
                stale(&absent, &changed).0,
                vec!["bench", "lib", "syn", "wasm"]
            );
        }
    }

    #[test]
    fn disabled_build_script_does_not_require_an_unexecuted_observation() {
        let data = serde_json::json!({"workspace_members":["root"],
            "packages":[{"id":"disabled", "name":"disabled",
                "targets":[{"kind":["custom-build"]}]}],
            "resolve":{"root":"root","nodes":[
                {"id":"root","deps":[{"pkg":"disabled",
                    "dep_kinds":[{"kind":null,"target":"cfg(any())"}]}]},
                {"id":"disabled","deps":[]}]}});
        let absent = std::env::temp_dir().join(format!(
            "sail-disabled-script-{}-absent.json",
            std::process::id()
        ));
        assert!(!absent.exists());
        let (clean, extras, covered) = stale(&absent, &data);
        assert_eq!(clean, Vec::<String>::new());
        assert_eq!(extras, Vec::<PathBuf>::new());
        assert!(covered);
        let mut variants = Vec::new();
        for predicate in [
            serde_json::Value::Null,
            serde_json::json!("cfg(unix)"),
            serde_json::json!("cfg(any(unix))"),
        ] {
            let mut changed = data.clone();
            changed["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["target"] = predicate;
            variants.push(changed);
        }
        let mut member = data.clone();
        member["workspace_members"] = serde_json::json!(["root", "disabled"]);
        variants.push(member);
        let mut root = data.clone();
        root["resolve"]["root"] = serde_json::json!("disabled");
        variants.push(root);
        let mut alternate = data.clone();
        alternate["resolve"]["nodes"][0]["deps"][0]["dep_kinds"] =
            serde_json::json!([{"kind":null,"target":"cfg(any())"},{"kind":null,"target":null}]);
        variants.push(alternate);
        let mut incomplete = data.clone();
        incomplete["resolve"] = serde_json::Value::Null;
        variants.push(incomplete);
        let mut no_edges = data.clone();
        no_edges["resolve"]["nodes"][0]["deps"] = serde_json::json!([]);
        variants.push(no_edges);
        for selection in [serde_json::json!(["disabled"]), serde_json::Value::Null] {
            let mut changed = data.clone();
            changed["sail_selected_package_ids"] = selection;
            variants.push(changed);
        }
        for kinds in [serde_json::json!([]), serde_json::Value::Null] {
            let mut changed = data.clone();
            changed["resolve"]["nodes"][0]["deps"][0]["dep_kinds"] = kinds;
            variants.push(changed);
        }
        let mut other_edge = data.clone();
        other_edge["resolve"]["nodes"][1]["deps"] =
            serde_json::json!([{"pkg":"disabled", "dep_kinds":[{"kind":null,"target":null}]}]);
        variants.push(other_edge);
        let mut duplicate = data.clone();
        duplicate["resolve"]["nodes"] = serde_json::json!([
            data["resolve"]["nodes"][0],
            data["resolve"]["nodes"][1],
            data["resolve"]["nodes"][0]
        ]);
        variants.push(duplicate);
        let mut selected_root = data.clone();
        selected_root["sail_selected_package_ids"] = serde_json::json!(["root"]);
        assert!(stale(&absent, &selected_root).2);
        for changed in variants {
            let (clean, _, covered) = stale(&absent, &changed);
            assert_eq!(clean, vec!["disabled"]);
            assert!(!covered);
        }
    }

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
