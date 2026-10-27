//! Reads of the verification phase, which the final dep-info omits (#386, #439).
//!
//! The Verus driver compiles a verified crate twice in one process: the
//! verification phase keeps ghost code, and the erased compilation phase that
//! follows does not. Both write the unit's dep-info, and the second write
//! replaces the first. An `env!` or `option_env!` read, or a file an
//! `include!`, `include_str!` or `include_bytes!` reads, that occurs only in
//! ghost code (a `proof fn` body, a spec function, an assertion) is therefore
//! missing from the `.d` file that Cargo fingerprints and Sail's receipts bind
//! (VI-0036, VI-0037).
//!
//! The pinned fork's compiler-input hook (`SAIL_COMPILER_INPUTS`) reports, for
//! both phases after a successful verification, rustc's tracked environment
//! (`sess.env_depinfo`), the local files in its source map and its tracked
//! files (`sess.file_depinfo`). The managed driver requests it for every
//! verified target ([`Observation`]), reads it after the compiler exits, and
//!
//! - appends each declaration and file missing from the unit's dep-info
//!   ([`merge`]), so a ghost read binds exactly like an exec read: Cargo
//!   recompiles when it changes, and receipts and unchanged-call replay bind
//!   its value or content;
//! - records the observed names in the invocation, which receipts require
//!   before they authorize reuse ([`super::receipts::Invocation::environment_observed`]).
//!
//! Anything missing, duplicated, malformed or unbound refuses the observation,
//! and the target then publishes no receipt.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};

const DIRECTORY: &str = "SAIL_COMPILER_INPUTS";
const BINDING: &str = "SAIL_COMPILER_INPUTS_BINDING";
const DOCUMENT: &str = "inputs.json";
const KIND: &str = "sail-compiler-read-inputs";
const LIMIT: u64 = 8 * 1024 * 1024;
/// The Verus options the observation needs, carried with Sail's other options
/// (`adapter_verus_channel.rs`), never on the compiler's command line: the
/// hook reports only files with a compiler checksum. They are requested only
/// when Cargo selected no checksum; when it did (checksum freshness), the
/// report's checksums use Cargo's algorithm, which [`merge`] relies on.
pub(crate) const OPTIONS: &[&str] = &["-Zchecksum-hash-algorithm=sha256"];

/// The variables rustc tracked in a compiler run: name and value, if set.
pub(crate) type Environment = BTreeMap<String, Option<String>>;

/// What both phases of one compiler run read.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Observed {
    /// The tracked environment.
    pub(crate) environment: Environment,
    /// Every local source-map file and tracked file, as rustc names them,
    /// with the checksum rustc computed of the bytes it read (source-map files
    /// only), except imported VIR, which the invocation binds by its extern
    /// ([`crate::adapter`]'s driver dependencies).
    pub(crate) files: BTreeMap<String, Option<Source>>,
}

/// rustc's checksum of a source file it read (`algorithm=hex`) and its length.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Source {
    pub(crate) checksum: String,
    pub(crate) size: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    kind: String,
    schema: u32,
    binding: String,
    pid: u32,
    complete: bool,
    phases: BTreeMap<String, Phase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Phase {
    sources: BTreeMap<String, Source>,
    extra_tracked_files: Vec<String>,
    tracked_environment: Environment,
}

/// A fresh private request for one actual verified-target compiler child.
pub(crate) struct Observation {
    directory: PathBuf,
    binding: String,
    pid: Option<u32>,
}

