//! Unchanged-call replay (ADR 0082).
//!
//! A managed run that ends Current without starting any verifier records the
//! complete stat-stamp vector of everything that run's decision rests on,
//! together with what it printed and reported. A later identical invocation
//! whose vector, cache state and environment all still match replays that
//! output instead of running Cargo. Anything mismatched, missing or
//! unreadable takes the ordinary path, unchanged.
//!
//! The vector covers:
//! - every path of the run's final source observation, as its mutation stamp
//!   covers them (dependency-closure files and every directory walked,
//!   workspace manifests and lockfile, every Cargo configuration and
//!   toolchain-file location, dep-info and build-script inputs, linked
//!   ancestors), checked against the run's own post-run stamp;
//! - every entry of the owned Cargo target (artifacts, fingerprints, build
//!   script outputs, directories);
//! - every installed file the proof context identifies (Sail, Cargo,
//!   Cargo-Verus, Verus and its engine, the solver, rustc, rustup, sysroot
//!   libraries), and rustup's settings;
//! - the proof context's and compiler family's cache records, by content;
//! - the original whole/exact-partial receipts and their selected or archived
//!   producer histories, revalidated through the ordinary receipt reader;
//! - the values of every variable a unit declares (`env-dep`,
//!   `rerun-if-env-changed`).
//!
//! The invocation itself (arguments, output mode, terminals, the selected
//! tools and every environment variable the tools or Sail read) is the
//! record's key. A stamp is recorded only when its ctime is at least
//! 2 s older than the run's start, so a later change to the file
//! moves its ctime past the recorded one even on coarse clocks.
//!
//! One exception is bound by content instead (ADR 0082, amendment of
//! 2026-10-26): Cargo's uplifted build-script copy
//! `build/<package>-<hash>/build-script-<stem>`, which Cargo removes and
//! copies again on every run where it does not hard-link it (macOS, #437).
//! The copy is bound by its type, size and mode and by bytes equal to its
//! original `build_script_<stem>-<hash>` (whose stamp keeps the ordinary
//! guard), and its directory by its identity and complete entry names.

use super::Config;
use sail_api::project::{ProjectResult, ResolvedRequest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time::UNIX_EPOCH;
use std::time::{Duration, SystemTime};

// Version six retains original requests/origins and content-bound Cargo script copies.
const VERSION: u32 = 6;
const HUMAN_VERSION: u32 = 1;
const HUMAN_KIND: &str = "sail-empty-human-output";
/// Cache records up to this size are compared by content; larger files
/// (the private executable aliases) by stamp.
const CONTENT_LIMIT: u64 = 16 << 20;

#[path = "noop_origins.rs"]
mod origins;

/// The complete stat stamp and its settle rule are shared with the daemon's
/// solver identity.
pub(crate) use sail_api::stamp::Stamp;

#[derive(Serialize, Deserialize)]
struct Record {
    version: u32,
    key: String,
    /// The workspace lock and the compiler family's ownership lock.
    locks: (PathBuf, PathBuf),
    /// The proof context's run directory, where a replay leaves its result.
    runs: PathBuf,
    /// `lstat` stamps; `None` records an absent path.
    lstat: Vec<(PathBuf, Option<Stamp>)>,
    /// Stamps of installed files, following links.
    stat: Vec<(PathBuf, Stamp)>,
    /// Cache directories, the children left out, and the complete sorted
    /// set of entries in each.
    trees: Vec<Tree>,
    /// Cache records compared by content.
    contents: Vec<(PathBuf, String)>,
    /// Cargo's uplifted build-script copies, bound by content.
    copies: Vec<Copied>,
    /// The directories holding them, bound by identity and entry names.
    directories: Vec<Directory>,
    /// Declared variables and their values.
    environment: BTreeMap<String, Option<Vec<u8>>>,
    /// Original whole/exact-partial receipts; every replay rechecks their producers.
    origins: Vec<super::receipts::Receipt>,
    original_request: ResolvedRequest,
    result: ProjectResult,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Separate storage envelope and namespace. A proof-record reader cannot
/// deserialize this as its flat Record, and this reader never returns a report.
#[derive(Serialize, Deserialize)]
struct HumanRecord {
    kind: String,
    version: u32,
    observation: Record,
}

/// Cargo's uplifted copy of a build-script executable (#437). Cargo
/// re-creates it on every run where it copies rather than hard-links, so
/// its stamp is never settled; its bytes must equal the original's, whose
/// own stamp is recorded under the ordinary guard.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Copied {
    path: PathBuf,
    original: PathBuf,
    size: u64,
    mode: u32,
}

/// A directory whose stamp churns only from its copy's re-creation: bound by
/// device, inode and mode and its complete sorted entry names. Every other
/// entry keeps its own stamp.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Directory {
    path: PathBuf,
    identity: (u64, u64, u32),
    names: Vec<String>,
}

/// The prefix of Cargo's name for an uplifted build-script executable,
/// `build-script-<stem of the build file>`.
const UPLIFTED_SCRIPT: &str = "build-script-";

/// Device, inode and mode; `None` where there is no such identity.
#[cfg(unix)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "other platforms have no identity and bind nothing"
)]
fn identity(metadata: &std::fs::Metadata) -> Option<(u64, u64, u32)> {
    use std::os::unix::fs::MetadataExt as _;
    Some((metadata.dev(), metadata.ino(), metadata.mode()))
}

#[cfg(not(unix))]
fn identity(_metadata: &std::fs::Metadata) -> Option<(u64, u64, u32)> {
    None
}

/// The sorted entry names of `directory`, `None` when unreadable or not
/// UTF-8.
fn entry_names(directory: &Path) -> Option<Vec<String>> {
    let mut names = std::fs::read_dir(directory)
        .ok()?
        .map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect::<Option<Vec<_>>>()?;
    names.sort();
    Some(names)
}

