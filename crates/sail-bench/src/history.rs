//! `sail-bench history`: does a check's outcome depend on the session's
//! history, or only on the state key v1 binds? (ADR 0004.)
//!
//! For every check of every captured session, a fresh-process input is
//! built from exact bytes, with no normalization:
//! 1. every `set-option` sent before the check, in the original order;
//! 2. the session's `set-logic`/`set-info` commands (Verus sends none);
//! 3. the assertion stack as `sail-session`'s snapshot records it: each
//!    level's commands in order, with `(push)` between levels;
//! 4. `(get-info :all-statistics)`, the check, `(get-info :all-statistics)`;
//! 5. the commands the session sent after the check, up to and including
//!    its next `(get-info :all-statistics)` and before its next check: the
//!    follow-ups (`get-info :reason-unknown`, `get-model`), the
//!    `set-option :rlimit 0` before them, and whatever else came in between.
//!    A final `(get-info :all-statistics)` is added if there was none.
//!
//! The fresh run is compared with the recorded in-session replies:
//! - the answer;
//! - each follow-up reply, byte for byte;
//! - the `rlimit-count` delta, measured the same way on both sides: from the
//!   last statistics before the check to the first statistics after its
//!   follow-ups.
//!
//! The check-only delta (between the two statistics around the check in the
//! fresh run) is reported alongside.
//!
//! Separately, checks whose v1 keys are equal are grouped across the whole
//! corpus, and their recorded outcomes compared: a natural experiment on the
//! same question.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use sail_key::{Mode, SolverIdentity, query_key};
use sail_session::{Event, Session, Snapshot};
use serde::Serialize;

use crate::solver::{self, Identity, Outcome, Solver};
use crate::stats::rlimit_count;
use crate::trace::SessionFiles;
use crate::transcript::{CheckObservation, Class, Transcript, split_commands, split_replies};

const STATS: &[u8] = b"(get-info :all-statistics)";

/// A check prepared for the fresh-process replay.
#[derive(Debug, Clone)]
pub struct PreparedCheck {
    /// Check index within the session.
    pub seq: usize,
    /// The snapshot at the check.
    pub snapshot: Box<Snapshot>,
    /// Indices of the `set-option` commands sent before the check.
    pub options: Vec<usize>,
    /// What the recorded session observed.
    pub recorded: CheckObservation,
    /// Indices of the commands after the check that the fresh input
    /// repeats.
    pub after: Vec<usize>,
}

/// A session prepared for the experiment.
#[derive(Debug, Clone)]
pub struct PreparedSession {
    /// Session id.
    pub id: String,
    /// The recorded transcript.
    pub transcript: Transcript,
    /// Its checks.
    pub checks: Vec<PreparedCheck>,
}

impl PreparedSession {
    /// Splits, pairs and models one recorded session.
    #[must_use]
    pub fn new(id: &str, input: &[u8], output: &[u8]) -> Self {
        let transcript = Transcript::new(input, output);
        let observations = transcript.checks();
        let mut session = Session::new();
        let mut options = Vec::new();
        let mut checks = Vec::new();
        for (i, cmd) in transcript.commands.iter().enumerate() {
            let event = session.apply(&cmd.bytes);
            if cmd.class == Class::SetOption {
                options.push(i);
            }
            if let Event::Check(snapshot) = event {
                let seq = checks.len();
                let Some(recorded) = observations.get(seq).cloned() else {
                    continue;
                };
                checks.push(PreparedCheck {
                    seq,
                    snapshot,
                    options: options.clone(),
                    recorded,
                    after: following(&transcript, i),
                });
            }
        }
        Self {
            id: id.to_owned(),
            transcript,
            checks,
        }
    }

    /// The exact fresh-process input for `check` (see the module docs).
    #[must_use]
    pub fn fresh_input(&self, check: &PreparedCheck) -> Vec<u8> {
        let mut out = Vec::new();
        let mut line = |b: &[u8]| {
            out.extend_from_slice(b);
            out.push(b'\n');
        };
        for &i in &check.options {
            if let Some(c) = self.transcript.commands.get(i) {
                line(&c.bytes);
            }
        }
        for g in &check.snapshot.globals {
            line(g);
        }
        for (n, level) in check.snapshot.levels.iter().enumerate() {
            if n > 0 {
                line(b"(push)");
            }
            for c in &level.commands {
                line(c);
            }
        }
        line(STATS);
        line(&check.snapshot.check);
        line(STATS);
        let mut saw_stats = false;
        for &i in &check.after {
            if let Some(c) = self.transcript.commands.get(i) {
                saw_stats |= c.class == Class::Statistics;
                line(&c.bytes);
            }
        }
        if !saw_stats {
            line(STATS);
        }
        out
    }
}

