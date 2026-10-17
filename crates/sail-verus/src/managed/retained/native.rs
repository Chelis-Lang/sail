//! Distinct actual native `SourceMap` evidence; never substitute a final file read.
//!
//! Native units always execute. This admission may establish provenance for a
//! later retained workspace proof only after successful complete child exit.

use super::super::dependency_snapshot::{Content, open};
use super::eligibility::Closure;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

const LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    checksum: String,
    size: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    sources: BTreeMap<String, Source>,
    extra_tracked_files: Vec<String>,
    tracked_environment: BTreeMap<String, Option<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    kind: String,
    schema: u32,
    binding: String,
    pid: u32,
    complete: bool,
    inputs: Inputs,
}

/// A request bound before spawning the one actual ordinary native child.
pub(super) struct Observation {
    path: PathBuf,
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
            path: directory.join("inputs.json"),
            directory: directory.to_owned(),
            binding: super::super::digest(format!("{run}\0{key}").as_bytes()),
            pid: None,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
    pub(super) fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub(super) fn configure(&self, command: &mut std::process::Command) {
        command
            .env("SAIL_NATIVE_COMPILER_INPUTS", &self.directory)
            .env("SAIL_NATIVE_COMPILER_INPUTS_BINDING", &self.binding);
    }

    pub(super) fn started(&mut self, pid: u32) {
        self.pid = Some(pid);
    }

    pub(super) fn finish(
        &self,
        success: bool,
        cwd: &Path,
        closure: &Closure,
        copies: &BTreeMap<PathBuf, Content>,
        environment: &BTreeMap<String, String>,
    ) -> Option<String> {
        if !success {
            return None;
        }
        let entries = std::fs::read_dir(self.path.parent()?).ok()?;
        let mut count = 0;
        for entry in entries {
            let entry = entry.ok()?;
            if entry.file_name() != "inputs.json" || !entry.file_type().ok()?.is_file() {
                return None;
            }
            count += 1;
        }
        if count != 1 {
            return None;
        }
        let file = open(&self.path)?;
        if file.metadata().ok()?.len() > LIMIT {
            return None;
        }
        let mut raw = Vec::new();
        file.take(LIMIT + 1).read_to_end(&mut raw).ok()?;
        if raw.len() as u64 > LIMIT {
            return None;
        }
        let document: Document = serde_json::from_slice(&raw).ok()?;
        let names: Vec<_> = document
            .inputs
            .sources
            .keys()
            .map(|name| super::lexical(&cwd.join(name)))
            .collect::<Option<_>>()?;
        (document.kind == "sail-native-compiler-read-inputs"
            && document.schema == 1
            && document.complete
            && document.pid != 0
            && Some(document.pid) == self.pid
            && document.binding == self.binding
            && closure.native_sources(
                document
                    .inputs
                    .sources
                    .values()
                    .zip(&names)
                    .map(|(source, path)| (path.as_path(), source.checksum.as_str(), source.size)),
            )
            && document.inputs.extra_tracked_files.iter().all(|path| {
                super::lexical(&cwd.join(path))
                    .is_some_and(|path| copies.contains_key(&path) || names.contains(&path))
            })
            && document
                .inputs
                .tracked_environment
                .iter()
                .all(|(name, value)| environment.get(name) == value.as_ref()))
        .then(|| super::super::digest(&raw))
    }
}

/// Recheck the retained completion document backing a qualified producer.
/// Its source/graph admission happened while the successful child and private
/// copies were still owned; this only checks that certificate's integrity.
pub(super) fn intact(path: &Path, expected: &str, pid: u32, run: &str, key: &str) -> bool {
    let Some(file) = open(path) else {
        return false;
    };
    if file.metadata().ok().is_none_or(|meta| meta.len() > LIMIT) {
        return false;
    }
    let mut raw = Vec::new();
    if file.take(LIMIT + 1).read_to_end(&mut raw).is_err()
        || raw.len() as u64 > LIMIT
        || super::super::files::digest(&raw) != expected
    {
        return false;
    }
    let Ok(document) = serde_json::from_slice::<Document>(&raw) else {
        return false;
    };
    document.kind == "sail-native-compiler-read-inputs"
        && document.schema == 1
        && document.complete
        && pid != 0
        && document.pid == pid
        && document.binding == super::super::files::digest(format!("{run}\0{key}").as_bytes())
        && !document.inputs.sources.is_empty()
}
