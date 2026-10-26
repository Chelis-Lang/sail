//! Compiler-declared inputs, including explicit files beneath excluded directories.

use super::receipts::read;
use crate::Result;
use sail_acceptance::inputs::{ArtifactRead, seal_compiler_provider};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Private producer records share the artifact endpoint, so truncating a retry
/// also truncates its intents. They never enter Cargo's forwarded stdout.
const PROVIDER: &str = "sail-source-provider";
const OWNER: &str = "sail_source_provider";

pub(crate) fn write_provider(
    writer: &mut impl std::io::Write,
    event: &sail_acceptance::inputs::ProviderEvent,
) -> std::io::Result<()> {
    write_record(
        writer,
        &serde_json::json!({"reason":PROVIDER,"kind":event.kind,
        "producer":event.producer,"child":event.child,"frames":event.frames,"exited":event.exited}),
    )
}

pub(crate) fn write_artifact(
    writer: &mut impl std::io::Write,
    mut value: Value,
    producer: u32,
    child: u32,
) -> std::io::Result<()> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("artifact is not an object"))?;
    if object.contains_key(OWNER) {
        return Err(std::io::Error::other("reserved artifact owner"));
    }
    object.insert(
        OWNER.to_owned(),
        serde_json::json!({"producer":producer,"child":child}),
    );
    write_record(writer, &value)
}

fn write_record(writer: &mut impl std::io::Write, value: &Value) -> std::io::Result<()> {
    let mut raw = serde_json::to_vec(value)?;
    raw.push(b'\n');
    writer.write_all(&raw)
}

struct Artifact {
    producer: u32,
    child: u32,
    frame: usize,
    value: Value,
}

struct Read {
    producer: u32,
    child: u32,
    frame: usize,
    raw: String,
}

struct Journal {
    sessions: BTreeMap<u32, Vec<sail_acceptance::inputs::ProviderEvent>>,
    artifacts: Vec<Artifact>,
}

/// Classify only well-formed private non-unit control records. Ordinary unit
/// records still require each consumer's original package/scope checks.
pub(super) fn provider_metadata(value: &Value) -> Option<bool> {
    if value["reason"] != PROVIDER {
        return Some(false);
    }
    provider_event(value).map(|_| true)
}

fn provider_event(value: &Value) -> Option<sail_acceptance::inputs::ProviderEvent> {
    use sail_acceptance::inputs::ProviderEvent;
    if value["reason"] != PROVIDER || value.as_object()?.len() != 6 {
        return None;
    }
    let event = ProviderEvent {
        kind: value["kind"].as_u64()?.try_into().ok()?,
        producer: value["producer"].as_u64()?.try_into().ok()?,
        child: value["child"].as_u64()?.try_into().ok()?,
        frames: value["frames"].as_u64()?.try_into().ok()?,
        exited: value["exited"].as_bool()?,
    };
    let valid = event.producer != 0
        && match event.kind {
            0 => event.child == 0 && event.frames == 0 && !event.exited,
            1 => event.child != 0 && event.frames == 0 && !event.exited,
            3 => event.child != 0,
            _ => false,
        };
    valid.then_some(event)
}

fn journal(raw: &str) -> Option<Journal> {
    use sail_acceptance::inputs::ProviderEvent;
    if !raw.ends_with('\n') {
        return None;
    }
    let mut sessions: BTreeMap<u32, Vec<ProviderEvent>> = BTreeMap::new();
    let mut artifacts = Vec::new();
    for line in raw.lines() {
        let value: Value = serde_json::from_str(line).ok()?;
        if value["reason"] == PROVIDER {
            let event = provider_event(&value)?;
            if event.kind == 0 {
                if sessions.contains_key(&event.producer) {
                    return None;
                }
                sessions.insert(event.producer, vec![event]);
            } else {
                sessions.get_mut(&event.producer)?.push(event);
            }
        } else if matches!(
            value["reason"].as_str(),
            Some("compiler-artifact" | "build-script-executed")
        ) {
            let producer = value[OWNER]["producer"].as_u64()?.try_into().ok()?;
            let child = value[OWNER]["child"].as_u64()?.try_into().ok()?;
            let events = sessions.get_mut(&producer)?;
            let frame = events.len();
            let needs_read = value["reason"] == "compiler-artifact" && !custom_build(&value);
            events.push(ProviderEvent {
                kind: if needs_read { 2 } else { 4 },
                producer,
                child,
                frames: 0,
                exited: false,
            });
            if needs_read {
                artifacts.push(Artifact {
                    producer,
                    child,
                    frame,
                    value,
                });
            }
        } else {
            return None;
        }
    }
    if sessions.is_empty() {
        return None;
    }
    Some(Journal {
        sessions,
        artifacts,
    })
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Inputs {
    /// Version two preserves the compiler's lexical source aliases.
    #[serde(default)]
    pub(super) version: u32,
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
        |v| {
            (
                v.paths,
                v.version == 2 && v.covered && v.by_package.is_some(),
            )
        },
    )
}