/// Indices of the commands after the check at `index` that the fresh input
/// repeats: up to and including the next statistics request, stopping
/// before the next check.
fn following(t: &Transcript, index: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for (j, c) in t.commands.iter().enumerate().skip(index + 1) {
        match c.class {
            Class::Check => break,
            Class::Statistics => {
                out.push(j);
                break;
            }
            _ => out.push(j),
        }
    }
    out
}

/// What the fresh run observed for its single check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshObservation {
    /// The answer.
    pub answer: Option<Vec<u8>>,
    /// The follow-up replies, in order (`None` where unpaired).
    pub followups: Vec<Option<Vec<u8>>>,
    /// `rlimit-count` delta across the check alone.
    pub check_only_delta: Option<u64>,
    /// `rlimit-count` delta from before the check to after the follow-ups,
    /// comparable with the in-session delta.
    pub delta: Option<u64>,
}

/// Reads the fresh run's transcript (`input` is what [`PreparedSession::fresh_input`] built).
#[must_use]
pub fn observe_fresh(input: &[u8], output: &[u8]) -> FreshObservation {
    let t = Transcript::from_parts(split_commands(input), &split_replies(output));
    let mut obs = FreshObservation {
        answer: None,
        followups: Vec::new(),
        check_only_delta: None,
        delta: None,
    };
    let Some(check) = t.commands.iter().position(|c| c.class == Class::Check) else {
        return obs;
    };
    let stats = |i: usize| -> Option<u64> { t.reply(i).and_then(rlimit_count) };
    let before = (0..check)
        .rev()
        .find(|&i| {
            t.commands
                .get(i)
                .is_some_and(|c| c.class == Class::Statistics)
        })
        .and_then(stats);
    obs.answer = t.reply(check).map(<[u8]>::to_vec);
    let after_check = check + 1;
    let first_after = (after_check..t.commands.len()).find(|&i| {
        t.commands
            .get(i)
            .is_some_and(|c| c.class == Class::Statistics)
    });
    let last = (after_check..t.commands.len()).rev().find(|&i| {
        t.commands
            .get(i)
            .is_some_and(|c| c.class == Class::Statistics)
    });
    if let (Some(b), Some(a)) = (before, first_after.and_then(stats)) {
        obs.check_only_delta = a.checked_sub(b);
    }
    if let (Some(b), Some(a)) = (before, last.and_then(stats)) {
        obs.delta = a.checked_sub(b);
    }
    let start = first_after.map_or(after_check, |i| i + 1);
    for (i, c) in t.commands.iter().enumerate().skip(start) {
        if c.class == Class::Query {
            obs.followups.push(t.reply(i).map(<[u8]>::to_vec));
        }
    }
    obs
}

/// Whether two sides agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Agreement {
    /// Both present and equal.
    Equal,
    /// Both present and different.
    Differ,
    /// One side is missing (unpaired, infra failure, no follow-ups).
    Unavailable,
}

fn agree<T: PartialEq>(a: Option<&T>, b: Option<&T>) -> Agreement {
    match (a, b) {
        (Some(a), Some(b)) if a == b => Agreement::Equal,
        (Some(_), Some(_)) => Agreement::Differ,
        _ => Agreement::Unavailable,
    }
}

