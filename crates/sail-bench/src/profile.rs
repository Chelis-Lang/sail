//! `sail-bench profile`: how solver time is distributed across queries.
//!
//! Solve time is `answered_us - sent_us` from the shim's check records: the
//! wall-clock time from the moment the batch holding the `check-sat`
//! reached the shim to the moment the answer came back. Checks without an
//! answer (pairing was lost) are counted but have no time.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use crate::stats::{Distribution, as_f64};
use crate::trace::{SessionFiles, SessionTrace};

/// Per-process totals.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcessProfile {
    /// Session id.
    pub id: String,
    /// Checks in the session.
    pub checks: usize,
    /// Checks with an answer.
    pub answered: usize,
    /// Sum of solve times, in microseconds.
    pub total_us: u64,
    /// Longest single check, in microseconds.
    pub max_us: u64,
    /// Whether the tap kept pairing to the end.
    pub paired: Option<bool>,
}

/// The slowest checks, for the report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SlowCheck {
    /// Session id.
    pub id: String,
    /// Check index in the session.
    pub seq: u64,
    /// Solve time in microseconds.
    pub solve_us: u64,
    /// The answer.
    pub answer: Option<String>,
}

/// The `profile` report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Profile {
    /// Sessions read.
    pub sessions: usize,
    /// Check records read.
    pub checks: usize,
    /// Checks with no paired answer.
    pub unanswered: usize,
    /// Sessions whose tap lost pairing.
    pub unpaired_sessions: usize,
    /// Sessions whose trace is marked incomplete or has no end record.
    pub incomplete_sessions: usize,
    /// Distribution of per-check solve time, in microseconds.
    pub solve_us: Distribution,
    /// Answer counts, by answer text.
    pub answers: BTreeMap<String, usize>,
    /// Per-trace-group (top-level directory entry) totals.
    pub groups: BTreeMap<String, GroupProfile>,
    /// Per-process totals, sorted by total time, largest first.
    pub processes: Vec<ProcessProfile>,
    /// The slowest checks, slowest first.
    pub slowest: Vec<SlowCheck>,
}

/// Totals for one group of sessions (one run, for example one example
/// file).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GroupProfile {
    /// Sessions.
    pub sessions: usize,
    /// Checks.
    pub checks: usize,
    /// Sum of solve times, in microseconds.
    pub total_us: u64,
}

/// How many slowest checks to keep.
const SLOWEST: usize = 25;

/// Builds the profile from loaded traces.
#[must_use]
pub fn profile(sessions: &[(SessionFiles, SessionTrace)]) -> Profile {
    let mut times = Vec::new();
    let mut answers = BTreeMap::new();
    let mut processes = Vec::new();
    let mut groups: BTreeMap<String, GroupProfile> = BTreeMap::new();
    let mut slowest = Vec::new();
    let mut unanswered = 0;
    let mut checks = 0;
    for (files, trace) in sessions {
        let mut p = ProcessProfile {
            id: files.id.clone(),
            checks: trace.checks.len(),
            answered: 0,
            total_us: 0,
            max_us: 0,
            paired: trace.end.as_ref().map(|e| e.paired),
        };
        for c in &trace.checks {
            checks += 1;
            let answer = c.answer.clone().unwrap_or_else(|| "(unpaired)".to_owned());
            *answers.entry(answer).or_insert(0) += 1;
            match c.solve_us() {
                Some(t) if c.answer.is_some() => {
                    times.push(t);
                    p.answered += 1;
                    p.total_us += t;
                    p.max_us = p.max_us.max(t);
                    slowest.push(SlowCheck {
                        id: files.id.clone(),
                        seq: c.seq,
                        solve_us: t,
                        answer: c.answer.clone(),
                    });
                }
                _ => unanswered += 1,
            }
        }
        let group = files.id.split('/').next().unwrap_or_default().to_owned();
        let g = groups.entry(group).or_default();
        g.sessions += 1;
        g.checks += p.checks;
        g.total_us += p.total_us;
        processes.push(p);
    }
    processes.sort_by(|a, b| b.total_us.cmp(&a.total_us).then(a.id.cmp(&b.id)));
    slowest.sort_by_key(|c| std::cmp::Reverse(c.solve_us));
    slowest.truncate(SLOWEST);
    let unpaired_sessions = sessions
        .iter()
        .filter(|(_, t)| t.end.as_ref().is_some_and(|e| !e.paired))
        .count();
    let incomplete_sessions = sessions
        .iter()
        .filter(|(_, t)| t.end.as_ref().is_none_or(|e| !e.complete))
        .count();
    Profile {
        sessions: sessions.len(),
        checks,
        unanswered,
        unpaired_sessions,
        incomplete_sessions,
        solve_us: Distribution::of(&times),
        answers,
        groups,
        processes,
        slowest,
    }
}

