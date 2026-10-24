//! Function-level attribution: which Verus function and obligation each
//! solver query belongs to (ADR 0014).
//!
//! Function names never reach the solver pipe (ADR 0003), so attribution
//! joins three things Verus can write without changing a byte of what Z3
//! receives:
//! - **`--output-json --time`**: Verus's own per-function verdict, rlimit and
//!   solver time;
//! - **`--log air`**: for every query op, the comment `;; <Kind> <function>`
//!   and the op's span, just before its `(check-valid …)`;
//! - **`--log smt-transcript`**: exactly what crossed the pipe, batch by
//!   batch, with the solver's replies.
//!
//! Each AIR `check-valid` becomes exactly one solver op: a `(push)` …
//! `(pop)` block holding one or more `check-sat`s (the first decides
//! validity; later ones locate further failing assertions), or, in a
//! single-check spinoff process, one `check-sat` with no push. The AIR log
//! and the transcript of one process share a file stem, so the k-th
//! `check-valid` in `X.air` is the k-th op in `X.smt_transcript`.
//!
//! The transcript also reconstructs the exact bytes the process sent
//! ([`Transcript::pipe_input`]): each batch, a newline, and the
//! `(echo "<<DONE>>")` Verus's writer thread appends. Its SHA-256 joins a
//! process to the shim's session record ([`SessionRecord`]), which says how
//! many replies the cache served before a solver started.

use std::collections::BTreeMap;
use std::path::Path;

use sail_ledger::{
    Answer, Assignment, CheckRecord, Effort, Establishment, FunctionRecord, Mode, Model,
    ObligationId, ObligationKind, ObligationRecord, Source,
};
use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, Item, ItemKind, ResponseSplitter};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// One batch Verus sent, and the reply lines it read back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch {
    /// The batch bytes, as written to the pipe before the newline and the
    /// `(echo "<<DONE>>")` Verus appends.
    pub query: Vec<u8>,
    /// Reply lines before `<<DONE>>`, newlines removed. `None` if the
    /// transcript ends before the reply.
    pub response: Option<Vec<String>>,
}

/// A parsed `--log smt-transcript` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Transcript {
    /// The batches, in order.
    pub batches: Vec<Batch>,
}

const QUERY_OPEN: &[u8] = b";;;>>> QUERY\n";
const RESPONSE_OPEN: &[u8] = b";;;>>> RESPONSE\n";
const CLOSE: &[u8] = b";;;<<<\n";
const DONE_ECHO: &[u8] = b"(echo \"<<DONE>>\")\n";

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

impl Transcript {
    /// Parses a transcript. Anything after the last complete batch is
    /// ignored.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Self {
        let mut batches = Vec::new();
        let mut pos = 0;
        while let Some(open) = find(bytes, QUERY_OPEN, pos) {
            let start = open + QUERY_OPEN.len();
            // The batch is written verbatim, then `;;;<<<` and a newline.
            let Some(end) = find(bytes, CLOSE, start) else {
                break;
            };
            let query = bytes.get(start..end).unwrap_or_default().to_vec();
            pos = end + CLOSE.len();
            let mut response = None;
            if bytes
                .get(pos..)
                .is_some_and(|rest| rest.starts_with(RESPONSE_OPEN))
            {
                let body = pos + RESPONSE_OPEN.len();
                let mut lines = Vec::new();
                let mut line_start = body;
                let mut closed = false;
                while let Some(nl) = find(bytes, b"\n", line_start) {
                    let line = bytes.get(line_start..nl).unwrap_or_default();
                    line_start = nl + 1;
                    if line == b";;;<<<" {
                        closed = true;
                        break;
                    }
                    lines.push(String::from_utf8_lossy(line).into_owned());
                }
                pos = line_start;
                if closed {
                    response = Some(lines);
                }
            }
            batches.push(Batch { query, response });
        }
        Self { batches }
    }

    /// The exact bytes the process sent to the solver.
    #[must_use]
    pub fn pipe_input(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for batch in &self.batches {
            out.extend_from_slice(&batch.query);
            out.push(b'\n');
            out.extend_from_slice(DONE_ECHO);
        }
        out
    }

    /// Hash the exact pipe stream without allocating a second full copy of
    /// the queries. Real-project attribution logs can hold hundreds of MiB.
    fn input_sha256(&self) -> String {
        let mut hasher = Sha256::new();
        for batch in &self.batches {
            hasher.update(&batch.query);
            hasher.update(b"\n");
            hasher.update(DONE_ECHO);
        }
        hex(&hasher.finalize())
    }
}

/// One `check-sat` in a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtCheck {
    /// Its position among the process's replies (every command that
    /// produces one, including Verus's `<<DONE>>` echoes), counting from 0.
    /// The shim's served count is in the same units.
    pub reply_index: u64,
    /// Its answer.
    pub answer: Option<Answer>,
    /// Effort and reason.
    pub effort: Effort,
    /// The `get-model` reply that followed, verbatim.
    pub model: Option<String>,
}

