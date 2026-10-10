//! Durable event ordering, replay and malformed-source boundaries.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::Journals;
use sail_api::proof::{Cursor, InputEvent, Observation, Phase, Target};

type TestResult = Result<(), Box<dyn std::error::Error>>;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Root(PathBuf);
impl Root {
    fn new() -> std::io::Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "sail-proof-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        Ok(Self(root))
    }
    fn append(&self, bytes: &[u8]) -> std::io::Result<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.0.join("proof-source.jsonl"))?
            .write_all(bytes)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn event(id: &str, kind: &str) -> InputEvent {
    InputEvent {
        version: 1,
        event_id: id.into(),
        run_id: "run1".into(),
        context_sha256: "context1".into(),
        input_revision: "revision1".into(),
        target: Target {
            package_id: "demo@1.0.0".into(),
            name: "demo".into(),
            kind: vec![kind.into()],
        },
        phase: Phase::Full,
        function: None,
        obligation: None,
        observation: Observation::PhaseStarted,
    }
}
fn line(event: &InputEvent) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(event)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[test]
fn duplicate_delivery_restart_and_same_name_targets_preserve_one_ordered_stream() -> TestResult {
    let root = Root::new()?;
    let journals = Journals::default();
    root.append(&line(&event("lib1", "lib"))?)?;
    root.append(&line(&event("lib1", "lib"))?)?;
    root.append(&line(&event("bin1", "bin"))?)?;
    let first = journals.page(&root.0, "job1", &Cursor::default(), 1, false)?;
    assert_eq!(first.events.len(), 1);
    assert_eq!(first.next.after, 1);
    let resumed = Journals::default().page(&root.0, "job1", &first.next, 10, true)?;
    assert_eq!(resumed.events.len(), 1);
    assert_eq!(resumed.events[0].sequence, 2);
    assert_ne!(resumed.events[0].input.target, first.events[0].input.target);
    assert!(resumed.gap.is_none());
    assert!(resumed.terminal);
    assert!(
        journals
            .page(&root.0, "different-job", &first.next, 10, true)
            .is_err()
    );
    Ok(())
}

#[test]
fn incomplete_frames_wait_and_malformed_or_conflicting_events_expose_a_gap() -> TestResult {
    let root = Root::new()?;
    let journals = Journals::default();
    let bytes = line(&event("id1", "lib"))?;
    let middle = bytes.len() / 2;
    root.append(&bytes[..middle])?;
    let partial = journals.page(&root.0, "job1", &Cursor::default(), 10, false)?;
    assert_eq!(partial.events.len(), 0);
    assert!(partial.gap.is_none());
    root.append(&bytes[middle..])?;
    root.append(b"malformed\n")?;
    let mut conflict = event("id1", "lib");
    conflict.input_revision = "different revision".into();
    root.append(&line(&conflict)?)?;
    let complete = journals.page(&root.0, "job1", &partial.next, 10, true)?;
    assert_eq!(complete.events.len(), 1);
    assert!(complete.gap.is_some());
    assert_eq!(complete.events[0].input.input_revision, "revision1");
    let restarted = Journals::default().page(&root.0, "job1", &complete.next, 10, true)?;
    assert_eq!(restarted.events.len(), 0);
    assert!(restarted.gap.is_some());
    Ok(())
}

#[test]
fn terminal_truncated_event_never_becomes_observed_evidence() -> TestResult {
    let root = Root::new()?;
    let journals = Journals::default();
    let bytes = line(&event("id1", "lib"))?;
    root.append(&bytes[..bytes.len() / 2])?;
    let final_page = journals.page(&root.0, "job1", &Cursor::default(), 10, true)?;
    assert_eq!(final_page.events.len(), 0);
    assert!(final_page.gap.is_some());
    assert!(final_page.terminal);
    Ok(())
}