/// One check's comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryCheck {
    /// Session id.
    pub id: String,
    /// Check index in the session.
    pub seq: usize,
    /// Checks before this one in the session.
    pub prior_checks: u64,
    /// Commands before this one in the session.
    pub position: u64,
    /// Number of stack commands.
    pub stack_len: usize,
    /// In-session answer.
    pub in_session: Option<String>,
    /// Fresh-process answer.
    pub fresh: Option<String>,
    /// Answer agreement.
    pub answer: Agreement,
    /// Follow-up agreement: equal only if every follow-up reply is
    /// byte-identical; unavailable if there are none.
    pub followups: Agreement,
    /// Number of follow-up replies compared.
    pub followups_compared: usize,
    /// rlimit delta agreement.
    pub rlimit: Agreement,
    /// In-session rlimit delta.
    pub rlimit_in_session: Option<u64>,
    /// Fresh rlimit delta (same span).
    pub rlimit_fresh: Option<u64>,
    /// Fresh rlimit delta across the check alone.
    pub rlimit_fresh_check_only: Option<u64>,
    /// Whether the error labels a model sets to `true` agree: the part of
    /// a model Verus reads to choose its diagnostics.
    pub labels: Agreement,
    /// The first follow-up difference, truncated: `(in_session, fresh)`.
    pub followup_diff: Option<(String, String)>,
    /// Set when the fresh run did not complete normally
    /// (`infra-timeout`, start failure, solver errors).
    pub infra: Option<String>,
    /// Fresh run wall-clock milliseconds.
    pub elapsed_ms: u64,
}

fn excerpt(a: &[u8], b: &[u8]) -> (String, String) {
    let first = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()));
    let from = first.saturating_sub(80);
    let cut = |s: &[u8]| {
        let end = (first + 160).min(s.len());
        format!(
            "@{first}: …{}…",
            String::from_utf8_lossy(s.get(from..end).unwrap_or_default())
        )
    };
    (cut(a), cut(b))
}

/// Runs one check fresh and compares it.
#[must_use]
pub fn compare(solver: &Solver, session: &PreparedSession, check: &PreparedCheck) -> HistoryCheck {
    let input = session.fresh_input(check);
    let mut result = HistoryCheck {
        id: session.id.clone(),
        seq: check.seq,
        prior_checks: check.snapshot.prior_checks,
        position: check.snapshot.position,
        stack_len: check.snapshot.stack_len(),
        in_session: check
            .recorded
            .answer
            .as_deref()
            .map(|a| String::from_utf8_lossy(a).into_owned()),
        fresh: None,
        answer: Agreement::Unavailable,
        followups: Agreement::Unavailable,
        followups_compared: 0,
        rlimit: Agreement::Unavailable,
        rlimit_in_session: check.recorded.rlimit_delta,
        rlimit_fresh: None,
        rlimit_fresh_check_only: None,
        labels: Agreement::Unavailable,
        followup_diff: None,
        infra: None,
        elapsed_ms: 0,
    };
    let run = match solver::run(solver, &input) {
        Ok(r) => r,
        Err(e) => {
            result.infra = Some(e.to_string());
            return result;
        }
    };
    result.elapsed_ms = u64::try_from(run.elapsed.as_millis()).unwrap_or(u64::MAX);
    if run.outcome == Outcome::InfraTimeout {
        result.infra = Some("infra-timeout".to_owned());
        return result;
    }
    let errors = split_replies(&run.stdout)
        .into_iter()
        .filter(|r| r.starts_with(b"(error"))
        .count();
    if errors > 0 {
        result.infra = Some(format!("{errors} solver error replies"));
    }
    let fresh = observe_fresh(&input, &run.stdout);
    result.fresh = fresh
        .answer
        .as_deref()
        .map(|a| String::from_utf8_lossy(a).into_owned());
    result.answer = agree(check.recorded.answer.as_ref(), fresh.answer.as_ref());
    result.rlimit_fresh = fresh.delta;
    result.rlimit_fresh_check_only = fresh.check_only_delta;
    result.rlimit = agree(check.recorded.rlimit_delta.as_ref(), fresh.delta.as_ref());
    compare_followups(&mut result, &check.recorded, &fresh);
    result
}

