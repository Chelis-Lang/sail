//! The observation tap: raw transcripts plus a JSON-lines trace of checks.
//!
//! For a session with id `<pid>-<nanos>`, the tap writes into the trace
//! directory:
//! - `<id>.in.smt2`: every byte sent to the solver;
//! - `<id>.out`: every byte the solver produced;
//! - `<id>.jsonl`: one `start` record, one `check` record per satisfiability
//!   check, `stats` records for `:all-statistics` replies, and one `end`
//!   record.
//!
//! Responses are paired with commands by order. Pairing stops, and the trace
//! says so, the first time the solver reports an error or the client sends a
//! command whose output cannot be predicted.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sail_key::{Mode, PrefixHasher, SolverIdentity, query_key, sha256};
use sail_session::{Event, Session, Snapshot};
use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, ResponseSplitter};
use serde::Serialize;

use crate::TapEvent;

/// Trace format version; bump on any incompatible change to the records.
const TRACE_VERSION: u32 = 2;

/// What the tap needs to know about its session.
pub(crate) struct Context {
    pub dir: PathBuf,
    pub solver: PathBuf,
    pub args: Vec<String>,
    pub started: Instant,
    pub overflowed: Arc<AtomicBool>,
}

#[derive(Serialize)]
struct SolverRecord {
    path: String,
    /// The version from `--version`, as `(get-info :version)` reports it.
    version: Option<String>,
    sha256: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Record<'a> {
    Start {
        trace_version: u32,
        pid: u32,
        args: &'a [String],
        solver: &'a SolverRecord,
    },
    Check {
        seq: u64,
        /// Key v1 (stack only; measurement, ADR 0004).
        key: Option<String>,
        /// Key v2 (whole process prefix; default mode, ADR 0008).
        prefix_key: Option<String>,
        answer: Option<String>,
        sent_us: u64,
        answered_us: Option<u64>,
        depth: usize,
        stack_len: usize,
        position: u64,
        prior_checks: u64,
        rlimit: Option<String>,
        inexact: Option<String>,
    },
    Stats {
        after_check: Option<u64>,
        rlimit_count: Option<u64>,
    },
    SolverError {
        text: String,
    },
    End {
        exit_code: Option<i32>,
        commands: u64,
        checks: u64,
        responses: u64,
        complete: bool,
        paired: bool,
    },
}

enum Pending {
    Check {
        seq: u64,
        snapshot: Box<Snapshot>,
        key: Option<String>,
        prefix_key: Option<String>,
        sent: Instant,
    },
    Statistics,
    Other,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn micros(since: Instant, at: Instant) -> u64 {
    u64::try_from(at.saturating_duration_since(since).as_micros()).unwrap_or(u64::MAX)
}

/// The version from `z3 --version` output such as `Z3 version 4.16.0 - 64 bit`.
fn solver_version(path: &PathBuf) -> Option<String> {
    let out = std::process::Command::new(path)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut words = text.split_whitespace();
    while let Some(w) = words.next() {
        if w == "version" {
            return words.next().map(str::to_owned);
        }
    }
    None
}

/// The number after `:rlimit-count` in an `:all-statistics` reply.
fn rlimit_count(reply: &[u8]) -> Option<u64> {
    const NEEDLE: &[u8] = b":rlimit-count";
    let at = reply.windows(NEEDLE.len()).position(|w| w == NEEDLE)?;
    let rest = reply.get(at + NEEDLE.len()..)?;
    let digits: Vec<u8> = rest
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .take_while(u8::is_ascii_digit)
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

struct Tap<'a> {
    ctx: &'a Context,
    raw_in: BufWriter<File>,
    raw_out: BufWriter<File>,
    records: BufWriter<File>,
    identity: Option<SolverIdentity>,
    /// Running v2 key over the raw input (ADR 0008).
    prefix: Option<PrefixHasher>,
    /// Raw input not yet fed to `prefix`.
    unhashed: Vec<u8>,
    commands: CommandSplitter,
    responses: ResponseSplitter,
    session: Session,
    pending: VecDeque<Pending>,
    paired: bool,
    command_count: u64,
    check_count: u64,
    response_count: u64,
    last_check: Option<u64>,
    failed: bool,
}

impl Tap<'_> {
    fn record(&mut self, record: &Record<'_>) {
        if self.failed {
            return;
        }
        let ok = serde_json::to_writer(&mut self.records, record).is_ok()
            && self.records.write_all(b"\n").is_ok();
        if !ok {
            self.failed = true;
        }
    }

