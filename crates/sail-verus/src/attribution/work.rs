//! Pure transcript/session work accounting shared by untimed diagnostics.
//!
//! These functions do not establish source, process, run or proof authority.
//! Whole-run and explicitly partial observers enforce their own unchanged gates.

use std::collections::BTreeMap;
use std::io;

use super::{LogFile, SessionRecord, Transcript};
use serde::Serialize;

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(message.to_string())
}

/// Complete per-session check/reply counts, constructed only from validated logs.
#[derive(Clone)]
pub struct Counts {
    checks: u64,
    replies: u64,
    check_reply_indices: Vec<u64>,
}

impl Counts {
    /// Every check command in the literal session input, including prefix replay.
    #[must_use]
    pub fn checks(&self) -> u64 {
        self.checks
    }
}

/// Count a complete literal transcript and its matching AIR attribution.
///
/// This validates command/reply coverage only. It supplies no process ownership,
/// source authority, currentness or whole-invocation completeness guarantee.
/// Callers must bound the input and independently bind its origin.
///
/// # Errors
/// Returns an error for incomplete, unmodeled or unattributed transcript bytes.
pub fn transcript(air: &str, bytes: &[u8], stem: &str) -> io::Result<(String, Counts)> {
    // Lossy transcript decoding must never authorize a complete observation.
    std::str::from_utf8(bytes).map_err(invalid)?;
    let transcript = Transcript::parse(bytes);
    if transcript.batches.is_empty()
        || transcript.batches.iter().any(|b| b.response.is_none())
        || !bytes.ends_with(b";;;<<<\n")
    {
        return Err(invalid("empty or interrupted transcript"));
    }
    let log = LogFile::new(stem, air, bytes);
    if log.air.len() != log.ops.len() {
        return Err(invalid("AIR/transcript query coverage differs"));
    }
    let mut checks = 0u64;
    let mut check_reply_indices = Vec::new();
    let mut command_checks = 0u64;
    let mut replies = u64::try_from(transcript.batches.len()).map_err(invalid)?;
    for batch in &transcript.batches {
        let mut parser = sail_smtlib::split::CommandSplitter::new();
        let mut items = Vec::new();
        parser.push(&batch.query, &mut items);
        parser.push(b"\n", &mut items);
        items.extend(parser.finish());
        for item in items {
            if item.kind != sail_smtlib::split::ItemKind::List {
                return Err(invalid("incomplete or non-list transcript command"));
            }
            let command = sail_smtlib::command::classify(&item.bytes).map_err(invalid)?;
            if matches!(command.kind, sail_smtlib::command::CommandKind::Other) {
                return Err(invalid("unmodeled transcript command"));
            }
            if command.kind.is_check() {
                command_checks = command_checks
                    .checked_add(1)
                    .ok_or_else(|| invalid("command count overflow"))?;
            }
        }
    }
    for op in &log.ops {
        for check in &op.checks {
            if check.answer.is_none() {
                return Err(invalid("unattributed check response"));
            }
            checks = checks
                .checked_add(1)
                .ok_or_else(|| invalid("check overflow"))?;
            check_reply_indices.push(check.reply_index);
        }
    }
    for batch in transcript.batches {
        let mut parser = sail_smtlib::split::ResponseSplitter::new();
        let mut items = Vec::new();
        let response = batch
            .response
            .ok_or_else(|| invalid("missing response"))?
            .join("\n")
            + "\n";
        parser.push(response.as_bytes(), &mut items);
        items.extend(parser.finish());
        if items
            .iter()
            .any(|item| item.kind == sail_smtlib::split::ItemKind::Stray)
        {
            return Err(invalid("incomplete solver response"));
        }
        replies = replies
            .checked_add(u64::try_from(items.len()).map_err(invalid)?)
            .ok_or_else(|| invalid("reply overflow"))?;
    }
    if checks != command_checks {
        return Err(invalid("solver checks are not completely attributed"));
    }
    Ok((
        log.input_sha256,
        Counts {
            checks,
            replies,
            check_reply_indices,
        },
    ))
}

/// Parse the unchanged strict cached-session provenance wire format.
///
/// This validates only the record; callers must bind the producer PID/run and
/// prove that every actual session is represented once.
///
/// # Errors
/// Returns an error for malformed or unsupported provenance fields.
pub fn session(bytes: &[u8]) -> io::Result<SessionRecord> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(invalid)?;
    let keys = value
        .as_object()
        .ok_or_else(|| invalid("session is not an object"))?;
    if keys.len() != 4
        || ![
            "version",
            "input_sha256",
            "replies_served",
            "solver_started",
        ]
        .iter()
        .all(|key| keys.contains_key(*key))
    {
        return Err(invalid("unknown session fields"));
    }
    let record: SessionRecord = serde_json::from_value(value).map_err(invalid)?;
    if record.version != 1
        || record.input_sha256.len() != 64
        || !record
            .input_sha256
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(invalid("unsupported session version or input digest"));
    }
    Ok(record)
}