fn ms(us: Option<u64>) -> String {
    us.map_or_else(
        || "-".to_owned(),
        |u| format!("{:.2} ms", as_f64(u) / 1000.0),
    )
}

fn pct(x: Option<f64>) -> String {
    x.map_or_else(|| "-".to_owned(), |v| format!("{:.1}%", v * 100.0))
}

/// A human-readable summary.
#[must_use]
pub fn render(p: &Profile) -> String {
    let mut s = String::new();
    let d = &p.solve_us;
    let _ = writeln!(
        s,
        "sessions: {}  checks: {}  answered: {}  unanswered: {}  (unpaired sessions: {}, incomplete: {})",
        p.sessions, p.checks, d.count, p.unanswered, p.unpaired_sessions, p.incomplete_sessions
    );
    let _ = writeln!(s, "solve time per check:");
    let _ = writeln!(
        s,
        "  total {}  mean {}  p50 {}  p90 {}  p99 {}  max {}",
        ms(Some(d.total)),
        d.mean
            .map_or_else(|| "-".to_owned(), |m| format!("{:.2} ms", m / 1000.0)),
        ms(d.p50),
        ms(d.p90),
        ms(d.p99),
        ms(d.max)
    );
    let _ = writeln!(
        s,
        "  share of total time in the slowest 1% of checks: {}, slowest 10%: {}",
        pct(d.top_1pct_share),
        pct(d.top_10pct_share)
    );
    let answers: Vec<String> = p.answers.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let _ = writeln!(s, "answers: {}", answers.join("  "));
    let _ = writeln!(s, "slowest checks:");
    for c in p.slowest.iter().take(10) {
        let _ = writeln!(
            s,
            "  {:>10}  {} #{} {}",
            ms(Some(c.solve_us)),
            c.id,
            c.seq,
            c.answer.as_deref().unwrap_or("-")
        );
    }
    let _ = writeln!(
        s,
        "processes by total solve time (top 10 of {}):",
        p.processes.len()
    );
    for proc in p.processes.iter().take(10) {
        let _ = writeln!(
            s,
            "  {:>10}  {} ({} checks, max {})",
            ms(Some(proc.total_us)),
            proc.id,
            proc.checks,
            ms(Some(proc.max_us))
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{CheckRecord, EndRecord};
    use std::path::PathBuf;

    fn check(seq: u64, answer: Option<&str>, sent: u64, answered: Option<u64>) -> CheckRecord {
        CheckRecord {
            seq,
            key: None,
            prefix_key: None,
            answer: answer.map(str::to_owned),
            sent_us: sent,
            answered_us: answered,
            depth: 1,
            stack_len: 1,
            position: 0,
            prior_checks: seq,
            rlimit: None,
            inexact: None,
        }
    }

    fn files(id: &str) -> SessionFiles {
        SessionFiles {
            id: id.to_owned(),
            jsonl: PathBuf::new(),
            input: PathBuf::new(),
            output: PathBuf::new(),
        }
    }

    #[test]
    fn profiles_checks_and_processes() {
        let end = |paired| EndRecord {
            exit_code: Some(0),
            commands: 0,
            checks: 0,
            responses: 0,
            complete: true,
            paired,
        };
        let sessions = vec![
            (
                files("a/1"),
                SessionTrace {
                    trace_version: Some(1),
                    checks: vec![
                        check(0, Some("unsat"), 0, Some(100)),
                        check(1, Some("unknown"), 200, Some(1200)),
                    ],
                    solver_errors: vec![],
                    end: Some(end(true)),
                },
            ),
            (
                files("b/2"),
                SessionTrace {
                    trace_version: Some(1),
                    checks: vec![check(0, None, 0, None), check(1, Some("sat"), 10, Some(20))],
                    solver_errors: vec![],
                    end: Some(end(false)),
                },
            ),
        ];
        let p = profile(&sessions);
        assert_eq!(p.checks, 4);
        assert_eq!(p.unanswered, 1);
        assert_eq!(p.unpaired_sessions, 1);
        assert_eq!(p.solve_us.total, 1110);
        assert_eq!(p.solve_us.max, Some(1000));
        assert_eq!(p.answers.get("(unpaired)"), Some(&1));
        assert_eq!(p.answers.get("unsat"), Some(&1));
        assert_eq!(p.processes[0].id, "a/1");
        assert_eq!(p.groups.get("b").map(|g| g.checks), Some(2));
        assert_eq!(p.slowest[0].solve_us, 1000);
        assert!(render(&p).contains("checks: 4"));
    }
}