/// The content binding of `path` if it is Cargo's uplifted build-script copy:
/// `build/<package>-<hash>/build-script-<stem>`, a regular file with the same
/// bytes as the regular `build_script_<stem>-<hash>` Cargo compiled next to
/// it. `None` otherwise, or when anything is unreadable.
fn uplifted_copy(path: &Path) -> Option<(Copied, Directory)> {
    let directory = path.parent()?;
    let name = path.file_name()?.to_str()?;
    let (_, hash) = directory.file_name()?.to_str()?.rsplit_once('-')?;
    if !name.starts_with(UPLIFTED_SCRIPT)
        || name.contains('.')
        || hash.is_empty()
        || directory.parent()?.file_name()? != "build"
    {
        return None;
    }
    let copy = std::fs::symlink_metadata(path).ok()?;
    let names = entry_names(directory)?;
    let original = format!("{}-{hash}", name.replace('-', "_"));
    if !names.contains(&original) {
        return None;
    }
    let original = directory.join(original);
    let source = std::fs::symlink_metadata(&original).ok()?;
    if !copy.is_file()
        || !source.is_file()
        || copy.len() != source.len()
        || !super::files::same_content(path, &original).ok()?
    {
        return None;
    }
    let entry = Copied {
        path: path.to_owned(),
        original,
        size: copy.len(),
        mode: identity(&copy)?.2,
    };
    let held = std::fs::symlink_metadata(directory).ok()?;
    if !held.is_dir() {
        return None;
    }
    let directory = Directory {
        path: directory.to_owned(),
        identity: identity(&held)?,
        names,
    };
    Some((entry, directory))
}

/// Stamps, content-bound copies and their directories.
type Bound = (Vec<(PathBuf, Option<Stamp>)>, Vec<Copied>, Vec<Directory>);

/// Cargo target stamps with the uplifted build-script copies whose stamps
/// are too recent (and their directories) moved to content bindings. Any
/// other recent stamp stays for the ordinary guard to refuse.
fn bind_copies(entries: Vec<(PathBuf, Option<Stamp>)>, start: SystemTime) -> Bound {
    let recent = |stamp: &Option<Stamp>| stamp.is_some_and(|stamp| !stamp.settled(start));
    let mut copies = Vec::new();
    let mut directories = Vec::new();
    for (path, stamp) in &entries {
        if recent(stamp)
            && let Some((copy, directory)) = uplifted_copy(path)
        {
            copies.push(copy);
            directories.push(directory);
        }
    }
    if copies.is_empty() {
        return (entries, copies, directories);
    }
    let bound: std::collections::BTreeSet<&Path> = copies
        .iter()
        .map(|copy| copy.path.as_path())
        .chain(directories.iter().map(|directory| directory.path.as_path()))
        .collect();
    let rest = entries
        .iter()
        .filter(|(path, _)| !bound.contains(path.as_path()))
        .cloned()
        .collect();
    (rest, copies, directories)
}

/// The first content-bound copy or directory that no longer holds.
fn copies_mismatch(copies: &[Copied], directories: &[Directory]) -> Option<String> {
    for directory in directories {
        let held = std::fs::symlink_metadata(&directory.path).ok();
        if held.as_ref().filter(|m| m.is_dir()).and_then(identity) != Some(directory.identity)
            || entry_names(&directory.path).as_ref() != Some(&directory.names)
        {
            return Some(format!("changed: {}", directory.path.display()));
        }
    }
    let same = crate::parallel::in_order(copies, |copy| {
        let metadata = std::fs::symlink_metadata(&copy.path).ok()?;
        (metadata.is_file()
            && metadata.len() == copy.size
            && identity(&metadata)?.2 == copy.mode
            && super::files::same_content(&copy.path, &copy.original).ok()?)
        .then_some(())
    });
    copies
        .iter()
        .zip(same)
        .find(|(_, same)| same.is_none())
        .map(|(copy, _)| format!("changed: {}", copy.path.display()))
}

/// What a recorded run still needs to know at its end: everything except
/// its output, which its caller adds with [`Pending::commit`].
pub struct Pending {
    path: PathBuf,
    other_path: PathBuf,
    human_only: bool,
    record: Record,
}

/// Validated empty human output. It has no `ProjectResult` or proof accessor.
pub struct HumanReplay {
    stderr: Vec<u8>,
}

impl HumanReplay {
    /// One validated ordinary Cargo completion line, with elapsed time updated.
    #[must_use]
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

/// An unchanged call's replayed output.
pub struct Replay {
    /// The recorded report, with this call's run identity and wall time.
    pub result: ProjectResult,
    /// Human stdout, exactly as recorded.
    pub stdout: Vec<u8>,
    /// Stderr as recorded, with Cargo's `Finished` time regenerated.
    pub stderr: Vec<u8>,
}

/// The facts a run supplies for its record.
pub(super) struct Facts<'a> {
    pub key: &'a str,
    pub cache: &'a Path,
    pub start: SystemTime,
    pub workspace_lock: PathBuf,
    pub family: &'a Path,
    pub context: &'a Path,
    pub target: &'a Path,
    pub observation: Vec<(PathBuf, Option<Stamp>)>,
    pub identity: &'a [PathBuf],
    /// Receipt directories of the family's other proof contexts, which a
    /// dependency's reuse may consult.
    pub siblings: Vec<PathBuf>,
    pub result: &'a ProjectResult,
    /// The ordinary runner's original pre-Cargo resolution, never rebuilt
    /// from the stored result or its target rows.
    pub original_request: &'a ResolvedRequest,
    /// The original caller actually displayed human stdout. JSON, job and MCP
    /// consumers cannot record a byte-only human replay.
    pub human: bool,
}