    fn check_record(&mut self, pending: Pending, answer: Option<&[u8]>, at: Option<Instant>) {
        let Pending::Check {
            seq,
            snapshot,
            key,
            prefix_key,
            sent,
        } = pending
        else {
            return;
        };
        let started = self.ctx.started;
        let record = Record::Check {
            seq,
            key,
            prefix_key,
            answer: answer.map(|a| String::from_utf8_lossy(a).into_owned()),
            sent_us: micros(started, sent),
            answered_us: at.map(|t| micros(started, t)),
            depth: snapshot.levels.len().saturating_sub(1),
            stack_len: snapshot.stack_len(),
            position: snapshot.position,
            prior_checks: snapshot.prior_checks,
            rlimit: snapshot
                .options
                .get(&b":rlimit"[..])
                .map(|v| String::from_utf8_lossy(v).into_owned()),
            inexact: snapshot.inexact.map(|i| format!("{i:?}")),
        };
        self.record(&record);
    }

    fn lose_pairing(&mut self) {
        self.paired = false;
        while let Some(p) = self.pending.pop_front() {
            self.check_record(p, None, None);
        }
    }

    fn on_input(&mut self, bytes: &[u8], at: Instant) {
        if self.raw_in.write_all(bytes).is_err() {
            self.failed = true;
        }
        self.unhashed.extend_from_slice(bytes);
        let mut items = Vec::new();
        self.commands.push(bytes, &mut items);
        for item in items {
            self.command_count += 1;
            // Feed the raw stream up to the end of this command, so the v2
            // key at a check covers exactly the bytes up to and including it.
            if let Some(prefix) = &mut self.prefix {
                let end = item.offset + item.bytes.len() as u64;
                let n = usize::try_from(end.saturating_sub(prefix.len()))
                    .unwrap_or(usize::MAX)
                    .min(self.unhashed.len());
                prefix.update(self.unhashed.get(..n).unwrap_or_default());
                self.unhashed.drain(..n);
            }
            let expects = self.session.expects_output(&item.bytes);
            let is_stats = matches!(
                classify(&item.bytes).map(|c| c.kind),
                Ok(CommandKind::GetInfo {
                    keyword: b":all-statistics"
                })
            );
            let pending = match self.session.apply(&item.bytes) {
                Event::Check(snapshot) => {
                    let seq = self.check_count;
                    self.check_count += 1;
                    self.last_check = Some(seq);
                    let key = self
                        .identity
                        .as_ref()
                        .and_then(|id| query_key(&snapshot, id, &Mode::Default))
                        .map(|k| k.to_string());
                    let prefix_key = self.prefix.as_ref().map(|p| p.key().to_string());
                    Some(Pending::Check {
                        seq,
                        snapshot,
                        key,
                        prefix_key,
                        sent: at,
                    })
                }
                Event::Command if is_stats => Some(Pending::Statistics),
                Event::Command => match expects {
                    Output::One => Some(Pending::Other),
                    Output::None => None,
                    Output::Unknown => {
                        self.lose_pairing();
                        None
                    }
                },
            };
            match pending {
                Some(p) if self.paired => self.pending.push_back(p),
                Some(p) => self.check_record(p, None, None),
                None => {}
            }
        }
    }

