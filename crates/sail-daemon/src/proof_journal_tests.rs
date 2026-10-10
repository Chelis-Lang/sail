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
        std::fs::write(root.join("events.jsonl"),serde_json::to_vec(&serde_json::json!({"kind":"context-ready","run_id":"run1","context_sha256":"context1","input_revision":"revision1","targets":[{"package_id":"demo@1.0.0","name":"demo","kind":["lib"]},{"package_id":"demo@1.0.0","name":"demo","kind":["bin"]}]})).map_err(std::io::Error::other)?)?;
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

#[test]
fn terminal_pages_drain_all_events_and_spool_replacement_is_explicitly_incomplete() -> TestResult {
    let root = Root::new()?;
    let journals = Journals::default();
    for ordinal in 0..3 {
        root.append(&line(&event(&format!("event-{ordinal}"), "lib"))?)?;
    }
    let first = journals.page(&root.0, "job1", &Cursor::default(), 1, true)?;
    assert!(!first.terminal);
    let second = journals.page(&root.0, "job1", &first.next, 1, true)?;
    assert!(!second.terminal);
    let last = journals.page(&root.0, "job1", &second.next, 1, true)?;
    assert!(last.terminal);
    assert_eq!(last.next.after, 3);
    std::fs::rename(root.0.join("proof-source.jsonl"), root.0.join("old-source"))?;
    root.append(&line(&event("replacement", "lib"))?)?;
    let replaced = journals.page(&root.0, "job1", &last.next, 10, true)?;
    assert_eq!(replaced.events.len(), 0);
    assert!(replaced.gap.is_some());
    let restarted = Journals::default().page(&root.0, "job1", &last.next, 10, true)?;
    assert_eq!(restarted.events.len(), 0);
    assert!(restarted.gap.is_some());
    Ok(())
}

#[test]
fn retention_budget_reports_gap_without_reassigning_or_inventing_event_ids() -> TestResult {
    let root = Root::new()?;
    let journals = Journals::default();
    let mut bytes = Vec::new();
    for ordinal in 0..super::MAX_EVENTS + 2 {
        bytes.extend(line(&event(&format!("event-{ordinal}"), "lib"))?);
    }
    root.append(&bytes)?;
    let mut cursor = Cursor::default();
    let mut count = 0;
    let mut gap = None;
    for _ in 0..200 {
        let page = journals.page(&root.0, "job1", &cursor, 64, true)?;
        count += page.events.len();
        gap = gap.or(page.gap);
        cursor = page.next;
        if page.terminal {
            break;
        }
    }
    assert_eq!(count, super::MAX_EVENTS);
    assert_eq!(cursor.after, super::MAX_EVENTS as u64);
    assert!(gap.is_some());
    let restarted = Journals::default().page(&root.0, "job1", &cursor, 64, true)?;
    assert_eq!(restarted.events.len(), 0);
    assert!(restarted.terminal);
    assert!(restarted.gap.is_some());
    Ok(())
}

#[test]
fn producer_events_must_match_the_worker_context_and_exact_target_inventory() -> TestResult {
    let root = Root::new()?;
    let journals = Journals::default();
    let valid = event("valid", "lib");
    let mut other = valid.clone();
    other.event_id = "wrong-run".into();
    other.run_id = "unrelated-run".into();
    root.append(&line(&other)?)?;
    other = valid.clone();
    other.event_id = "wrong-target".into();
    other.target.name = "other".into();
    root.append(&line(&other)?)?;
    root.append(&line(&valid)?)?;
    let page = journals.page(&root.0, "job1", &Cursor::default(), 10, true)?;
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].input.event_id, "valid");
    assert!(page.gap.is_some());
    let restarted = Journals::default().page(&root.0, "job1", &Cursor::default(), 10, true)?;
    assert_eq!(restarted.events.len(), 1);
    assert_eq!(restarted.events[0].sequence, 1);
    assert!(restarted.gap.is_some());
    Ok(())
}