/// One solver op: the checks of one AIR `check-valid`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmtOp {
    /// Its checks, in order.
    pub checks: Vec<SmtCheck>,
}

/// The number after `:rlimit-count` in a statistics reply.
fn rlimit_count(reply: &str) -> Option<u64> {
    let at = reply.find(":rlimit-count")?;
    let digits: String = reply
        .get(at + ":rlimit-count".len()..)?
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn split_commands(query: &[u8]) -> Vec<Item> {
    let mut splitter = CommandSplitter::new();
    let mut items = Vec::new();
    splitter.push(query, &mut items);
    splitter.push(b"\n", &mut items);
    items.extend(splitter.finish());
    items
}

fn split_replies(lines: &[String]) -> Vec<String> {
    let mut text = lines.join("\n");
    text.push('\n');
    let mut splitter = ResponseSplitter::new();
    let mut items = Vec::new();
    splitter.push(text.as_bytes(), &mut items);
    items.extend(splitter.finish());
    items
        .into_iter()
        .map(|i| String::from_utf8_lossy(&i.bytes).trim_end().to_owned())
        .collect()
}

/// Running state while walking a transcript.
#[derive(Default)]
struct Walk {
    ops: Vec<SmtOp>,
    depth: u64,
    /// The op of the current depth-1 push block, once it has a check.
    block: Option<usize>,
    rlimit: Option<u64>,
    print_success: bool,
    reply_index: u64,
    last_count: Option<u64>,
    /// The latest check: (op, check, rlimit-count before it, effort known).
    last_check: Option<(usize, usize, Option<u64>, bool)>,
}

impl Walk {
    fn last_check_mut(&mut self) -> Option<&mut SmtCheck> {
        let (op, check, _, _) = self.last_check?;
        self.ops.get_mut(op)?.checks.get_mut(check)
    }

    fn command(&mut self, item: &Item, reply: Option<&str>) {
        let Ok(cmd) = classify(&item.bytes) else {
            return;
        };
        match cmd.kind {
            CommandKind::Push(n) => {
                if self.depth == 0 {
                    self.block = None;
                }
                self.depth = self.depth.saturating_add(n);
            }
            CommandKind::Pop(n) => {
                self.depth = self.depth.saturating_sub(n);
                if self.depth == 0 {
                    self.block = None;
                }
            }
            CommandKind::SetOption { keyword, value } => match keyword {
                b":rlimit" => {
                    self.rlimit = std::str::from_utf8(value).ok().and_then(|v| v.parse().ok());
                }
                b":print-success" => self.print_success = value == b"true",
                _ => {}
            },
            k if k.is_check() => {
                let op = match self.block {
                    Some(op) if self.depth > 0 => op,
                    _ => {
                        self.ops.push(SmtOp::default());
                        let op = self.ops.len() - 1;
                        if self.depth > 0 {
                            self.block = Some(op);
                        }
                        op
                    }
                };
                let check = SmtCheck {
                    reply_index: self.reply_index,
                    answer: reply.and_then(Answer::parse),
                    effort: Effort {
                        limit: self.rlimit,
                        used: None,
                        reason: None,
                    },
                    model: None,
                };
                if let Some(o) = self.ops.get_mut(op) {
                    o.checks.push(check);
                    self.last_check = Some((op, o.checks.len() - 1, self.last_count, false));
                }
            }
            CommandKind::GetInfo {
                keyword: b":reason-unknown",
            } => {
                let reason = reply.map(str::to_owned);
                if let Some(c) = self.last_check_mut() {
                    c.effort.reason = reason;
                }
            }
            CommandKind::GetInfo {
                keyword: b":all-statistics",
            } => {
                let count = reply.and_then(rlimit_count);
                if let Some((op, check, before, false)) = self.last_check {
                    if let (Some(after), Some(before)) = (count, before)
                        && let Some(c) = self.ops.get_mut(op).and_then(|o| o.checks.get_mut(check))
                    {
                        c.effort.used = Some(after.saturating_sub(before));
                    }
                    self.last_check = Some((op, check, before, true));
                }
                self.last_count = count.or(self.last_count);
            }
            CommandKind::Get if cmd.name == b"get-model" => {
                let model = reply.map(str::to_owned);
                if let Some(c) = self.last_check_mut() {
                    c.model = model;
                }
            }
            _ => {}
        }
    }
}

/// The solver ops of a transcript, in order.
#[must_use]
pub fn ops(transcript: &Transcript) -> Vec<SmtOp> {
    let mut walk = Walk::default();
    for batch in &transcript.batches {
        let items = split_commands(&batch.query);
        let replies = batch
            .response
            .as_deref()
            .map(split_replies)
            .unwrap_or_default();
        let mut replies = replies.iter();
        for item in &items {
            let expects = if item.kind != ItemKind::List {
                Output::Unknown
            } else if walk.print_success {
                Output::One
            } else {
                classify(&item.bytes).map_or(Output::Unknown, |c| c.kind.output())
            };
            let reply = if expects == Output::One {
                replies.next().map(String::as_str)
            } else {
                None
            };
            walk.command(item, reply);
            if expects == Output::One {
                walk.reply_index += 1;
            }
        }
        // The `(echo "<<DONE>>")` Verus appends to every batch.
        walk.reply_index += 1;
    }
    walk.ops
}

/// The solver version from a transcript's `(get-info :version)` reply.
#[must_use]
pub fn solver_version(transcript: &Transcript) -> Option<String> {
    transcript
        .batches
        .iter()
        .filter(|b| b.query.trim_ascii_end().ends_with(b"(get-info :version)"))
        .find_map(|b| {
            let line = b.response.as_ref()?.first()?;
            let rest = line.trim().strip_prefix("(:version")?;
            Some(
                rest.trim()
                    .trim_end_matches(')')
                    .trim()
                    .trim_matches('"')
                    .to_owned(),
            )
        })
}

/// One query op from an AIR log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirQuery {
    /// The op kind (`Function-Def`, `Function-Recommends`, …).
    pub kind: String,
    /// The function, as Verus names it.
    pub function: String,
    /// The op's span comment, verbatim.
    pub span: Option<String>,
}

