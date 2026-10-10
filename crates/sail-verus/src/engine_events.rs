//! Source-engine observation relay. It never participates in solver output.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sail_api::proof::{InputEvent, Observation, Phase, Target, VERSION};
use serde_json::Value;

use crate::managed::{Runtime, TargetIdentity};

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
                if let Ok(bytes) = std::fs::read(&path) {
                    if bytes.len() >= offset {
                        buffer.extend_from_slice(&bytes[offset..]);
                        offset = bytes.len();
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
            kind: vec![if target.crate_type == "rlib" {
                "lib".into()
            } else {
                target.crate_type.clone()
            }],
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

/// Checks that the source completed the late compiler pass. It does not prove
/// current inputs or a successful full proof; those belong to the managed runner.
pub(crate) fn sealed(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| {
        if !text.ends_with('\n') {
            return false;
        }
        let events: Option<Vec<Value>> = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).ok())
            .collect();
        events.is_some_and(|events| {
            events.iter().all(|event| event["version"] == 1)
                && events.last().is_some_and(|event| {
                    event["event"] == "compiler-sealed" && event["phase"] == "full"
                })
        })
    })
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
