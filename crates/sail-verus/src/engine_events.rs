//! Source-engine observation relay. It never participates in solver output.
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sail_api::proof::{InputEvent, Observation, Phase, Target, VERSION};
use serde_json::Value;

use crate::managed::{Runtime, TargetIdentity};

const MAX_SOURCE: u64 = 16 * 1024 * 1024;
const MAX_EVENT: usize = 4 * 1024 * 1024;
const MAX_RESULT: usize = 32 * 1024 * 1024;

pub(crate) struct Relay {
    done: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Relay {
    pub(crate) fn start(path: PathBuf, runtime: &Runtime, identity: &TargetIdentity) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&done);
        let runtime = runtime.clone();
        let identity = identity.clone();
        let destination = std::env::var_os("SAIL_PROOF_EVENTS").map(PathBuf::from);
        let thread = std::thread::spawn(move || {
            let Some(destination) = destination else {
                return;
            };
            let mut buffer = Vec::new();
            let mut offset = 0;
            let mut ordinal = 0u64;
            loop {
                if let Ok(mut source) = std::fs::File::open(&path) {
                    let length = source
                        .metadata()
                        .map_or(MAX_SOURCE + 1, |metadata| metadata.len());
                    if length < offset || length > MAX_SOURCE {
                        let input = unavailable(
                            &runtime,
                            &identity,
                            ordinal + 1,
                            "source observation file truncated or exceeds 16 MiB",
                        );
                        let _ = append(&destination, &input);
                        return;
                    }
                    if source.seek(SeekFrom::Start(offset)).is_err() {
                        let input = unavailable(
                            &runtime,
                            &identity,
                            ordinal + 1,
                            "cannot seek source observation stream",
                        );
                        let _ = append(&destination, &input);
                        return;
                    }
                    let mut bytes = Vec::new();
                    if source
                        .take((MAX_EVENT + 1 - buffer.len()) as u64)
                        .read_to_end(&mut bytes)
                        .is_err()
                    {
                        let input = unavailable(
                            &runtime,
                            &identity,
                            ordinal + 1,
                            "cannot read source observation stream",
                        );
                        let _ = append(&destination, &input);
                        return;
                    }
                    offset += bytes.len() as u64;
                    buffer.extend_from_slice(&bytes);
                    if buffer.len() > MAX_EVENT && !buffer[..MAX_EVENT].contains(&b'\n') {
                        let input = unavailable(
                            &runtime,
                            &identity,
                            ordinal + 1,
                            "source observation exceeds 4 MiB",
                        );
                        let _ = append(&destination, &input);
                        return;
                    }
                    while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
                        let line = buffer.drain(..=end).collect::<Vec<_>>();
                        ordinal += 1;
                        let raw = serde_json::from_slice::<Value>(&line);
                        let input = convert(raw.as_ref().ok(), &runtime, &identity, ordinal);
                        if let Some(input) = input {
                            let _ = append(&destination, &input);
                        }
                    }
                }
                if stop.load(Ordering::Acquire) {
                    if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > offset) {
                        continue;
                    }
                    if !buffer.is_empty() {
                        let input = unavailable(
                            &runtime,
                            &identity,
                            ordinal + 1,
                            "truncated engine observation",
                        );
                        let _ = append(&destination, &input);
                    }
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        });
        Self {
            done,
            thread: Some(thread),
        }
    }
    pub(crate) fn finish(mut self) {
        self.done.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
    }
}

fn append(path: &Path, input: &InputEvent) -> std::io::Result<()> {
    input.validate().map_err(std::io::Error::other)?;
    let mut bytes = serde_json::to_vec(input).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    // Serialize appenders across processes too; each complete producer document
    // occupies a single transaction in the separate observation sidecar.
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    out.lock()?;
    out.write_all(&bytes)
}