impl Observation {
    /// A new private directory beneath `parent`, bound to this run and target.
    /// `None` when the directory cannot be created fresh.
    pub(crate) fn begin(parent: &Path, run: &str, key: &str) -> Option<Self> {
        let parent = std::path::absolute(parent).ok()?;
        let directory = parent.join(format!("verification-environment-{}", std::process::id()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(&directory).ok()?;
        Some(Self {
            directory,
            binding: binding(run, key),
            pid: None,
        })
    }

    /// Request the observation from the child; [`OPTIONS`] travel separately.
    pub(crate) fn configure(
        &self,
        command: &mut impl super::command_environment::ConfigureCommand,
    ) {
        command
            .env(DIRECTORY, &self.directory)
            .env(BINDING, &self.binding);
    }

    pub(crate) fn started(&mut self, pid: u32) {
        self.pid = Some(pid);
    }

    /// The union of both phases' reads, once the child exited.
    pub(crate) fn observed(&self) -> Option<Observed> {
        read(&self.directory.join(DOCUMENT), &self.binding, self.pid?)
    }
}

/// The request binding the fork echoes: 64 hex digits.
pub(crate) fn binding(run: &str, key: &str) -> String {
    super::files::digest(format!("{run}\0{key}").as_bytes())
}

/// Whether the compiler arguments already select a source checksum.
pub(crate) fn checksum_requested(arguments: &[OsString]) -> bool {
    arguments.iter().enumerate().any(|(index, argument)| {
        let text = argument.to_string_lossy();
        text.starts_with("-Zchecksum-hash-algorithm")
            || (text == "-Z"
                && arguments.get(index + 1).is_some_and(|next| {
                    next.to_string_lossy()
                        .starts_with("checksum-hash-algorithm")
                }))
    })
}

/// Read one completed observation document at `path`, the only entry of its
/// directory, written by the process `pid` for `binding`. Returns the union of
/// both phases' reads; `None` for anything else.
pub(crate) fn read(path: &Path, binding: &str, pid: u32) -> Option<Observed> {
    let directory = path.parent()?;
    let mut entries = 0;
    for entry in std::fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        if entry.file_name() != DOCUMENT || !entry.file_type().ok()?.is_file() {
            return None;
        }
        entries += 1;
    }
    if entries != 1 || std::fs::symlink_metadata(path).ok()?.len() > LIMIT {
        return None;
    }
    let mut raw = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(LIMIT + 1)
        .read_to_end(&mut raw)
        .ok()?;
    if raw.len() as u64 > LIMIT {
        return None;
    }
    let document: Document = serde_json::from_slice(&raw).ok()?;
    let phases: BTreeSet<&str> = document.phases.keys().map(String::as_str).collect();
    if document.kind != KIND
        || document.schema != 1
        || !document.complete
        || pid == 0
        || document.pid != pid
        || document.binding != binding
        || phases != BTreeSet::from(["verification", "compilation"])
    {
        return None;
    }
    let mut environment = Environment::new();
    let mut files: BTreeMap<String, Option<Source>> = BTreeMap::new();
    for phase in document.phases.into_values() {
        for (file, source) in phase.sources {
            // Both phases read one file once; different bytes are not one read.
            if let Some(Some(old)) = files.insert(file, Some(source.clone()))
                && old != source
            {
                return None;
            }
        }
        for file in phase.extra_tracked_files {
            if Path::new(&file).extension().is_none_or(|e| e != "vir") {
                files.entry(file).or_insert(None);
            }
        }
        for (name, value) in phase.tracked_environment {
            // One process has one environment; disagreement is not an observation.
            if environment
                .insert(name, value.clone())
                .is_some_and(|old| old != value)
            {
                return None;
            }
        }
    }
    if files.keys().any(String::is_empty) {
        return None;
    }
    Some(Observed { environment, files })
}

/// The dep-info path rustc writes for these compiler arguments, if any is
/// requested; `None` when it cannot be determined.
pub(crate) fn dep_info(arguments: &[OsString]) -> Option<Vec<PathBuf>> {
    let arguments: Vec<&str> = arguments
        .iter()
        .map(|argument| argument.to_str())
        .collect::<Option<_>>()?;
    let value = |flag: &str| -> Option<Option<&str>> {
        let mut found = None;
        for (index, argument) in arguments.iter().enumerate() {
            let value = if *argument == flag {
                Some(*arguments.get(index + 1)?)
            } else {
                argument
                    .strip_prefix(flag)
                    .and_then(|rest| rest.strip_prefix('='))
            };
            if let Some(value) = value {
                // A repeated flag is not something this reader models.
                if found.replace(value).is_some() {
                    return None;
                }
            }
        }
        Some(found)
    };
    let mut requested = None;
    for emission in value("--emit")?.unwrap_or_default().split(',') {
        if emission == "dep-info" {
            requested = Some(None);
        } else if let Some(path) = emission.strip_prefix("dep-info=") {
            requested = Some(Some(PathBuf::from(path)));
        }
    }
    let Some(requested) = requested else {
        return Some(Vec::new());
    };
    if let Some(path) = requested {
        return Some(vec![path]);
    }
    let directory = value("--out-dir")??;
    let name = value("--crate-name")??;
    let mut extra = None;
    for (index, argument) in arguments.iter().enumerate() {
        let codegen = if *argument == "-C" {
            arguments.get(index + 1).copied()
        } else {
            argument.strip_prefix("-C")
        };
        if let Some(value) = codegen.and_then(|option| option.strip_prefix("extra-filename="))
            && extra.replace(value).is_some()
        {
            return None;
        }
    }
    Some(vec![
        Path::new(directory).join(format!("{name}{}.d", extra.unwrap_or_default())),
    ])
}

/// rustc's dep-info escaping of environment names and values
/// (`rustc_interface::passes::escape_dep_env`).
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// The names a dep-info's `# env-dep:` lines declare, in rustc's escaped form.
fn declared(text: &str) -> BTreeSet<&str> {
    text.lines()
        .filter_map(|line| line.strip_prefix("# env-dep:"))
        .map(|rest| rest.split_once('=').map_or(rest, |(name, _)| name))
        .collect()
}

/// Whether the proof context already binds this variable, so its dep-info
/// declaration is unnecessary. The driver's own declarations are among them:
/// `VERUS_Z3_PATH` and the forwarded arguments, which Cargo-Verus spells with
/// a leading space (` __VERUS_DRIVER_ARGS__`, ` __VERUS_DRIVER_ARGS_FOR_<id>`).
/// Declaring the forwarded arguments would recompile a library whenever a new
/// dependent changes them, and every repair (VI-0005).
fn context_bound(name: &str) -> bool {
    super::environment::input_environment_key(OsStr::new(name))
        || name.trim_start().starts_with("__VERUS_DRIVER_")
}

/// rustc's dep-info escaping of file names
/// (`rustc_interface::passes::escape_dep_filename`).
fn escape_file(name: &str) -> String {
    name.replace(' ', "\\ ")
}

/// Add `files` to every output rule of the dep-info `text` and give each its
/// own empty rule, as rustc writes them. `None` when the text does not have
/// rustc's layout: output rules, a blank line, then the file rules.
fn declare_files(text: &str, files: &[String]) -> Option<String> {
    if files.is_empty() {
        return Some(text.to_owned());
    }
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let outputs = lines.iter().position(String::is_empty)?;
    let rules = outputs + 1 + lines[outputs + 1..].iter().position(String::is_empty)?;
    if outputs == 0 || !lines[..outputs].iter().all(|line| line.contains(':')) {
        return None;
    }
    let escaped: Vec<String> = files.iter().map(|file| escape_file(file)).collect();
    for line in &mut lines[..outputs] {
        for file in &escaped {
            line.push(' ');
            line.push_str(file);
        }
    }
    for (offset, file) in escaped.iter().enumerate() {
        lines.insert(rules + offset, format!("{file}:"));
    }
    Some(lines.join("\n"))
}

/// Append to the dep-info at `path` each observed declaration and file it
/// lacks, in rustc's own form, replacing the file atomically. When the
/// dep-info records checksums (Cargo's checksum freshness), each added file
/// gets rustc's checksum of the bytes the compiler read; a file without one
/// refuses the merge, since a checksum taken now could describe later bytes.
/// `None` when the file cannot be read, parsed or replaced.
pub(crate) fn merge(path: &Path, observed: &Observed) -> Option<()> {
    let original = std::fs::read_to_string(path).ok()?;
    let present: BTreeSet<String> = super::source_inputs::dependencies(&original)?
        .into_iter()
        .collect();
    let missing: Vec<(&String, &Option<Source>)> = observed
        .files
        .iter()
        .filter(|(file, _)| !present.contains(*file))
        .collect();
    let checksums = original.lines().any(|line| line.starts_with("# checksum:"));
    let mut addition = String::new();
    if checksums {
        for (file, source) in &missing {
            let source = source.as_ref()?;
            writeln!(
                addition,
                "# checksum:{} file_len:{} {}",
                source.checksum,
                source.size,
                escape_file(file)
            )
            .ok()?;
        }
    }
    let names: Vec<String> = missing.iter().map(|(file, _)| (*file).clone()).collect();
    let text = declare_files(&original, &names)?;
    let present = declared(&text);
    for (name, value) in &observed.environment {
        let escaped = escape(name);
        if context_bound(name) || present.contains(escaped.as_str()) {
            continue;
        }
        addition.push_str("# env-dep:");
        addition.push_str(&escaped);
        if let Some(value) = value {
            addition.push('=');
            addition.push_str(&escape(value));
        }
        addition.push('\n');
    }
    if addition.is_empty() && missing.is_empty() {
        return Some(());
    }
    let mut merged = text;
    if !merged.is_empty() && !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged.push_str(&addition);
    let temporary = path.with_extension(format!("d.sail-{}", std::process::id()));
    if std::fs::write(&temporary, merged)
        .and_then(|()| std::fs::rename(&temporary, path))
        .is_err()
    {
        let _ = std::fs::remove_file(&temporary);
        return None;
    }
    Some(())
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

    fn root(name: &str) -> std::io::Result<Root> {
        let path =
            std::env::temp_dir().join(format!("sail-ghost-env-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path)?;
        Ok(Root(path))
    }

    fn document(binding: &str, pid: u32, verification: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "kind": KIND, "schema": 1, "binding": binding, "pid": pid, "complete": true,
            "phases": {
                "verification": {"sources": {"/w/src/lib.rs": {"checksum": "sha256=00", "size": 1},
                        "/w/src/value.rs": {"checksum": "sha256=01", "size": 2}},
                    "extra_tracked_files": ["/w/data.txt", "/t/deps/libvstd-1.vir"],
                    "tracked_environment": verification},
                "compilation": {"sources": {"/w/src/lib.rs": {"checksum": "sha256=00", "size": 1}},
                    "extra_tracked_files": [], "tracked_environment": {"VERUSDOC": null}},
            }
        })
    }

    #[test]
    fn observation_reads_the_verification_phase_and_refuses_anything_unbound()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = root("read")?;
        let mut observation = Observation::begin(&root.0, "run", "key").ok_or("begin")?;
        // The directory is created fresh: a second request in it is refused.
        assert!(Observation::begin(&root.0, "run", "key").is_none());
        assert!(observation.observed().is_none(), "no child started");
        observation.started(42);
        assert!(observation.observed().is_none(), "no document written");
        let path = observation.directory.join(DOCUMENT);
        let ghost = serde_json::json!({"ORDINARY_PROOF_FLAG": "7"});
        std::fs::write(
            &path,
            serde_json::to_vec(&document(&observation.binding, 42, &ghost))?,
        )?;
        assert_eq!(
            observation.observed(),
            Some(Observed {
                environment: Environment::from([
                    ("ORDINARY_PROOF_FLAG".to_owned(), Some("7".to_owned())),
                    ("VERUSDOC".to_owned(), None),
                ]),
                // Imported VIR is bound by its extern, not here.
                files: BTreeMap::from([
                    ("/w/data.txt".to_owned(), None),
                    (
                        "/w/src/lib.rs".to_owned(),
                        Some(Source {
                            checksum: "sha256=00".to_owned(),
                            size: 1,
                        }),
                    ),
                    (
                        "/w/src/value.rs".to_owned(),
                        Some(Source {
                            checksum: "sha256=01".to_owned(),
                            size: 2,
                        }),
                    ),
                ]),
            })
        );
        let refused = |value: serde_json::Value| -> Result<bool, Box<dyn std::error::Error>> {
            std::fs::write(&path, serde_json::to_vec(&value)?)?;
            Ok(observation.observed().is_none())
        };
        let valid = document(&observation.binding, 42, &ghost);
        assert!(
            refused(document(&observation.binding, 43, &ghost))?,
            "foreign process"
        );
        assert!(
            refused(document(&binding("other", "key"), 42, &ghost))?,
            "foreign binding"
        );
        let mut incomplete = valid.clone();
        incomplete["complete"] = false.into();
        assert!(refused(incomplete)?);
        let mut missing = valid.clone();
        missing["phases"]
            .as_object_mut()
            .ok_or("phases")?
            .remove("verification");
        assert!(refused(missing)?, "the verification phase is required");
        let mut extra = valid.clone();
        extra["phases"]["focus"] = valid["phases"]["compilation"].clone();
        assert!(refused(extra)?);
        let mut kind = valid.clone();
        kind["kind"] = "other".into();
        assert!(refused(kind)?);
        let mut unknown = valid.clone();
        unknown["extra"] = true.into();
        assert!(refused(unknown)?);
        let mut conflict = valid.clone();
        conflict["phases"]["compilation"]["tracked_environment"]["ORDINARY_PROOF_FLAG"] =
            "8".into();
        assert!(refused(conflict)?, "phases disagree on one value");
        std::fs::write(&path, serde_json::to_vec(&valid)?)?;
        std::fs::write(observation.directory.join("other"), "")?;
        assert!(observation.observed().is_none(), "an unexpected entry");
        Ok(())
    }