/// Compares follow-up replies byte for byte, and the true error labels of
/// any models among them.
fn compare_followups(
    result: &mut HistoryCheck,
    recorded: &CheckObservation,
    fresh: &FreshObservation,
) {
    let pairs = recorded
        .followups
        .iter()
        .map(|f| f.reply.as_ref())
        .zip(fresh.followups.iter().map(Option::as_ref));
    let mut agreement =
        if recorded.followups.is_empty() || recorded.followups.len() != fresh.followups.len() {
            Agreement::Unavailable
        } else {
            Agreement::Equal
        };
    for (a, b) in pairs {
        match agree(a, b) {
            Agreement::Equal => result.followups_compared += 1,
            Agreement::Differ => {
                result.followups_compared += 1;
                if agreement != Agreement::Differ {
                    if let (Some(a), Some(b)) = (a, b) {
                        result.followup_diff = Some(excerpt(a, b));
                    }
                    agreement = Agreement::Differ;
                }
            }
            Agreement::Unavailable => {
                if agreement == Agreement::Equal {
                    agreement = Agreement::Unavailable;
                }
            }
        }
    }
    result.followups = agreement;
    let in_session: Vec<&[u8]> = recorded
        .followups
        .iter()
        .filter_map(|f| f.reply.as_deref())
        .collect();
    let fresh_replies: Vec<&[u8]> = fresh
        .followups
        .iter()
        .flatten()
        .map(Vec::as_slice)
        .collect();
    if agreement != Agreement::Unavailable && in_session.iter().any(|r| is_model(r)) {
        let labels = |replies: &[&[u8]]| -> Vec<Vec<String>> {
            replies
                .iter()
                .filter(|r| is_model(r))
                .map(|r| true_labels(r))
                .collect()
        };
        result.labels = agree(Some(&labels(&in_session)), Some(&labels(&fresh_replies)));
    }
}

/// Whether a reply looks like a `get-model` model (a list of definitions).
fn is_model(reply: &[u8]) -> bool {
    reply.starts_with(b"(") && reply.windows(10).any(|w| w == b"define-fun")
}

/// The names of the Verus error labels (`%%location_label%%n` and
/// `%%global_location_label%%n`) that `model` defines as `true`, sorted.
#[must_use]
pub fn true_labels(model: &[u8]) -> Vec<String> {
    use sail_smtlib::sexpr::parse_one;
    let Ok(tree) = parse_one(model) else {
        return Vec::new();
    };
    let mut out: Vec<String> = tree
        .as_list()
        .unwrap_or_default()
        .iter()
        .filter_map(|def| {
            let items = def.as_list()?;
            let [head, name, params, sort, value] = items else {
                return None;
            };
            let name = name.text(model);
            let is_label = head.text(model) == b"define-fun"
                && name.windows(16).any(|w| w == b"location_label%%")
                && params.as_list().is_some_and(<[_]>::is_empty)
                && sort.text(model) == b"Bool"
                && value.text(model) == b"true";
            is_label.then(|| String::from_utf8_lossy(name).into_owned())
        })
        .collect();
    out.sort();
    out
}

/// Counts of each agreement kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Tally {
    /// Both present and equal.
    pub equal: usize,
    /// Both present and different.
    pub differ: usize,
    /// Not comparable.
    pub unavailable: usize,
}

impl Tally {
    fn add(&mut self, a: Agreement) {
        match a {
            Agreement::Equal => self.equal += 1,
            Agreement::Differ => self.differ += 1,
            Agreement::Unavailable => self.unavailable += 1,
        }
    }
}

/// A group of checks, across the corpus, with the same v1 key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyGroup {
    /// The key.
    pub key: String,
    /// `id#seq` of each member.
    pub members: Vec<String>,
    /// Distinct recorded answers.
    pub answers: Vec<String>,
    /// Whether all members' follow-up replies are byte-identical.
    pub followups_equal: bool,
    /// Distinct recorded rlimit deltas.
    pub rlimit_deltas: Vec<Option<u64>>,
}

/// Recorded outcomes of checks that share a v1 key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct KeyReport {
    /// Checks with a key (exact snapshots).
    pub keyed_checks: usize,
    /// Checks without one (inexact snapshots).
    pub unkeyed_checks: usize,
    /// Keys computed here that differ from the key in the shim's trace.
    pub trace_key_mismatches: usize,
    /// Distinct keys.
    pub distinct_keys: usize,
    /// Keys shared by two or more checks.
    pub shared_keys: usize,
    /// Shared keys whose members all gave the same answer.
    pub shared_same_answer: usize,
    /// Shared keys whose members all gave the same answer and follow-ups.
    pub shared_same_answer_and_followups: usize,
    /// Shared keys whose members all had the same rlimit delta.
    pub shared_same_rlimit: usize,
    /// Groups with differing answers or follow-ups (up to the example
    /// limit).
    pub disagreeing: Vec<KeyGroup>,
}