/// An AIR log's query ops, in order, and its spinoff reason if it is a
/// spinoff log.
#[must_use]
pub fn air_queries(text: &str) -> (Vec<AirQuery>, Option<String>) {
    let mut out = Vec::new();
    let mut reason = None;
    let mut pending: Option<AirQuery> = None;
    let mut want_span = false;
    for line in text.lines() {
        if let Some(comment) = line.strip_prefix(";; ") {
            if let Some(r) = comment.strip_prefix("query spun off because: ") {
                reason = Some(r.trim().to_owned());
                want_span = false;
                continue;
            }
            let (word, rest) = comment.split_once(' ').unwrap_or((comment, ""));
            if ObligationKind::is_query_kind(word) && !rest.is_empty() {
                pending = Some(AirQuery {
                    kind: word.to_owned(),
                    function: rest.to_owned(),
                    span: None,
                });
                want_span = true;
            } else if want_span {
                if let Some(p) = pending.as_mut() {
                    p.span = Some(comment.to_owned());
                }
                want_span = false;
            }
        } else if line.starts_with("(check-valid") {
            out.push(pending.take().unwrap_or(AirQuery {
                kind: String::new(),
                function: String::new(),
                span: None,
            }));
            want_span = false;
        }
    }
    (out, reason)
}

/// The spinoff counter in a log file stem (`<bucket><fn-path>_NN[_expand]`),
/// if it names a spinoff.
#[must_use]
pub fn spinoff_index(stem: &str) -> Option<u32> {
    let stem = stem.strip_suffix("_expand").unwrap_or(stem);
    let (_, n) = stem.rsplit_once("._")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

/// One process's logs.
#[derive(Debug, Clone)]
pub struct LogFile {
    /// The file stem shared by its `.air` and `.smt_transcript` files.
    pub stem: String,
    /// Its AIR query ops.
    pub air: Vec<AirQuery>,
    /// Why it was spun off, for spinoff logs.
    pub spinoff_reason: Option<String>,
    /// Its solver ops.
    pub ops: Vec<SmtOp>,
    /// SHA-256 of the bytes it sent, as lowercase hex.
    pub input_sha256: String,
    /// The solver version it reported.
    pub solver_version: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

impl LogFile {
    /// Builds a log file from an AIR log and a transcript.
    #[must_use]
    pub fn new(stem: &str, air: &str, transcript: &[u8]) -> Self {
        let transcript = Transcript::parse(transcript);
        let (air, spinoff_reason) = air_queries(air);
        Self {
            stem: stem.to_owned(),
            air,
            spinoff_reason,
            ops: ops(&transcript),
            input_sha256: transcript.input_sha256(),
            solver_version: solver_version(&transcript),
        }
    }
}

/// Reads every `<stem>.smt_transcript` in `dir` and its `<stem>.air`, in
/// stem order. Transcripts without an AIR log are skipped.
///
/// Under `cargo verus`, Sail's Verus writes each crate's logs to
/// `<log-dir>/<crate name>` (VI-0004 fix). When `dir` holds no transcript
/// itself and exactly one subdirectory does, that subdirectory's logs are
/// read; Sail asks for logs only when one crate writes them.
///
/// # Errors
///
/// `SAIL3030` if the directory cannot be read.
pub fn read_logs(dir: &Path) -> crate::Result<Vec<LogFile>> {
    let dir = crate_log_dir(dir);
    let dir = dir.as_path();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        crate::Error::new(
            "SAIL3030",
            format!("cannot read Verus logs in {}: {e}", dir.display()),
        )
    })?;
    let mut stems: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.strip_suffix(".smt_transcript").map(str::to_owned)
        })
        .collect();
    stems.sort();
    // Each log pair is read and parsed independently; results keep stem order.
    Ok(crate::parallel::in_order(&stems, |stem| {
        let (Ok(air), Ok(transcript)) = (
            std::fs::read_to_string(dir.join(format!("{stem}.air"))),
            std::fs::read(dir.join(format!("{stem}.smt_transcript"))),
        ) else {
            return None;
        };
        Some(LogFile::new(stem, &air, &transcript))
    })
    .into_iter()
    .flatten()
    .collect())
}

