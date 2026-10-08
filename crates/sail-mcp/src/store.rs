//! The per-project result store behind `cached_result` and
//! `counterexample`.
//!
//! Every `verify` call that produces results is recorded as a [`RunRecord`]:
//! the schema-v1 report, when and how it was established, and a content
//! snapshot (SHA-256) of the project's sources taken just before the run.
//! For each function the store remembers the latest run that reported it,
//! so a run scoped to one module or function updates only those functions.
//! An unscoped run replaces the whole function map (functions it no longer
//! reports are forgotten).
//!
//! The store lives in memory and, when the server has a store directory, is
//! also written to `<dir>/<project-hash>.json` after every run, so results
//! survive a server restart and are shared between servers. A file that
//! cannot be read or written is ignored: the store is a convenience and
//! never the source of a verdict.
//!
//! Staleness is judged by comparing the run's snapshot with the sources now.
//! The snapshot covers `*.rs` and `Cargo.toml` under the project directory,
//! skipping `target/` and hidden directories: the files `sail watch`
//! watches. `Cargo.lock` is left out because the build itself writes it (a
//! first build creates it), which would make every such run look edited
//! while it ran. Sources outside the project directory (path dependencies
//! elsewhere, the toolchain) are not tracked either.

use std::collections::{BTreeMap, HashMap};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sail_api::output::{FunctionReport, Report};
use serde::{Deserialize, Serialize};

/// The on-disk format version of [`ProjectRecord`].
pub const STORE_FORMAT: u32 = 1;

/// Relative source path → SHA-256 of its contents.
pub type Snapshot = BTreeMap<String, String>;

/// One recorded run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// The run's number within its project (1, 2, …).
    pub run: u64,
    /// When the run finished, in milliseconds since the Unix epoch.
    pub finished_unix_ms: u64,
    /// The same instant in RFC 3339 (UTC).
    pub finished_at: String,
    /// The `sail` arguments the run used.
    pub sail_args: Vec<String>,
    /// `sail verify`'s exit code.
    pub exit_code: Option<i32>,
    /// How long the run took.
    pub duration_ms: u64,
    /// Whether the run was limited to part of the project (a package, a
    /// module or a function).
    pub scoped: bool,
    /// The sources as they were just before the run.
    pub sources: Snapshot,
    /// Whether the sources changed while the run was in progress. If so,
    /// the results may reflect either version.
    pub sources_changed_during_run: bool,
    /// The run's schema-v1 report.
    pub report: Report,
}

/// Everything recorded for one project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProjectRecord {
    /// [`STORE_FORMAT`].
    pub format: u32,
    /// The project directory (canonical).
    pub project_dir: String,
    /// The number the next run gets.
    pub next_run: u64,
    /// Runs still referenced by [`Self::functions`] or [`Self::last_run`].
    pub runs: BTreeMap<u64, RunRecord>,
    /// Each function's latest run.
    pub functions: BTreeMap<String, u64>,
    /// The latest run.
    pub last_run: Option<u64>,
}

/// How a source file differs from a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// The path, relative to the project directory.
    pub path: String,
    /// `modified`, `added` or `removed`.
    pub change: &'static str,
}

/// A function found in the store.
#[derive(Debug, Clone, Copy)]
pub struct Found<'a> {
    /// The run that last reported it.
    pub run: &'a RunRecord,
    /// Its report from that run.
    pub function: &'a FunctionReport,
}

/// Why a lookup found nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// No function matched; holds the known functions (some of them).
    NotFound(Vec<String>),
    /// Several functions matched; holds them.
    Ambiguous(Vec<String>),
}

impl ProjectRecord {
    /// Records a finished run and returns its number.
    pub fn record(&mut self, mut run: RunRecord) -> u64 {
        self.format = STORE_FORMAT;
        self.next_run = self.next_run.max(1);
        let number = self.next_run;
        self.next_run += 1;
        run.run = number;
        if !run.scoped {
            self.functions.clear();
        }
        for f in run.report.crates.iter().flat_map(|c| &c.functions) {
            self.functions.insert(f.function.clone(), number);
        }
        self.runs.insert(number, run);
        self.last_run = Some(number);
        let live: std::collections::BTreeSet<u64> = self
            .functions
            .values()
            .copied()
            .chain(self.last_run)
            .collect();
        self.runs.retain(|n, _| live.contains(n));
        number
    }