/// The `history` report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryReport {
    /// The solver replayed against.
    pub solver: Identity,
    /// Sessions read.
    pub sessions: usize,
    /// Checks seen.
    pub checks: usize,
    /// Checks skipped because the session model was inexact.
    pub inexact_skipped: usize,
    /// Checks run fresh.
    pub replayed: usize,
    /// Answer agreement.
    pub answer: Tally,
    /// Follow-up agreement.
    pub followups: Tally,
    /// Follow-up replies compared in total.
    pub followup_replies_compared: usize,
    /// rlimit agreement (same span on both sides).
    pub rlimit: Tally,
    /// rlimit agreement for the first check of each session.
    pub rlimit_first_check: Tally,
    /// rlimit agreement for every later check.
    pub rlimit_later_checks: Tally,
    /// Checks whose fresh check-only delta equals the in-session delta.
    pub rlimit_check_only_equal: usize,
    /// Agreement of the true error labels in models (what Verus reads).
    pub labels: Tally,
    /// Answer agreement by in-session answer: `answer -> tally`.
    pub answer_by_in_session: BTreeMap<String, Tally>,
    /// Checks with answer, follow-ups and rlimit all equal.
    pub all_equal: usize,
    /// Fresh runs that hit the safety-net timeout (not solver answers).
    pub infra_timeouts: usize,
    /// Fresh runs that failed or had solver errors.
    pub infra_errors: Vec<String>,
    /// Total fresh-run wall-clock milliseconds.
    pub replay_ms: u64,
    /// Same-key groups across the corpus.
    pub keys: KeyReport,
    /// Examples of disagreement: answers first, then follow-ups, then
    /// rlimit (or every check with `--all-checks`).
    pub examples: Vec<HistoryCheck>,
}

/// Options for [`history`].
#[derive(Debug, Clone)]
pub struct HistoryOptions {
    /// Worker threads.
    pub jobs: usize,
    /// How many examples to keep.
    pub examples: usize,
    /// Report every check.
    pub all_checks: bool,
}

/// Parses the `version` word from `--version` output.
fn version_word(line: &str) -> Option<String> {
    let mut words = line.split_whitespace();
    words.find(|w| *w == "version")?;
    words.next().map(str::to_owned)
}

fn key_report(
    sessions: &[PreparedSession],
    traces: &[Option<Vec<Option<String>>>],
    solver: &Solver,
    identity: &Identity,
    limit: usize,
) -> KeyReport {
    let mut report = KeyReport::default();
    let id = match (
        identity.version.as_deref().and_then(version_word),
        std::fs::read(&solver.path).ok(),
    ) {
        (Some(version), Some(binary)) => Some(SolverIdentity {
            name: "z3".to_owned(),
            version,
            binary_sha256: sail_key::sha256(&binary),
        }),
        _ => None,
    };
    let Some(id) = id else {
        return report;
    };
    let mut groups: BTreeMap<String, Vec<(&str, &PreparedCheck)>> = BTreeMap::new();
    for (session, trace_keys) in sessions.iter().zip(traces) {
        for check in &session.checks {
            match query_key(&check.snapshot, &id, &Mode::Default) {
                Some(k) => {
                    report.keyed_checks += 1;
                    let k = k.to_string();
                    if let Some(Some(Some(t))) = trace_keys.as_ref().map(|keys| keys.get(check.seq))
                        && *t != k
                    {
                        report.trace_key_mismatches += 1;
                    }
                    groups.entry(k).or_default().push((&session.id, check));
                }
                None => report.unkeyed_checks += 1,
            }
        }
    }
    report.distinct_keys = groups.len();
    for (key, members) in groups.iter().filter(|(_, m)| m.len() > 1) {
        report.shared_keys += 1;
        let mut answers: Vec<String> = members
            .iter()
            .map(|(_, c)| {
                c.recorded.answer.as_deref().map_or_else(
                    || "(none)".to_owned(),
                    |a| String::from_utf8_lossy(a).into_owned(),
                )
            })
            .collect();
        answers.sort();
        answers.dedup();
        let first = members.first().map(|(_, c)| &c.recorded.followups);
        let followups_equal = members
            .iter()
            .all(|(_, c)| Some(&c.recorded.followups) == first);
        let mut rlimit_deltas: Vec<Option<u64>> = members
            .iter()
            .map(|(_, c)| c.recorded.rlimit_delta)
            .collect();
        rlimit_deltas.sort_unstable();
        rlimit_deltas.dedup();
        if answers.len() == 1 {
            report.shared_same_answer += 1;
            if followups_equal {
                report.shared_same_answer_and_followups += 1;
            }
        }
        if rlimit_deltas.len() == 1 {
            report.shared_same_rlimit += 1;
        }
        if (answers.len() > 1 || !followups_equal) && report.disagreeing.len() < limit {
            report.disagreeing.push(KeyGroup {
                key: key.clone(),
                members: members
                    .iter()
                    .map(|(s, c)| format!("{s}#{}", c.seq))
                    .collect(),
                answers,
                followups_equal,
                rlimit_deltas,
            });
        }
    }
    report
}