fn has_transcripts(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.ends_with(".smt_transcript"))
        })
    })
}

/// `dir`, or its one subdirectory holding transcripts when `dir` holds none
/// (see [`read_logs`]).
fn crate_log_dir(dir: &Path) -> std::path::PathBuf {
    if has_transcripts(dir) {
        return dir.to_owned();
    }
    let crates: Vec<_> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir() && has_transcripts(p))
                .collect()
        })
        .unwrap_or_default();
    match crates.as_slice() {
        [only] => only.clone(),
        _ => dir.to_owned(),
    }
}

/// What the shim records about one cached session, when
/// `SAIL_PROVENANCE_DIR` is set (`<dir>/<pid>-<nanos>.json`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SessionRecord {
    /// Record format version (1).
    pub version: u32,
    /// SHA-256 of every byte the session received, as lowercase hex.
    pub input_sha256: String,
    /// How many replies the cache served before a solver started (all of
    /// them, if none started).
    pub replies_served: u64,
    /// Whether a solver was started.
    pub solver_started: bool,
}

/// Reads every session record in `dir`; unreadable files are skipped.
#[must_use]
pub fn read_session_records(dir: &Path) -> Vec<SessionRecord> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice::<SessionRecord>(&b).ok())
        .filter(|r| r.version == 1)
        .collect()
}

/// Where answers came from.
#[derive(Debug, Clone)]
pub enum Provenance {
    /// No cache was in use: every answer is a fresh pinned-solver run.
    NoCache,
    /// The shim's session records.
    Records(Vec<SessionRecord>),
}

/// Whether attribution covered everything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Every solver process was attributed.
    Complete,
    /// Some processes or functions could not be attributed; see the notes.
    Partial,
    /// No attribution was attempted or possible; see the notes.
    Unavailable,
}

/// One crate's results (one Verus `--output-json` document).
#[derive(Debug, Clone)]
pub struct CrateResult {
    /// The crate name, from its functions' paths, if it has any.
    pub name: Option<String>,
    /// `verification-results.verified`.
    pub verified: Option<u64>,
    /// `verification-results.errors`.
    pub errors: Option<u64>,
    /// `verification-results.success` (only for whole-crate runs).
    pub success: Option<bool>,
    /// `verification-results.encountered-vir-error`.
    pub encountered_vir_error: Option<bool>,
    /// Its functions.
    pub functions: Vec<FunctionRecord>,
}

/// The result of attribution.
#[derive(Debug, Clone)]
pub struct Attribution {
    /// One entry per Verus document, in stdout order.
    pub crates: Vec<CrateResult>,
    /// Coverage.
    pub status: Status,
    /// Why coverage is not complete, in plain words.
    pub notes: Vec<String>,
    /// Session records that matched no transcript.
    pub unmatched_sessions: u64,
    /// The solver version the transcripts report.
    pub solver_version: Option<String>,
}

/// Verus's per-function entries in one `--output-json --time` document:
/// `(module, function, mode, success, rlimit, time_micros)`.
type VerusFunction = (
    String,
    String,
    Option<String>,
    Option<bool>,
    Option<u64>,
    Option<u64>,
);

fn verus_functions(doc: &Value) -> Vec<VerusFunction> {
    let mut out = Vec::new();
    let modules = doc
        .pointer("/times-ms/smt/smt-run-module-times")
        .and_then(Value::as_array);
    for module in modules.into_iter().flatten() {
        let name = module
            .get("module")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let fns = module.get("function-breakdown").and_then(Value::as_array);
        for f in fns.into_iter().flatten() {
            let Some(function) = f.get("function").and_then(Value::as_str) else {
                continue;
            };
            out.push((
                name.to_owned(),
                function.to_owned(),
                f.get("mode:").and_then(Value::as_str).map(str::to_owned),
                f.get("success").and_then(Value::as_bool),
                f.get("rlimit").and_then(Value::as_u64),
                f.get("time-micros").and_then(Value::as_u64),
            ));
        }
    }
    out
}

