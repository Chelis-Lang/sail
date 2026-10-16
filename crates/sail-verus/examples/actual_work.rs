//! Strict, untimed work observer for the default full-prefix-replay cache shim.
//!
//! `cargo run --release -p sail-verus --example actual_work -- --result RESULT --run-dir RUN`
//! joins only that completed run's sidecars. It counts every check in a session
//! which started Z3, including cached-prefix checks replayed into the solver.
//! It does not count historical report provenance and does not support snapshot
//! or warm-prefix process experiments. Missing evidence is an error, never zero.
#![forbid(unsafe_code)]

use sail_api::project::{Currentness, ProjectResult, SCHEMA_VERSION};
use sail_verus::attribution::{LogFile, SessionRecord, Transcript};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[path = "actual_work_support/completion.rs"]
mod completion;

const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_FILES: usize = 100_000;

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(message.to_string())
}

fn read(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return Err(invalid(format!(
            "unsupported evidence file {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    let file = std::fs::File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    #[cfg(not(unix))]
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("evidence is no longer a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_err(invalid)? > MAX_FILE_BYTES {
        return Err(invalid("evidence file grew beyond the limit"));
    }
    Ok(bytes)
}

fn directory(path: &Path) -> io::Result<()> {
    if !std::fs::symlink_metadata(path)?.is_dir() {
        return Err(invalid(format!(
            "not an evidence directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn entries(path: &Path) -> io::Result<Vec<PathBuf>> {
    directory(path)?;
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(path)? {
        paths.push(entry?.path());
        if paths.len() > MAX_FILES {
            return Err(invalid("too many evidence entries"));
        }
    }
    paths.sort();
    Ok(paths)
}

fn transcripts(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut pending = vec![path.to_owned()];
    let mut files = Vec::new();
    let mut examined = 0usize;
    while let Some(directory) = pending.pop() {
        for path in entries(&directory)? {
            examined += 1;
            if examined > MAX_FILES {
                return Err(invalid("too many log entries"));
            }
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.is_dir() {
                pending.push(path);
            } else if !metadata.is_file() {
                return Err(invalid("special file or alias in logs"));
            } else if path.extension().is_some_and(|e| e == "smt_transcript") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

#[derive(Clone)]
struct Counts {
    checks: u64,
    replies: u64,
    check_reply_indices: Vec<u64>,
}

fn log_counts(path: &Path) -> io::Result<(String, Counts)> {
    let bytes = read(path)?;
    log_counts_payload(path, &bytes, None)
}

fn log_counts_payload(
    path: &Path,
    bytes: &[u8],
    completed_air: Option<&[u8]>,
) -> io::Result<(String, Counts)> {
    // Lossy transcript decoding must never authorize a complete observation.
    std::str::from_utf8(bytes).map_err(invalid)?;
    let transcript = Transcript::parse(bytes);
    if transcript.batches.is_empty()
        || transcript.batches.iter().any(|b| b.response.is_none())
        || !bytes.ends_with(b";;;<<<\n")
    {
        return Err(invalid("empty or interrupted transcript"));
    }
    let air = String::from_utf8(match completed_air {
        Some(bytes) => bytes.to_vec(),
        None => read(&path.with_extension("air"))?,
    })
    .map_err(invalid)?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("non-UTF log name"))?;
    let log = LogFile::new(stem, &air, bytes);
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

fn record(path: &Path) -> io::Result<SessionRecord> {
    let bytes = read(path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(invalid)?;
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

#[derive(Default, Serialize)]
struct Work {
    sessions: u64,
    solver_sessions: u64,
    transcript_checks: u64,
    actual_solver_checks: u64,
    checks_answered_from_cache: u64,
    new_solver_answers: u64,
    cached_prefix_checks_replayed: u64,
}

fn join(mut logs: BTreeMap<String, Vec<Counts>>, records: Vec<SessionRecord>) -> io::Result<Work> {
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

fn observe(result_path: &Path, run: &Path) -> io::Result<serde_json::Value> {
    observe_with_completion(result_path, run, None)
}

fn observe_with_completion(
    result_path: &Path,
    run: &Path,
    validator: Option<&completion::Validator>,
) -> io::Result<serde_json::Value> {
    let result: ProjectResult = serde_json::from_slice(&read(result_path)?).map_err(invalid)?;
    if result.kind != "sail-project-result"
        || result.schema_version != SCHEMA_VERSION
        || result.currentness != Currentness::Current
        || result.exit_code != Some(0)
        || result.error.is_some()
        || result.targets.iter().any(|target| !target.complete)
        || run.file_name().and_then(|s| s.to_str()) != Some(result.run_id.as_str())
    {
        return Err(invalid(
            "result does not identify a successful complete Current run",
        ));
    }
    directory(run)?;
    let sidecars = run.join("sidecars");
    let children = match entries(&sidecars) {
        Ok(children) => children,
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                && result.execution.verifier_invocations == 0 =>
        {
            Vec::new()
        }
        Err(error) => return Err(error),
    };
    if u64::try_from(children.len()).map_err(invalid)? != result.execution.verifier_invocations {
        return Err(invalid(
            "current-run sidecars do not cover every verifier invocation",
        ));
    }
    let mut logs: BTreeMap<String, Vec<Counts>> = BTreeMap::new();
    let mut records = Vec::new();
    let mut evidence_files = 0usize;
    let mut completed_zero_contexts = Vec::new();
    let mut completed_used_contexts = 0u64;
    for child in children {
        let paths = transcripts(&child.join("logs"))?;
        if paths.is_empty() {
            return Err(invalid(
                "invocation has no complete solver transcript evidence",
            ));
        }
        for path in paths {
            evidence_files += 1;
            if evidence_files > MAX_FILES {
                return Err(invalid("too many evidence files"));
            }
            let (hash, counts) = if let Some(validator) = validator {
                let transcript = read(&path)?;
                let air = read(&path.with_extension("air"))?;
                let (transcript_body, mode) = completion::payload(&transcript, "transcript")?;
                let (air_body, air_mode) = completion::payload(&air, "air")?;
                if mode != air_mode {
                    return Err(invalid("AIR/transcript completion modes differ"));
                }
                if mode == completion::Mode::Zero {
                    if !transcript_body.is_empty() {
                        return Err(invalid("zero-process transcript contains solver bytes"));
                    }
                    validator.validate(air_body)?;
                    use sha2::{Digest, Sha256};
                    completed_zero_contexts.push(serde_json::json!({
                        "transcript": path,
                        "transcript_sha256": format!("{:x}", Sha256::digest(&transcript)),
                        "air_sha256": format!("{:x}", Sha256::digest(&air)),
                    }));
                    continue;
                }
                completed_used_contexts += 1;
                log_counts_payload(&path, transcript_body, Some(air_body))?
            } else {
                log_counts(&path)?
            };
            logs.entry(hash).or_default().push(counts);
        }
        for path in entries(&child.join("provenance"))? {
            evidence_files += 1;
            if evidence_files > MAX_FILES {
                return Err(invalid("too many evidence files"));
            }
            if path.extension().is_none_or(|extension| extension != "json") {
                return Err(invalid("unfinished or unknown session evidence"));
            }
            let record = record(&path)?;
            if validator.is_some()
                && record.input_sha256
                    == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            {
                return Err(invalid(
                    "completed zero-mode evidence cannot have an empty-input session",
                ));
            }
            records.push(record);
        }
    }
    let work = join(logs, records)?;
    let mut value = serde_json::json!({"schema_version": 1, "run_id": result.run_id,
        "scope": "default full-prefix-replay shim, separate untimed capture", "work": work});
    if let Some(validator) = validator {
        value["paired_log_completion"] = serde_json::json!({
            "version": 1, "validator": validator,
            "completed_zero_contexts": completed_zero_contexts,
            "completed_used_contexts": completed_used_contexts,
            "scope": "explicit source-bound logging experiment; not fork adoption",
        });
    }
    Ok(value)
}

fn run() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: --result RESULT --run-dir RUN [--paired-completion-validator PATH --validator-sha256 SHA --validator-source COMMIT]";
    let [result_option, result, run_option, run, tail @ ..] = args.as_slice() else {
        return Err(invalid(usage));
    };
    if result_option != "--result" || run_option != "--run-dir" {
        return Err(invalid(usage));
    }
    let validator = match tail {
        [] => None,
        [option, path, sha_option, sha, source_option, source]
            if option == "--paired-completion-validator"
                && sha_option == "--validator-sha256"
                && source_option == "--validator-source" =>
        {
            Some(completion::Validator::new(Path::new(path), sha, source)?)
        }
        _ => return Err(invalid(usage)),
    };
    let value = match validator.as_ref() {
        Some(validator) => {
            observe_with_completion(Path::new(result), Path::new(run), Some(validator))?
        }
        None => observe(Path::new(result), Path::new(run))?,
    };
    println!("{}", serde_json::to_string_pretty(&value).map_err(invalid)?);
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error[SAIL0930]: {error}");
            std::process::ExitCode::FAILURE
        }
    }
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

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> io::Result<Self> {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "sail-work-observer-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&root)?;
            Ok(Self(root))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn zero_invocations_still_require_the_actual_run_directory() -> io::Result<()> {
        let fixture = Fixture::new()?;
        let result = fixture.0.join("result.json");
        let run = fixture.0.join("run");
        let document = serde_json::json!({"kind": "sail-project-result", "schema_version": SCHEMA_VERSION,
            "run_id": "run", "workspace": fixture.0, "context_sha256": "0".repeat(64),
            "input_revision": "0".repeat(64), "currentness": "current", "exit_code": 0,
            "targets": [], "execution": {"verifier_invocations": 0, "reused_targets": 0, "wall_ms": 0},
            "notes": [], "error": null});
        std::fs::write(&result, serde_json::to_vec(&document).map_err(invalid)?)?;
        assert!(observe(&result, &run).is_err());
        std::fs::create_dir(&run)?;
        assert_eq!(observe(&result, &run)?["work"]["actual_solver_checks"], 0);
        #[cfg(unix)]
        {
            std::fs::rename(&run, fixture.0.join("saved"))?;
            std::os::unix::fs::symlink(fixture.0.join("saved"), &run)?;
            assert!(observe(&result, &run).is_err());
        }
        Ok(())
    }

    #[test]
    fn incomplete_model_reply_is_never_a_complete_work_observation() -> io::Result<()> {
        let fixture = Fixture::new()?;
        let transcript = fixture.0.join("module.smt_transcript");
        std::fs::write(
            transcript.with_extension("air"),
            ";; Function-Def demo\n(check-valid dummy)\n",
        )?;
        let query =
            ";;;>>> QUERY\n(push)\n(check-sat)\n(get-model)\n(pop)\n;;;<<<\n;;;>>> RESPONSE\n";
        std::fs::write(
            &transcript,
            format!("{query}sat\n(model (define-fun x () Bool true))\n;;;<<<\n"),
        )?;
        assert_eq!(log_counts(&transcript)?.1.checks, 1);
        std::fs::write(
            &transcript,
            format!("{query}sat\n(model (define-fun x () Bool true)\n;;;<<<\n"),
        )?;
        assert!(log_counts(&transcript).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn paired_zero_evidence_is_opt_in_and_cannot_hide_any_session() -> io::Result<()> {
        use sha2::{Digest, Sha256};
        use std::os::unix::fs::PermissionsExt as _;
        let fixture = Fixture::new()?;
        let run = fixture.0.join("run");
        let child = run.join("sidecars/target");
        let logs = child.join("logs");
        let provenance = child.join("provenance");
        std::fs::create_dir_all(&logs)?;
        std::fs::create_dir(&provenance)?;
        let result = fixture.0.join("result.json");
        let mut document = serde_json::json!({"kind": "sail-project-result", "schema_version": SCHEMA_VERSION,
            "run_id": "run", "workspace": fixture.0, "context_sha256": "0".repeat(64),
            "input_revision": "0".repeat(64), "currentness": "current", "exit_code": 0,
            "targets": [], "execution": {"verifier_invocations": 1, "reused_targets": 0, "wall_ms": 0},
            "notes": [], "error": null});
        std::fs::write(&result, serde_json::to_vec(&document).map_err(invalid)?)?;
        // This script tests the observer's helper protocol only. Genuine AIR
        // grammar positives/negatives belong to the source-bound AIR example.
        let helper = fixture.0.join("validator");
        std::fs::write(
            &helper,
            b"#!/bin/sh\ncat >/dev/null\nprintf 'SAIL-AIR-NO-QUERY 1\\n'\n",
        )?;
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700))?;
        let digest = format!("{:x}", Sha256::digest(read(&helper)?));
        let validator = completion::Validator::new(&helper, &digest, &"a".repeat(40))?;
        let transcript = logs.join("zero.smt_transcript");
        let air = transcript.with_extension("air");
        std::fs::write(&transcript, b";; SAIL-COMPLETE 1 transcript zero-process\n")?;
        std::fs::write(
            &air,
            b"(declare-sort T 0)\n;; SAIL-COMPLETE 1 air zero-process\n",
        )?;
        assert!(observe(&result, &run).is_err());
        let observed = observe_with_completion(&result, &run, Some(&validator))?;
        assert_eq!(observed["work"]["actual_solver_checks"], 0);
        assert_eq!(
            observed["paired_log_completion"]["completed_zero_contexts"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        for bad in [
            b"".as_slice(),
            b"(check-sat)\n;; SAIL-COMPLETE 1 transcript zero-process\n",
            b";; SAIL-COMPLETE 1 transcript process-used\n",
        ] {
            std::fs::write(&transcript, bad)?;
            assert!(observe_with_completion(&result, &run, Some(&validator)).is_err());
        }
        std::fs::write(&transcript, b";; SAIL-COMPLETE 1 transcript zero-process\n")?;
        std::fs::remove_file(&air)?;
        assert!(observe_with_completion(&result, &run, Some(&validator)).is_err());
        std::fs::write(
            &air,
            b"(declare-sort T 0)\n;; SAIL-COMPLETE 1 air zero-process\n",
        )?;
        for hash in [
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned(),
            "a".repeat(64),
        ] {
            std::fs::write(
                provenance.join("unexpected.json"),
                serde_json::to_vec(&serde_json::json!({
                    "version":1, "input_sha256":hash, "replies_served":0, "solver_started":false
                }))
                .map_err(invalid)?,
            )?;
            assert!(observe_with_completion(&result, &run, Some(&validator)).is_err());
        }
        std::fs::remove_file(provenance.join("unexpected.json"))?;
        document["currentness"] = serde_json::json!("unknown");
        std::fs::write(&result, serde_json::to_vec(&document).map_err(invalid)?)?;
        assert!(observe_with_completion(&result, &run, Some(&validator)).is_err());
        Ok(())
    }
}