/// Adds every result to the report's tallies.
fn tally(report: &mut HistoryReport, all: &[HistoryCheck]) {
    for r in all {
        report.replayed += 1;
        report.replay_ms += r.elapsed_ms;
        match r.infra.as_deref() {
            Some("infra-timeout") => report.infra_timeouts += 1,
            Some(why) => report
                .infra_errors
                .push(format!("{}#{}: {why}", r.id, r.seq)),
            None => {}
        }
        report.answer.add(r.answer);
        report.followups.add(r.followups);
        report.followup_replies_compared += r.followups_compared;
        report.rlimit.add(r.rlimit);
        if r.prior_checks == 0 {
            report.rlimit_first_check.add(r.rlimit);
        } else {
            report.rlimit_later_checks.add(r.rlimit);
        }
        if r.labels != Agreement::Unavailable {
            report.labels.add(r.labels);
        }
        if r.rlimit_in_session.is_some() && r.rlimit_in_session == r.rlimit_fresh_check_only {
            report.rlimit_check_only_equal += 1;
        }
        report
            .answer_by_in_session
            .entry(r.in_session.clone().unwrap_or_else(|| "(none)".to_owned()))
            .or_default()
            .add(r.answer);
        if r.answer == Agreement::Equal
            && r.followups != Agreement::Differ
            && r.rlimit == Agreement::Equal
        {
            report.all_equal += 1;
        }
    }
}

/// Disagreement examples: answers first, then follow-ups, then rlimit.
fn pick_examples(mut all: Vec<HistoryCheck>, limit: usize) -> Vec<HistoryCheck> {
    let mut examples = Vec::new();
    for pick in [
        |r: &HistoryCheck| r.answer == Agreement::Differ,
        |r: &HistoryCheck| r.answer != Agreement::Differ && r.followups == Agreement::Differ,
        |r: &HistoryCheck| {
            r.answer != Agreement::Differ
                && r.followups != Agreement::Differ
                && r.rlimit == Agreement::Differ
        },
    ] {
        let (take, rest): (Vec<HistoryCheck>, Vec<HistoryCheck>) = all.into_iter().partition(pick);
        examples.extend(take.into_iter().take(limit));
        all = rest;
    }
    examples
}

/// Runs the history-dependence experiment.
#[must_use]
pub fn history(
    solver: &Solver,
    sessions: &[SessionFiles],
    options: &HistoryOptions,
) -> HistoryReport {
    let loaded = crate::parallel::map(
        sessions,
        options.jobs,
        |files| {
            let input = std::fs::read(&files.input).ok()?;
            let output = std::fs::read(&files.output).ok()?;
            let trace_keys = crate::trace::load(files)
                .ok()
                .map(|t| t.checks.into_iter().map(|c| c.key).collect::<Vec<_>>());
            Some((PreparedSession::new(&files.id, &input, &output), trace_keys))
        },
        |_| {},
    );
    let identity = solver.identity();
    let mut report = HistoryReport {
        solver: identity.clone(),
        sessions: sessions.len(),
        checks: 0,
        inexact_skipped: 0,
        replayed: 0,
        answer: Tally::default(),
        followups: Tally::default(),
        followup_replies_compared: 0,
        rlimit: Tally::default(),
        rlimit_first_check: Tally::default(),
        rlimit_later_checks: Tally::default(),
        rlimit_check_only_equal: 0,
        labels: Tally::default(),
        answer_by_in_session: BTreeMap::new(),
        all_equal: 0,
        infra_timeouts: 0,
        infra_errors: Vec::new(),
        replay_ms: 0,
        keys: KeyReport::default(),
        examples: Vec::new(),
    };
    let mut prepared = Vec::new();
    let mut trace_keys = Vec::new();
    for (files, l) in sessions.iter().zip(loaded) {
        match l {
            Some((p, k)) => {
                prepared.push(p);
                trace_keys.push(k);
            }
            None => report
                .infra_errors
                .push(format!("{}: cannot read the recorded transcript", files.id)),
        }
    }
    report.keys = key_report(&prepared, &trace_keys, solver, &identity, options.examples);
    let mut units = Vec::new();
    for (s, session) in prepared.iter().enumerate() {
        for (c, check) in session.checks.iter().enumerate() {
            report.checks += 1;
            if check.snapshot.inexact.is_some() {
                report.inexact_skipped += 1;
            } else {
                units.push((s, c));
            }
        }
    }
    let total = units.len();
    let results = crate::parallel::map(
        &units,
        options.jobs,
        |&(s, c)| {
            let session = prepared.get(s)?;
            let check = session.checks.get(c)?;
            Some(compare(solver, session, check))
        },
        |done| crate::progress("history", done, total),
    );
    let all: Vec<HistoryCheck> = results.into_iter().flatten().collect();
    tally(&mut report, &all);
    report.examples = if options.all_checks {
        all
    } else {
        pick_examples(all, options.examples)
    };
    report
}