    #[test]
    fn dep_info_path_follows_cargo_arguments() {
        let args = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(
            dep_info(&args(&[
                "--crate-name",
                "demo",
                "--emit=dep-info,metadata,link",
                "-C",
                "extra-filename=-0123",
                "--out-dir",
                "/t/debug/deps",
            ])),
            Some(vec![PathBuf::from("/t/debug/deps/demo-0123.d")])
        );
        assert_eq!(
            dep_info(&args(&[
                "--crate-name=demo",
                "--emit",
                "metadata,dep-info",
                "-Cextra-filename=-9",
                "--out-dir=out",
            ])),
            Some(vec![PathBuf::from("out/demo-9.d")])
        );
        assert_eq!(
            dep_info(&args(&[
                "--emit=dep-info=/x/y.d,link",
                "--crate-name",
                "demo"
            ])),
            Some(vec![PathBuf::from("/x/y.d")])
        );
        assert_eq!(
            dep_info(&args(&["--emit=metadata", "--crate-name", "demo"])),
            Some(Vec::new())
        );
        assert_eq!(
            dep_info(&args(&["--emit=dep-info", "--crate-name", "demo"])),
            None
        );
        assert_eq!(
            dep_info(&args(&[
                "--emit=dep-info",
                "--crate-name",
                "a",
                "--crate-name",
                "b",
                "--out-dir",
                "o",
            ])),
            None
        );
        assert!(checksum_requested(&args(&[
            "-Z",
            "checksum-hash-algorithm=blake3"
        ])));
        assert!(checksum_requested(&args(&[
            "-Zchecksum-hash-algorithm=sha256"
        ])));
        assert!(!checksum_requested(&args(&["-Z", "time-passes"])));
    }

