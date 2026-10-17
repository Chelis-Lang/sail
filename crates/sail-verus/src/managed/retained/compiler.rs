//! Consumer of the existing research fork's GHOST/EXEC observation protocol.
//!
//! Uses the same strict wire schema and request/PID binding as the vstd
//! boundary consumer. Workspace sources keep literal paths and compare actual
//! SHA-256 source bytes; no dep-info observation substitutes for either phase.

use super::super::dependency_snapshot::{Content, open};
use super::super::files::digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

const LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Source {
    checksum: String,
    size: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Phase {
    sources: BTreeMap<String, Source>,
    extra_tracked_files: Vec<String>,
    tracked_environment: BTreeMap<String, Option<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    kind: String,
    schema: u32,
    binding: String,
    pid: u32,
    complete: bool,
    phases: BTreeMap<String, Phase>,
}

/// Only an admitted complete actual compiler observation can construct this.
pub(super) struct Accepted {
    pub document_sha256: String,
    pub tracked_environment: BTreeMap<String, Option<String>>,
}

/// Storage integrity only; actual source admission still occurs at recording.
pub(super) fn stored_header(raw: &[u8], key: &str, run: &str) -> bool {
    if raw.len() as u64 > LIMIT {
        return false;
    }
    let Ok(document) = serde_json::from_slice::<Document>(raw) else {
        return false;
    };
    document.kind == "sail-compiler-read-inputs"
        && document.schema == 1
        && document.complete
        && document.pid != 0
        && document.binding == digest(format!("{run}\0{key}").as_bytes())
        && document.phases.len() == 2
        && document.phases.contains_key("verification")
        && document.phases.contains_key("compilation")
        && document
            .phases
            .values()
            .all(|phase| !phase.sources.is_empty())
}

/// Fresh request kept through actual compiler exit. Source/closure admission
/// remains a separate caller obligation; this does not enable retained proofs.
pub(super) struct Observation {
    directory: PathBuf,
    binding: String,
    pid: Option<u32>,
}

impl Observation {
    pub(super) fn begin(directory: &Path, run: &str, key: &str) -> Option<Self> {
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(directory).ok()?;
        Some(Self {
            directory: directory.to_owned(),
            binding: digest(format!("{run}\0{key}").as_bytes()),
            pid: None,
        })
    }

    /// SHA-256 is explicitly requested in both real compiler phases; a
    /// runtime that does not honor it cannot match the complete source basis.
    pub(super) fn configure(&self, command: &mut std::process::Command) {
        command
            .arg("-Zchecksum-hash-algorithm=sha256")
            .env("SAIL_COMPILER_INPUTS", &self.directory)
            .env("SAIL_COMPILER_INPUTS_BINDING", &self.binding);
    }

    pub(super) fn started(&mut self, actual_child_pid: u32) {
        self.pid = Some(actual_child_pid);
    }

    /// `sources` are full literal hashes from the complete current source
    /// inventory. `copies` contains only immutable, independently checked
    /// dependency snapshots; no arbitrary extra-file reopening is permitted.
    pub(super) fn finish(
        &self,
        actual_success: bool,
        working_directory: &Path,
        sources: &BTreeMap<PathBuf, Content>,
        copies: &BTreeMap<PathBuf, Content>,
        environment: &BTreeMap<String, String>,
    ) -> Option<Accepted> {
        if !actual_success {
            return None;
        }
        let entries = std::fs::read_dir(&self.directory)
            .ok()?
            .collect::<std::io::Result<Vec<_>>>()
            .ok()?;
        if entries.len() != 1
            || entries[0].file_name() != "inputs.json"
            || !entries[0].file_type().ok()?.is_file()
        {
            return None;
        }
        let file = open(&entries[0].path())?;
        if file.metadata().ok()?.len() > LIMIT {
            return None;
        }
        let mut raw = Vec::new();
        file.take(LIMIT + 1).read_to_end(&mut raw).ok()?;
        if raw.len() as u64 > LIMIT {
            return None;
        }
        let document: Document = serde_json::from_slice(&raw).ok()?;
        self.admit(&document, working_directory, sources, copies, environment)
            .map(|tracked_environment| Accepted {
                document_sha256: digest(&raw),
                tracked_environment,
            })
    }

    fn admit(
        &self,
        document: &Document,
        directory: &Path,
        sources: &BTreeMap<PathBuf, Content>,
        copies: &BTreeMap<PathBuf, Content>,
        environment: &BTreeMap<String, String>,
    ) -> Option<BTreeMap<String, Option<String>>> {
        if document.kind != "sail-compiler-read-inputs"
            || document.schema != 1
            || !document.complete
            || document.pid == 0
            || Some(document.pid) != self.pid
            || document.binding != self.binding
            || document.phases.len() != 2
            || !document.phases.contains_key("verification")
            || !document.phases.contains_key("compilation")
        {
            return None;
        }
        let mut tracked = BTreeMap::new();
        for phase in document.phases.values() {
            if phase.sources.is_empty() || phase.sources.len() > 16_384 {
                return None;
            }
            for (name, loaded) in &phase.sources {
                let path = directory.join(name).canonicalize().ok()?;
                let expected = sources.get(&path)?;
                if loaded.checksum != format!("sha256={}", expected.sha256)
                    || loaded.size != expected.size
                {
                    return None;
                }
            }
            for name in &phase.extra_tracked_files {
                // The compiler saw this exact private file, not a mutable
                // original that merely happens to have matching bytes now.
                if !copies.contains_key(&PathBuf::from(name)) {
                    return None;
                }
            }
            for (name, value) in &phase.tracked_environment {
                if environment.get(name) != value.as_ref()
                    || tracked
                        .insert(name.clone(), value.clone())
                        .is_some_and(|old| old != *value)
                {
                    return None;
                }
            }
        }
        Some(tracked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_b_exec_a_and_final_disk_a_cannot_create_an_a_proof()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-retained-source-phases-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root)?;
        let source = root.join("lib.rs");
        let outcome = (|| -> Result<(), Box<dyn std::error::Error>> {
            std::fs::write(&source, b"source A")?;
            let source = source.canonicalize()?;
            let expected = BTreeMap::from([(
                source.clone(),
                Content {
                    sha256: digest(b"source A"),
                    size: 8,
                },
            )]);
            let mut request =
                Observation::begin(&root.join("request"), "run", "key").ok_or("request")?;
            request.started(123);
            let phase = serde_json::json!({"sources":{source.to_string_lossy():{"checksum":format!("sha256={}",digest(b"source A")),"size":8}},
                "extra_tracked_files":[],"tracked_environment":{}});
            let raw = serde_json::json!({"kind":"sail-compiler-read-inputs","schema":1,"binding":request.binding,
                "pid":123,"complete":true,"phases":{"verification":phase,"compilation":phase}});
            let good: Document = serde_json::from_value(raw.clone())?;
            assert!(
                request
                    .admit(&good, &root, &expected, &BTreeMap::new(), &BTreeMap::new())
                    .is_some()
            );
            let mut changed = raw.clone();
            changed["phases"]["verification"]["sources"][source.to_string_lossy().as_ref()]["checksum"] =
                format!("sha256={}", digest(b"source B")).into();
            let ghost_b: Document = serde_json::from_value(changed)?;
            assert!(
                request
                    .admit(
                        &ghost_b,
                        &root,
                        &expected,
                        &BTreeMap::new(),
                        &BTreeMap::new()
                    )
                    .is_none()
            );
            let mut unprefixed = raw.clone();
            unprefixed["phases"]["verification"]["sources"][source.to_string_lossy().as_ref()]["checksum"] =
                digest(b"source A").into();
            let unprefixed: Document = serde_json::from_value(unprefixed)?;
            assert!(
                request
                    .admit(
                        &unprefixed,
                        &root,
                        &expected,
                        &BTreeMap::new(),
                        &BTreeMap::new()
                    )
                    .is_none()
            );
            let mut extra = raw.clone();
            extra["phases"]["verification"]["extra_tracked_files"] = serde_json::json!([source]);
            let extra: Document = serde_json::from_value(extra)?;
            assert!(
                request
                    .admit(&extra, &root, &expected, &BTreeMap::new(), &BTreeMap::new())
                    .is_none()
            );
            let mut missing = raw;
            missing["phases"]
                .as_object_mut()
                .ok_or("phases")?
                .remove("verification");
            let missing: Document = serde_json::from_value(missing)?;
            assert!(
                request
                    .admit(
                        &missing,
                        &root,
                        &expected,
                        &BTreeMap::new(),
                        &BTreeMap::new()
                    )
                    .is_none()
            );
            Ok(())
        })();
        std::fs::remove_dir_all(root)?;
        outcome
    }
}