fn disabled() -> bool {
    std::env::var_os("SAIL_MANAGED_NOOP").is_some_and(|value| value == "0")
        // Job workers and observers expect the records a full run writes.
        || [
            "SAIL_JOB_EVENTS",
            "SAIL_PROOF_EVENTS",
            "SAIL_JOB_GUARD_PARENT",
            "SAIL_MANAGED_FORCE",
        ]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

/// One line per decision in the file `SAIL_MANAGED_NOOP_TRACE` names, for
/// tests and diagnosis. Never written to the run's own output.
fn trace(line: &str) {
    use std::io::Write as _;
    if let Some(path) = std::env::var_os("SAIL_MANAGED_NOOP_TRACE")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// Retain the exact recording refusal in the existing optional decision trace.
pub(super) fn unrecorded(reason: &str) {
    trace(&format!("not recorded: {reason}"));
}

/// Variables that make up the invocation's key: every variable the context
/// binds, every Sail variable, and the ones that select tools and terminal
/// rendering.
fn key_environment(
    variables: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) -> Option<Vec<(String, String)>> {
    let mut selected = Vec::new();
    for (name, value) in variables {
        if super::environment::input_environment_key(&name)
            || name.as_encoded_bytes().starts_with(b"SAIL_")
            || ["PATH", "HOME", "TERM", "PWD", "CARGO_TARGET_DIR"]
                .iter()
                .any(|n| name == *n)
        {
            selected.push((name.into_string().ok()?, value.into_string().ok()?));
        }
    }
    selected.sort();
    Some(selected)
}

/// The invocation's key, or `None` when it cannot be replayed.
#[must_use]
pub fn key(config: &Config, json: bool) -> Option<String> {
    use std::io::IsTerminal as _;
    if disabled() || config.force || config.request_capture.is_some() {
        return None;
    }
    if super::research::selected().ok()? != super::research::Mode::Off {
        return None;
    }
    let workspace = std::path::absolute(&config.workspace).ok()?;
    let cache = workspace.join(&config.cache);
    let identity = serde_json::to_vec(&(
        ("sail-noop-key-v1", VERSION, &workspace, &cache),
        (
            &config.sail,
            &config.cargo_verus,
            &config.cargo,
            format!("{:?}", config.cargo_toolchain),
            &config.shim,
            &config.z3,
            config.socket.is_some(),
        ),
        (&config.args, config.attribution_logs, json),
        (
            std::io::stdout().is_terminal(),
            std::io::stderr().is_terminal(),
        ),
        key_environment(std::env::vars_os())?,
    ))
    .ok()?;
    Some(super::files::digest(&identity))
}

fn record_path(cache: &Path, key: &str) -> PathBuf {
    cache.join("noop").join(format!("{key}.json"))
}

fn human_record_path(cache: &Path, key: &str) -> PathBuf {
    cache
        .join("noop")
        .join(format!("human-empty-v{HUMAN_VERSION}-{key}.json"))
}

/// Every entry below `root`, without following links, sorted; `skip` names
/// direct children of `root` that are left out.
fn walk(root: &Path, skip: &[&str]) -> Option<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).ok()? {
            let entry = entry.ok()?;
            if directory == root && skip.iter().any(|name| entry.file_name() == *name) {
                continue;
            }
            let path = entry.path();
            if entry.file_type().ok()?.is_dir() {
                pending.push(path.clone());
            }
            found.push(path);
        }
    }
    found.sort();
    Some(found)
}

/// The `lstat` stamp, `Ok(None)` for an absent path.
fn lstat(path: &Path) -> std::io::Result<Option<Stamp>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(Stamp::of(&metadata))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// A cache directory, the children left out, and its sorted entries.
type Tree = (PathBuf, Vec<String>, Vec<PathBuf>);
/// Cache trees, the records compared by content, and the stamped rest.
type CacheState = (
    Vec<Tree>,
    Vec<(PathBuf, String)>,
    Vec<(PathBuf, Option<Stamp>)>,
);

/// Cache trees compared by content (small regular files) or stamp.
fn cache_state(trees: &[(PathBuf, &[&str])]) -> Option<CacheState> {
    let mut listed = Vec::new();
    let mut contents = Vec::new();
    let mut stamps = Vec::new();
    for (root, skip) in trees {
        let entries = walk(root, skip)?;
        for path in &entries {
            let metadata = std::fs::symlink_metadata(path).ok()?;
            if metadata.is_file() && metadata.len() <= CONTENT_LIMIT {
                contents.push(path.clone());
            } else if !metadata.is_dir() {
                stamps.push((path.clone(), Some(Stamp::of(&metadata))));
            }
        }
        listed.push((
            root.clone(),
            skip.iter().map(|name| (*name).to_owned()).collect(),
            entries,
        ));
    }
    let digests = crate::parallel::in_order(&contents, |path| super::files::file_digest(path).ok());
    let contents = contents
        .into_iter()
        .zip(digests)
        .map(|(path, digest)| Some((path, digest?)))
        .collect::<Option<_>>()?;
    Some((listed, contents, stamps))
}

fn cache_trees<'a>(
    cache: &Path,
    family: &Path,
    context: &Path,
    siblings: &[PathBuf],
) -> Vec<(PathBuf, &'a [&'a str])> {
    let mut trees = vec![
        (context.to_owned(), &["runs"][..]),
        (family.to_owned(), &["target"][..]),
        (cache.join("runtime-identities"), &[][..]),
    ];
    trees.extend(siblings.iter().map(|receipts| (receipts.clone(), &[][..])));
    trees
}

fn rustup_settings() -> Option<PathBuf> {
    std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")))
        .map(|home| home.join("settings.toml"))
}

