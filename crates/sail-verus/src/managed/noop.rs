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

use super::Config;
use sail_api::project::{ProjectResult, ResolvedRequest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Version four distinguishes checked whole and exact-partial request transfers.
const VERSION: u32 = 4;
/// How much older than the run a recorded ctime must be.
pub(super) const SETTLED: Duration = Duration::from_secs(2);
/// Cache records up to this size are compared by content; larger files
/// (the private executable aliases) by stamp.
const CONTENT_LIMIT: u64 = 16 << 20;

#[path = "noop_origins.rs"]
mod origins;

/// A complete stat stamp: device, inode, size, mode, mtime and ctime with
/// nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Stamp([i64; 8]);

impl Stamp {
    #[cfg(unix)]
    pub(crate) fn of(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;
        let wide = |value: u64| i64::try_from(value).unwrap_or(i64::MAX);
        Self([
            wide(metadata.dev()),
            wide(metadata.ino()),
            wide(metadata.size()),
            i64::from(metadata.mode()),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ])
    }

    #[cfg(not(unix))]
    pub(crate) fn of(_metadata: &std::fs::Metadata) -> Self {
        Self([0; 8])
    }

    /// Whether the last status change happened at least [`SETTLED`] before
    /// `start`.
    fn settled(self, start: SystemTime) -> bool {
        let (Ok(seconds), Ok(nanos)) = (u64::try_from(self.0[6]), u32::try_from(self.0[7])) else {
            return false;
        };
        start
            .duration_since(UNIX_EPOCH + Duration::new(seconds, nanos))
            .is_ok_and(|age| age >= SETTLED)
    }
}

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
    /// Declared variables and their values.
    environment: BTreeMap<String, Option<Vec<u8>>>,
    /// Original whole/exact-partial receipts; every replay rechecks their producers.
    origins: Vec<super::receipts::Receipt>,
    original_request: ResolvedRequest,
    result: ProjectResult,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
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
    pub result: &'a ProjectResult,
    /// The ordinary runner's original pre-Cargo resolution, never rebuilt
    /// from the stored result or its target rows.
    pub original_request: &'a ResolvedRequest,
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
    // Both branches bind the independently captured original request. Partial
    // evidence uses its own scope contract and cannot acquire whole authority.
    if sail_api::acceptance::focused_command(&facts.original_request.args) {
        sail_api::acceptance::complete_exact_partial_request(facts.original_request, facts.result)?;
    } else {
        sail_api::acceptance::complete_request(facts.original_request, facts.result)?;
    }
    let origins = origins::capture(facts.context, facts.result)?;
    let mut lstat_entries = facts.observation;
    let target = walk(facts.target, &[])?;
    let target_stamps = crate::parallel::in_order(&target, |path| lstat(path));
    for (path, stamp) in target.into_iter().zip(target_stamps) {
        lstat_entries.push((path, stamp.ok()?));
    }
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
                "recorded: {} stamps, {} contents",
                self.record.lstat.len() + self.record.stat.len(),
                self.record.contents.len()
            ));
        }
    }
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

/// The first recorded observation that no longer holds.
fn mismatch(record: &Record) -> Option<String> {
    let now = crate::parallel::in_order(&record.lstat, |(path, _)| lstat(path));
    for ((path, recorded), now) in record.lstat.iter().zip(now) {
        if now.ok().as_ref() != Some(recorded) {
            return Some(format!("changed: {}", path.display()));
        }
    }
    let now = crate::parallel::in_order(&record.stat, |(path, _)| {
        std::fs::metadata(path).ok().map(|m| Stamp::of(&m))
    });
    for ((path, recorded), now) in record.stat.iter().zip(now) {
        if now.as_ref() != Some(recorded) {
            return Some(format!("changed: {}", path.display()));
        }
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
    fn recent_status_changes_are_not_settled() {
        let start = SystemTime::now();
        let at = |age: Duration| {
            let time = start - age;
            let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
            let mut fields = [0i64; 8];
            fields[6] = i64::try_from(since.as_secs()).unwrap_or_default();
            fields[7] = i64::from(since.subsec_nanos());
            Stamp(fields)
        };
        assert!(at(Duration::from_secs(3)).settled(start));
        assert!(!at(Duration::from_millis(1500)).settled(start));
        assert!(!at(Duration::ZERO).settled(start));
    }
}

#[cfg(test)]
#[path = "noop_authority_tests.rs"]
mod authority_tests;