    /// The latest run.
    #[must_use]
    pub fn last(&self) -> Option<&RunRecord> {
        self.runs.get(&self.last_run?)
    }

    /// Finds a function by its full path (`demo::arith::mul_pos`) or, if
    /// that is not known, by a unique `::`-suffix (`mul_pos`,
    /// `arith::mul_pos`).
    ///
    /// # Errors
    ///
    /// [`LookupError`] when nothing, or more than one function, matches.
    pub fn function(&self, query: &str) -> Result<Found<'_>, LookupError> {
        let name = if self.functions.contains_key(query) {
            query.to_owned()
        } else {
            let suffix = format!("::{query}");
            let matches: Vec<&String> = self
                .functions
                .keys()
                .filter(|f| f.ends_with(&suffix))
                .collect();
            match matches.as_slice() {
                [one] => (*one).clone(),
                [] => {
                    return Err(LookupError::NotFound(
                        self.functions.keys().take(50).cloned().collect(),
                    ));
                }
                many => {
                    return Err(LookupError::Ambiguous(
                        many.iter().map(|s| (*s).clone()).collect(),
                    ));
                }
            }
        };
        let run = self
            .functions
            .get(&name)
            .and_then(|n| self.runs.get(n))
            .ok_or_else(|| LookupError::NotFound(Vec::new()))?;
        let function = run
            .report
            .crates
            .iter()
            .flat_map(|c| &c.functions)
            .find(|f| f.function == name)
            .ok_or_else(|| LookupError::NotFound(Vec::new()))?;
        Ok(Found { run, function })
    }

    /// Every function with its latest run, in name order.
    #[must_use]
    pub fn all_functions(&self) -> Vec<Found<'_>> {
        self.functions
            .iter()
            .filter_map(|(name, n)| {
                let run = self.runs.get(n)?;
                let function = run
                    .report
                    .crates
                    .iter()
                    .flat_map(|c| &c.functions)
                    .find(|f| &f.function == name)?;
                Some(Found { run, function })
            })
            .collect()
    }
}

/// The store: one [`ProjectRecord`] per canonical project directory.
#[derive(Debug, Default)]
pub struct Store {
    dir: Option<PathBuf>,
    projects: Mutex<HashMap<PathBuf, ProjectRecord>>,
}

impl Store {
    /// A store persisted under `dir` (memory only when `None`).
    #[must_use]
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            projects: Mutex::new(HashMap::new()),
        }
    }

    /// Where `project`'s record is persisted.
    #[must_use]
    pub fn file(&self, project: &Path) -> Option<PathBuf> {
        let digest = sha256_hex(project.to_string_lossy().as_bytes());
        Some(
            self.dir
                .as_ref()?
                .join(format!("{}.json", digest.get(..16).unwrap_or(&digest))),
        )
    }

    fn load(&self, project: &Path) -> ProjectRecord {
        self.file(project)
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| serde_json::from_slice::<ProjectRecord>(&bytes).ok())
            .filter(|r| r.format == STORE_FORMAT && Path::new(&r.project_dir) == project)
            .unwrap_or_else(|| ProjectRecord {
                format: STORE_FORMAT,
                project_dir: project.to_string_lossy().into_owned(),
                next_run: 1,
                ..ProjectRecord::default()
            })
    }

    /// Runs `f` on `project`'s record (loaded from disk on first use).
    pub fn with<T>(&self, project: &Path, f: impl FnOnce(&ProjectRecord) -> T) -> T {
        match self.projects.lock() {
            Ok(mut projects) => {
                let record = projects
                    .entry(project.to_path_buf())
                    .or_insert_with(|| self.load(project));
                f(record)
            }
            Err(_) => f(&self.load(project)),
        }
    }

    /// Records a run for `project` and persists the record. Returns the
    /// run's number, and an error message if persisting failed.
    pub fn record(&self, project: &Path, run: RunRecord) -> (u64, Option<String>) {
        let Ok(mut projects) = self.projects.lock() else {
            return (0, Some("the result store is unavailable".to_owned()));
        };
        let record = projects
            .entry(project.to_path_buf())
            .or_insert_with(|| self.load(project));
        let number = record.record(run);
        let persisted = self.file(project).map(|file| persist(&file, record));
        (number, persisted.and_then(Result::err))
    }
}