/// Gather a record at the end of a run that ended Current without starting a
/// verifier. `None` when any part cannot be observed exactly or is too recent.
#[allow(
    clippy::too_many_lines,
    reason = "Keep both admission policies beside the complete original observation vector."
)]
pub(super) fn prepare(facts: Facts<'_>) -> Option<Pending> {
    let human_only = sail_api::acceptance::empty_human_candidate(
        facts.original_request,
        facts.result,
        facts.human,
    );
    // Both branches bind the independently captured original request. Partial
    // evidence uses its own scope contract and cannot acquire whole authority.
    if human_only {
        // This candidate cannot supply proof authority. Actual output bytes are
        // checked after the caller finishes teeing them, before any write.
    } else if sail_api::acceptance::focused_command(&facts.original_request.args) {
        if sail_api::acceptance::complete_exact_partial_request(
            facts.original_request,
            facts.result,
        )
        .is_none()
        {
            unrecorded("original exact-partial request has no admitted result");
            return None;
        }
    } else if sail_api::acceptance::complete_request(facts.original_request, facts.result).is_none()
    {
        unrecorded("original whole request has no admitted result");
        return None;
    }
    let origins = if human_only {
        Vec::new()
    } else {
        let Some(origins) = origins::capture(facts.context, facts.result) else {
            unrecorded("original proof producers unavailable or mismatched");
            return None;
        };
        origins
    };
    let mut lstat_entries = facts.observation;
    let target = walk(facts.target, &[])?;
    let target_stamps = crate::parallel::in_order(&target, |path| lstat(path));
    let target = target
        .into_iter()
        .zip(target_stamps)
        .map(|(path, stamp)| Some((path, stamp.ok()?)))
        .collect::<Option<Vec<_>>>()?;
    let (target, copies, directories) = bind_copies(target, facts.start);
    lstat_entries.extend(target);
    if let Some(settings) = rustup_settings() {
        lstat_entries.push((settings.clone(), lstat(&settings).ok()?));
    }
    let identity_stamps = crate::parallel::in_order(facts.identity, |path| {
        std::fs::metadata(path).ok().map(|m| Stamp::of(&m))
    });
    let identity_links = crate::parallel::in_order(facts.identity, |path| lstat(path));
    let mut stat_entries = Vec::new();
    for ((path, stamp), link) in facts
        .identity
        .iter()
        .zip(identity_stamps)
        .zip(identity_links)
    {
        stat_entries.push((path.clone(), stamp?));
        lstat_entries.push((path.clone(), link.ok()?));
    }
    let (trees, contents, cache_stamps) = cache_state(&cache_trees(
        facts.cache,
        facts.family,
        facts.context,
        &facts.siblings,
    ))?;
    lstat_entries.extend(cache_stamps);
    let recent = lstat_entries
        .iter()
        .filter_map(|(path, stamp)| stamp.map(|stamp| (path, stamp)))
        .chain(stat_entries.iter().map(|(path, stamp)| (path, *stamp)))
        .find(|(_, stamp)| !stamp.settled(facts.start));
    if let Some((path, _)) = recent {
        trace(&format!(
            "not recorded: {} changed too recently",
            path.display()
        ));
        return None;
    }
    let environment =
        super::environment::values(&super::environment::declared_names(facts.target)?);
    Some(Pending {
        path: if human_only {
            human_record_path(facts.cache, facts.key)
        } else {
            record_path(facts.cache, facts.key)
        },
        other_path: if human_only {
            record_path(facts.cache, facts.key)
        } else {
            human_record_path(facts.cache, facts.key)
        },
        human_only,
        record: Record {
            version: VERSION,
            key: facts.key.to_owned(),
            locks: (facts.workspace_lock, facts.family.join("ownership.lock")),
            runs: facts.context.join("runs"),
            lstat: lstat_entries,
            stat: stat_entries,
            trees,
            contents,
            copies,
            directories,
            environment,
            origins,
            original_request: facts.original_request.clone(),
            result: facts.result.clone(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        },
    })
}

impl Pending {
    /// Store the record with what the run printed. A failure only means the
    /// next unchanged call takes the ordinary path.
    pub fn commit(mut self, stdout: Vec<u8>, stderr: Vec<u8>) {
        self.record.stdout = stdout;
        self.record.stderr = stderr;
        if self.human_only
            && !sail_api::acceptance::supported_empty_human_bytes(
                &self.record.stdout,
                &self.record.stderr,
            )
        {
            unrecorded("human no-op output is not empty stdout plus one Cargo footer");
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(&self.other_path);
            return;
        }
        let description = format!(
            "recorded: {} stamps, {} contents, {} copies{}",
            self.record.lstat.len() + self.record.stat.len(),
            self.record.contents.len(),
            self.record.copies.len(),
            if self.human_only {
                ", empty human output only"
            } else {
                ""
            }
        );
        let temporary_name = format!(".{}.{}", self.record.key, std::process::id());
        let serialized = if self.human_only {
            serde_json::to_vec(&HumanRecord {
                kind: HUMAN_KIND.to_owned(),
                version: HUMAN_VERSION,
                observation: self.record,
            })
        } else {
            serde_json::to_vec(&self.record)
        };
        let Ok(bytes) = serialized else {
            return;
        };
        let Some(parent) = self.path.parent() else {
            return;
        };
        let temporary = parent.join(temporary_name);
        if std::fs::create_dir_all(parent).is_err()
            || std::fs::write(&temporary, bytes).is_err()
            || std::fs::rename(&temporary, &self.path).is_err()
        {
            let _ = std::fs::remove_file(&temporary);
        } else {
            let _ = std::fs::remove_file(&self.other_path);
            trace(&description);
        }
    }
}

/// Remove any record of `key`: an invocation that did not end unchanged
/// must not leave an older record behind.
pub fn forget(cache: &Path, key: &str) {
    let _ = std::fs::remove_file(record_path(cache, key));
    let _ = std::fs::remove_file(human_record_path(cache, key));
}

/// Cargo's elapsed-time rendering (`cargo::util::elapsed`).
fn elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}.{:02}s", seconds, duration.subsec_nanos() / 10_000_000)
    }
}