    #[test]
    fn merge_declares_ghost_reads_like_rustc_and_leaves_context_names_to_the_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = root("merge")?;
        let deps = root.0.join("debug/deps");
        std::fs::create_dir_all(&deps)?;
        let path = deps.join("demo-0123.d");
        let original = "/t/libdemo.rmeta: src/lib.rs\n\nsrc/lib.rs:\n\n# env-dep:VERUSDOC\n";
        std::fs::write(&path, original)?;
        let environment = Environment::from([
            ("VERUSDOC".to_owned(), None),
            ("ORDINARY_PROOF_FLAG".to_owned(), Some("7".to_owned())),
            ("ABSENT_FLAG".to_owned(), None),
            ("ODD\nNAME".to_owned(), Some("back\\slash".to_owned())),
            ("VERUS_Z3_PATH".to_owned(), Some("/z3".to_owned())),
            (
                "__VERUS_DRIVER_ARGS_FOR_demo".to_owned(),
                Some("--rlimit".to_owned()),
            ),
            (
                " __VERUS_DRIVER_ARGS_FOR_demo-0.1.0-0123".to_owned(),
                Some("import-dep-if-present=vstd".to_owned()),
            ),
            (
                " __VERUS_DRIVER_ARGS__".to_owned(),
                Some("compile".to_owned()),
            ),
        ]);
        let observed = Observed {
            environment,
            files: BTreeMap::from([("src/lib.rs".to_owned(), None)]),
        };
        merge(&path, &observed).ok_or("merge")?;
        let merged = std::fs::read_to_string(&path)?;
        assert_eq!(
            merged,
            format!(
                "{original}# env-dep:ABSENT_FLAG\n# env-dep:ODD\\nNAME=back\\\\slash\n# env-dep:ORDINARY_PROOF_FLAG=7\n"
            )
        );
        // Sail's own declaration scan now sees the ghost read.
        let names = super::super::environment::declared_names(&root.0).ok_or("scan")?;
        assert!(names.contains("ORDINARY_PROOF_FLAG") && names.contains("ABSENT_FLAG"));
        // Merging again changes nothing.
        merge(&path, &observed).ok_or("merge again")?;
        assert_eq!(std::fs::read_to_string(&path)?, merged);
        assert_eq!(std::fs::read_dir(&deps)?.count(), 1, "no temporary is left");
        assert!(merge(&deps.join("missing.d"), &observed).is_none());
        Ok(())
    }

    #[test]
    fn merge_declares_ghost_only_files_like_rustc() -> Result<(), Box<dyn std::error::Error>> {
        let root = root("files")?;
        let deps = root.0.join("debug/deps");
        std::fs::create_dir_all(&deps)?;
        let path = deps.join("demo-0123.d");
        let original = "/t/demo-0123.d: src/lib.rs\n/t/libdemo-0123.rmeta: src/lib.rs\n\nsrc/lib.rs:\n\n# env-dep:VERUSDOC\n# checksum:blake3=00 file_len:1 src/lib.rs\n";
        std::fs::write(&path, original)?;
        let observed = Observed {
            environment: Environment::from([("VERUSDOC".to_owned(), None)]),
            files: BTreeMap::from([
                ("src/lib.rs".to_owned(), None),
                (
                    "src/value.rs".to_owned(),
                    Some(Source {
                        checksum: "blake3=01".to_owned(),
                        size: 2,
                    }),
                ),
                (
                    "src/a b.txt".to_owned(),
                    Some(Source {
                        checksum: "blake3=02".to_owned(),
                        size: 3,
                    }),
                ),
            ]),
        };
        merge(&path, &observed).ok_or("merge")?;
        let merged = std::fs::read_to_string(&path)?;
        assert_eq!(
            merged,
            "/t/demo-0123.d: src/lib.rs src/a\\ b.txt src/value.rs\n/t/libdemo-0123.rmeta: src/lib.rs src/a\\ b.txt src/value.rs\n\nsrc/lib.rs:\nsrc/a\\ b.txt:\nsrc/value.rs:\n\n# env-dep:VERUSDOC\n# checksum:blake3=00 file_len:1 src/lib.rs\n# checksum:blake3=02 file_len:3 src/a\\ b.txt\n# checksum:blake3=01 file_len:2 src/value.rs\n"
        );
        // Sail's own reader of the unit's inputs now sees the ghost-only files.
        assert_eq!(
            super::super::source_inputs::dependencies(&merged),
            Some(vec![
                "src/lib.rs".to_owned(),
                "src/a b.txt".to_owned(),
                "src/value.rs".to_owned()
            ])
        );
        merge(&path, &observed).ok_or("merge again")?;
        assert_eq!(std::fs::read_to_string(&path)?, merged);
        // Under checksum freshness, a file the report gives no checksum for
        // cannot be declared truthfully: refuse.
        let mut unchecked = observed;
        unchecked.files.insert("src/data.txt".to_owned(), None);
        assert!(merge(&path, &unchecked).is_none());
        let observed = unchecked;
        // Without rustc's layout the files cannot be declared: refuse.
        std::fs::write(&path, "/t/libdemo.rmeta: src/lib.rs\n")?;
        assert!(merge(&path, &observed).is_none());
        Ok(())
    }
}