fn identity(runtime: &Runtime, target: &TargetIdentity, ordinal: u64) -> InputEvent {
    InputEvent {
        version: VERSION,
        event_id: format!("{}:{}:{ordinal}", runtime.run_id, target.key()),
        run_id: runtime.run_id.clone(),
        context_sha256: runtime.context_sha256.clone(),
        input_revision: runtime.input_revision.clone(),
        target: Target {
            package_id: runtime
                .packages
                .get(&target.manifest)
                .cloned()
                .unwrap_or_default(),
            name: target.name.clone(),
            kind: runtime
                .target_kinds
                .get(&target.manifest)
                .and_then(|targets| targets.get(&target.name))
                .cloned()
                .unwrap_or_default(),
        },
        phase: Phase::Full,
        function: None,
        obligation: None,
        observation: Observation::Unavailable("unclassified source observation".into()),
    }
}
fn unavailable(
    runtime: &Runtime,
    target: &TargetIdentity,
    ordinal: u64,
    message: &str,
) -> InputEvent {
    let mut input = identity(runtime, target, ordinal);
    input.observation = Observation::Unavailable(message.into());
    input
}
fn convert(
    raw: Option<&Value>,
    runtime: &Runtime,
    target: &TargetIdentity,
    ordinal: u64,
) -> Option<InputEvent> {
    let Some(raw) = raw else {
        return Some(unavailable(
            runtime,
            target,
            ordinal,
            "invalid source observation JSON",
        ));
    };
    if raw["version"] != 1 {
        return Some(unavailable(
            runtime,
            target,
            ordinal,
            "unsupported source observation version",
        ));
    }
    let mut input = identity(runtime, target, ordinal);
    input.phase = match raw["phase"].as_str() {
        Some("focus") => Phase::Focus,
        Some("full") => Phase::Full,
        _ => {
            return Some(unavailable(
                runtime,
                target,
                ordinal,
                "invalid source observation phase",
            ));
        }
    };
    input.function = raw["payload"]["function"].as_str().map(str::to_owned);
    input.observation = match raw["event"].as_str()? {
        "function-started" => Observation::FunctionStarted,
        "obligation-progress" => {
            input.obligation = raw["payload"]["obligation"].as_str().map(str::to_owned);
            Observation::ObligationProgress(sail_api::proof::ObligationProgress {
                description: raw["payload"]["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                completed: raw["payload"]["completed"].as_bool().unwrap_or(false),
            })
        }
        "function-observed" => Observation::FunctionObserved(sail_api::output::FunctionReport {
            function: input.function.clone().unwrap_or_default(),
            module: raw["payload"]["module"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            fn_mode: raw["payload"]["mode"].as_str().map(str::to_owned),
            verdict: sail_api::output::VerdictName::Provisional,
            tier: sail_api::output::TierName::Provisional,
            unstable: false,
            verus_success: raw["payload"]["success"].as_bool(),
            provenance: sail_api::output::ProvenanceReport {
                configuration: None,
                mode: "default".into(),
                source: sail_api::output::SourceSummaryName::Unrecorded,
                checks_solved: 0,
                checks_served: 0,
                checks_unrecorded: 0,
            },
            timing: sail_api::output::Timing {
                smt_micros: raw["payload"]["smt_micros"].as_u64(),
                rlimit: raw["payload"]["rlimit"].as_u64(),
            },
            fragility: None,
            obligations: None,
        }),
        "phase-started" => Observation::PhaseStarted,
        "phase-finished" => Observation::PhaseFinished,
        "compiler-sealed" => Observation::CompilerSealed,
        "focus-unavailable" => Observation::Unavailable(format!(
            "exact function has {} matches in this target",
            raw["payload"]["matches"]
        )),
        "diagnostic" => Observation::Diagnostic(diagnostic(&raw["payload"])),
        _ => return None,
    };
    Some(input)
}

/// Checks only for a terminal full-phase observation. The pinned producer can
/// emit it after a caught compiler panic, so it cannot establish compiler
/// completion, current inputs or a successful full proof by itself.
pub(crate) fn sealed(path: &Path) -> bool {
    terminal(path).is_some()
}

#[derive(serde::Deserialize)]
struct Terminal {
    version: u32,
    event: String,
    phase: String,
    payload: Termination,
}

#[derive(serde::Deserialize)]
struct Termination {
    normal_termination: Option<bool>,
    success: Option<bool>,
}

fn terminal(path: &Path) -> Option<Terminal> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_SOURCE + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_SOURCE {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    if !text.ends_with('\n') {
        return None;
    }
    let events: Vec<Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()
        .ok()?;
    if !events.iter().all(|event| event["version"] == 1) {
        return None;
    }
    // Decode the original terminal line again through typed fields, so duplicate
    // normal-termination/success fields cannot be silently overwritten by Value.
    let last: Terminal = serde_json::from_str(text.lines().last()?).ok()?;
    (last.version == 1 && last.event == "compiler-sealed" && last.phase == "full").then_some(last)
}

/// A terminal observation alone is not a completed verifier invocation. The
/// pinned driver prints its structured result before normal exit 0 or 1;
/// a panic (101), signal, missing document or partial document needs fallback.
/// This validates completion, not successful proof. The source must explicitly
/// distinguish normal compiler errors from caught internal panics. Older source
/// events without that distinction need conservative stock fallback.
pub(crate) fn completed(path: &Path, stdout: &[u8], exit: Option<i32>) -> bool {
    if !matches!(exit, Some(0 | 1)) || stdout.len() > MAX_RESULT {
        return false;
    }
    let Some(terminal) = terminal(path) else {
        return false;
    };
    if terminal.payload.normal_termination != Some(true) || terminal.payload.success.is_none() {
        return false;
    }
    let (documents, remainder) = crate::attribution::json_documents(stdout);
    let [document] = documents.as_slice() else {
        return false;
    };
    if !remainder.iter().all(u8::is_ascii_whitespace)
        || !document["func-details"].is_object()
        || !["version", "profile", "toolchain", "commit"]
            .iter()
            .all(|key| {
                document["verus"][key]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
            })
        || !["os", "arch"].iter().all(|key| {
            document["verus"]["platform"][key]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        })
    {
        return false;
    }
    let results = &document["verification-results"];
    let Some(vir_error) = results["encountered-vir-error"].as_bool() else {
        return false;
    };
    // Normal compiler errors and ordinary failed proofs retain their exact
    // document. The terminal marker, not diagnostic text, excludes internal
    // panic termination that older producers folded into this same shape.
    if results["encountered-error"].as_bool().is_none()
        || results
            .get("success")
            .is_some_and(|value| !value.is_boolean())
    {
        return false;
    }
    // The pinned JSON producer deliberately omits counts and scope after VIR
    // errors. Such a document is a legitimate compiler failure, not a crash.
    vir_error
        || (results["verified"].as_u64().is_some()
            && results["errors"].as_u64().is_some()
            && results["is-verifying-entire-crate"].as_bool().is_some()
            && (results["is-verifying-entire-crate"] != true || results["success"].is_boolean()))
}

/// Converts exact source message text while preserving locations when supplied.
pub(crate) fn diagnostic(raw: &Value) -> sail_api::output::Diagnostic {
    let message = &raw["message"];
    sail_api::output::Diagnostic {
        level: raw["level"].as_str().unwrap_or("Note").to_lowercase(),
        code: None,
        message: message["note"].as_str().unwrap_or_default().into(),
        spans: message["spans"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|span| {
                let parsed = sail_api::output::SourceSpan::parse(span["as_string"].as_str()?);
                Some(sail_api::output::DiagnosticSpan {
                    file: parsed.file?,
                    line: parsed.line?,
                    column: parsed.column?,
                    primary: true,
                })
            })
            .collect(),
        rendered: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_or_malformed_stream_cannot_seal() -> Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("sail-engine-seal-{}.jsonl", std::process::id()));
        let sealed_line =
            r#"{"version":1,"event":"compiler-sealed","phase":"full","payload":{"success":false}}"#;
        std::fs::write(&path, format!("{sealed_line}\n"))?;
        assert!(sealed(&path));
        std::fs::write(&path, sealed_line)?;
        assert!(!sealed(&path));
        std::fs::write(&path, format!("broken\n{sealed_line}\n"))?;
        assert!(!sealed(&path));
        std::fs::write(&path, format!("{sealed_line}\n{{\n"))?;
        assert!(!sealed(&path));
        std::fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn terminal_seal_requires_complete_full_document_and_normal_exit()
    -> Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("sail-engine-result-{}.jsonl", std::process::id()));
        std::fs::write(
            &path,
            "{\"version\":1,\"event\":\"compiler-sealed\",\"phase\":\"full\",\"payload\":{\"success\":false}}\n",
        )?;
        let proof = include_bytes!("../tests/fixtures/engine-terminal/failed-proof.json");
        let late = include_bytes!("../tests/fixtures/engine-terminal/late-exec.json");
        assert!(!completed(&path, proof, Some(1)));
        for payload in [
            r#"{"success":false,"normal_termination":false}"#,
            r#"{"success":false,"normal_termination":"true"}"#,
            r#"{"success":false,"normal_termination":null}"#,
            r#"{"success":false,"normal_termination":true,"normal_termination":false}"#,
            r#"{"success":false,"normal_termination":false,"normal_termination":true}"#,
            r#"{"normal_termination":true}"#,
        ] {
            std::fs::write(
                &path,
                format!(
                    "{{\"version\":1,\"event\":\"compiler-sealed\",\"phase\":\"full\",\"payload\":{payload}}}\n"
                ),
            )?;
            assert!(!completed(&path, proof, Some(1)));
        }
        std::fs::write(
            &path,
            include_bytes!("../tests/fixtures/engine-terminal/terminal-failure.jsonl"),
        )?;
        for document in [proof.as_slice(), late.as_slice()] {
            assert!(completed(&path, document, Some(1)));
            assert!(!completed(&path, document, Some(101)));
            assert!(!completed(&path, document, None));
            assert!(!completed(&path, &document[..document.len() - 2], Some(1)));
            let mut trailing = document.to_vec();
            trailing.extend_from_slice(b"{\"verification-results\":");
            assert!(!completed(&path, &trailing, Some(1)));
        }
        for document in [b"".as_slice(), b"{broken", b"{}", b"null", b"{\"func-details\":{},\"verus\":{},\"verification-results\":{\"encountered-error\":true}}"] {
            assert!(!completed(&path, document, Some(1)));
        }
        let mut invalid: Value = serde_json::from_slice(proof)?;
        invalid["verification-results"]["encountered-error"] = serde_json::json!(false);
        assert!(completed(&path, &serde_json::to_vec(&invalid)?, Some(1)));
        invalid["verification-results"]["errors"] = serde_json::json!(-1);
        assert!(!completed(&path, &serde_json::to_vec(&invalid)?, Some(1)));
        invalid["verification-results"] = serde_json::json!({"encountered-error":true,"encountered-vir-error":true,"success":false});
        assert!(completed(&path, &serde_json::to_vec(&invalid)?, Some(1)));
        let mut repeated = proof.to_vec();
        repeated.extend_from_slice(late);
        assert!(!completed(&path, &repeated, Some(1)));
        assert!(!completed(&path, &vec![b' '; MAX_RESULT + 1], Some(1)));
        std::fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn source_diagnostic_retains_exact_message_and_location() {
        let raw = serde_json::json!({"message":{"note":"postcondition not satisfied","spans":[{"as_string":"crate:part.rs:5:30: 5:35 (#0)"}]},"level":"Error"});
        let parsed = diagnostic(&raw);
        assert_eq!(parsed.message, "postcondition not satisfied");
        assert_eq!(parsed.level, "error");
        assert_eq!(parsed.spans[0].file, "crate:part.rs");
        assert_eq!(parsed.spans[0].line, 5);
        assert_eq!(parsed.spans[0].column, 30);
    }
}
