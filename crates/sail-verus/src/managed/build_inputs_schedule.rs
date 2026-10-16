//! An optional memo of scripts a successful Current Cargo run never scheduled.
//!
//! This cannot supply a script contract or a proof receipt. It only avoids
//! cleaning an unobserved script again under the same complete input snapshot,
//! resolved graph, selection and executable/environment context. Every changed
//! source byte conservatively misses, including ordinary function-body edits.

use crate::managed::input::InputSnapshot;
use sail_api::project::Currentness;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Memo {
    version: u32,
    binding: String,
    run_id: String,
    unscheduled: BTreeSet<String>,
    checksum: String,
}

impl Memo {
    fn checksum(&self) -> Option<String> {
        super::encode(&(self.version, &self.binding, &self.run_id, &self.unscheduled))
            .map(|bytes| crate::managed::files::digest(&bytes))
    }
}

/// Full scheduling inputs, already read by the ordinary Currentness checks.
pub(in crate::managed) struct Scope<'a> {
    pub data: &'a Value,
    pub snapshot: &'a InputSnapshot,
    pub context: &'a str,
    pub compiler: &'a str,
}

struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Scope<'_> {
    fn binding(&self) -> Option<String> {
        if !self.snapshot.complete || self.context.is_empty() || self.compiler.is_empty() {
            return None;
        }
        // An ambiguous/unknown package selection retains original cleanup.
        super::super::selection_closure(self.data)?;
        let mut writer = HashWriter(Sha256::new());
        serde_json::to_writer(
            &mut writer,
            &(
                "sail-unscheduled-scripts-v1",
                self.context,
                self.compiler,
                &self.snapshot.workspace,
            ),
        )
        .ok()?;
        for key in [
            "packages",
            "resolve",
            "workspace_members",
            "workspace_default_members",
            "workspace_root",
            "target_directory",
            "sail_selected_package_ids",
            "sail_invocation_directory",
        ] {
            serde_json::to_writer(&mut writer, &(key, self.data.get(key)?)).ok()?;
        }
        // Use the raw path spelling as ordinary content revisions do. These
        // are complete byte reads, never stat observations. Generated target
        // outputs have already been excluded by the caller's run snapshot.
        for (path, digest) in &self.snapshot.digests {
            let bytes = path.as_os_str().as_encoded_bytes();
            writer.0.update((bytes.len() as u64).to_le_bytes());
            writer.0.update(bytes);
            writer.0.update((digest.len() as u64).to_le_bytes());
            writer.0.update(digest.as_bytes());
        }
        Some(format!("{:x}", writer.0.finalize()))
    }

    fn scripts(&self) -> Option<BTreeSet<String>> {
        self.data["packages"]
            .as_array()?
            .iter()
            .filter(|package| {
                package["targets"].as_array().is_some_and(|targets| {
                    targets.iter().any(|target| {
                        target["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "custom-build"))
                    })
                })
            })
            .map(|package| package["id"].as_str().map(str::to_owned))
            .collect()
    }
}

/// A bounded optional read; missing, malformed or changed inputs mean a miss.
pub(in crate::managed) fn load(path: &Path, scope: &Scope<'_>) -> BTreeSet<String> {
    let read = || -> Option<BTreeSet<String>> {
        let memo: Memo = serde_json::from_slice(&super::bytes(path)?).ok()?;
        (memo.version == 1
            && !memo.run_id.is_empty()
            && memo.binding == scope.binding()?
            && memo.checksum == memo.checksum()?
            && memo.unscheduled.is_subset(&scope.scripts()?))
        .then_some(memo.unscheduled)
    };
    read().unwrap_or_default()
}

/// Confirm the optimization against this run's actual successful scheduling.
/// A changed snapshot or an unexpectedly scheduled script prevents Current.
pub(in crate::managed) fn confirmed(
    skipped: &BTreeSet<String>,
    before: &Scope<'_>,
    after: &Scope<'_>,
    actual_unscheduled: &BTreeSet<String>,
) -> bool {
    skipped.is_empty()
        || (skipped.is_subset(actual_unscheduled)
            && before
                .binding()
                .is_some_and(|binding| Some(binding) == after.binding()))
}