/// Verus's own verdicts: `(function, success)` for every function in every
/// document's `--time` breakdown, in document order.
#[must_use]
pub fn verus_verdicts(docs: &[Value]) -> Vec<(String, Option<bool>)> {
    docs.iter()
        .flat_map(verus_functions)
        .map(|(_, f, _, ok, _, _)| (f, ok))
        .collect()
}

/// Parses a model's zero-arity `define-fun`s.
#[must_use]
pub fn parse_model(text: &str) -> Model {
    let src = text.as_bytes();
    let mut assignments = Vec::new();
    if let Ok(model) = sail_smtlib::sexpr::parse_one(src.trim_ascii()) {
        let src = src.trim_ascii();
        let lossy =
            |e: &sail_smtlib::sexpr::SExpr| String::from_utf8_lossy(e.text(src)).into_owned();
        for def in model.as_list().unwrap_or_default() {
            if let Some([head, name, params, sort, value]) = def.as_list()
                && head.text(src) == b"define-fun"
                && params.as_list().is_some_and(<[_]>::is_empty)
            {
                assignments.push(Assignment {
                    name: lossy(name),
                    sort: lossy(sort),
                    value: lossy(value),
                });
            }
        }
    }
    Model {
        text: text.to_owned(),
        assignments,
    }
}

