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
    if matches!(
        value["reason"].as_str(),
        Some(crate::adapter::JOURNAL_BEGIN | crate::adapter::JOURNAL_END)
    ) {
        return (value.as_object()?.len() == 1).then_some(true);
    }
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
    for section in completed_sections(raw)? {
        let mut owners = BTreeSet::new();
        for value in section {
            if value["reason"] == PROVIDER {
                let event = provider_event(&value)?;
                if event.kind == 0 {
                    if sessions.contains_key(&event.producer) {
                        return None;
                    }
                    owners.insert(event.producer);
                    sessions.insert(event.producer, vec![event]);
                } else {
                    if !owners.contains(&event.producer) {
                        return None;
                    }
                    sessions.get_mut(&event.producer)?.push(event);
                }
            } else if matches!(
                value["reason"].as_str(),
                Some("compiler-artifact" | "build-script-executed")
            ) {
                let producer = value[OWNER]["producer"].as_u64()?.try_into().ok()?;
                let child = value[OWNER]["child"].as_u64()?.try_into().ok()?;
                if !owners.contains(&producer) {
                    return None;
                }
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
        if owners.is_empty() {
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

/// Private declaration-record revision shared with bounded inventory transfer.
pub(super) const VERSION: u32 = 4;

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Inputs {
    /// Version four combines exact typed producer/declaration journals with
    /// per-target inventories and removal-aware paths. Earlier records keep
    /// their paths, but cannot establish complete current coverage.
    #[serde(default)]
    pub(super) version: u32,
    pub(super) paths: Vec<PathBuf>,
    pub(super) covered: bool,
    #[serde(default)]
    pub(super) by_package: Option<BTreeMap<String, Vec<PathBuf>>>,
    /// The declarations of each compiled unit, by [`target_key`]. Each run
    /// replaces the entries of the units it reports, so a unit's entry is
    /// exactly its latest dep-info. Older records have none; their targets
    /// keep whole-package revisions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) by_target: Option<BTreeMap<String, Vec<PathBuf>>>,
}

/// The inventory key of one Cargo target: its package id, exact name and
/// kinds, as Cargo's unit records and proof receipts name them.
pub(super) fn target_key(package_id: &str, name: &str, kinds: &[String]) -> String {
    serde_json::to_string(&(package_id, name, kinds)).unwrap_or_default()
}

/// The recorded compiler-declared inputs and whether they are complete.
///
/// A missing record counts as complete only for a proof context's first
/// observation (`observed_before` false): its run's own inputs are then
/// recorded and compared after the run. Once a context has run, a missing,
/// unreadable or malformed record is incomplete.
pub(super) fn load(path: &Path, observed_before: bool) -> (Vec<PathBuf>, bool) {
    // Older observations either lack consuming-package attribution or used a
    // filename decoder that could omit literal tabs, backslashes and Unicode.
    // Preserve their paths while withholding their claimed declaration coverage.
    read::<Inputs>(path).map_or_else(
        || {
            let first = !observed_before
                && matches!(
                    std::fs::symlink_metadata(path),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound
                );
            (Vec::new(), first)
        },
        |v| {
            (
                present(v.paths),
                v.version == VERSION && v.covered && v.by_package.is_some(),
            )
        },
    )
}

pub(super) fn packages(path: &Path) -> Option<BTreeMap<String, Vec<PathBuf>>> {
    read::<Inputs>(path)
        .filter(|v| v.version == VERSION)
        .and_then(|v| v.by_package)
        .map(present_entries)
}

/// The recorded declarations of each unit ([`Inputs::by_target`]).
pub(super) fn targets(path: &Path) -> Option<BTreeMap<String, Vec<PathBuf>>> {
    read::<Inputs>(path)
        .filter(|v| v.version == VERSION)
        .and_then(|v| v.by_target)
        .map(present_entries)
}

fn present_entries(entries: BTreeMap<String, Vec<PathBuf>>) -> BTreeMap<String, Vec<PathBuf>> {
    entries
        .into_iter()
        .map(|(key, paths)| (key, present(paths)))
        .collect()
}

/// The recorded declarations that still name a directory entry. A unit whose
/// dep-info names a file that no longer exists is stale to Cargo, which
/// compiles it again and records its new declarations. Its old input is not
/// an input of anything this run can reuse: snapshots leave it out, so its
/// removal changes the revision, and a file created again under that name
/// enters the next snapshot (or the after-run snapshot) by its content.
/// Keeping it made every later snapshot incomplete, so every later run was
/// forced and cleaned every build script (#401). Only a definite absence of
/// the path itself drops it; any other lookup failure keeps it, and the
/// snapshot then reports the observation incomplete as before.
fn present(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|path| {
            !matches!(
                std::fs::symlink_metadata(path),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound
            )
        })
        .collect()
}

pub(super) fn dependencies(text: &str) -> Option<Vec<String>> {
    // Rustc escapes ASCII spaces only. Other backslashes and whitespace are
    // literal filename bytes, not generic make escapes or token separators.
    // The first logical rule remains the declaration source; phony targets and
    // environment comments are separate records, not additional input tokens.
    let joined = text.replace("\\\n", "");
    let mut lines = joined.lines();
    let rule = lines.next()?.split_once(": ")?.1;
    let mut paths = Vec::new();
    let mut word = String::new();
    let mut start = 0;
    let mut chars = rule.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some((_, ' ')) => {
                    chars.next();
                    word.push(' ');
                }
                Some(_) => word.push(c),
                None => return None,
            }
        } else if c == ' ' {
            if !word.is_empty() {
                if !sail_acceptance::inputs::dependency_filename_matches(
                    &rule.as_bytes()[start..index],
                    word.as_bytes(),
                ) {
                    return None;
                }
                paths.push(std::mem::take(&mut word));
            }
            start = index + 1;
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        if !sail_acceptance::inputs::dependency_filename_matches(
            &rule.as_bytes()[start..],
            word.as_bytes(),
        ) {
            return None;
        }
        paths.push(word);
    }
    // Refuse unsupported multiline records rather than silently accepting only
    // a prefix of one filename. This is a conservative format check, not a
    // proof that rustc's line-oriented format represents every possible path.
    for line in lines {
        if line.is_empty() || line.starts_with("# env-dep:") || line.starts_with("# checksum:") {
            continue;
        }
        if let Some((_, body)) = line.split_once(": ") {
            if body != rule {
                return None;
            }
        } else {
            let encoded = line.strip_suffix(':')?;
            if !paths.iter().any(|path| {
                sail_acceptance::inputs::dependency_filename_matches(
                    encoded.as_bytes(),
                    path.as_bytes(),
                )
            }) {
                return None;
            }
        }
    }
    Some(paths)
}

/// Original unit records with closed outer sections and a complete typed
/// lifecycle for every recorded producer. This establishes attribution only;
/// no dep-info read is invented and no compiler-input closure is asserted.
pub(super) fn provider_records(artifacts: &Path) -> Option<Vec<Value>> {
    let raw = std::fs::read_to_string(artifacts).ok()?;
    let observed = journal(&raw)?;
    if !observed
        .sessions
        .values()
        .all(|events| sail_acceptance::inputs::seal_provider(events).is_some())
    {
        return None;
    }
    Some(
        completed_sections(&raw)?
            .into_iter()
            .flatten()
            .filter(|record| {
                matches!(
                    record["reason"].as_str(),
                    Some("compiler-artifact" | "build-script-executed")
                )
            })
            .collect(),
    )
}

/// The unit records of a complete artifact journal, without its section
/// markers: every line parses, and every record lies in a section the
/// producer opened and closed (see [`crate::adapter::JOURNAL_END`]). `None`
/// for a missing, unreadable, malformed, truncated or unclosed journal.
#[cfg(test)]
pub(super) fn completed_journal(artifacts: &Path) -> Option<Vec<Value>> {
    let text = std::fs::read_to_string(artifacts).ok()?;
    completed_sections(&text).map(|sections| sections.into_iter().flatten().collect())
}

fn completed_sections(text: &str) -> Option<Vec<Vec<Value>>> {
    use crate::adapter::{JOURNAL_BEGIN, JOURNAL_END};
    let mut open = false;
    let mut sections = Vec::new();
    let mut records = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let record: Value = serde_json::from_str(line).ok()?;
        match record["reason"].as_str() {
            Some(JOURNAL_BEGIN) if !open && record.as_object()?.len() == 1 => open = true,
            Some(JOURNAL_END) if open && record.as_object()?.len() == 1 => {
                open = false;
                sections.push(std::mem::take(&mut records));
            }
            Some(JOURNAL_BEGIN | JOURNAL_END) => return None,
            _ if open => records.push(record),
            _ => return None,
        }
    }
    (!open && !sections.is_empty() && text.ends_with('\n')).then_some(sections)
}

