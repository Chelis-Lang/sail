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
//! - caller values for every variable a unit declares (`env-dep`,
//!   `rerun-if-env-changed`) or an admitted proof receipt requires, including
//!   ghost-only names absent from dep-info.
//!
//! The invocation itself (arguments, output mode, the colour Sail asks Cargo
//! for, the selected tools, the daemon socket and every environment variable
//! the tools or Sail read, terminal capabilities included) is the record's
//! key. A run whose output holds Cargo's transient status lines is never
//! recorded. A stamp is recorded only when its ctime is at least
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
use sail_api::project::ProjectResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Version 5 keys the terminal-capability variables, the colour Sail asks
/// Cargo for and the exact daemon socket, and never records Cargo's
/// transient status lines (#571, #573, #574). Version 4 also bound names
/// required by proof receipts, including ghost reads absent from dep-info
/// (#552).
const VERSION: u32 = 5;
/// Cache records up to this size are compared by content; larger files
/// (the private executable aliases) by stamp.
const CONTENT_LIMIT: u64 = 16 << 20;

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
    /// Caller values for unit declarations and admitted proof-receipt
    /// requirements, including ghost-only names absent from dep-info.
    environment: BTreeMap<String, Option<Vec<u8>>>,
    result: ProjectResult,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
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
    record: Record,
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
    /// Caller-variable requirements captured from the actual admitted proof
    /// receipts. Unknown requirements for any included report make recording
    /// ineligible; omitted targets confer no report authority.
    pub receipt_environment: Option<std::collections::BTreeSet<String>>,
    pub result: &'a ProjectResult,
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

/// What the pinned Cargo's terminal detection reads besides `TERM` (cargo
/// 1.99.1, `crates/cargo-util-terminal/src/shell.rs`): `anstyle-query` 1.1.5
/// through `anstream` 1.0.0 for colour (`CI`, `CLICOLOR`, `CLICOLOR_FORCE`,
/// `COLORTERM`, `NO_COLOR`), and `supports-hyperlinks` 3.2.0 for OSC 8 links
/// (`COLORTERM`, `DOMTERM`, `FORCE_HYPERLINK`, `KONSOLE_VERSION`,
/// `TERM_PROGRAM`, `VTE_VERSION`, `WT_SESSION`). Cargo writes to Sail's
/// pipes, but `CLICOLOR_FORCE` colours a pipe and the hyperlink decision
/// reads no stream at all, so each changes what a run prints (#573).
const TERMINAL_CAPABILITIES: &[&str] = &[
    "CI",
    "CLICOLOR",
    "CLICOLOR_FORCE",
    "COLORTERM",
    "DOMTERM",
    "FORCE_HYPERLINK",
    "KONSOLE_VERSION",
    "NO_COLOR",
    "TERM_PROGRAM",
    "VTE_VERSION",
    "WT_SESSION",
];

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
                .chain(TERMINAL_CAPABILITIES)
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
    if disabled() || config.force {
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
            // The exact value Cargo's environment receives as
            // `SAIL_DAEMON_SOCKET`, unnormalized (#571).
            &config.socket,
        ),
        (&config.args, config.attribution_logs, json),
        // Cargo and everything it starts write only to Sail's pipes, so the
        // caller's terminals reach the run's output only through the colour
        // Sail asks Cargo for (#573).
        super::runner::invocation_colour(&config.args, !json),
        key_environment(std::env::vars_os())?,
    ))
    .ok()?;
    Some(super::files::digest(&identity))
}