/// `stderr` with the elapsed time of Cargo's `Finished` line replaced.
fn regenerate_finished(stderr: &[u8], duration: Duration) -> Vec<u8> {
    let mut out = Vec::with_capacity(stderr.len());
    for line in stderr.split_inclusive(|b| *b == b'\n') {
        let plain: Vec<u8> = strip_ansi(line);
        let text = String::from_utf8_lossy(&plain);
        let is_finished =
            text.trim_start().starts_with("Finished ") && text.contains(" target(s) in ");
        match (is_finished, find_last(line, b" in ")) {
            (true, Some(at)) => {
                let end = line
                    .iter()
                    .rposition(|b| !matches!(b, b'\n' | b'\r'))
                    .map_or(line.len(), |i| i + 1);
                out.extend_from_slice(&line[..at + 4]);
                out.extend_from_slice(elapsed(duration).as_bytes());
                out.extend_from_slice(&line[end..]);
            }
            _ => out.extend_from_slice(line),
        }
    }
    out
}

fn find_last(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

fn strip_ansi(line: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(line.len());
    let mut i = 0;
    while i < line.len() {
        if line[i] == 0x1b && line.get(i + 1) == Some(&b'[') {
            i += 2;
            while i < line.len() && !(0x40..=0x7e).contains(&line[i]) {
                i += 1;
            }
            i += 1;
        } else {
            out.push(line[i]);
            i += 1;
        }
    }
    out
}

fn lock(path: &Path, shared: bool) -> Option<std::fs::File> {
    super::projects::lock(
        path,
        shared,
        std::time::Instant::now() + Duration::from_mins(30),
    )
    .ok()
}

/// Replays only empty human stdout and one validated Cargo completion line.
/// Callers receive bytes, never a `ProjectResult` or verification authority.
/// JSON, jobs, MCP, independent-request capture and engine paths keep their
/// ordinary result-producing execution or checked whole/partial proof replay.
#[must_use]
pub fn replay_empty_human_output(
    config: &Config,
    started: std::time::Instant,
) -> Option<HumanReplay> {
    if config.request_capture.is_some() {
        return None;
    }
    let key = key(config, false)?;
    let workspace = std::path::absolute(&config.workspace).ok()?;
    if !human_record_path(&workspace.join(&config.cache), &key).is_file() {
        return None;
    }
    let mut reason = None;
    let replay = (|| {
        let preflight = super::runner::replay_preflight(config).ok()?;
        let location = ReplayLocation {
            cache: &preflight.config.cache,
            context: &preflight.context,
            workspace_lock: &preflight.workspace_lock,
            family: &preflight.family,
        };
        let _workspace = lock(location.workspace_lock, true)?;
        let _family = lock(&location.family.join("ownership.lock"), false)?;
        preflight
            .with_current_request(|current, paths, readback| {
                let bytes = std::fs::read(human_record_path(location.cache, &key)).ok()?;
                let saved: HumanRecord = serde_json::from_slice(&bytes).ok()?;
                let record = &saved.observation;
                if saved.kind != HUMAN_KIND
                    || saved.version != HUMAN_VERSION
                    || record.version != VERSION
                    || record.key != key
                    || !record.origins.is_empty()
                    || record.locks.0 != location.workspace_lock
                    || record.locks.1 != location.family.join("ownership.lock")
                    || record.runs != location.context.join("runs")
                {
                    return None;
                }
                if let Some(changed) = mismatch(record) {
                    reason = Some(changed);
                    return None;
                }
                let plain = sail_api::acceptance::admit_empty_human_output(
                    &record.original_request,
                    current,
                    &record.result,
                    true,
                    &record.stdout,
                    &record.stderr,
                    Some(paths),
                    Some(readback),
                );
                let stderr = if let Some(admitted) = plain {
                    let mut duration = elapsed(started.elapsed()).into_bytes();
                    duration.push(b'\n');
                    sail_api::acceptance::render_empty_human_output(&admitted, &duration)?
                } else {
                    let admitted = sail_api::acceptance::admit_colored_empty_human_output(
                        &record.original_request,
                        current,
                        &record.result,
                        true,
                        &record.stdout,
                        &record.stderr,
                        Some(paths),
                        Some(readback),
                    )?;
                    let mut duration = elapsed(started.elapsed()).into_bytes();
                    duration.push(b'\n');
                    sail_api::acceptance::render_colored_empty_human_output(&admitted, &duration)?
                };
                if !sail_api::acceptance::supported_empty_human_bytes(&[], &stderr) {
                    return None;
                }
                Some(HumanReplay { stderr })
            })
            .ok()??
    })();
    trace("replay kind: empty human output only");
    if replay.is_some() {
        trace("hit");
    } else {
        trace(&format!(
            "miss: {}",
            reason
                .as_deref()
                .unwrap_or("human-output request or readback unavailable")
        ));
    }
    replay
}

/// Whether this invocation has a byte-only candidate to attempt. Presence
/// grants no authority: an unreadable, changed or invalid candidate still runs
/// ordinary verification. It only prevents a second proof-cache miss from
/// overwriting the decision trace of the candidate already checked.
#[must_use]
pub fn has_empty_human_candidate(config: &Config) -> bool {
    let Some(key) = key(config, false) else {
        return false;
    };
    let Ok(workspace) = std::path::absolute(&config.workspace) else {
        return false;
    };
    human_record_path(&workspace.join(&config.cache), &key).is_file()
}

/// The replay for an unchanged call of `key`, when every recorded
/// observation still matches.
#[must_use]
pub fn replay(config: &Config, key: &str, started: std::time::Instant) -> Option<Replay> {
    // The caller requires a new independent request record. A saved result
    // cannot create that observation, even when its output could be replayed.
    if config.request_capture.is_some() {
        return None;
    }
    let workspace = std::path::absolute(&config.workspace).ok()?;
    let cache = workspace.join(&config.cache);
    if !record_path(&cache, key).is_file() {
        trace("miss: no record");
        return None;
    }
    // Independently resolve scope and current runtime before deserializing any
    // saved result. Unavailable metadata or readback falls through normally.
    let preflight = super::runner::replay_preflight(config).ok()?;
    let location = ReplayLocation {
        cache: &preflight.config.cache,
        context: &preflight.context,
        workspace_lock: &preflight.workspace_lock,
        family: &preflight.family,
    };
    // Match ordinary execution's workspace -> family lock order. The saved
    // record is not allowed to choose either lock or the new run directory.
    let _workspace = lock(location.workspace_lock, true)?;
    let _family = lock(&location.family.join("ownership.lock"), false)?;
    preflight
        .with_current_request(|current, paths, readback| {
            replay_bound(&location, key, started, current, paths, readback)
        })
        .ok()??
}

struct ReplayLocation<'a> {
    cache: &'a Path,
    context: &'a Path,
    workspace_lock: &'a Path,
    family: &'a Path,
}

