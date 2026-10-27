//! Distinct actual native `SourceMap` evidence; never substitute a final file read.
//!
//! Native units always execute. This admission may establish provenance for a
//! later retained workspace proof only after successful complete child exit.

use super::super::dependency_snapshot::{Content, open};
use super::admission::Closure;
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

    pub(super) fn configure(
        &self,
        command: &mut impl crate::managed::command_environment::ConfigureCommand,
    ) {
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
            .map(|name| cwd.join(name))
            .collect();
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
                copies.contains_key(Path::new(path))
                        // include_str! appears in both inventories. Its
                        // retained SourceMap checksum passed admission above.
                        || document.inputs.sources.contains_key(path)
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
        || super::super::digest(&raw) != expected
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
        && document.binding == super::super::digest(format!("{run}\0{key}").as_bytes())
        && !document.inputs.sources.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "Sequential mutations preserve the same bound request and completion certificate."
    )]
    fn native_consumption_requires_bound_successful_complete_source_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root(
            std::env::temp_dir().join(format!("sail-native-read-admission-{}", std::process::id())),
        );
        std::fs::create_dir(&root.0)?;
        let (closure, path, checksum, size) = super::super::admission::native_test_fixture(&root.0)
            .ok_or("source authority fixture")?;
        let mut observation =
            Observation::begin(&root.0.join("read"), "run", "key").ok_or("request")?;
        observation.started(42);
        let name = path.to_str().ok_or("source path")?.to_owned();
        let mut document = Document {
            kind: "sail-native-compiler-read-inputs".into(),
            schema: 1,
            binding: observation.binding.clone(),
            pid: 42,
            complete: true,
            inputs: Inputs {
                sources: BTreeMap::from([(
                    name.clone(),
                    Source {
                        checksum: checksum.clone(),
                        size,
                    },
                )]),
                extra_tracked_files: Vec::new(),
                tracked_environment: BTreeMap::new(),
            },
        };
        let copies = BTreeMap::new();
        let environment = BTreeMap::new();
        let write = |document: &Document| -> std::io::Result<()> {
            std::fs::write(observation.path(), serde_json::to_vec(document)?)
        };
        write(&document)?;
        let accepted = observation
            .finish(true, &root.0, &closure, &copies, &environment)
            .ok_or("valid bound certificate")?;
        assert!(intact(observation.path(), &accepted, 42, "run", "key"));
        assert!(!intact(observation.path(), &accepted, 43, "run", "key"));
        assert!(!intact(
            observation.path(),
            &accepted,
            42,
            "other-run",
            "key"
        ));
        assert!(
            observation
                .finish(false, &root.0, &closure, &copies, &environment)
                .is_none()
        );
        document
            .inputs
            .sources
            .get_mut(&name)
            .ok_or("source")?
            .checksum = "blake3=foreign".into();
        write(&document)?;
        assert!(
            observation
                .finish(true, &root.0, &closure, &copies, &environment)
                .is_none()
        );
        document
            .inputs
            .sources
            .get_mut(&name)
            .ok_or("source")?
            .checksum = checksum;
        document.inputs.extra_tracked_files = vec!["/foreign/unobserved-bytes".into()];
        write(&document)?;
        assert!(
            observation
                .finish(true, &root.0, &closure, &copies, &environment)
                .is_none()
        );
        // A marked path already covered by the same admitted SourceMap record
        // needs no substituted end-of-run file hash (include_str! does this).
        document.inputs.extra_tracked_files = vec![name];
        write(&document)?;
        assert!(
            observation
                .finish(true, &root.0, &closure, &copies, &environment)
                .is_some()
        );
        document.pid = 43;
        write(&document)?;
        assert!(
            observation
                .finish(true, &root.0, &closure, &copies, &environment)
                .is_none()
        );
        document.pid = 42;
        document.complete = false;
        write(&document)?;
        assert!(
            observation
                .finish(true, &root.0, &closure, &copies, &environment)
                .is_none()
        );
        document.complete = true;
        document.kind = "sail-compiler-read-inputs".into();
        write(&document)?;
        assert!(
            observation
                .finish(true, &root.0, &closure, &copies, &environment)
                .is_none()
        );
        assert!(!intact(observation.path(), &accepted, 42, "run", "key"));
        Ok(())
    }
}
