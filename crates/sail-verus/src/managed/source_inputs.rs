//! Compiler-declared inputs, including explicit files beneath excluded directories.

use super::receipts::read;
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Inputs {
    pub(super) paths: Vec<PathBuf>,
    pub(super) covered: bool,
    #[serde(default)]
    pub(super) by_package: Option<BTreeMap<String, Vec<PathBuf>>>,
}

pub(super) fn load(path: &Path) -> (Vec<PathBuf>, bool) {
    // Older observations do not attribute includes to their consuming package.
    // Establish the scoped declarations before authorizing new current receipts.
    read::<Inputs>(path).map_or_else(
        || (Vec::new(), !path.exists()),
        |v| (v.paths, v.covered && v.by_package.is_some()),
    )
}

pub(super) fn packages(path: &Path) -> Option<BTreeMap<String, Vec<PathBuf>>> {
    read::<Inputs>(path).and_then(|v| v.by_package)
}

pub(super) fn dependencies(text: &str) -> Option<Vec<String>> {
    // Rust emits make-style dep-info. Only the first rule contains inputs;
    // subsequent empty rules and env-dep comments are not file dependencies.
    let joined = text.replace("\\\n", "");
    let rule = joined.lines().next()?.split_once(": ")?.1;
    let mut paths = Vec::new();
    let mut word = String::new();
    let mut escape = false;
    for c in rule.chars() {
        if escape {
            word.push(c);
            escape = false;
        } else if c == '\\' {
            escape = true;
        } else if c.is_whitespace() {
            if !word.is_empty() {
                paths.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if escape {
        return None;
    }
    if !word.is_empty() {
        paths.push(word);
    }
    Some(paths)
}

pub(super) fn update(path: &Path, artifacts: &Path, data: &Value) -> Result<(Vec<PathBuf>, bool)> {
    let workspace = Path::new(data["workspace_root"].as_str().unwrap_or_default());
    let (previous, _) = load(path);
    let mut paths: BTreeSet<PathBuf> = previous.into_iter().collect();
    let mut by_package: BTreeMap<String, BTreeSet<PathBuf>> = packages(path)
        .unwrap_or_default()
        .into_iter()
        .map(|(package, paths)| (package, paths.into_iter().collect()))
        .collect();
    let mut covered = true;
    for artifact in std::fs::read_to_string(artifacts)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["reason"] == "compiler-artifact")
    {
        if artifact["target"]["kind"]
            .as_array()
            .is_some_and(|v| v.iter().any(|k| k == "custom-build"))
        {
            continue;
        }
        let Some(package) = artifact["package_id"].as_str() else {
            covered = false;
            continue;
        };
        let dep_info = artifact["filenames"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(Path::new)
            .filter_map(|p| {
                let name = p.file_stem()?.to_str()?;
                let name = name.strip_prefix("lib").unwrap_or(name);
                Some(p.with_file_name(name).with_extension("d"))
            })
            .find(|p| p.is_file());
        let Some(inputs) = dep_info
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| dependencies(&text))
        else {
            covered = false;
            continue;
        };
        for input in inputs {
            let declared = workspace.join(input);
            let resolved = declared.canonicalize().unwrap_or(declared);
            by_package
                .entry(package.to_owned())
                .or_default()
                .insert(resolved.clone());
            paths.insert(resolved);
        }
    }
    let paths: Vec<PathBuf> = paths.into_iter().collect();
    super::files::json_metadata(
        path,
        &Inputs {
            paths: paths.clone(),
            covered,
            by_package: Some(
                by_package
                    .into_iter()
                    .map(|(package, paths)| (package, paths.into_iter().collect()))
                    .collect(),
            ),
        },
    )?;
    Ok((paths, covered))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiler_dep_info_preserves_escaped_external_paths() {
        assert_eq!(
            dependencies("out.d: src/lib.rs ../shared\\ file.rs \\\n ../other.rs\n\nsrc/lib.rs:\n"),
            Some(vec![
                "src/lib.rs".to_owned(),
                "../shared file.rs".to_owned(),
                "../other.rs".to_owned()
            ])
        );
        assert!(dependencies("truncated dep-info").is_none());
        assert!(dependencies("out.d: trailing\\").is_none());
    }
    #[test]
    fn external_include_paths_join_future_revision_and_watch_coverage() {
        let root =
            std::env::temp_dir().join(format!("sail-source-input-test-{}", std::process::id()));
        let project = root.join("project");
        let target = root.join("target");
        assert!(std::fs::create_dir_all(&project).is_ok());
        assert!(std::fs::create_dir_all(&target).is_ok());
        let external = root.join("shared file.rs");
        assert!(std::fs::write(&external, "good").is_ok());
        assert!(std::fs::write(target.join("demo-123.d"), "out.d: ../shared\\ file.rs\n").is_ok());
        let artifacts = root.join("artifacts.jsonl");
        let artifact = serde_json::json!({"reason":"compiler-artifact","package_id":"demo","target":{"kind":["lib"]},"filenames":[target.join("libdemo-123.rmeta")]});
        assert!(std::fs::write(&artifacts, artifact.to_string()).is_ok());
        let state = root.join("sources.json");
        let mut data = serde_json::json!({"workspace_root":project,"packages":[{"manifest_path":project.join("Cargo.toml")}]});
        let baseline = super::super::input::snapshot_metadata(&data, &[]);
        let (paths, covered) =
            update(&state, &artifacts, &data).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(covered && paths.len() == 1);
        assert_eq!(
            packages(&state).unwrap_or_default().get("demo"),
            Some(&paths)
        );
        data["sail_source_inputs"] = serde_json::to_value(paths).unwrap_or_default();
        let established = super::super::input::snapshot_metadata(&data, &[]);
        assert_ne!(
            baseline.revision, established.revision,
            "new unobserved external inputs invalidate the establishing run"
        );
        assert!(
            established
                .roots
                .iter()
                .any(|p| external.canonicalize().is_ok_and(|target| target == *p))
        );
        assert!(std::fs::write(&external, "fail").is_ok());
        let changed = super::super::input::snapshot_metadata(&data, &[]);
        assert_ne!(established.revision, changed.revision);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    #[test]
    fn legacy_global_observations_require_scoped_establishment() {
        let path = std::env::temp_dir().join(format!(
            "sail-source-input-legacy-{}.json",
            std::process::id()
        ));
        assert!(std::fs::write(&path, r#"{"paths":[],"covered":true}"#).is_ok());
        assert!(!load(&path).1);
        assert!(packages(&path).is_none());
        assert!(std::fs::write(&path, "corrupt source observation").is_ok());
        assert!(!load(&path).1);
        assert!(std::fs::remove_file(path).is_ok());
    }
}
