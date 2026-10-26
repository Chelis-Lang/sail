//! Read the experimental fork's distinct verification and compilation inputs.

use super::Call;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;

const LIMIT: u64 = 8 * 1024 * 1024;
const DIRECTORY: &str = "SAIL_COMPILER_INPUTS";
const BINDING: &str = "SAIL_COMPILER_INPUTS_BINDING";

#[cfg(unix)]
fn open(path: &std::path::Path) -> Option<std::fs::File> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    let file = std::fs::File::from(fd);
    file.metadata().ok()?.is_file().then_some(file)
}

#[cfg(not(unix))]
fn open(_: &std::path::Path) -> Option<std::fs::File> {
    None
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
    kind: String,
    schema: u32,
    binding: String,
    pid: u32,
    complete: bool,
    phases: BTreeMap<String, Phase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Phase {
    sources: BTreeMap<String, Source>,
    extra_tracked_files: Vec<String>,
    tracked_environment: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Source {
    checksum: String,
    size: u64,
}

/// A fresh private request around this exact real compiler child.
pub(super) struct Observation {
    directory: PathBuf,
    binding: String,
    pid: Option<u32>,
}

fn binding(key: &str, run: &str) -> String {
    super::digest(format!("{run}\0{key}").as_bytes())
}

pub(super) fn publication(
    call: &Call<'_>,
    declared: &mut BTreeMap<String, Option<String>>,
) -> Result<Option<Document>, String> {
    if call.plan.closure.is_none() {
        return Ok(None);
    }
    if !call.adapter_intact() {
        return Err("configured solver adapter changed before publication".to_owned());
    }
    let reads = call
        .compiler_reads
        .as_ref()
        .filter(|reads| reads.valid(call, &call.plan.run_id))
        .ok_or("complete compiler-read observations unavailable")?;
    let environment = call
        .mapped_environment(
            &reads.environment().ok_or("compiler environment changed")?,
            false,
        )
        .ok_or("ambiguous compiler environment names")?;
    for (name, value) in environment {
        if declared
            .insert(name, value.clone())
            .is_some_and(|old| old != value)
        {
            return Err("compiler phases disagree on declared environment".to_owned());
        }
    }
    Ok(Some(
        reads
            .mapped(call, false)
            .ok_or("ambiguous compiler input paths")?,
    ))
}

#[cfg(test)]
pub(super) fn stored(call: &Call<'_>, unit: &super::Unit) -> bool {
    stored_for(call, unit, &call.key, None)
}

/// Whether the unit stored under `key` holds valid compiler reads, with one variable read as
/// `recorded` (`(name, value)`): a unit served under a raised rlimit was
/// compiled without it (ADR 0060).
pub(super) fn stored_for(
    call: &Call<'_>,
    unit: &super::Unit,
    key: &str,
    recorded: Option<(&str, &str)>,
) -> bool {
    call.adapter_intact()
        && (call.plan.closure.is_none()
            || unit
                .compiler_reads
                .as_ref()
                .and_then(|reads| reads.mapped(call, true))
                .is_some_and(|reads| reads.valid_for(call, key, &unit.establishing_run, recorded)))
}

impl Observation {
    pub(super) fn begin(call: &Call<'_>) -> Option<Self> {
        call.plan.closure.as_ref()?;
        let parent = call
            .plan
            .target
            .join(".sail-boundary-inputs")
            .join(&call.plan.run_id);
        std::fs::create_dir_all(&parent).ok()?;
        let directory = parent.join(format!("{}-{}", call.key, std::process::id()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(&directory).ok()?;
        Some(Self {
            directory,
            binding: binding(&call.key, &call.plan.run_id),
            pid: None,
        })
    }

    pub(super) fn configure(&self, command: &mut std::process::Command) {
        command
            .env(DIRECTORY, &self.directory)
            .env(BINDING, &self.binding);
    }

    pub(super) fn started(&mut self, pid: u32) {
        self.pid = Some(pid);
    }

    pub(super) fn read(&self, call: &Call<'_>) -> Option<Document> {
        let entries = std::fs::read_dir(&self.directory).ok()?;
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
        let file = open(&self.directory.join("inputs.json"))?;
        if file.metadata().ok()?.len() > LIMIT {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(LIMIT + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > LIMIT {
            return None;
        }
        let mut document: Document = serde_json::from_slice(&bytes).ok()?;
        for phase in document.phases.values_mut() {
            crate::adapter::verus_channel::normalize(&mut phase.tracked_environment);
        }
        (Some(document.pid) == self.pid
            && document.binding == self.binding
            && document.valid(call, &call.plan.run_id))
        .then_some(document)
    }
}

impl Document {
    fn header(&self, key: &str, establishing_run: &str) -> bool {
        self.kind == "sail-compiler-read-inputs"
            && self.schema == 1
            && self.complete
            && self.pid != 0
            && self.binding == binding(key, establishing_run)
            && self.phases.len() == 2
            && self.phases.contains_key("verification")
            && self.phases.contains_key("compilation")
    }

    pub(super) fn valid(&self, call: &Call<'_>, establishing_run: &str) -> bool {
        self.valid_for(call, &call.key, establishing_run, None)
    }

    fn valid_for(
        &self,
        call: &Call<'_>,
        key: &str,
        establishing_run: &str,
        recorded: Option<(&str, &str)>,
    ) -> bool {
        if !self.header(key, establishing_run) {
            return false;
        }
        let Some(closure) = &call.plan.closure else {
            return false;
        };
        self.phases.values().all(|phase| {
            phase.extra_tracked_files.is_empty()
                && closure.loaded_sources(
                    phase.sources.iter().map(|(path, source)| {
                        (path.as_str(), source.checksum.as_str(), source.size)
                    }),
                )
                && phase.tracked_environment.iter().all(|(name, value)| {
                    let current = match recorded {
                        Some((recorded_name, recorded)) if recorded_name == name => Some(recorded),
                        _ => call.vars.get(name).map(String::as_str),
                    };
                    current == value.as_deref()
                })
        })
    }

    /// Bind the union of environment observations from both actual phases.
    pub(super) fn environment(&self) -> Option<BTreeMap<String, Option<String>>> {
        let mut result = BTreeMap::new();
        for phase in self.phases.values() {
            for (name, value) in &phase.tracked_environment {
                if result
                    .insert(name.clone(), value.clone())
                    .is_some_and(|old| old != *value)
                {
                    return None;
                }
            }
        }
        Some(result)
    }

    /// Apply the same explicit path mapping as the containing vstd unit.
    pub(super) fn mapped(&self, call: &Call<'_>, localize: bool) -> Option<Self> {
        let normalizer = &call.normalizer;
        let map = |text: &str| {
            if localize {
                normalizer.localize(text)
            } else {
                normalizer.normalize(text)
            }
        };
        let mut document = self.clone();
        for phase in document.phases.values_mut() {
            let mut sources = BTreeMap::new();
            for (name, source) in &phase.sources {
                if sources.insert(map(name), source.clone()).is_some() {
                    return None;
                }
            }
            phase.sources = sources;
            phase.extra_tracked_files = phase
                .extra_tracked_files
                .iter()
                .map(|name| map(name))
                .collect();
            phase.tracked_environment =
                call.mapped_environment(&phase.tracked_environment, localize)?;
        }
        Some(document)
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, binding};

    #[test]
    fn compiler_record_header_binds_both_phases_and_the_establishing_invocation()
    -> Result<(), Box<dyn std::error::Error>> {
        let phase = serde_json::json!({
            "sources":{},"extra_tracked_files":[],"tracked_environment":{}
        });
        let raw = serde_json::json!({
            "kind":"sail-compiler-read-inputs","schema":1,"complete":true,
            "pid":123,"binding":binding("key", "run"),
            "phases":{"verification":phase,"compilation":phase},
        });
        let document: Document = serde_json::from_value(raw.clone())?;
        assert!(document.header("key", "run"));
        assert!(!document.header("key", "different-run"));
        assert!(!document.header("different-key", "run"));
        for (name, value) in [
            ("kind", serde_json::json!("another-kind")),
            ("schema", serde_json::json!(0)),
            ("complete", serde_json::json!(false)),
            ("pid", serde_json::json!(0)),
            ("phases", serde_json::json!({"verification":phase})),
        ] {
            let mut changed = raw.clone();
            changed[name] = value;
            let changed: Document = serde_json::from_value(changed)?;
            assert!(!changed.header("key", "run"), "{name}");
        }
        let mut unknown = raw.clone();
        unknown["ignored"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Document>(unknown).is_err());
        let mut phases = raw;
        phases["phases"]["compilation"]["ignored"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Document>(phases).is_err());
        Ok(())
    }

    #[test]
    fn compiler_phase_environment_union_refuses_conflicting_reads()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut document: Document = serde_json::from_value(serde_json::json!({
            "kind":"sail-compiler-read-inputs","schema":1,"complete":true,
            "pid":123,"binding":binding("key", "run"),
            "phases": {
                "verification":{"sources":{},"extra_tracked_files":[],
                    "tracked_environment":{"GHOST_ONLY":"bound","BOTH":null}},
                "compilation":{"sources":{},"extra_tracked_files":[],
                    "tracked_environment":{"EXEC_ONLY":"also-bound","BOTH":null}},
            }
        }))?;
        let variables = document.environment().ok_or("union refused")?;
        assert_eq!(variables.len(), 3);
        assert_eq!(variables["GHOST_ONLY"].as_deref(), Some("bound"));
        document
            .phases
            .get_mut("compilation")
            .ok_or("phase")?
            .tracked_environment
            .insert("BOTH".into(), Some("changed".into()));
        assert!(document.environment().is_none());
        Ok(())
    }
}