#[cfg(test)]
pub(super) fn update(path: &Path, artifacts: &Path, data: &Value) -> Result<(Vec<PathBuf>, bool)> {
    update_with_history(path, artifacts, data, false, true)
}

pub(super) fn update_with_history(
    path: &Path,
    artifacts: &Path,
    data: &Value,
    observed_before: bool,
    prior_covered: bool,
) -> Result<(Vec<PathBuf>, bool)> {
    let workspace = Path::new(data["workspace_root"].as_str().unwrap_or_default());
    let (previous, previous_covered) = load(path, observed_before);
    let mut paths: BTreeSet<PathBuf> = previous.into_iter().collect();
    let mut by_package: BTreeMap<String, BTreeSet<PathBuf>> = packages(path)
        .unwrap_or_default()
        .into_iter()
        .map(|(package, paths)| (package, paths.into_iter().collect()))
        .collect();
    let mut by_target: BTreeMap<String, Vec<PathBuf>> = targets(path).unwrap_or_default();
    let mut reported: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();
    let observed = std::fs::read_to_string(artifacts)
        .ok()
        .and_then(|raw| journal(&raw));
    let mut covered = prior_covered && previous_covered && observed.is_some();
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
        let kinds: Vec<String> = artifact["target"]["kind"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let unit = reported
            .entry(target_key(
                package,
                artifact["target"]["name"].as_str().unwrap_or_default(),
                &kinds,
            ))
            .or_default();
        for input in inputs {
            let declared = workspace.join(input);
            // Retain the compiler's path through any symlink, as well as the
            // resolved content. Canonical bytes alone lose source selection.
            let resolved = declared.canonicalize().unwrap_or_else(|_| declared.clone());
            by_package
                .entry(package.to_owned())
                .or_default()
                .extend([declared.clone(), resolved.clone()]);
            unit.extend([declared.clone(), resolved.clone()]);
            paths.insert(declared);
            paths.insert(resolved);
        }
    }
    covered &= observed
        .as_ref()
        .is_some_and(|journal| ledger_complete(journal, &reads));
    let paths: Vec<PathBuf> = paths.into_iter().collect();
    // A unit this run reported replaces its earlier declarations; units it
    // did not report (another selection's) keep theirs.
    by_target.extend(
        reported
            .into_iter()
            .map(|(key, paths)| (key, paths.into_iter().collect())),
    );
    super::files::json(
        path,
        &Inputs {
            version: VERSION,
            paths: paths.clone(),
            covered,
            by_package: Some(
                by_package
                    .into_iter()
                    .map(|(package, paths)| (package, paths.into_iter().collect()))
                    .collect(),
            ),
            by_target: Some(by_target),
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
    fixture_journal_for(artifacts, 7, 8)
}

#[cfg(test)]
fn fixture_journal_for(artifacts: &[Value], producer: u32, child: u32) -> std::io::Result<Vec<u8>> {
    use sail_acceptance::inputs::ProviderEvent;
    let mut raw = (serde_json::json!({"reason":crate::adapter::JOURNAL_BEGIN}).to_string() + "\n")
        .into_bytes();
    for (kind, child, frames, exited) in [(0, 0, 0, false), (1, child, 0, false)] {
        write_provider(
            &mut raw,
            &ProviderEvent {
                kind,
                producer,
                child,
                frames,
                exited,
            },
        )?;
    }
    for artifact in artifacts {
        write_artifact(&mut raw, artifact.clone(), producer, child)?;
    }
    write_provider(
        &mut raw,
        &ProviderEvent {
            kind: 3,
            producer,
            child,
            frames: artifacts.len(),
            exited: true,
        },
    )?;
    raw.extend_from_slice(
        (serde_json::json!({"reason":crate::adapter::JOURNAL_END}).to_string() + "\n").as_bytes(),
    );
    Ok(raw)
}

#[cfg(test)]
pub(crate) fn fixture_provider_coverage(root: &Path, raw: &[u8]) -> crate::Result<bool> {
    let artifacts = root.join("artifacts.jsonl");
    std::fs::write(&artifacts, raw).map_err(|e| crate::Error::new("SAIL3115", e.to_string()))?;
    update(
        &root.join("sources.json"),
        &artifacts,
        &serde_json::json!({"workspace_root":root}),
    )
    .map(|(_, covered)| covered)
}

#[cfg(test)]
pub(crate) fn fixture_complete(raw: &[u8]) -> bool {
    std::str::from_utf8(raw)
        .ok()
        .and_then(journal)
        .is_some_and(|journal| ledger_complete(&journal, &[]))
}

#[cfg(test)]
fn closed_journal(records: &[Value]) -> String {
    String::from_utf8(
        fixture_journal(records).unwrap_or_else(|e| unreachable!("journal fixture: {e}")),
    )
    .unwrap_or_else(|e| unreachable!("journal UTF-8 fixture: {e}"))
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
    fn each_outer_section_requires_its_own_complete_typed_producer()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let begin = (serde_json::json!({"reason":crate::adapter::JOURNAL_BEGIN}).to_string()
            + "\n")
            .into_bytes();
        let end = (serde_json::json!({"reason":crate::adapter::JOURNAL_END}).to_string() + "\n")
            .into_bytes();
        let complete = fixture_journal(&[])?;
        assert!(fixture_complete(&complete));
        let mut extra_empty = complete.clone();
        extra_empty.extend_from_slice(&begin);
        extra_empty.extend_from_slice(&end);
        assert!(
            !fixture_complete(&extra_empty),
            "a section without an observed child cannot disappear"
        );
        let mut cross_section = begin.clone();
        for (kind, child) in [(0, 0), (1, 8)] {
            write_provider(
                &mut cross_section,
                &ProviderEvent {
                    kind,
                    producer: 7,
                    child,
                    frames: 0,
                    exited: false,
                },
            )?;
        }
        cross_section.extend_from_slice(&end);
        cross_section.extend_from_slice(&begin);
        write_provider(
            &mut cross_section,
            &ProviderEvent {
                kind: 3,
                producer: 7,
                child: 8,
                frames: 0,
                exited: true,
            },
        )?;
        cross_section.extend_from_slice(&end);
        assert!(
            !fixture_complete(&cross_section),
            "a later section cannot finish an earlier child"
        );
        let mut distinct = complete;
        distinct.extend(fixture_journal_for(&[], 9, 10)?);
        assert!(
            fixture_complete(&distinct),
            "independent complete sections remain admissible"
        );
        Ok(())
    }

    #[test]
    fn completed_journal_cannot_repair_lost_or_uncovered_history()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-journal-history-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let artifacts = root.join("artifacts.jsonl");
        std::fs::write(&artifacts, fixture_journal(&[])?)?;
        let data = serde_json::json!({"workspace_root":root});
        assert!(update_with_history(&root.join("first.json"), &artifacts, &data, false, true)?.1);
        assert!(!update_with_history(&root.join("lost.json"), &artifacts, &data, true, true)?.1);
        assert!(
            !update_with_history(
                &root.join("uncovered.json"),
                &artifacts,
                &data,
                false,
                false
            )?
            .1
        );
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
        let mut raw = (serde_json::json!({"reason":crate::adapter::JOURNAL_BEGIN}).to_string()
            + "\n")
            .into_bytes();
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
        let end = (serde_json::json!({"reason":crate::adapter::JOURNAL_END}).to_string() + "\n")
            .into_bytes();
        raw.extend_from_slice(&end);
        assert!(fixture_complete(&raw));
        raw.truncate(raw.len() - end.len());
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
        raw.extend_from_slice(&end);
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
            lines
                .iter_mut()
                .rfind(|value| value["reason"] == PROVIDER)
                .ok_or("terminal")?[field] = wrong;
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
    fn compiler_dep_info_keeps_literal_unicode_tabs_and_backslashes() {
        for name in [
            "left\u{a0}right.txt",
            "tab\tname",
            "slash\\name",
            "hash#name",
            "colon:name",
        ] {
            let text = format!("out.d: source.rs {name}\n\n{name}:\n");
            assert_eq!(
                dependencies(&text),
                Some(vec!["source.rs".into(), name.into()])
            );
        }
        assert!(dependencies("out.d: bad\0name\n").is_none());
        assert!(dependencies("out.d: bad\rname\n").is_none());
        assert!(dependencies("out.d: source.rs line\nname\n\nsource.rs:\nline\nname:\n").is_none());
        assert!(dependencies("out.d: source.rs\n\nforeign.rs:\n").is_none());
    }

    #[test]
    fn actual_rustc_dep_info_does_not_replace_one_consumed_path_with_two_existing_files()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-dep-actual-{}", std::process::id()));
        let project = root.join("project");
        let target = root.join("target");
        std::fs::create_dir_all(&project)?;
        std::fs::create_dir_all(&target)?;
        let name = "left\u{a0}right.txt";
        let external = root.join(name);
        std::fs::write(&external, "7")?;
        // Both wrongly split names exist, so checking readability cannot repair
        // the old parser's omission of the actual file consumed by rustc.
        std::fs::write(root.join("left"), "unchanged decoy")?;
        std::fs::write(project.join("right.txt"), "unchanged decoy")?;
        std::fs::write(
            project.join("lib.rs"),
            format!("pub const VALUE: &str = include_str!(\"../{name}\");\n"),
        )?;
        let output = std::process::Command::new("rustc")
            .args([
                "--crate-name",
                "demo",
                "--crate-type",
                "lib",
                "--emit=dep-info,metadata",
                "--out-dir",
            ])
            .arg(&target)
            .arg("lib.rs")
            .current_dir(&project)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let raw = std::fs::read_to_string(target.join("demo.d"))?;
        let actual = format!("../{name}");
        let decoded = dependencies(&raw).ok_or("actual compiler dep-info refused")?;
        assert!(decoded.contains(&actual));
        assert!(!decoded.contains(&"../left".to_owned()));
        assert!(!decoded.contains(&"right.txt".to_owned()));
        let artifacts = root.join("artifacts.jsonl");
        let artifact = serde_json::json!({"reason":"compiler-artifact","package_id":"demo",
            "target":{"kind":["lib"]},"filenames":[target.join("libdemo.rmeta")]});
        std::fs::write(&artifacts, fixture_journal(&[artifact])?)?;
        let data = serde_json::json!({"workspace_root":project,"packages":[{"manifest_path":project.join("Cargo.toml")}]});
        let (paths, covered) = update(&root.join("sources.json"), &artifacts, &data)?;
        assert!(covered);
        assert!(paths.contains(&project.join(&actual)));
        assert!(paths.contains(&external.canonicalize()?));
        assert!(!paths.contains(&root.join("left")));
        assert!(!paths.contains(&project.join("right.txt")));
        let before = super::super::input::snapshot_metadata(&data, &paths);
        std::fs::write(&external, "9")?;
        let after = super::super::input::snapshot_metadata(&data, &paths);
        assert_ne!(before.revision, after.revision);
        std::fs::remove_dir_all(root)?;
        Ok(())
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
    fn units_keep_their_own_declarations_and_vanished_inputs_leave_the_record()
    -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-source-units-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("demo");
        let target = root.join("target");
        std::fs::create_dir_all(project.join("src/bin"))?;
        std::fs::create_dir_all(&target)?;
        let project = project.canonicalize()?;
        let target = target.canonicalize()?;
        std::fs::write(project.join("src/lib.rs"), "lib")?;
        std::fs::write(project.join("src/bin/probe.rs"), "bin")?;
        std::fs::write(target.join("demo-1.d"), "lib.d: src/lib.rs\n")?;
        std::fs::write(target.join("probe-2.d"), "bin.d: src/bin/probe.rs\n")?;
        let unit = |name: &str, kind: &str, file: &str| {
            serde_json::json!({"reason":"compiler-artifact","package_id":"demo",
                "target":{"name":name,"kind":[kind]},"filenames":[target.join(file)]})
        };
        let lib = unit("demo", "lib", "libdemo-1.rmeta");
        let bin = unit("probe", "bin", "probe-2");
        let artifacts = root.join("artifacts.jsonl");
        let state = root.join("sources.json");
        let data = serde_json::json!({"workspace_root":project});
        std::fs::write(&artifacts, closed_journal(&[lib.clone(), bin]))?;
        let (_, covered) = update(&state, &artifacts, &data).map_err(std::io::Error::other)?;
        assert!(covered);
        let lib_key = target_key("demo", "demo", &["lib".to_owned()]);
        let bin_key = target_key("demo", "probe", &["bin".to_owned()]);
        let units = targets(&state).unwrap_or_default();
        assert_eq!(units.get(&lib_key), Some(&vec![project.join("src/lib.rs")]));
        assert_eq!(
            units.get(&bin_key),
            Some(&vec![project.join("src/bin/probe.rs")])
        );
        // The bin is removed (#401): its declaration leaves every view of the
        // record, so later snapshots stay complete, and the lib keeps its own.
        std::fs::remove_dir_all(project.join("src/bin"))?;
        let (paths, covered) = load(&state, true);
        assert!(covered);
        assert_eq!(paths, vec![project.join("src/lib.rs")]);
        assert_eq!(
            packages(&state).unwrap_or_default().get("demo"),
            Some(&vec![project.join("src/lib.rs")])
        );
        assert_eq!(
            targets(&state).unwrap_or_default().get(&bin_key),
            Some(&vec![])
        );
        // A later run reporting only the lib leaves the stored record without
        // the vanished file too.
        std::fs::write(&artifacts, closed_journal(&[lib]))?;
        let (paths, _) = update(&state, &artifacts, &data).map_err(std::io::Error::other)?;
        assert_eq!(paths, vec![project.join("src/lib.rs")]);
        let stored: Inputs = read(&state).ok_or_else(|| std::io::Error::other("record"))?;
        assert_eq!(stored.paths, vec![project.join("src/lib.rs")]);
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn legacy_global_observations_require_scoped_establishment() {
        let path = std::env::temp_dir().join(format!(
            "sail-source-input-legacy-{}.json",
            std::process::id()
        ));
        assert!(std::fs::write(&path, r#"{"paths":[],"covered":true}"#).is_ok());
        assert!(!load(&path, true).1);
        assert!(packages(&path).is_none());
        assert!(std::fs::write(&path, r#"{"paths":[],"covered":true,"by_package":{}}"#).is_ok());
        assert!(
            !load(&path, true).1,
            "legacy scoped records omitted source aliases too"
        );
        assert!(packages(&path).is_none());
        let previous = path.with_extension("rs");
        assert!(std::fs::write(&previous, "// previous source").is_ok());
        let legacy = serde_json::json!({"version":2,"paths":[previous],"covered":true,
            "by_package":{"demo":[previous]}});
        assert!(std::fs::write(&path, legacy.to_string()).is_ok());
        assert_eq!(
            load(&path, true),
            (vec![previous.clone()], false),
            "old decoder coverage cannot authorize the corrected filename domain"
        );
        assert!(packages(&path).is_none());
        assert!(std::fs::remove_file(&previous).is_ok());
        assert!(std::fs::write(&path, "corrupt source observation").is_ok());
        assert!(!load(&path, true).1);
        assert!(
            !load(&path, false).1,
            "a corrupt record is never a first observation"
        );
        assert!(std::fs::remove_file(&path).is_ok());
        assert!(load(&path, false).1, "a context's first observation");
        assert!(!load(&path, true).1, "a lost record after earlier runs");
    }

    #[test]
    fn outer_sections_refuse_unclosed_and_foreign_records() {
        let root = std::env::temp_dir().join(format!("sail-source-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert!(std::fs::create_dir_all(&root).is_ok());
        let data = serde_json::json!({"workspace_root":root});
        let artifacts = root.join("artifacts.jsonl");
        let begin = serde_json::json!({"reason":crate::adapter::JOURNAL_BEGIN}).to_string() + "\n";
        let unit = serde_json::json!({"reason":"compiler-artifact","package_id":"demo",
            "target":{"kind":["lib"]},"filenames":[]})
        .to_string()
            + "\n";
        let complete = closed_journal(&[]);
        let incomplete: [Option<String>; 9] = [
            None,
            Some(String::new()),
            Some("{not json\n".to_owned()),
            Some(begin.clone()),
            Some(begin.clone() + &unit),
            Some(unit.clone()),
            Some(complete.clone() + &begin),
            Some(complete.clone() + &unit),
            Some(complete.trim_end().to_owned() + "\n{\"reason\":\"compiler-art"),
        ];
        for (index, journal) in incomplete.iter().enumerate() {
            let state = root.join(format!("sources-{index}.json"));
            let _ = std::fs::remove_file(&artifacts);
            if let Some(journal) = journal {
                assert!(std::fs::write(&artifacts, journal).is_ok());
            }
            let (_, covered) =
                update(&state, &artifacts, &data).unwrap_or_else(|e| unreachable!("fixture: {e}"));
            assert!(!covered, "journal {index} must not be complete coverage");
            assert!(!load(&state, true).1, "record {index}");
        }
        // A completed empty child, and one closed section per Cargo build.
        let another = String::from_utf8(
            fixture_journal_for(&[], 9, 10)
                .unwrap_or_else(|e| unreachable!("second child fixture: {e}")),
        )
        .unwrap_or_else(|e| unreachable!("second child UTF-8: {e}"));
        for journal in [complete.clone(), complete.clone() + &another] {
            assert!(std::fs::write(&artifacts, journal).is_ok());
            let state = root.join("sources-complete.json");
            let (paths, covered) =
                update(&state, &artifacts, &data).unwrap_or_else(|e| unreachable!("fixture: {e}"));
            assert!(covered && paths.is_empty());
            assert!(load(&state, true).1);
            assert!(completed_journal(&artifacts).is_some());
        }
        assert!(std::fs::remove_dir_all(root).is_ok());
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
        assert!(load(&state, true).1);
        std::fs::remove_dir_all(root)
    }
}
