//! Compiler-declared inputs, including explicit files beneath excluded directories.

use super::receipts::read;
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Inputs {
    /// Version two preserves the compiler's lexical source aliases.
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
    // Older observations do not attribute includes to their consuming package.
    // Establish the scoped declarations before authorizing new current receipts.
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
                v.version == 2 && v.covered && v.by_package.is_some(),
            )
        },
    )
}

pub(super) fn packages(path: &Path) -> Option<BTreeMap<String, Vec<PathBuf>>> {
    read::<Inputs>(path)
        .filter(|v| v.version == 2)
        .and_then(|v| v.by_package)
        .map(present_entries)
}

/// The recorded declarations of each unit ([`Inputs::by_target`]).
pub(super) fn targets(path: &Path) -> Option<BTreeMap<String, Vec<PathBuf>>> {
    read::<Inputs>(path)
        .filter(|v| v.version == 2)
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

/// The most output rules one dep-info is read with: rustc writes one per
/// emitted output (dep-info, metadata, link), never more than a few.
const MAX_OUTPUT_RULES: usize = 16;

/// Decodes one file name exactly as pinned rustc's `escape_dep_filename`
/// encodes it: an ASCII space is written `\ ` and every other character is
/// written as itself, backslashes, tabs and non-ASCII spaces such as U+00A0
/// included. A backslash before a space is therefore always the escape: the
/// encoding never writes a bare space.
fn decode_file(encoded: &str) -> String {
    let mut path = String::with_capacity(encoded.len());
    let mut chars = encoded.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&' ') {
            chars.next();
            path.push(' ');
        } else {
            path.push(c);
        }
    }
    path
}

/// The files a rustc dep-info declares, decoded exactly (#408).
///
/// Rustc writes one output rule per emitted output, `<output>: <files>` with
/// the encoded files joined by single spaces, then one empty rule
/// `<file>:` per file, then `# ` comments (`env-dep`, `checksum`), with blank
/// lines between the sections. The joined list alone is ambiguous (a file
/// name may end in a backslash, and an output name is not escaped at all),
/// so the files are read from the empty rules, one per line, and every
/// output rule must end with exactly their joined list. Any other line, a
/// file name with a newline among them, makes the document malformed:
/// `None`, and its unit's inputs are unknown.
pub(super) fn dependencies(text: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = text.split('\n').collect();
    let (_, files) = rule_layout(&lines)?;
    Some(
        files
            .into_iter()
            .map(|index| decode_file(lines[index].strip_suffix(':').unwrap_or_default()))
            .collect(),
    )
}

/// The line indices of a rustc dep-info's output rules and of its file
/// rules, in order, as [`dependencies`] reads them. `None` for a document
/// without exactly one consistent reading.
pub(super) fn rule_layout(lines: &[&str]) -> Option<(Vec<usize>, Vec<usize>)> {
    let rules: Vec<usize> = (0..lines.len())
        .filter(|&index| !lines[index].is_empty() && !lines[index].starts_with("# "))
        .collect();
    if rules.is_empty() {
        return None;
    }
    let mut found = None;
    for outputs in 1..=rules.len().min(MAX_OUTPUT_RULES + 1) {
        let (output_rules, file_rules) = rules.split_at(outputs);
        let Some(files) = file_rules
            .iter()
            .map(|&index| lines[index].strip_suffix(':'))
            .collect::<Option<Vec<&str>>>()
        else {
            continue;
        };
        let joined = format!(": {}", files.join(" "));
        if output_rules
            .iter()
            .all(|&index| lines[index].ends_with(&joined) && lines[index].len() > joined.len())
        {
            if found.is_some() {
                return None;
            }
            found = Some((output_rules.to_vec(), file_rules.to_vec()));
        }
    }
    found
}

/// The unit records of a complete artifact journal, without its section
/// markers: every line parses, and every record lies in a section the
/// producer opened and closed (see [`crate::adapter::JOURNAL_END`]). `None`
/// for a missing, unreadable, malformed, truncated or unclosed journal.
pub(super) fn completed_journal(artifacts: &Path) -> Option<Vec<Value>> {
    use crate::adapter::{JOURNAL_BEGIN, JOURNAL_END};
    let text = std::fs::read_to_string(artifacts).ok()?;
    let mut open = false;
    let mut sections = 0usize;
    let mut records = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let record: Value = serde_json::from_str(line).ok()?;
        match record["reason"].as_str() {
            Some(JOURNAL_BEGIN) if !open => open = true,
            Some(JOURNAL_END) if open => {
                open = false;
                sections += 1;
            }
            Some(JOURNAL_BEGIN | JOURNAL_END) => return None,
            _ if open => records.push(record),
            _ => return None,
        }
    }
    (!open && sections > 0 && text.ends_with('\n')).then_some(records)
}