fn replay_bound(
    location: &ReplayLocation<'_>,
    key: &str,
    started: std::time::Instant,
    current: &ResolvedRequest,
    paths: &sail_api::acceptance::RequestPathBinding<'_>,
    readback: &sail_acceptance::inputs::GuardedReadback<'_>,
) -> Option<Replay> {
    let Ok(bytes) = std::fs::read(record_path(location.cache, key)) else {
        trace("miss: no record");
        return None;
    };
    let record: Record = serde_json::from_slice(&bytes).ok()?;
    if record.version != VERSION || record.key != key {
        trace("miss: record version");
        return None;
    }
    if record.locks.0 != location.workspace_lock
        || record.locks.1 != location.family.join("ownership.lock")
        || record.runs != location.context.join("runs")
    {
        trace("miss: independently resolved proof location differs");
        return None;
    }
    if let Some(reason) = mismatch(&record) {
        trace(&format!("miss: {reason}"));
        return None;
    }
    if !origins::intact(&record.origins, &record.result) {
        trace("miss: original proof producers unavailable or mismatched");
        return None;
    }
    // Consume the independently scoped current-path/readback witnesses through
    // the shared conjunction after the unchanged ordinary origin/history gate.
    // The admitted token is declared-domain evidence, not full service authority.
    let Some(admitted) = sail_api::acceptance::admit_declared_replay(
        record.result,
        &record.original_request,
        current,
        Some(paths),
        Some(readback),
    ) else {
        trace("miss: independent request or guarded original revision differs");
        return None;
    };
    let mut result = sail_api::acceptance::present_declared_replay(admitted);
    result.execution.wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    // Keep the project's last-use time, as a full run's result does.
    let directory = location.context.join("runs").join(&current.run_id);
    if std::fs::create_dir_all(&directory).is_err()
        || super::files::json(&directory.join("result.json"), &result).is_err()
    {
        return None;
    }
    trace("hit");
    Some(Replay {
        result,
        stdout: record.stdout,
        stderr: regenerate_finished(&record.stderr, started.elapsed()),
    })
}

/// The first recorded `lstat` stamp that no longer holds.
fn stamps_mismatch(recorded: &[(PathBuf, Option<Stamp>)]) -> Option<String> {
    let now = crate::parallel::in_order(recorded, |(path, _)| lstat(path));
    recorded
        .iter()
        .zip(now)
        .find(|((_, recorded), now)| now.as_ref().ok() != Some(recorded))
        .map(|((path, _), _)| format!("changed: {}", path.display()))
}