/// SHA-256 of literal bytes, without rewriting their content.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

/// Exact joined work totals; serializable observations, never proof verdicts.
#[derive(Default, Serialize)]
pub struct Work {
    sessions: u64,
    solver_sessions: u64,
    transcript_checks: u64,
    actual_solver_checks: u64,
    checks_answered_from_cache: u64,
    new_solver_answers: u64,
    cached_prefix_checks_replayed: u64,
}

/// Join every provenance record to one distinct literal input transcript.
///
/// Repeated identical inputs retain multiplicity. A started solver counts all
/// checks, including cached prefix checks replayed into that actual process.
/// Missing, unmatched or inconsistent coverage is an error, never zero work.
///
/// # Errors
/// Returns an error for missing/duplicate coverage, impossible counts or overflow.
pub fn join(
    mut logs: BTreeMap<String, Vec<Counts>>,
    records: Vec<SessionRecord>,
) -> io::Result<Work> {
    let mut work = Work::default();
    for record in records {
        let counts = logs
            .get_mut(&record.input_sha256)
            .and_then(Vec::pop)
            .ok_or_else(|| invalid("session has no distinct matching transcript"))?;
        if record.replies_served > counts.replies
            || (!record.solver_started && record.replies_served != counts.replies)
        {
            return Err(invalid("session reply coverage differs"));
        }
        work.sessions += 1;
        let cached = u64::try_from(
            counts
                .check_reply_indices
                .iter()
                .filter(|index| **index < record.replies_served)
                .count(),
        )
        .map_err(invalid)?;
        work.checks_answered_from_cache = work
            .checks_answered_from_cache
            .checked_add(cached)
            .ok_or_else(|| invalid("cached check overflow"))?;
        work.transcript_checks = work
            .transcript_checks
            .checked_add(counts.checks)
            .ok_or_else(|| invalid("total check overflow"))?;
        if record.solver_started {
            work.solver_sessions += 1;
            work.actual_solver_checks = work
                .actual_solver_checks
                .checked_add(counts.checks)
                .ok_or_else(|| invalid("solver check overflow"))?;
            work.new_solver_answers = work
                .new_solver_answers
                .checked_add(
                    counts
                        .checks
                        .checked_sub(cached)
                        .ok_or_else(|| invalid("cached checks exceed transcript"))?,
                )
                .ok_or_else(|| invalid("new answer overflow"))?;
            work.cached_prefix_checks_replayed = work
                .cached_prefix_checks_replayed
                .checked_add(cached)
                .ok_or_else(|| invalid("replayed check overflow"))?;
        }
    }
    if logs.values().any(|counts| !counts.is_empty()) {
        return Err(invalid("transcript has no distinct matching session"));
    }
    Ok(work)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(started: bool) -> SessionRecord {
        SessionRecord {
            version: 1,
            input_sha256: "a".repeat(64),
            replies_served: 4,
            solver_started: started,
        }
    }

    fn logs(count: usize) -> BTreeMap<String, Vec<Counts>> {
        BTreeMap::from([(
            "a".repeat(64),
            vec![
                Counts {
                    checks: 3,
                    replies: 4,
                    check_reply_indices: vec![0, 1, 2],
                };
                count
            ],
        )])
    }

    #[test]
    fn counts_replayed_prefix_checks_even_when_their_answers_were_cached() -> io::Result<()> {
        let work = join(logs(2), vec![session(false), session(true)])?;
        assert_eq!(work.sessions, 2);
        assert_eq!(work.transcript_checks, 6);
        assert_eq!(work.actual_solver_checks, 3);
        assert_eq!(work.solver_sessions, 1);
        assert_eq!(work.checks_answered_from_cache, 6);
        assert_eq!(work.new_solver_answers, 0);
        assert_eq!(work.cached_prefix_checks_replayed, 3);
        let mut partial = session(true);
        partial.replies_served = 2;
        let partial = join(logs(1), vec![partial])?;
        assert_eq!(partial.actual_solver_checks, 3);
        assert_eq!(partial.new_solver_answers, 1);
        assert_eq!(partial.cached_prefix_checks_replayed, 2);
        assert_eq!(join(logs(1), vec![session(false)])?.actual_solver_checks, 0);
        Ok(())
    }

    #[test]
    fn identical_inputs_still_require_one_transcript_per_session() {
        assert!(join(logs(1), vec![session(false), session(true)]).is_err());
        assert!(join(logs(2), vec![session(true)]).is_err());
        let mut uncovered = session(false);
        uncovered.replies_served = 3;
        assert!(join(logs(1), vec![uncovered]).is_err());
    }
}