/// A human-readable summary.
#[must_use]
pub fn render(r: &HistoryReport) -> String {
    let mut s = String::new();
    let t = |t: &Tally| {
        format!(
            "equal {}  differ {}  n/a {}",
            t.equal, t.differ, t.unavailable
        )
    };
    let _ = writeln!(
        s,
        "solver: {} ({})",
        r.solver.version.as_deref().unwrap_or("?"),
        r.solver.sha256.as_deref().unwrap_or("?")
    );
    let _ = writeln!(
        s,
        "sessions: {}  checks: {}  inexact (skipped): {}  replayed fresh: {}",
        r.sessions, r.checks, r.inexact_skipped, r.replayed
    );
    let _ = writeln!(s, "answer:     {}", t(&r.answer));
    let _ = writeln!(
        s,
        "follow-ups: {}  ({} replies compared)",
        t(&r.followups),
        r.followup_replies_compared
    );
    let _ = writeln!(s, "model labels (true error labels): {}", t(&r.labels));
    let _ = writeln!(s, "rlimit:     {}", t(&r.rlimit));
    let _ = writeln!(
        s,
        "  first check of a session: {}; later checks: {}",
        t(&r.rlimit_first_check),
        t(&r.rlimit_later_checks)
    );
    let _ = writeln!(
        s,
        "rlimit (fresh check-only span equals in-session span): {}",
        r.rlimit_check_only_equal
    );
    for (answer, tally) in &r.answer_by_in_session {
        let _ = writeln!(s, "  in-session {answer}: {}", t(tally));
    }
    let _ = writeln!(s, "all equal (answer, follow-ups, rlimit): {}", r.all_equal);
    let k = &r.keys;
    let _ = writeln!(
        s,
        "v1 keys: {} keyed, {} unkeyed, {} distinct, {} shared; shared with same answer {} (and follow-ups {}), same rlimit {}; trace-key mismatches {}",
        k.keyed_checks,
        k.unkeyed_checks,
        k.distinct_keys,
        k.shared_keys,
        k.shared_same_answer,
        k.shared_same_answer_and_followups,
        k.shared_same_rlimit,
        k.trace_key_mismatches
    );
    let _ = writeln!(
        s,
        "infra: {} timeouts, {} errors",
        r.infra_timeouts,
        r.infra_errors.len()
    );
    for e in r.examples.iter().take(15) {
        let _ = writeln!(
            s,
            "  {} #{} (prior checks {}): answer {:?} -> {:?} [{:?}], follow-ups {:?}, rlimit {:?} -> {:?} (check-only {:?})",
            e.id,
            e.seq,
            e.prior_checks,
            e.in_session,
            e.fresh,
            e.answer,
            e.followups,
            e.rlimit_in_session,
            e.rlimit_fresh,
            e.rlimit_fresh_check_only
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &[u8] = b"(set-option :auto_config false)\n\
(declare-const x Int)\n\
(push)\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n\
(declare-const y Int)\n(assert (> x y))\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n\
(set-option :rlimit 100)\n(check-sat)\n(echo \"<<DONE>>\")\n\
(set-option :rlimit 0)\n(get-model)\n(echo \"<<DONE>>\")\n\
(assert (not l))\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n\
(pop)\n(push)\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n\
(assert false)\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n\
(set-option :rlimit 100)\n(check-sat)\n(echo \"<<DONE>>\")\n\
(set-option :rlimit 0)\n(pop)\n(push)\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n";

    const OUTPUT: &[u8] = b"(:rlimit-count 10)\n<<DONE>>\n(:rlimit-count 12)\n<<DONE>>\n\
sat\n<<DONE>>\n(model)\n<<DONE>>\n(:rlimit-count 40)\n<<DONE>>\n\
(:rlimit-count 41)\n<<DONE>>\n(:rlimit-count 41)\n<<DONE>>\nunsat\n<<DONE>>\n\
(:rlimit-count 50)\n<<DONE>>\n";

    fn session() -> PreparedSession {
        PreparedSession::new("s", INPUT, OUTPUT)
    }

    #[test]
    fn builds_exact_fresh_inputs() {
        let s = session();
        assert_eq!(s.checks.len(), 2);
        let first = String::from_utf8_lossy(&s.fresh_input(&s.checks[0])).into_owned();
        assert_eq!(
            first,
            "(set-option :auto_config false)\n(set-option :rlimit 100)\n\
(declare-const x Int)\n(push)\n(declare-const y Int)\n(assert (> x y))\n\
(get-info :all-statistics)\n(check-sat)\n(get-info :all-statistics)\n\
(echo \"<<DONE>>\")\n(set-option :rlimit 0)\n(get-model)\n(echo \"<<DONE>>\")\n\
(assert (not l))\n(get-info :all-statistics)\n"
        );
        let second = String::from_utf8_lossy(&s.fresh_input(&s.checks[1])).into_owned();
        assert!(second.starts_with(
            "(set-option :auto_config false)\n(set-option :rlimit 100)\n(set-option :rlimit 0)\n(set-option :rlimit 100)\n(declare-const x Int)\n(push)\n(assert false)\n"
        ));
        // The follow-ups run through the next statistics request.
        assert!(
            second.ends_with("(set-option :rlimit 0)\n(pop)\n(push)\n(get-info :all-statistics)\n")
        );
        assert_eq!(s.checks[1].recorded.rlimit_delta, Some(9));
    }

    #[test]
    fn observes_the_fresh_run() {
        let s = session();
        let input = s.fresh_input(&s.checks[0]);
        // One reply per output-producing command, echo lines included.
        let output = b"(:rlimit-count 3)\nsat\n(:rlimit-count 30)\n<<DONE>>\n(model)\n<<DONE>>\n(:rlimit-count 31)\n";
        let fresh = observe_fresh(&input, output);
        assert_eq!(fresh.answer.as_deref(), Some(&b"sat"[..]));
        assert_eq!(fresh.check_only_delta, Some(27));
        assert_eq!(fresh.delta, Some(28));
        assert_eq!(fresh.followups, vec![Some(b"(model)".to_vec())]);
        // In session the same span was 40 - 12.
        assert_eq!(s.checks[0].recorded.rlimit_delta, Some(28));
    }

    #[test]
    fn reads_true_labels_from_models() {
        let model = b"(\n  (define-fun %%location_label%%1 () Bool\n    true)\n  \
(define-fun %%location_label%%0 () Bool\n    false)\n  \
(define-fun %%global_location_label%%3 () Bool\n    true)\n  \
(define-fun f ((x!0 Int)) Bool\n    true)\n)";
        assert!(is_model(model));
        assert_eq!(
            true_labels(model),
            ["%%global_location_label%%3", "%%location_label%%1"]
        );
        assert_eq!(true_labels(b"(error \"x\")"), Vec::<String>::new());
        assert!(!is_model(b"(:reason-unknown \"canceled\")"));
    }

    #[test]
    fn agreement() {
        assert_eq!(agree(Some(&1), Some(&1)), Agreement::Equal);
        assert_eq!(agree(Some(&1), Some(&2)), Agreement::Differ);
        assert_eq!(agree::<u8>(None, Some(&2)), Agreement::Unavailable);
        assert_eq!(
            version_word("Z3 version 4.16.0 - 64 bit"),
            Some("4.16.0".to_owned())
        );
        let (a, b) = excerpt(b"abcdef", b"abcxef");
        assert!(a.starts_with("@3: ") && a.contains("abcdef"));
        assert!(b.contains("abcxef"));
    }
}