pub(super) fn update(path: &Path, artifacts: &Path, data: &Value) -> Result<(Vec<PathBuf>, bool)> {
    let workspace = Path::new(data["workspace_root"].as_str().unwrap_or_default());
    let (previous, _) = load(path, true);
    let mut paths: BTreeSet<PathBuf> = previous.into_iter().collect();
    let mut by_package: BTreeMap<String, BTreeSet<PathBuf>> = packages(path)
        .unwrap_or_default()
        .into_iter()
        .map(|(package, paths)| (package, paths.into_iter().collect()))
        .collect();
    let mut by_target: BTreeMap<String, Vec<PathBuf>> = targets(path).unwrap_or_default();
    let mut reported: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();
    // Only a journal the producer closed after Cargo's output ended is
    // evidence; a lost or partial one never reads as a completed empty build.
    let journal = completed_journal(artifacts);
    let mut covered = journal.is_some();
    for artifact in journal
        .iter()
        .flatten()
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
            version: 2,
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

/// A journal section as the producer writes it for `records`.
#[cfg(test)]
pub(super) fn closed_journal(records: &[Value]) -> String {
    let marker = |reason: &str| serde_json::json!({ "reason": reason }).to_string() + "\n";
    let mut text = marker(crate::adapter::JOURNAL_BEGIN);
    for record in records {
        text += &(record.to_string() + "\n");
    }
    text + &marker(crate::adapter::JOURNAL_END)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiler_dep_info_preserves_escaped_external_paths() {
        let files = [
            "src/lib.rs",
            "../shared file.rs",
            "left\u{a0}right.txt",
            "back\\slash.txt",
            "tab\tname.txt",
            "back\\ space.txt",
            "end\\",
        ];
        let encoded: Vec<String> = files.iter().map(|f| f.replace(' ', "\\ ")).collect();
        let list = encoded.join(" ");
        let empty = encoded
            .iter()
            .map(|f| format!("{f}:\n"))
            .collect::<Vec<_>>()
            .concat();
        let document = format!(
            "/t/a b/demo.d: {list}\n\n/t/a b/libdemo.rmeta: {list}\n\n{empty}\n# env-dep:X=1\n"
        );
        assert_eq!(
            dependencies(&document),
            Some(files.iter().map(|f| (*f).to_owned()).collect())
        );
        // Without the empty rules a trailing backslash and an escaped space
        // cannot be told apart; the empty rules settle it.
        assert_eq!(
            dependencies("o: end\\ next.rs\n\nend\\:\nnext.rs:\n"),
            Some(vec!["end\\".to_owned(), "next.rs".to_owned()])
        );
        assert_eq!(
            dependencies("o: end\\ next.rs\n\nend\\ next.rs:\n"),
            Some(vec!["end next.rs".to_owned()])
        );
        for malformed in [
            "",
            "truncated dep-info",
            // A file rule missing, extra, or out of order.
            "o: a.rs b.rs\n\na.rs:\n",
            "o: a.rs\n\na.rs:\nb.rs:\n",
            "o: a.rs b.rs\n\nb.rs:\na.rs:\n",
            // Output rules that disagree, and a file name split by a newline.
            "o: a.rs\np: b.rs\n\na.rs:\n",
            "o: new\nline.rs\n\nnew\nline.rs:\n",
            // A make line continuation is not rustc's layout.
            "o: a.rs \\\n b.rs\n\na.rs:\nb.rs:\n",
        ] {
            assert!(dependencies(malformed).is_none(), "{malformed:?}");
        }
    }

    /// The pinned compiler's own dep-info for file names that a general make
    /// or whitespace parser splits or rewrites (#408).
    #[test]
    fn actual_rustc_dep_info_names_every_consumed_file() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-dep-info-rustc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("a b"))?;
        let names = [
            "left\u{a0}right.txt",
            "em\u{2003}space.txt",
            "back\\slash.txt",
            "a b/x.txt",
            "tab\tname.txt",
            "trailing\\",
        ];
        let mut source = Vec::new();
        for (index, name) in names.iter().enumerate() {
            std::fs::write(root.join(name), index.to_string())?;
            source.push(format!(
                "pub const F{index}: &str = include_str!({});\n",
                serde_json::to_string(name).map_err(std::io::Error::other)?
            ));
        }
        std::fs::write(root.join("lib.rs"), source.concat())?;
        let output =
            std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .current_dir(&root)
                .args([
                    "--edition",
                    "2024",
                    "--crate-type",
                    "lib",
                    "--crate-name",
                    "names",
                    "--emit=dep-info,metadata",
                    "lib.rs",
                ])
                .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = std::fs::read_to_string(root.join("names.d"))?;
        let mut expected = vec!["lib.rs".to_owned()];
        expected.extend(names.iter().map(|name| (*name).to_owned()));
        assert_eq!(dependencies(&text), Some(expected), "{text:?}");
        std::fs::remove_dir_all(root)
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
        assert!(
            std::fs::write(
                target.join("demo-123.d"),
                "out.d: ../shared\\ file.rs\n\n../shared\\ file.rs:\n"
            )
            .is_ok()
        );
        let artifacts = root.join("artifacts.jsonl");
        let artifact = serde_json::json!({"reason":"compiler-artifact","package_id":"demo","target":{"kind":["lib"]},"filenames":[target.join("libdemo-123.rmeta")]});
        assert!(std::fs::write(&artifacts, closed_journal(&[artifact])).is_ok());
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
        std::fs::write(
            target.join("demo-1.d"),
            "lib.d: src/lib.rs\n\nsrc/lib.rs:\n",
        )?;
        std::fs::write(
            target.join("probe-2.d"),
            "bin.d: src/bin/probe.rs\n\nsrc/bin/probe.rs:\n",
        )?;
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
    fn missing_malformed_or_truncated_journal_is_not_a_completed_empty_child() {
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
        for journal in [complete.clone(), complete.clone() + &complete] {
            assert!(std::fs::write(&artifacts, journal).is_ok());
            let state = root.join("sources-complete.json");
            let (paths, covered) =
                update(&state, &artifacts, &data).unwrap_or_else(|e| unreachable!("fixture: {e}"));
            assert!(covered && paths.is_empty());
            assert!(load(&state, true).1);
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
        std::fs::write(target.join("demo.d"), "artifact: lib.rs\n\nlib.rs:\n")?;
        let artifacts = root.join("artifacts.jsonl");
        std::fs::write(
            &artifacts,
            closed_journal(&[serde_json::json!({"reason":"compiler-artifact",
            "package_id":"demo","target":{"kind":["lib"]},
            "filenames":[target.join("libdemo.rmeta")]})]),
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