fn crate_name(functions: &[VerusFunction]) -> Option<String> {
    functions
        .first()
        .and_then(|(_, f, ..)| f.split("::").next())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// The obligations in `logs`, by function. Files whose AIR and solver op
/// counts disagree are skipped and noted.
fn obligations(
    logs: &[LogFile],
    provenance: &Provenance,
    notes: &mut Vec<String>,
    unmatched_sessions: &mut u64,
) -> BTreeMap<String, Vec<ObligationRecord>> {
    let mut by_fn: BTreeMap<String, Vec<ObligationRecord>> = BTreeMap::new();
    let mut matched = vec![false; logs.len()];
    let records = match provenance {
        Provenance::NoCache => &[][..],
        Provenance::Records(r) => r.as_slice(),
    };
    for record in records {
        match logs
            .iter()
            .position(|l| l.input_sha256 == record.input_sha256)
        {
            Some(i) => {
                if let Some(m) = matched.get_mut(i) {
                    *m = true;
                }
            }
            None => *unmatched_sessions += 1,
        }
    }
    for log in logs {
        if log.air.len() != log.ops.len() {
            notes.push(format!(
                "log {}: {} AIR queries but {} solver ops; not attributed",
                log.stem,
                log.air.len(),
                log.ops.len()
            ));
            continue;
        }
        let served = match provenance {
            Provenance::NoCache => Some(0),
            Provenance::Records(r) => r
                .iter()
                .find(|s| s.input_sha256 == log.input_sha256)
                .map(|s| s.replies_served),
        };
        let spinoff = spinoff_index(&log.stem);
        let mut ordinals: BTreeMap<(&str, &str), u32> = BTreeMap::new();
        for (air, op) in log.air.iter().zip(&log.ops) {
            let ordinal = ordinals
                .entry((air.function.as_str(), air.kind.as_str()))
                .or_insert(0);
            let id = ObligationId {
                function: air.function.clone(),
                kind: ObligationKind(air.kind.clone()),
                ordinal: if spinoff.is_some() { 0 } else { *ordinal },
                spinoff,
            };
            *ordinal += 1;
            let checks = op
                .checks
                .iter()
                .map(|c| CheckRecord {
                    answer: c.answer,
                    effort: c.effort.clone(),
                    source: served.map(|s| {
                        if c.reply_index < s {
                            Source::Cache
                        } else {
                            Source::Solved
                        }
                    }),
                    model: c.model.as_deref().map(parse_model),
                })
                .collect();
            by_fn
                .entry(air.function.clone())
                .or_default()
                .push(ObligationRecord {
                    id,
                    span: air.span.clone(),
                    spinoff_reason: log.spinoff_reason.clone(),
                    checks,
                });
        }
    }
    for list in by_fn.values_mut() {
        list.sort_by(|a, b| {
            (
                a.id.spinoff.is_some(),
                &a.id.kind,
                a.id.ordinal,
                a.id.spinoff,
            )
                .cmp(&(
                    b.id.spinoff.is_some(),
                    &b.id.kind,
                    b.id.ordinal,
                    b.id.spinoff,
                ))
        });
    }
    by_fn
}

fn remove_ambiguous_joins(
    docs: &[Value],
    by_fn: &mut BTreeMap<String, Vec<ObligationRecord>>,
    notes: &mut Vec<String>,
) {
    // Verus's friendly names omit trait implementation identity. Multiple
    // timing entries can therefore name different implementations identically,
    // even in one module. Neither log order nor source position establishes a
    // stable join: leave every such record unattributed rather than attaching
    // all operations to the first entry or inventing obligation identities.
    let mut occurrences = BTreeMap::new();
    for (_, function, ..) in docs.iter().flat_map(verus_functions) {
        *occurrences.entry(function).or_insert(0usize) += 1;
    }
    for (function, count) in occurrences {
        if count > 1 && by_fn.remove(&function).is_some() {
            notes.push(format!(
                "Verus reports {count} entries named {function}; obligations are not attributed because the friendly name does not distinguish implementations"
            ));
        }
    }
}

/// Joins Verus's documents with its logs (if any) and the provenance.
///
/// `logs` is `None` when sail did not ask Verus for logs; `unavailable`
/// then says why, for the notes.
#[must_use]
pub fn attribute(
    docs: &[Value],
    logs: Option<&[LogFile]>,
    provenance: &Provenance,
    unavailable: Option<&str>,
) -> Attribution {
    let mut notes = Vec::new();
    let mut unmatched_sessions = 0;
    let mut by_fn = logs
        .map(|l| obligations(l, provenance, &mut notes, &mut unmatched_sessions))
        .unwrap_or_default();
    remove_ambiguous_joins(docs, &mut by_fn, &mut notes);
    let mut missing = 0u64;
    let mut crates = Vec::new();
    for doc in docs {
        let functions = verus_functions(doc);
        let name = crate_name(&functions);
        let records = functions
            .into_iter()
            .map(|(module, function, fn_mode, ok, rlimit, micros)| {
                let obligations = match logs {
                    None => None,
                    Some(_) => by_fn.remove(&function).filter(|o| !o.is_empty()),
                };
                if logs.is_some() && obligations.is_none() {
                    missing += 1;
                }
                FunctionRecord {
                    function,
                    module,
                    fn_mode,
                    verus_success: ok,
                    smt_micros: micros,
                    rlimit,
                    obligations,
                    establishment: Establishment::Confirmed,
                    mode: Mode::Default,
                }
            })
            .collect();
        let results = doc.get("verification-results");
        let field = |k: &str| results.and_then(|r| r.get(k));
        crates.push(CrateResult {
            name,
            verified: field("verified").and_then(Value::as_u64),
            errors: field("errors").and_then(Value::as_u64),
            success: field("success").and_then(Value::as_bool),
            encountered_vir_error: field("encountered-vir-error").and_then(Value::as_bool),
            functions: records,
        });
    }
    // Obligations of functions Verus's breakdown does not list.
    if !by_fn.is_empty() {
        match crates.as_mut_slice() {
            [only] => {
                for (function, obligations) in std::mem::take(&mut by_fn) {
                    only.functions.push(FunctionRecord {
                        function,
                        module: String::new(),
                        fn_mode: None,
                        verus_success: None,
                        smt_micros: None,
                        rlimit: None,
                        obligations: Some(obligations),
                        establishment: Establishment::Confirmed,
                        mode: Mode::Default,
                    });
                }
            }
            _ => notes.push(format!(
                "{} functions in the logs match no Verus document; not reported",
                by_fn.len()
            )),
        }
    }
    if missing > 0 {
        notes.push(format!(
            "{missing} functions have no attributed obligations (their logs are missing, overwritten, or ambiguous)"
        ));
    }
    if unmatched_sessions > 0 {
        notes.push(format!(
            "{unmatched_sessions} solver sessions match no Verus log (Verus overwrote or did not write it)"
        ));
    }
    let status = match logs {
        None => {
            notes.insert(0, unavailable.unwrap_or("no Verus logs").to_owned());
            Status::Unavailable
        }
        Some(_) if notes.is_empty() => Status::Complete,
        Some(_) => Status::Partial,
    };
    Attribution {
        crates,
        status,
        notes,
        unmatched_sessions,
        solver_version: logs
            .into_iter()
            .flatten()
            .find_map(|l| l.solver_version.clone()),
    }
}

/// Splits `cargo verus` stdout into Verus's `--output-json` documents and
/// everything else. A document starts on a line that begins with `{`; any
/// other text is returned verbatim, in order.
#[must_use]
pub fn json_documents(stdout: &[u8]) -> (Vec<Value>, Vec<u8>) {
    let mut docs = Vec::new();
    let mut rest = Vec::new();
    let mut pos = 0;
    while pos < stdout.len() {
        let tail = stdout.get(pos..).unwrap_or_default();
        if tail.first() == Some(&b'{') {
            let mut stream = serde_json::Deserializer::from_slice(tail).into_iter::<Value>();
            if let Some(Ok(doc)) = stream.next() {
                docs.push(doc);
                pos += stream.byte_offset();
                // The newline after the document.
                if stdout.get(pos) == Some(&b'\n') {
                    pos += 1;
                }
                continue;
            }
        }
        let end = tail
            .iter()
            .position(|&b| b == b'\n')
            .map_or(tail.len(), |i| i + 1);
        rest.extend_from_slice(tail.get(..end).unwrap_or_default());
        pos += end;
    }
    (docs, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_documents_are_split_from_other_output() {
        let (docs, rest) = json_documents(b"{\"a\": 1}\nnote\n{\n \"b\": [2]\n}\n{broken\n");
        assert_eq!(
            docs,
            vec![serde_json::json!({"a": 1}), serde_json::json!({"b": [2]})]
        );
        assert_eq!(rest, b"note\n{broken\n");
        assert_eq!(json_documents(b""), (vec![], vec![]));
    }

    const TRANSCRIPT: &str = ";;;>>> QUERY
(set-option :rlimit 30000000)
(declare-const a Int)
(get-info :all-statistics)
;;;<<<
;;;>>> RESPONSE
(:memory 1
 :rlimit-count 10)
;;;<<<
;;;>>> QUERY
(push)
(get-info :all-statistics)
;;;<<<
;;;>>> RESPONSE
(:rlimit-count 12)
;;;<<<
;;;>>> QUERY
(get-info :version)
;;;<<<
;;;>>> RESPONSE
(:version \"4.16.0\")
;;;<<<
;;;>>> QUERY
(set-option :rlimit 30000000)
(check-sat)
;;;<<<
;;;>>> RESPONSE
unknown
;;;<<<
;;;>>> QUERY
(set-option :rlimit 0)
(get-info :reason-unknown)
;;;<<<
;;;>>> RESPONSE
(:reason-unknown \"(incomplete quantifiers)\")
;;;<<<
;;;>>> QUERY
(get-model)
;;;<<<
;;;>>> RESPONSE
(
  (define-fun a () Int
    5)
  (define-fun f ((x Int)) Int x)
)
;;;<<<
;;;>>> QUERY
(assert (not b))
(get-info :all-statistics)
;;;<<<
;;;>>> RESPONSE
(:rlimit-count 40)
;;;<<<
;;;>>> QUERY
(set-option :rlimit 30000000)
(check-sat)
;;;<<<
;;;>>> RESPONSE
unsat
;;;<<<
;;;>>> QUERY
(set-option :rlimit 0)
(pop)
(push)
(get-info :all-statistics)
;;;<<<
;;;>>> RESPONSE
(:rlimit-count 50)
;;;<<<
;;;>>> QUERY
(check-sat)
;;;<<<
;;;>>> RESPONSE
unsat
;;;<<<
";

    #[test]
    fn transcripts_parse_and_reconstruct_the_pipe() {
        let t = Transcript::parse(TRANSCRIPT.as_bytes());
        assert_eq!(t.batches.len(), 10);
        let input = String::from_utf8(t.pipe_input()).unwrap_or_default();
        assert!(input.starts_with(
            "(set-option :rlimit 30000000)\n(declare-const a Int)\n(get-info :all-statistics)\n\n(echo \"<<DONE>>\")\n(push)\n"
        ));
        assert_eq!(input.matches("<<DONE>>").count(), 10);
        assert_eq!(t.input_sha256(), hex(&Sha256::digest(input.as_bytes())));
        assert_eq!(solver_version(&t).as_deref(), Some("4.16.0"));
        // A truncated transcript keeps its complete batches.
        let cut = TRANSCRIPT.get(..TRANSCRIPT.len() - 20).unwrap_or_default();
        let t = Transcript::parse(cut.as_bytes());
        assert_eq!(t.batches.len(), 10);
        assert_eq!(t.batches.last().and_then(|b| b.response.as_ref()), None);
        assert_eq!(t.input_sha256(), hex(&Sha256::digest(t.pipe_input())));
    }

    #[test]
    fn transcripts_group_checks_into_ops() {
        let ops = ops(&Transcript::parse(TRANSCRIPT.as_bytes()));
        assert_eq!(ops.len(), 2);
        let first = &ops[0].checks;
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].answer, Some(Answer::Unknown));
        assert_eq!(first[0].effort.limit, Some(30_000_000));
        assert_eq!(first[0].effort.used, Some(28));
        assert_eq!(
            first[0].effort.reason.as_deref(),
            Some("(:reason-unknown \"(incomplete quantifiers)\")")
        );
        assert!(
            first[0]
                .model
                .as_deref()
                .is_some_and(|m| m.contains("define-fun a"))
        );
        // Replies: stats, echo, stats, echo, version, echo, then the check.
        assert_eq!(first[0].reply_index, 6);
        assert_eq!(first[1].answer, Some(Answer::Unsat));
        assert_eq!(first[1].effort.used, Some(10));
        assert_eq!(ops[1].checks.len(), 1);
        assert_eq!(ops[1].checks[0].effort.limit, Some(0));
        assert_eq!(ops[1].checks[0].effort.used, None);
    }

    #[test]
    fn repeated_friendly_names_never_join_distinct_implementations() {
        let function = "demo::S::check";
        let entry = |success| {
            serde_json::json!({
                "function": function, "success": success, "time-micros": 7
            })
        };
        let doc = serde_json::json!({
            "verification-results": {"verified": 3, "errors": 0, "success": true},
            "times-ms": {"smt": {"smt-run-module-times": [
                {"module": "first", "function-breakdown": [entry(true), entry(true)]},
                {"module": "second", "function-breakdown": [entry(true)]}
            ]}}
        });
        let log = |stem: &str| LogFile {
            stem: stem.to_owned(),
            air: vec![AirQuery {
                function: function.to_owned(),
                kind: "Function-Def".to_owned(),
                span: Some(format!("src/{stem}.rs:1:1: 1:9 (#0)")),
            }],
            ops: vec![SmtOp::default()],
            spinoff_reason: None,
            input_sha256: stem.to_owned(),
            solver_version: None,
        };
        for logs in [
            vec![log("first"), log("second")],
            vec![log("second"), log("first")],
        ] {
            let result = attribute(
                std::slice::from_ref(&doc),
                Some(&logs),
                &Provenance::NoCache,
                None,
            );
            assert_eq!(result.status, Status::Partial);
            assert_eq!(result.crates[0].success, Some(true));
            assert_eq!(result.crates[0].functions.len(), 3);
            assert!(
                result.crates[0]
                    .functions
                    .iter()
                    .all(|f| f.function == function
                        && f.verus_success == Some(true)
                        && f.obligations.is_none())
            );
            assert_eq!(result.crates[0].functions[0].module, "first");
            assert_eq!(result.crates[0].functions[2].module, "second");
            assert!(
                result
                    .notes
                    .iter()
                    .any(|n| n.contains("does not distinguish implementations"))
            );
            let build = |attribution: &Attribution| {
                crate::report::build(&crate::report::Inputs {
                    args: &[],
                    exit_code: Some(0),
                    solver_path: None,
                    solver_sha256: None,
                    daemon: false,
                    attribution,
                    diagnostics: &[],
                })
            };
            let report = build(&result);
            assert_eq!(report.invariant_violations(), [] as [String; 0]);
            assert!(crate::managed::receipt_complete(&report));
            let mut failing = result.clone();
            failing.crates[0].errors = Some(1);
            failing.crates[0].success = Some(false);
            let report = build(&failing);
            assert!(!crate::managed::receipt_complete(&report));
            assert!(
                report.crates[0]
                    .functions
                    .iter()
                    .all(|f| f.verdict == sail_api::output::VerdictName::Unknown)
            );
        }
    }

    #[test]
    fn models_become_assignments() {
        let m = parse_model(
            "(\n  (define-fun a () Int\n    5)\n  (define-fun f ((x Int)) Int x)\n  (define-fun %%location_label%%0 () Bool false)\n)",
        );
        let got: Vec<(&str, &str, &str)> = m
            .assignments
            .iter()
            .map(|a| (a.name.as_str(), a.sort.as_str(), a.value.as_str()))
            .collect();
        assert_eq!(
            got,
            [("a", "Int", "5"), ("%%location_label%%0", "Bool", "false")]
        );
        assert_eq!(parse_model("not a model (").assignments, []);
    }

    #[test]
    fn air_logs_name_each_query() {
        let air = ";; MODULE 'root module'
;; Function-Specs demo::f
(axiom true)
;; Function-Def demo::f
;; src/lib.rs:3:1: 3:20 (#0)
(check-valid
 (block))
;; Function-Recommends demo::<T as Tr>::m
;; src/lib.rs:9:1: 9:20 (#0)
(check-valid (block))
";
        let (queries, reason) = air_queries(air);
        assert_eq!(reason, None);
        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0].kind, "Function-Def");
        assert_eq!(queries[0].function, "demo::f");
        assert_eq!(
            queries[0].span.as_deref(),
            Some("src/lib.rs:3:1: 3:20 (#0)")
        );
        assert_eq!(queries[1].function, "demo::<T as Tr>::m");
        let spun = ";; MODULE 'module m'\n;; src/lib.rs:5:9: 5:15 (#0)\n\n;; query spun off because: bitvector\n;; Function-Def demo::m::g\n;; src/lib.rs:5:9: 5:15 (#0)\n(check-valid\n";
        let (queries, reason) = air_queries(spun);
        assert_eq!(reason.as_deref(), Some("bitvector"));
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].function, "demo::m::g");
    }

    #[test]
    fn spinoff_indices_come_from_file_stems() {
        assert_eq!(spinoff_index("root"), None);
        assert_eq!(spinoff_index("foo__bar"), None);
        assert_eq!(spinoff_index("arithinput!arith.bv_and._01"), Some(1));
        assert_eq!(spinoff_index("m_reruncrate!m.f._12_expand"), Some(12));
        assert_eq!(spinoff_index("x._ab"), None);
    }
}