pub(super) fn packages(path: &Path) -> Option<BTreeMap<String, Vec<PathBuf>>> {
    read::<Inputs>(path)
        .filter(|v| v.version == 2)
        .and_then(|v| v.by_package)
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
    let (previous, previous_covered) = load(path);
    let mut paths: BTreeSet<PathBuf> = previous.into_iter().collect();
    let mut by_package: BTreeMap<String, BTreeSet<PathBuf>> = packages(path)
        .unwrap_or_default()
        .into_iter()
        .map(|(package, paths)| (package, paths.into_iter().collect()))
        .collect();
    let observed = std::fs::read_to_string(artifacts)
        .ok()
        .and_then(|raw| journal(&raw));
    let mut covered = previous_covered && observed.is_some();
    let mut reads = Vec::new();
    for descriptor in observed
        .as_ref()
        .into_iter()
        .flat_map(|journal| &journal.artifacts)
    {
        let artifact = &descriptor.value;
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
        // The read owns its descriptor. No positional join can attach a
        // different artifact's outcome when these reads are parallelized.
        let read = dep_info.and_then(|path| {
            let raw = std::fs::read_to_string(path).ok()?;
            let inputs = dependencies(&raw)?;
            (!inputs.is_empty()).then_some((
                Read {
                    producer: descriptor.producer,
                    child: descriptor.child,
                    frame: descriptor.frame,
                    raw,
                },
                inputs,
            ))
        });
        let Some((read, inputs)) = read else {
            covered = false;
            continue;
        };
        reads.push(read);
        for input in inputs {
            let declared = workspace.join(input);
            // Retain the compiler's path through any symlink, as well as the
            // resolved content. Canonical bytes alone lose source selection.
            let resolved = declared.canonicalize().unwrap_or_else(|_| declared.clone());
            by_package
                .entry(package.to_owned())
                .or_default()
                .extend([declared.clone(), resolved.clone()]);
            paths.insert(declared);
            paths.insert(resolved);
        }
    }
    covered &= observed
        .as_ref()
        .is_some_and(|journal| ledger_complete(journal, &reads));
    let paths: Vec<PathBuf> = paths.into_iter().collect();
    super::files::json(
        path,
        &Inputs {
            version: 2,
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

fn custom_build(value: &Value) -> bool {
    value["target"]["kind"]
        .as_array()
        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "custom-build"))
}

fn ledger_complete(journal: &Journal, reads: &[Read]) -> bool {
    !journal.sessions.is_empty()
        && reads
            .iter()
            .all(|read| journal.sessions.contains_key(&read.producer))
        && journal.sessions.iter().all(|(&producer, events)| {
            let mut actual: Vec<_> = reads
                .iter()
                .filter(|read| read.producer == producer)
                .map(|read| ArtifactRead {
                    producer: read.producer,
                    child: read.child,
                    frame: read.frame,
                    raw: Some(read.raw.as_bytes()),
                })
                .collect();
            // A candidate sorter may reorder results. The checked consumer
            // requires exact actual frame identities and successful outcomes.
            actual.sort_unstable_by_key(|read| read.frame);
            seal_compiler_provider(events, &actual).is_some()
        })
}

/// A complete producer fixture; production uses actual spawn/read/wait events.
#[cfg(test)]
pub(super) fn fixture_journal(artifacts: &[Value]) -> std::io::Result<Vec<u8>> {
    use sail_acceptance::inputs::ProviderEvent;
    let mut raw = Vec::new();
    for (kind, child, frames, exited) in [(0, 0, 0, false), (1, 8, 0, false)] {
        write_provider(
            &mut raw,
            &ProviderEvent {
                kind,
                producer: 7,
                child,
                frames,
                exited,
            },
        )?;
    }
    for artifact in artifacts {
        write_artifact(&mut raw, artifact.clone(), 7, 8)?;
    }
    write_provider(
        &mut raw,
        &ProviderEvent {
            kind: 3,
            producer: 7,
            child: 8,
            frames: artifacts.len(),
            exited: true,
        },
    )?;
    Ok(raw)
}

#[cfg(test)]
pub(crate) fn fixture_complete(raw: &[u8]) -> bool {
    std::str::from_utf8(raw)
        .ok()
        .and_then(journal)
        .is_some_and(|journal| ledger_complete(&journal, &[]))
}

#[cfg(test)]
mod tests {
    use sail_acceptance::inputs::ProviderEvent;

    #[test]
    fn metadata_classification_refuses_malformed_and_unknown_controls()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let raw = fixture_journal(&[])?;
        let mut controls: Vec<Value> = std::str::from_utf8(&raw)?
            .lines()
            .map(serde_json::from_str)
            .collect::<std::result::Result<_, _>>()?;
        for control in &controls {
            assert_eq!(provider_metadata(control), Some(true));
        }
        controls[0]["package_id"] = "in-scope".into();
        assert_eq!(provider_metadata(&controls[0]), None);
        for mutation in [
            serde_json::json!({"reason":PROVIDER}),
            serde_json::json!({"reason":PROVIDER,"kind":4,"producer":7,"child":8,"frames":0,"exited":false}),
            serde_json::json!({"reason":PROVIDER,"kind":0,"producer":0,"child":0,"frames":0,"exited":false}),
        ] {
            assert_eq!(provider_metadata(&mutation), None);
        }
        assert_eq!(
            provider_metadata(&serde_json::json!({"unknown":"record"})),
            Some(false)
        );
        Ok(())
    }

    #[test]
    fn missing_malformed_or_truncated_journal_is_not_a_completed_empty_child()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-journal-refusal-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let artifacts = root.join("artifacts.jsonl");
        let data = serde_json::json!({"workspace_root":root});
        for (index, raw) in [
            None,
            Some(b"".as_slice()),
            Some(b"{not-json\n".as_slice()),
            Some(b"{\"reason\":\"compiler-artifact\"}\n".as_slice()),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(raw) = raw {
                std::fs::write(&artifacts, raw)?;
            }
            let (_, covered) =
                update(&root.join(format!("state-{index}.json")), &artifacts, &data)?;
            assert!(!covered, "missing/malformed journal index {index}");
        }
        let completed = fixture_journal(&[])?;
        assert!(fixture_complete(&completed));
        std::fs::write(&artifacts, &completed)?;
        assert!(update(&root.join("complete.json"), &artifacts, &data)?.1);
        std::fs::write(&artifacts, &completed[..completed.len() - 1])?;
        assert!(!update(&root.join("truncated.json"), &artifacts, &data)?.1);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn provider_read_ledger_refuses_missing_duplicate_and_foreign_results()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let artifact = serde_json::json!({"reason":"compiler-artifact","package_id":"demo","target":{"kind":["lib"]}});
        let raw = String::from_utf8(fixture_journal(&[artifact.clone(), artifact])?)?;
        let journal = journal(&raw).ok_or("journal")?;
        let read = |frame| Read {
            producer: 7,
            child: 8,
            frame,
            raw: "artifact: source.rs".to_owned(),
        };
        assert!(ledger_complete(&journal, &[read(3), read(2)]));
        assert!(!ledger_complete(&journal, &[read(2)]));
        assert!(!ledger_complete(&journal, &[read(2), read(2)]));
        assert!(!ledger_complete(&journal, &[read(2), read(4)]));
        assert!(!ledger_complete(
            &journal,
            &[
                read(2),
                Read {
                    producer: 9,
                    ..read(3)
                }
            ]
        ));
        assert!(!ledger_complete(
            &journal,
            &[
                read(2),
                Read {
                    child: 9,
                    ..read(3)
                }
            ]
        ));
        Ok(())
    }

    #[test]
    fn append_and_interleaving_preserve_independent_child_intents()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut raw = Vec::new();
        for (kind, producer, child, frames) in [
            (0, 7, 0, 0),
            (0, 9, 0, 0),
            (1, 9, 10, 0),
            (1, 7, 8, 0),
            (3, 9, 10, 0),
            (3, 7, 8, 0),
        ] {
            write_provider(
                &mut raw,
                &ProviderEvent {
                    kind,
                    producer,
                    child,
                    frames,
                    exited: kind == 3,
                },
            )?;
        }
        assert!(fixture_complete(&raw));
        // A third required child cannot disappear merely because the first two
        // finished, even when it emitted no artifacts.
        write_provider(
            &mut raw,
            &ProviderEvent {
                kind: 0,
                producer: 11,
                child: 0,
                frames: 0,
                exited: false,
            },
        )?;
        assert!(!fixture_complete(&raw));
        let mut duplicate = fixture_journal(&[])?;
        duplicate.extend(fixture_journal(&[])?);
        assert!(!fixture_complete(&duplicate));
        Ok(())
    }

    #[test]
    fn terminal_count_pid_and_read_failure_refuse_coverage()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let raw = fixture_journal(&[])?;
        for (field, wrong) in [
            ("frames", serde_json::json!(1)),
            ("child", serde_json::json!(9)),
            ("producer", serde_json::json!(9)),
            ("exited", serde_json::json!(false)),
        ] {
            let mut lines: Vec<Value> = std::str::from_utf8(&raw)?
                .lines()
                .map(serde_json::from_str)
                .collect::<std::result::Result<_, _>>()?;
            lines.last_mut().ok_or("terminal")?[field] = wrong;
            let mutated = lines
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            assert!(!fixture_complete(mutated.as_bytes()), "guard {field}");
        }
        Ok(())
    }

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
        assert!(
            std::fs::write(&artifacts, fixture_journal(&[artifact]).unwrap_or_default()).is_ok()
        );
        let state = root.join("sources.json");
        let mut data = serde_json::json!({"workspace_root":project,"packages":[{"manifest_path":project.join("Cargo.toml")}]});
        let baseline = super::super::input::snapshot_metadata(&data, &[]);
        let (paths, covered) =
            update(&state, &artifacts, &data).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(covered);
        assert_eq!(
            paths.iter().cloned().collect::<BTreeSet<_>>(),
            BTreeSet::from([
                project.join("../shared file.rs"),
                external.canonicalize().unwrap_or_default(),
            ])
        );
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
        assert!(std::fs::write(&path, r#"{"paths":[],"covered":true,"by_package":{}}"#).is_ok());
        assert!(
            !load(&path).1,
            "legacy scoped records omitted source aliases too"
        );
        assert!(packages(&path).is_none());
        assert!(std::fs::write(&path, "corrupt source observation").is_ok());
        assert!(!load(&path).1);
        assert!(std::fs::remove_file(path).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn compiler_source_alias_and_destination_are_both_persisted() -> std::io::Result<()> {
        let root = std::env::temp_dir()
            .canonicalize()?
            .join(format!("sail-dep-alias-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("ws");
        let target = root.join("target");
        std::fs::create_dir_all(&workspace)?;
        std::fs::create_dir_all(&target)?;
        let destination = root.join("source.rs");
        std::fs::write(&destination, "pub fn f() {}")?;
        let alias = workspace.join("lib.rs");
        std::os::unix::fs::symlink(&destination, &alias)?;
        std::fs::write(target.join("demo.d"), "artifact: lib.rs\n")?;
        let artifacts = root.join("artifacts.jsonl");
        std::fs::write(
            &artifacts,
            fixture_journal(&[serde_json::json!({"reason":"compiler-artifact",
            "package_id":"demo","target":{"kind":["lib"]},
            "filenames":[target.join("libdemo.rmeta")]})])?,
        )?;
        let state = root.join("sources.json");
        let data = serde_json::json!({"workspace_root":workspace});
        let (paths, covered) =
            update(&state, &artifacts, &data).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(covered);
        assert_eq!(
            paths.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([alias, destination])
        );
        assert!(load(&state).1);
        std::fs::remove_dir_all(root)
    }
}