/// Only the caller's successful Current result may establish scheduling facts.
pub(in crate::managed) fn publish(
    path: &Path,
    scope: &Scope<'_>,
    run_id: &str,
    currentness: Currentness,
    exit_code: Option<i32>,
    actual_unscheduled: &BTreeSet<String>,
) {
    if currentness != Currentness::Current || exit_code != Some(0) {
        return;
    }
    let publish = || -> Option<()> {
        let mut memo = Memo {
            version: 1,
            binding: scope.binding()?,
            run_id: run_id.to_owned(),
            unscheduled: actual_unscheduled
                .intersection(&scope.scripts()?)
                .cloned()
                .collect(),
            checksum: String::new(),
        };
        memo.checksum = memo.checksum()?;
        super::encode(&memo)?;
        super::json(path, &memo).ok()
    };
    let _ = publish();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed::build_inputs::{BuildInput, current, stale, stale_after};
    use crate::managed::files::{digest, json};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    struct Fixture {
        root: PathBuf,
        data: Value,
        snapshot: InputSnapshot,
    }
    impl Fixture {
        fn new(label: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
            let root = std::env::temp_dir().join(format!(
                "sail-script-schedule-{label}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&root)?;
            let script = root.join("build.rs");
            std::fs::write(&script, "fn main() {}")?;
            let package = |id: &str| {
                serde_json::json!({"id":id,"name":"same-name","version":id,
                    "manifest_path":root.join(format!("{id}/Cargo.toml")),
                    "targets":[{"kind":["custom-build"]}]})
            };
            let data = serde_json::json!({
                "workspace_root":root, "target_directory":root.join("target"),
                "sail_invocation_directory":root, "workspace_members":["a"],
                "workspace_default_members":["a"], "sail_selected_package_ids":["a"],
                "packages":[{"id":"a","name":"a","targets":[{"kind":["lib"]}]},
                    package("old"),package("new")],
                "resolve":{"root":"a","nodes":[
                    {"id":"a","dependencies":["old","new"],"features":[]},
                    {"id":"old","dependencies":[],"features":[]},
                    {"id":"new","dependencies":[],"features":[]}]}});
            let snapshot = InputSnapshot {
                workspace: root.clone(),
                revision: "not-used-instead-of-file-digests".to_owned(),
                roots: vec![root.clone()],
                excluded: vec![],
                complete: true,
                observation_paths: BTreeSet::new(),
                digests: BTreeMap::from([(script, digest(b"fn main() {}"))]),
            };
            Ok(Self {
                root,
                data,
                snapshot,
            })
        }
        fn scope(&self) -> Scope<'_> {
            Scope {
                data: &self.data,
                snapshot: &self.snapshot,
                context: "proof-context-including-arguments-and-environment",
                compiler: "compiler-runtime",
            }
        }
        fn skipped() -> BTreeSet<String> {
            BTreeSet::from(["old".to_owned()])
        }
        fn establish(&self, path: &Path) {
            publish(
                path,
                &self.scope(),
                "successful-current-run",
                Currentness::Current,
                Some(0),
                &Self::skipped(),
            );
        }
    }

    #[test]
    fn same_name_memo_never_exempts_an_observed_stale_script() -> TestResult {
        let fixture = Fixture::new("same-name")?;
        let state = fixture.root.join("build-inputs.json");
        let memo = fixture.root.join("unscheduled.json");
        let mut record = BuildInput {
            package_id: "new".to_owned(),
            package_name: "same-name".to_owned(),
            manifest: fixture.root.join("new/Cargo.toml"),
            paths: vec![fixture.root.join("build.rs")],
            env: BTreeMap::new(),
            digest: String::new(),
            covered: true,
            output: None,
        };
        record.digest = current(&record, None).0;
        json(&state, &vec![record])?;
        assert_eq!(stale(&state, &fixture.data).0, vec!["same-name"]);
        fixture.establish(&memo);
        let skipped = load(&memo, &fixture.scope());
        assert_eq!(skipped, Fixture::skipped());
        let (clean, _, covered) = stale_after(&state, &fixture.data, &skipped);
        assert!(clean.is_empty() && covered);
        std::fs::write(
            fixture.root.join("build.rs"),
            "fn main() { println!(\"changed\"); }",
        )?;
        // Even if a caller handed us the old set, recorded contracts are
        // checked first. The scheduled sibling must still be cleaned.
        assert_eq!(
            stale_after(&state, &fixture.data, &skipped).0,
            vec!["same-name"]
        );
        std::fs::remove_dir_all(fixture.root)?;
        Ok(())
    }

    #[test]
    fn full_graph_selection_context_and_content_changes_miss() -> TestResult {
        let fixture = Fixture::new("binding")?;
        let path = fixture.root.join("memo.json");
        fixture.establish(&path);
        assert_eq!(load(&path, &fixture.scope()), Fixture::skipped());
        for (pointer, value) in [
            (
                "/resolve/nodes/0/features",
                serde_json::json!(["feature-added"]),
            ),
            ("/sail_selected_package_ids", serde_json::json!(["new"])),
            (
                "/packages/1/version",
                serde_json::json!("different-version"),
            ),
            ("/target_directory", serde_json::json!("other-target")),
            (
                "/sail_invocation_directory",
                serde_json::json!("other-member"),
            ),
        ] {
            let mut changed = fixture.data.clone();
            *changed.pointer_mut(pointer).ok_or("missing test field")? = value;
            assert!(
                load(
                    &path,
                    &Scope {
                        data: &changed,
                        ..fixture.scope()
                    }
                )
                .is_empty()
            );
        }
        for name in [
            "build.rs",
            "Cargo.toml",
            "Cargo.lock",
            ".cargo/config.toml",
            "src/bin/new.rs",
        ] {
            let mut snapshot = fixture.snapshot.clone();
            snapshot
                .digests
                .insert(fixture.root.join(name), digest(b"changed"));
            assert!(
                load(
                    &path,
                    &Scope {
                        snapshot: &snapshot,
                        ..fixture.scope()
                    }
                )
                .is_empty()
            );
        }
        let mut incomplete = fixture.snapshot.clone();
        incomplete.complete = false;
        for scope in [
            Scope {
                snapshot: &incomplete,
                ..fixture.scope()
            },
            Scope {
                context: "different-arguments-or-environment",
                ..fixture.scope()
            },
            Scope {
                compiler: "different-runtime",
                ..fixture.scope()
            },
        ] {
            assert!(load(&path, &scope).is_empty());
        }
        std::fs::remove_dir_all(fixture.root)?;
        Ok(())
    }

    #[test]
    fn failed_or_uncurrent_runs_publish_nothing_and_new_scheduling_is_unconfirmed() -> TestResult {
        let fixture = Fixture::new("publication")?;
        let path = fixture.root.join("memo.json");
        for (currentness, exit) in [
            (Currentness::Historical, Some(0)),
            (Currentness::Unknown, Some(0)),
            (Currentness::Current, Some(1)),
            (Currentness::Current, None),
        ] {
            publish(
                &path,
                &fixture.scope(),
                "run",
                currentness,
                exit,
                &Fixture::skipped(),
            );
            assert!(!path.exists());
        }
        fixture.establish(&path);
        let skipped = load(&path, &fixture.scope());
        assert!(confirmed(
            &skipped,
            &fixture.scope(),
            &fixture.scope(),
            &skipped
        ));
        // A failed/malformed Cargo observation returns an empty actual set,
        // as does a successful run which actually scheduled this script.
        assert!(!confirmed(
            &skipped,
            &fixture.scope(),
            &fixture.scope(),
            &BTreeSet::new()
        ));
        assert!(!confirmed(
            &skipped,
            &fixture.scope(),
            &Scope {
                context: "changed",
                ..fixture.scope()
            },
            &skipped
        ));
        std::fs::remove_dir_all(fixture.root)?;
        Ok(())
    }

    #[test]
    fn missing_corrupt_oversized_and_nonregular_memos_are_misses() -> TestResult {
        let fixture = Fixture::new("malformed")?;
        let path = fixture.root.join("memo.json");
        assert!(load(&path, &fixture.scope()).is_empty());
        fixture.establish(&path);
        let original: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        for (key, value) in [
            ("version", serde_json::json!(2)),
            ("checksum", serde_json::json!("bad")),
            ("run_id", serde_json::json!("")),
            ("unexpected", serde_json::json!(true)),
        ] {
            let mut changed = original.clone();
            changed[key] = value;
            json(&path, &changed)?;
            assert!(load(&path, &fixture.scope()).is_empty());
        }
        std::fs::write(&path, vec![b' '; super::super::MAX_BYTES + 1])?;
        assert!(load(&path, &fixture.scope()).is_empty());
        std::fs::remove_file(&path)?;
        std::fs::create_dir(&path)?;
        assert!(load(&path, &fixture.scope()).is_empty());
        std::fs::remove_dir(&path)?;
        #[cfg(unix)]
        {
            let real = fixture.root.join("valid.json");
            fixture.establish(&real);
            std::os::unix::fs::symlink(real, &path)?;
            assert!(load(&path, &fixture.scope()).is_empty());
        }
        std::fs::remove_dir_all(fixture.root)?;
        Ok(())
    }
}