/// The first recorded observation that no longer holds.
fn mismatch(record: &Record) -> Option<String> {
    if let Some(reason) = stamps_mismatch(&record.lstat) {
        return Some(reason);
    }
    let now = crate::parallel::in_order(&record.stat, |(path, _)| {
        std::fs::metadata(path).ok().map(|m| Stamp::of(&m))
    });
    for ((path, recorded), now) in record.stat.iter().zip(now) {
        if now.as_ref() != Some(recorded) {
            return Some(format!("changed: {}", path.display()));
        }
    }
    if let Some(reason) = copies_mismatch(&record.copies, &record.directories) {
        return Some(reason);
    }
    for (root, skip, entries) in &record.trees {
        let skip: Vec<&str> = skip.iter().map(String::as_str).collect();
        if walk(root, &skip).as_ref() != Some(entries) {
            return Some(format!("changed cache records: {}", root.display()));
        }
    }
    let digests = crate::parallel::in_order(&record.contents, |(path, _)| {
        super::files::file_digest(path).ok()
    });
    for ((path, recorded), now) in record.contents.iter().zip(digests) {
        if now.as_ref() != Some(recorded) {
            return Some(format!("changed: {}", path.display()));
        }
    }
    if super::environment::values(&record.environment.keys().cloned().collect())
        != record.environment
    {
        return Some("a declared variable changed".to_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_time_is_the_only_regenerated_text() {
        let recorded = b"warning: something\n    \x1b[1m\x1b[32mFinished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 0.13s\n";
        let replayed = regenerate_finished(recorded, Duration::from_millis(42));
        assert_eq!(
            replayed,
            b"warning: something\n    \x1b[1m\x1b[32mFinished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 0.04s\n"
        );
        let plain = b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 02s\n";
        assert_eq!(
            regenerate_finished(plain, Duration::from_secs(61)),
            b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 01s\n"
        );
        let other = b"note: verified in 3s\n";
        assert_eq!(regenerate_finished(other, Duration::ZERO), other);
    }

    #[test]
    fn every_tool_and_sail_variable_is_part_of_the_key() {
        let base = || {
            vec![
                ("PATH".into(), "/usr/bin".into()),
                ("HOME".into(), "/home/u".into()),
                ("EDITOR".into(), "vi".into()),
            ]
        };
        let selection = |extra: &[(&str, &str)]| {
            let mut variables: Vec<(std::ffi::OsString, std::ffi::OsString)> = base();
            variables.extend(extra.iter().map(|(n, v)| ((*n).into(), (*v).into())));
            key_environment(variables)
        };
        let plain = selection(&[]);
        assert!(plain.is_some());
        for changed in [
            ("RUSTFLAGS", "--cfg probe"),
            ("RUSTUP_TOOLCHAIN", "1.99.0"),
            ("CARGO_TARGET_DIR", "/elsewhere"),
            ("CARGO_TERM_COLOR", "always"),
            ("VERUS_Z3_PATH", "/z3"),
            ("SAIL_EXPLAIN", "1"),
            ("LD_LIBRARY_PATH", "/lib"),
            ("TERM", "dumb"),
            ("NO_COLOR", "1"),
        ] {
            assert_ne!(selection(&[changed]), plain, "{changed:?}");
        }
        // Variables no tool or Sail reads keep the key.
        assert_eq!(selection(&[("EDITOR", "vi"), ("OLDPWD", "/tmp")]), plain);
        let mut moved = base();
        moved[0].1 = "/opt/bin".into();
        assert_ne!(key_environment(moved), plain);
    }

    #[test]
    fn original_request_capture_cannot_replay_without_a_new_capture() {
        let mut config = Config {
            workspace: PathBuf::from("/fixture/workspace"),
            cache: PathBuf::from("/fixture/cache"),
            sail: PathBuf::from("/fixture/sail"),
            cargo_verus: PathBuf::from("/fixture/cargo-verus"),
            cargo: PathBuf::from("/fixture/cargo"),
            cargo_toolchain: super::super::CargoToolchain::Caller,
            shim: PathBuf::from("/fixture/sail-z3"),
            z3: PathBuf::from("/fixture/z3"),
            socket: None,
            args: Vec::new(),
            force: false,
            attribution_logs: false,
            request_capture: None,
        };
        assert!(
            key(&config, false).is_some(),
            "ordinary fixture must be replay eligible"
        );
        config.request_capture = Some(super::super::RequestCaptureDestination {
            path: PathBuf::from("/fixture/new-request.json"),
            worker_id: "new-request-worker".into(),
        });
        assert!(
            key(&config, false).is_none(),
            "a replay cannot supply this original request capture"
        );
    }

    #[test]
    fn recent_status_changes_are_not_settled() -> Result<(), Box<dyn std::error::Error>> {
        let start = SystemTime::now();
        let at = |age: Duration| {
            let time = start - age;
            let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
            let mut fields = [0i64; 8];
            fields[6] = i64::try_from(since.as_secs()).unwrap_or_default();
            fields[7] = i64::from(since.subsec_nanos());
            serde_json::from_value::<Stamp>(serde_json::json!(fields))
        };
        assert!(at(Duration::from_secs(3))?.settled(start));
        assert!(!at(Duration::from_millis(1500))?.settled(start));
        assert!(!at(Duration::ZERO)?.settled(start));
        Ok(())
    }

    /// A Cargo target with one build-script unit: the original, its
    /// dep-info and the uplifted copy.
    struct Unit {
        root: PathBuf,
        directory: PathBuf,
        original: PathBuf,
        copy: PathBuf,
    }

    impl Unit {
        fn new(name: &str) -> std::io::Result<Self> {
            Self::script(name, "build")
        }

        /// A unit whose build file has the stem `stem` (`build = "build/main.rs"`).
        fn script(name: &str, stem: &str) -> std::io::Result<Self> {
            let root =
                std::env::temp_dir().join(format!("sail-noop-copy-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let directory = root.join("debug/build/proc-macro2-e4fc813f5dbf0dcb");
            std::fs::create_dir_all(&directory)?;
            let original = directory.join(format!("build_script_{stem}-e4fc813f5dbf0dcb"));
            std::fs::write(&original, b"\x7fELF build script")?;
            std::fs::write(
                directory.join(format!("build_script_{stem}-e4fc813f5dbf0dcb.d")),
                b"deps",
            )?;
            let copy = directory.join(format!("build-script-{stem}"));
            Ok(Self {
                root,
                directory,
                original,
                copy,
            })
        }

        /// What Cargo does on every macOS run: remove the copy, copy again.
        fn recopy(&self) -> std::io::Result<()> {
            let _ = std::fs::remove_file(&self.copy);
            std::fs::copy(&self.original, &self.copy).map(|_| ())
        }

        /// The target's stamps, bound as a recording run binds them. Every
        /// stamp here is as recent as Cargo's re-created copy.
        fn bind(&self) -> std::io::Result<Bound> {
            let mut entries = Vec::new();
            for path in walk(&self.root, &[]).ok_or_else(|| std::io::Error::other("walk"))? {
                let stamp = lstat(&path)?;
                entries.push((path, stamp));
            }
            Ok(bind_copies(
                entries,
                SystemTime::now() + Duration::from_secs(1),
            ))
        }
    }

    fn replayed(bound: &Bound) -> Option<String> {
        stamps_mismatch(&bound.0).or_else(|| copies_mismatch(&bound.1, &bound.2))
    }

    impl Drop for Unit {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_recopied_build_script_with_the_same_bytes_replays() -> std::io::Result<()> {
        let unit = Unit::new("same")?;
        unit.recopy()?;
        let bound = unit.bind()?;
        // The copy and its directory are bound by content; the original,
        // its dep-info and the other directories keep their stamps.
        assert_eq!(bound.1.len(), 1);
        assert_eq!(bound.1[0].path, unit.copy);
        assert_eq!(bound.1[0].original, unit.original);
        assert_eq!(bound.2.len(), 1);
        assert_eq!(bound.2[0].path, unit.directory);
        let stamped: Vec<&Path> = bound.0.iter().map(|(path, _)| path.as_path()).collect();
        assert!(!stamped.contains(&unit.copy.as_path()));
        assert!(!stamped.contains(&unit.directory.as_path()));
        assert!(stamped.contains(&unit.original.as_path()));
        assert!(stamped.contains(&unit.root.join("debug/build").as_path()));
        assert_eq!(replayed(&bound), None);
        // Cargo's next run re-creates the copy: a new inode and ctime, and
        // a changed directory stamp, but the same bytes.
        unit.recopy()?;
        assert_eq!(replayed(&bound), None);
        // A build file other than `build.rs` (openssl-sys's `build/main.rs`).
        let unit = Unit::script("main", "main")?;
        unit.recopy()?;
        let bound = unit.bind()?;
        assert_eq!(bound.1.len(), 1);
        assert_eq!(bound.1[0].original, unit.original);
        unit.recopy()?;
        assert_eq!(replayed(&bound), None);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_changed_copy_misses() -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt as _;
        let unit = Unit::new("changed")?;
        unit.recopy()?;
        let bound = unit.bind()?;
        // Same size, different bytes.
        std::fs::write(&unit.copy, b"\x7fELF build scrip!")?;
        assert_eq!(
            replayed(&bound),
            Some(format!("changed: {}", unit.copy.display()))
        );
        // Another size.
        std::fs::write(&unit.copy, b"\x7fELF")?;
        assert!(replayed(&bound).is_some());
        // The same bytes with another mode.
        unit.recopy()?;
        assert_eq!(replayed(&bound), None);
        std::fs::set_permissions(&unit.copy, std::fs::Permissions::from_mode(0o600))?;
        assert!(replayed(&bound).is_some());
        // A link to the original in place of the copy.
        std::fs::remove_file(&unit.copy)?;
        std::os::unix::fs::symlink(&unit.original, &unit.copy)?;
        assert!(replayed(&bound).is_some());
        // No copy at all.
        std::fs::remove_file(&unit.copy)?;
        assert!(replayed(&bound).is_some());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_changed_original_misses() -> std::io::Result<()> {
        let unit = Unit::new("original")?;
        unit.recopy()?;
        let bound = unit.bind()?;
        // A rebuilt script, uplifted again: both change together.
        std::fs::write(&unit.original, b"\x7fELF other script!")?;
        unit.recopy()?;
        assert_eq!(
            replayed(&bound),
            Some(format!("changed: {}", unit.original.display()))
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn any_other_change_in_the_directory_misses() -> std::io::Result<()> {
        let unit = Unit::new("extra")?;
        unit.recopy()?;
        let bound = unit.bind()?;
        let extra = unit.directory.join("extra");
        std::fs::write(&extra, b"")?;
        assert_eq!(
            replayed(&bound),
            Some(format!("changed: {}", unit.directory.display()))
        );
        std::fs::remove_file(&extra)?;
        assert_eq!(replayed(&bound), None);
        // A renamed entry.
        let info = unit.directory.join("build_script_build-e4fc813f5dbf0dcb.d");
        std::fs::rename(&info, unit.directory.join("renamed.d"))?;
        assert!(replayed(&bound).is_some());
        // The directory replaced by one with the same entries.
        let unit = Unit::new("replaced")?;
        unit.recopy()?;
        let bound = unit.bind()?;
        let moved = unit.root.join("moved");
        std::fs::rename(&unit.directory, &moved)?;
        std::fs::create_dir(&unit.directory)?;
        for name in [
            "build_script_build-e4fc813f5dbf0dcb",
            "build_script_build-e4fc813f5dbf0dcb.d",
            "build-script-build",
        ] {
            std::fs::rename(moved.join(name), unit.directory.join(name))?;
        }
        assert!(replayed(&bound).is_some());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_copy_without_its_original_keeps_the_ordinary_guard() -> std::io::Result<()> {
        let unit = Unit::new("missing")?;
        unit.recopy()?;
        std::fs::remove_file(&unit.original)?;
        let bound = unit.bind()?;
        assert!(bound.1.is_empty() && bound.2.is_empty());
        assert!(bound.0.iter().any(|(path, _)| *path == unit.copy));
        assert!(bound.0.iter().any(|(path, _)| *path == unit.directory));
        // An original of another unit is not this copy's.
        let unit = Unit::new("another")?;
        unit.recopy()?;
        std::fs::rename(
            &unit.original,
            unit.directory.join("build_script_build-0000"),
        )?;
        assert_eq!(unit.bind()?.1, []);
        // Different bytes are not Cargo's copy.
        let unit = Unit::new("different")?;
        std::fs::write(&unit.copy, b"something else")?;
        assert_eq!(unit.bind()?.1, []);
        // Nor is a file of that name outside `build/<unit>/`.
        let unit = Unit::new("elsewhere")?;
        let other = unit.root.join("debug/deps");
        std::fs::create_dir_all(&other)?;
        std::fs::copy(
            &unit.original,
            other.join("build_script_build-e4fc813f5dbf0dcb"),
        )?;
        std::fs::copy(&unit.original, other.join("build-script-build"))?;
        let bound = unit.bind()?;
        assert_eq!(bound.1.len(), 0);
        Ok(())
    }
}

#[cfg(test)]
#[path = "noop_authority_tests.rs"]
mod authority_tests;