fn record_path(cache: &Path, key: &str) -> PathBuf {
    cache.join("noop").join(format!("{key}.json"))
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
pub(super) fn prepare(facts: Facts<'_>) -> Option<Pending> {
    let required_environment = facts.receipt_environment?;
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
    let mut names = super::environment::declared_names(facts.target)?;
    names.extend(required_environment);
    let environment = super::environment::values(&names);
    Some(Pending {
        path: record_path(facts.cache, facts.key),
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
        if let Some(line) = transient_status(&stderr) {
            // A later stock run would not print it, so this output is not
            // the unchanged call's transcript (#574). Never leave an older
            // record of this key behind either.
            let _ = std::fs::remove_file(&self.path);
            trace(&format!("not recorded: transient Cargo status {line:?}"));
            return;
        }
        self.record.stdout = stdout;
        self.record.stderr = stderr;
        let Ok(bytes) = serde_json::to_vec(&self.record) else {
            return;
        };
        let Some(parent) = self.path.parent() else {
            return;
        };
        let temporary = parent.join(format!(".{}.{}", self.record.key, std::process::id()));
        if std::fs::create_dir_all(parent).is_err()
            || std::fs::write(&temporary, bytes).is_err()
            || std::fs::rename(&temporary, &self.path).is_err()
        {
            let _ = std::fs::remove_file(&temporary);
        } else {
            trace(&format!(
                "recorded: {} stamps, {} contents, {} copies",
                self.record.lstat.len() + self.record.stat.len(),
                self.record.contents.len(),
                self.record.copies.len()
            ));
        }
    }
}

/// Cargo status verbs whose lines depend on state outside the recorded
/// observation, so an unchanged rerun need not print them (cargo 1.99.1):
/// `Blocking` waits on a contended lock (`src/util/flock.rs`, package cache,
/// build and artifact directories); `Updating` refreshes a registry index or
/// git source, `Locking` resolves the lock file, and `Downloading` and
/// `Downloaded` fill the package cache.
const TRANSIENT_STATUS: &[&str] = &[
    "Blocking",
    "Updating",
    "Locking",
    "Downloading",
    "Downloaded",
];

/// The first line of `stderr` that is a transient Cargo status line: Cargo's
/// right-aligned 12-column verb from [`TRANSIENT_STATUS`], ignoring colour,
/// or a `spurious network error` retry warning.
fn transient_status(stderr: &[u8]) -> Option<String> {
    stderr.split(|b| *b == b'\n').find_map(|line| {
        let plain = String::from_utf8_lossy(&strip_ansi(line)).into_owned();
        let status = TRANSIENT_STATUS
            .iter()
            .any(|verb| plain.starts_with(&format!("{verb:>12} ")));
        let retry = plain.starts_with("warning: spurious network error");
        (status || retry).then(|| plain.trim_end().to_owned())
    })
}

/// Remove any record of `key`: an invocation that did not end unchanged
/// must not leave an older record behind.
pub fn forget(cache: &Path, key: &str) {
    let _ = std::fs::remove_file(record_path(cache, key));
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

/// The replay for an unchanged call of `key`, when every recorded
/// observation still matches.
#[must_use]
pub fn replay(config: &Config, key: &str, started: std::time::Instant) -> Option<Replay> {
    let workspace = std::path::absolute(&config.workspace).ok()?;
    let cache = workspace.join(&config.cache);
    let Ok(bytes) = std::fs::read(record_path(&cache, key)) else {
        trace("miss: no record");
        return None;
    };
    let record: Record = serde_json::from_slice(&bytes).ok()?;
    if record.version != VERSION || record.key != key {
        trace("miss: record version");
        return None;
    }
    // The same order as a full run: the shared workspace lock, then the
    // exclusive family ownership, so no run or prune changes the state
    // while it is compared.
    let _workspace = lock(&record.locks.0, true)?;
    let _family = lock(&record.locks.1, false)?;
    if let Some(reason) = mismatch(&record) {
        trace(&format!("miss: {reason}"));
        return None;
    }
    let run_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    let mut result = record.result;
    result.run_id.clone_from(&run_id);
    result.execution.wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    // Keep the project's last-use time, as a full run's result does.
    let directory = record.runs.join(&run_id);
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

    const SHADOW: &str = " __VERUS_DRIVER_SHADOW__";

    fn receipt_fixture_result(root: &Path) -> ProjectResult {
        ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: sail_api::project::SCHEMA_VERSION,
            run_id: "establishing-call".into(),
            workspace: root.to_string_lossy().into_owned(),
            context_sha256: "context".into(),
            input_revision: "revision".into(),
            currentness: sail_api::project::Currentness::Current,
            exit_code: Some(0),
            targets: Vec::new(),
            execution: sail_api::project::Execution {
                verifier_invocations: 0,
                reused_targets: 0,
                wall_ms: 1.0,
            },
            notes: Vec::new(),
            error: None,
            engine: None,
            no_reuse: None,
        }
    }

    /// Exercise preparation and replay in separate callers, without mutating
    /// the parallel test process's environment. The operation file is outside
    /// every recorded tree and is not an input to the invocation key.
    #[test]
    fn receipt_environment_process_probe() -> Result<(), Box<dyn std::error::Error>> {
        let Some(root) = std::env::var_os("SAIL_TEST_NOOP_RECEIPTS") else {
            return Ok(());
        };
        let root = PathBuf::from(root);
        let cache = root.join("cache");
        let family = cache.join("family");
        let context = cache.join("context");
        let target = family.join("target");
        let executable = root.join("sail");
        let config = Config {
            workspace: root.clone(),
            cache: cache.clone(),
            sail: executable.clone(),
            cargo_verus: executable.clone(),
            cargo: executable.clone(),
            cargo_toolchain: crate::managed::CargoToolchain::Caller,
            shim: executable.clone(),
            z3: executable.clone(),
            socket: None,
            args: Vec::new(),
            force: false,
            attribution_logs: true,
        };
        let key = key(&config, true).ok_or("fixture key unavailable")?;
        assert_ne!(Some(key.clone()), super::key(&config, false));
        let operation = std::fs::read_to_string(root.join("operation"))?;
        if operation != "prepare" {
            assert_eq!(key, std::fs::read_to_string(root.join("key"))?);
        }
        if matches!(operation.as_str(), "prepare" | "unknown") {
            let result = receipt_fixture_result(&root);
            let declared = super::super::environment::declared_names(&target)
                .ok_or("fixture dep-info unreadable")?;
            assert!(!declared.contains(SHADOW));
            assert!(declared.contains("SAIL_TEST_NOOP_DEP_ONLY_INPUT"));
            let pending = prepare(Facts {
                key: &key,
                cache: &cache,
                // Unit-test stamps predate this simulated run start; no real
                // tool invocation or native qualification is claimed here.
                start: SystemTime::now() + Duration::from_secs(3),
                workspace_lock: root.join("workspace.lock"),
                family: &family,
                context: &context,
                target: &target,
                observation: Vec::new(),
                identity: std::slice::from_ref(&executable),
                siblings: Vec::new(),
                receipt_environment: (operation == "prepare")
                    .then(|| std::collections::BTreeSet::from([SHADOW.into()])),
                result: &result,
            });
            if operation == "unknown" {
                assert!(pending.is_none(), "unknown admission must not record");
            } else {
                assert!(std::env::var_os(SHADOW).is_none());
                pending.ok_or("actual prepare refused fixture")?.commit(
                    b"verified report\n".to_vec(),
                    b"Finished dev target(s) in 0.01s\n".to_vec(),
                );
                let record: Record =
                    serde_json::from_slice(&std::fs::read(record_path(&cache, &key))?)?;
                assert_eq!(record.version, 5);
                assert_eq!(record.environment.get(SHADOW), Some(&None));
                assert!(
                    record
                        .environment
                        .contains_key("SAIL_TEST_NOOP_DEP_ONLY_INPUT")
                );
                std::fs::write(root.join("key"), &key)?;
            }
        } else {
            if operation == "legacy" {
                let path = record_path(&cache, &key);
                let mut record: Record = serde_json::from_slice(&std::fs::read(&path)?)?;
                record.version = 3;
                std::fs::write(path, serde_json::to_vec(&record)?)?;
            }
            let replay = replay(&config, &key, std::time::Instant::now());
            if operation == "same" {
                let replay = replay.ok_or("unchanged receipt input did not replay")?;
                assert_eq!(replay.stdout, b"verified report\n");
                assert_eq!(replay.result.execution.verifier_invocations, 0);
                assert_ne!(replay.result.run_id, "establishing-call");
            } else {
                assert!(matches!(operation.as_str(), "changed" | "legacy"));
                assert!(replay.is_none(), "changed input or legacy record replayed");
            }
        }
        Ok(())
    }

    #[test]
    fn actual_prepare_and_replay_bind_ghost_only_receipt_names()
    -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-noop-receipt-env-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for directory in [
            "cache/family/target/deps",
            "cache/context/runs",
            "cache/runtime-identities",
        ] {
            std::fs::create_dir_all(root.join(directory))?;
        }
        for path in ["sail", "workspace.lock", "cache/family/ownership.lock"] {
            std::fs::write(root.join(path), "fixture")?;
        }
        std::fs::write(
            root.join("cache/family/target/deps/input.d"),
            "input: input.rs\n# env-dep:SAIL_TEST_NOOP_DEP_ONLY_INPUT\n",
        )?;
        for operation in ["prepare", "same", "changed", "unknown", "legacy"] {
            std::fs::write(root.join("operation"), operation)?;
            let mut command = std::process::Command::new(std::env::current_exe()?);
            command
                .args([
                    "--exact",
                    "managed::noop::tests::receipt_environment_process_probe",
                ])
                .env("SAIL_TEST_NOOP_RECEIPTS", &root)
                .env_remove(SHADOW)
                .env_remove("SAIL_MANAGED_FORCE")
                .env_remove("SAIL_MANAGED_NOOP")
                .env_remove("SAIL_JOB_EVENTS")
                .env_remove("SAIL_JOB_GUARD_PARENT")
                .env_remove("SAIL_PROOF_EVENTS");
            if operation == "changed" {
                command.env(SHADOW, "new-source-visible-value");
            }
            let output = command.output()?;
            assert!(
                output.status.success(),
                "{operation}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

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

    fn key_fixture(socket: Option<&str>) -> Config {
        let root = std::env::temp_dir().join("sail-noop-key-fixture");
        Config {
            workspace: root.clone(),
            cache: root.join("cache"),
            sail: root.join("sail"),
            cargo_verus: root.join("cargo-verus"),
            cargo: root.join("cargo"),
            cargo_toolchain: crate::managed::CargoToolchain::Caller,
            shim: root.join("shim"),
            z3: root.join("z3"),
            socket: socket.map(PathBuf::from),
            args: Vec::new(),
            force: false,
            attribution_logs: false,
        }
    }

    #[test]
    fn the_exact_daemon_socket_is_part_of_the_key() {
        if disabled() {
            // This process's environment never replays.
            return;
        }
        let key_of = |socket| key(&key_fixture(socket), true);
        let socket_a = key_of(Some("daemon-a"));
        assert!(socket_a.is_some());
        assert_eq!(key_of(Some("daemon-a")), socket_a);
        assert_ne!(key_of(Some("daemon-b")), socket_a);
        assert_ne!(key_of(None), socket_a);
        assert_ne!(key_of(None), key_of(Some("")));
        // No normalization: an equivalent spelling is a different value of
        // `SAIL_DAEMON_SOCKET`, so a different key.
        assert_ne!(key_of(Some("./daemon-a")), socket_a);
        assert_ne!(key_of(Some("daemon-a/")), socket_a);
        assert_eq!(key_of(None), key_of(None));
    }

    #[test]
    fn every_terminal_capability_is_part_of_the_key() {
        let base = || -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
            vec![("PATH".into(), "/usr/bin".into())]
        };
        let plain = key_environment(base());
        assert!(plain.is_some());
        for name in TERMINAL_CAPABILITIES {
            let mut variables = base();
            variables.push(((*name).into(), "1".into()));
            let set = key_environment(variables);
            assert_ne!(set, plain, "{name}");
            let mut variables = base();
            variables.push(((*name).into(), "2".into()));
            assert_ne!(key_environment(variables), set, "{name}");
        }
    }

    #[test]
    fn the_key_binds_the_colour_sail_asks_for() {
        use std::io::IsTerminal as _;
        let colour = super::super::runner::invocation_colour;
        let args = |list: &[&str]| list.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
        // JSON output (no shown output) never asks for colour.
        assert!(!colour(&args(&[]), false));
        // An explicit Cargo colour choice is the caller's and is keyed as
        // an argument; one after `--` is the verifier's.
        assert!(!colour(&args(&["--color=never"]), true));
        assert!(!colour(&args(&["--color", "always"]), true));
        let terminal =
            std::io::stderr().is_terminal() && std::env::var_os("CARGO_TERM_COLOR").is_none();
        assert_eq!(colour(&args(&[]), true), terminal);
        assert_eq!(colour(&args(&["--", "--color=never"]), true), terminal);
    }

    #[test]
    fn transient_cargo_status_lines_are_found() {
        for line in [
            "    Blocking waiting for file lock on package cache\n",
            "    Blocking waiting for file lock on build directory\n",
            "\x1b[1m\x1b[36m    Blocking\x1b[0m waiting for file lock on artifact directory\n",
            "    Updating crates.io index\n",
            "     Locking 3 packages to latest compatible versions\n",
            " Downloading crates ...\n",
            "  Downloaded serde v1.0.0\n",
            "\x1b[1m\x1b[32m  Downloaded\x1b[0m 2 crates (100 KB) in 0.5s\n",
            "warning: spurious network error (2 tries remaining): timeout\n",
            "\x1b[1m\x1b[33mwarning\x1b[0m: spurious network error (1 try remaining): reset\n",
        ] {
            let stderr = format!("   Compiling x v0.1.0\n{line}    Finished `dev` profile\n");
            assert!(transient_status(stderr.as_bytes()).is_some(), "{line:?}");
        }
        for stderr in [
            "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.20s\n",
            "verification results:: 3 verified, 0 errors\n",
            // Not Cargo's right-aligned status column.
            "Blocking waiting for file lock on package cache\n",
            "        Blocking waiting\n",
            "note: Updating crates.io index\n",
            "warning: unused variable: `spurious`\n",
            "",
        ] {
            assert_eq!(transient_status(stderr.as_bytes()), None, "{stderr:?}");
        }
    }

    #[test]
    fn a_run_with_transient_cargo_status_is_never_recorded() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-noop-transient-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("cache");
        let pending = |key: &str| Pending {
            path: record_path(&cache, key),
            record: Record {
                version: VERSION,
                key: key.to_owned(),
                locks: (root.join("w.lock"), root.join("o.lock")),
                runs: root.join("runs"),
                lstat: Vec::new(),
                stat: Vec::new(),
                trees: Vec::new(),
                contents: Vec::new(),
                copies: Vec::new(),
                directories: Vec::new(),
                environment: BTreeMap::new(),
                result: receipt_fixture_result(&root),
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
        };
        let finished = b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.20s\n";
        pending("k").commit(Vec::new(), finished.to_vec());
        assert!(record_path(&cache, "k").is_file());
        // The same key under contention: not recorded, and the older record
        // is gone, so the next unchanged call takes the full path.
        let mut blocked = b"    Blocking waiting for file lock on package cache\n".to_vec();
        blocked.extend_from_slice(finished);
        pending("k").commit(Vec::new(), blocked.clone());
        assert!(!record_path(&cache, "k").exists());
        pending("fresh").commit(Vec::new(), blocked);
        assert!(!record_path(&cache, "fresh").exists());
        std::fs::remove_dir_all(&root)
    }
}