fn persist(file: &Path, record: &ProjectRecord) -> Result<(), String> {
    let write = || -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = file.with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(
            &tmp,
            serde_json::to_vec(record).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(&tmp, file)
    };
    write().map_err(|e| format!("cannot save results to {}: {e}", file.display()))
}

/// SHA-256 of `bytes` as lowercase hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    hex(&sha2::Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn file_sha256(path: &Path) -> Option<String> {
    use sha2::Digest as _;
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(buf.get(..n)?),
            Err(_) => return None,
        }
    }
    Some(hex(&hasher.finalize()))
}

/// The project's tracked sources and their contents' SHA-256.
#[must_use]
pub fn snapshot(root: &Path) -> Snapshot {
    let mut out = Snapshot::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !name.starts_with('.') && name != "target" {
                    stack.push(path);
                }
            } else if (name.ends_with(".rs") || name == "Cargo.toml")
                && let Some(digest) = file_sha256(&path)
                && let Ok(rel) = path.strip_prefix(root)
            {
                out.insert(rel.to_string_lossy().into_owned(), digest);
            }
        }
    }
    out
}

/// How `now` differs from `then`, in path order.
#[must_use]
pub fn changes(then: &Snapshot, now: &Snapshot) -> Vec<Change> {
    let mut out = Vec::new();
    for (path, digest) in then {
        match now.get(path) {
            None => out.push(Change {
                path: path.clone(),
                change: "removed",
            }),
            Some(d) if d != digest => out.push(Change {
                path: path.clone(),
                change: "modified",
            }),
            Some(_) => {}
        }
    }
    for path in now.keys().filter(|p| !then.contains_key(*p)) {
        out.push(Change {
            path: path.clone(),
            change: "added",
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Milliseconds since the Unix epoch.
#[must_use]
pub fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// `ms` since the Unix epoch as RFC 3339 UTC, to the millisecond.
#[must_use]
pub fn rfc3339(ms: u64) -> String {
    let secs = ms / 1000;
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        ms % 1000
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sail_api::output::{
        AttributionInfo, AttributionStatus, CacheInfo, CrateReport, ProvenanceReport,
        SourceSummaryName, Summary, TierName, Timing, VerdictName, VerusResults,
    };

    pub(crate) fn function(name: &str, verdict: VerdictName) -> FunctionReport {
        FunctionReport {
            function: name.to_owned(),
            module: String::new(),
            fn_mode: Some("exec".to_owned()),
            verdict,
            tier: if verdict == VerdictName::Provisional {
                TierName::Provisional
            } else {
                TierName::Confirmed
            },
            unstable: false,
            verus_success: Some(verdict == VerdictName::Verified),
            provenance: ProvenanceReport {
                mode: "default".to_owned(),
                source: SourceSummaryName::Unrecorded,
                checks_solved: 0,
                checks_served: 0,
                checks_unrecorded: 0,
            },
            timing: Timing {
                smt_micros: None,
                rlimit: None,
            },
            fragility: None,
            obligations: None,
        }
    }

    pub(crate) fn report(functions: Vec<FunctionReport>) -> Report {
        let crates = vec![CrateReport {
            name: Some("demo".to_owned()),
            verus: VerusResults {
                verified: None,
                errors: None,
                success: None,
                encountered_vir_error: None,
            },
            functions,
        }];
        Report {
            schema_version: 1,
            sail_version: "0.0.0".to_owned(),
            mode: "default".to_owned(),
            command: Vec::new(),
            exit_code: Some(0),
            solver: None,
            cache: CacheInfo { daemon: false },
            attribution: AttributionInfo {
                status: AttributionStatus::Unavailable,
                notes: Vec::new(),
                unmatched_sessions: 0,
            },
            summary: Summary::of(&crates),
            crates,
            diagnostics: Vec::new(),
            error: None,
        }
    }

    fn run(scoped: bool, functions: Vec<FunctionReport>) -> RunRecord {
        RunRecord {
            run: 0,
            finished_unix_ms: 0,
            finished_at: rfc3339(0),
            sail_args: Vec::new(),
            exit_code: Some(0),
            duration_ms: 0,
            scoped,
            sources: Snapshot::new(),
            sources_changed_during_run: false,
            report: report(functions),
        }
    }

    #[test]
    fn scoped_runs_merge_and_full_runs_replace() {
        let mut p = ProjectRecord::default();
        let first = p.record(run(
            false,
            vec![
                function("demo::a", VerdictName::Verified),
                function("demo::m::b", VerdictName::Failed),
            ],
        ));
        let second = p.record(run(
            true,
            vec![function("demo::m::b", VerdictName::Verified)],
        ));
        assert_eq!((first, second), (1, 2));
        let a = p
            .function("demo::a")
            .map(|f| (f.run.run, f.function.verdict));
        assert_eq!(a, Ok((1, VerdictName::Verified)));
        let b = p.function("b").map(|f| (f.run.run, f.function.verdict));
        assert_eq!(b, Ok((2, VerdictName::Verified)));
        assert_eq!(p.all_functions().len(), 2);
        // A full run forgets functions it no longer reports, and old runs.
        p.record(run(false, vec![function("demo::a", VerdictName::Failed)]));
        assert!(matches!(p.function("b"), Err(LookupError::NotFound(_))));
        assert_eq!(p.runs.keys().copied().collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn suffix_lookup_must_be_unique() {
        let mut p = ProjectRecord::default();
        p.record(run(
            false,
            vec![
                function("demo::x::get", VerdictName::Verified),
                function("demo::y::get", VerdictName::Verified),
                function("demo::getter", VerdictName::Verified),
            ],
        ));
        assert!(matches!(p.function("get"), Err(LookupError::Ambiguous(v)) if v.len() == 2));
        assert!(p.function("x::get").is_ok());
        assert!(p.function("etter").is_err());
    }

    #[test]
    fn snapshots_track_content_and_skip_build_dirs() {
        let root = std::env::temp_dir().join(format!("sail-mcp-snap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["src", "target", ".git"] {
            assert!(std::fs::create_dir_all(root.join(dir)).is_ok());
        }
        for (file, text) in [
            ("src/lib.rs", "fn a() {}"),
            ("Cargo.toml", "[package]"),
            ("target/x.rs", "x"),
            (".git/y.rs", "y"),
            ("README.md", "r"),
        ] {
            assert!(std::fs::write(root.join(file), text).is_ok());
        }
        let then = snapshot(&root);
        assert_eq!(
            then.keys().cloned().collect::<Vec<_>>(),
            ["Cargo.toml", "src/lib.rs"]
        );
        assert!(std::fs::write(root.join("src/lib.rs"), "fn a() { }").is_ok());
        assert!(std::fs::write(root.join("src/new.rs"), "").is_ok());
        assert!(std::fs::remove_file(root.join("Cargo.toml")).is_ok());
        let found: Vec<(String, &str)> = changes(&then, &snapshot(&root))
            .into_iter()
            .map(|c| (c.path, c.change))
            .collect();
        assert_eq!(
            found,
            [
                ("Cargo.toml".to_owned(), "removed"),
                ("src/lib.rs".to_owned(), "modified"),
                ("src/new.rs".to_owned(), "added")
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn records_persist_across_stores() {
        let dir = std::env::temp_dir().join(format!("sail-mcp-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let project = Path::new("/some/project");
        let store = Store::new(Some(dir.clone()));
        let (n, err) = store.record(
            project,
            run(false, vec![function("demo::a", VerdictName::Verified)]),
        );
        assert_eq!((n, err), (1, None));
        let again = Store::new(Some(dir.clone()));
        let verdict = again.with(project, |p| p.function("a").map(|f| f.function.verdict));
        assert_eq!(verdict, Ok(VerdictName::Verified));
        // Another project's record is separate.
        assert!(again.with(Path::new("/other"), |p| p.last().is_none()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn timestamps() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(951_782_400_123), "2000-02-29T00:00:00.123Z");
        assert_eq!(rfc3339(1_791_460_800_000), "2026-10-08T12:00:00.000Z");
    }
}