    fn on_output(&mut self, bytes: &[u8], at: Instant) {
        if self.raw_out.write_all(bytes).is_err() {
            self.failed = true;
        }
        let mut items = Vec::new();
        self.responses.push(bytes, &mut items);
        for item in items {
            self.response_count += 1;
            if item.bytes.starts_with(b"(error") {
                let text = String::from_utf8_lossy(&item.bytes).into_owned();
                self.record(&Record::SolverError { text });
                self.lose_pairing();
                continue;
            }
            if !self.paired {
                continue;
            }
            match self.pending.pop_front() {
                Some(p @ Pending::Check { .. }) => {
                    self.check_record(p, Some(&item.bytes), Some(at));
                }
                Some(Pending::Statistics) => {
                    let record = Record::Stats {
                        after_check: self.last_check,
                        rlimit_count: rlimit_count(&item.bytes),
                    };
                    self.record(&record);
                }
                Some(Pending::Other) => {}
                None => self.lose_pairing(),
            }
        }
    }

    fn on_end(&mut self, exit_code: Option<i32>) {
        let paired = self.paired;
        while let Some(p) = self.pending.pop_front() {
            self.check_record(p, None, None);
        }
        let record = Record::End {
            exit_code,
            commands: self.command_count,
            checks: self.check_count,
            responses: self.response_count,
            complete: !self.ctx.overflowed.load(Ordering::Relaxed) && !self.failed,
            paired,
        };
        self.record(&record);
        let _ = self.raw_in.flush();
        let _ = self.raw_out.flush();
        let _ = self.records.flush();
    }
}

fn create(dir: &std::path::Path, name: &str) -> Option<BufWriter<File>> {
    File::create(dir.join(name)).ok().map(BufWriter::new)
}

/// Runs the tap until the session ends. Any failure to set up simply
/// returns, which disconnects the channel and turns the tap off.
pub(crate) fn run(ctx: &Context, rx: &Receiver<TapEvent>) {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let id = format!("{}-{nanos}", std::process::id());
    if std::fs::create_dir_all(&ctx.dir).is_err() {
        return;
    }
    let (Some(raw_in), Some(raw_out), Some(records)) = (
        create(&ctx.dir, &format!("{id}.in.smt2")),
        create(&ctx.dir, &format!("{id}.out")),
        create(&ctx.dir, &format!("{id}.jsonl")),
    ) else {
        return;
    };
    let version = solver_version(&ctx.solver);
    let digest = std::fs::read(&ctx.solver).ok().map(|b| sha256(&b));
    let identity = match (&version, digest) {
        (Some(version), Some(binary_sha256)) => Some(SolverIdentity {
            name: "z3".to_owned(),
            version: version.clone(),
            binary_sha256,
            platform: SolverIdentity::host_platform(),
        }),
        _ => None,
    };
    let solver = SolverRecord {
        path: ctx.solver.display().to_string(),
        version,
        sha256: digest.map(|d| hex(&d)),
    };
    let mut tap = Tap {
        ctx,
        raw_in,
        raw_out,
        records,
        prefix: identity
            .as_ref()
            .map(|id| PrefixHasher::new(id, &Mode::Default)),
        unhashed: Vec::new(),
        identity,
        commands: CommandSplitter::new(),
        responses: ResponseSplitter::new(),
        session: Session::new(),
        pending: VecDeque::new(),
        paired: true,
        command_count: 0,
        check_count: 0,
        response_count: 0,
        last_check: None,
        failed: false,
    };
    tap.record(&Record::Start {
        trace_version: TRACE_VERSION,
        pid: std::process::id(),
        args: &ctx.args,
        solver: &solver,
    });
    while let Ok(event) = rx.recv() {
        match event {
            TapEvent::Input(bytes, at) => tap.on_input(&bytes, at),
            TapEvent::Output(bytes, at) => tap.on_output(&bytes, at),
            TapEvent::End(code) => {
                tap.on_end(code);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::rlimit_count;

    #[test]
    fn reads_rlimit_count() {
        assert_eq!(
            rlimit_count(b"(:added-eqs 73\n :rlimit-count        24862)"),
            Some(24862)
        );
        assert_eq!(rlimit_count(b"(:memory 17.69)"), None);
    }
}
