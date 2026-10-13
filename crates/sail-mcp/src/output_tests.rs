//! Output ownership, cancellation and framing controls using the real encoder.

use super::*;
use std::sync::mpsc;
use std::time::Duration;

#[derive(Clone)]
struct Shared(Arc<Mutex<Vec<u8>>>);
impl Write for Shared {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("fixture poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn output() -> (Out, Shared) {
    let bytes = Shared(Arc::default());
    (Arc::new(Mutex::new(Box::new(bytes.clone()))), bytes)
}
fn messages(bytes: &Shared) -> Vec<Value> {
    let bytes = bytes
        .0
        .lock()
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(bytes.last(), Some(&b'\n'));
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap_or_else(|error| unreachable!("{error}")))
        .collect()
}
struct Paused {
    value: Value,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}
impl serde::Serialize for Paused {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.entered.send(()).map_err(serde::ser::Error::custom)?;
        self.release
            .recv_timeout(Duration::from_secs(2))
            .map_err(serde::ser::Error::custom)?;
        self.value.serialize(serializer)
    }
}
fn paused(value: Value) -> (Paused, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    (
        Paused {
            value,
            entered,
            release: resume,
        },
        ready,
        release,
    )
}
fn large(id: u64) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "result":{"model":"λ\n\\\"".repeat(400_000)}})
}

#[test]
fn large_encoding_does_not_hold_output_and_old_lock_order_does() {
    for old_order in [false, true] {
        let (out, bytes) = output();
        let message = large(1);
        let (paused, ready, release) = paused(message.clone());
        let worker_out = Arc::clone(&out);
        let worker = crate::test_io::Thread::spawn(
            move || {
                if old_order {
                    // Exact old critical section: output lock precedes JSON encoding.
                    let mut writer = worker_out
                        .lock()
                        .unwrap_or_else(|error| unreachable!("{error}"));
                    let line = jsonrpc::line(&paused.value);
                    // Pause the Serialize operation while holding that same lock.
                    let encoded = serde_json::to_string(&paused).unwrap_or_default();
                    assert_eq!(encoded, line);
                    assert!(writeln!(writer, "{encoded}").is_ok());
                } else {
                    send_checked(&worker_out, &paused, || false);
                }
            },
            move || {
                let _ = release.send(());
            },
        );
        assert!(ready.recv_timeout(Duration::from_secs(2)).is_ok());
        assert_eq!(out.try_lock().is_ok(), !old_order);
        if !old_order {
            let context = Context::new(Config {
                sail: "sail".into(),
                store_dir: None,
                cwd: ".".into(),
            });
            let mut server = Server {
                ctx: Arc::new(context),
                out: Arc::clone(&out),
                inflight: Arc::default(),
                workers: vec![],
            };
            server.request(&json!(2), "ping", &Map::new());
            assert_eq!(messages(&bytes)[0]["id"], 2);
        }
        assert!(worker.finish());
        let frames = messages(&bytes);
        assert_eq!(frames.last(), Some(&message));
        assert_eq!(frames.len(), if old_order { 1 } else { 2 });
    }
}

#[test]
fn cancellation_during_encoding_remains_registered_and_suppresses_output() {
    let (out, bytes) = output();
    let cancel = Arc::new(Cancel::default());
    let inflight = Arc::new(Mutex::new(HashMap::from([(
        "1".to_owned(),
        Arc::clone(&cancel),
    )])));
    let cleanup = InflightCleanup {
        inflight: Arc::clone(&inflight),
        key: "1".to_owned(),
        cancel: Arc::clone(&cancel),
    };
    let (paused, ready, release) = paused(large(1));
    let worker_out = Arc::clone(&out);
    let worker_cancel = Arc::clone(&cancel);
    let worker = crate::test_io::Thread::spawn(
        move || {
            let _cleanup = cleanup;
            send_checked(&worker_out, &paused, || worker_cancel.is_cancelled());
        },
        move || {
            let _ = release.send(());
        },
    );
    assert!(ready.recv_timeout(Duration::from_secs(2)).is_ok());
    let context = Context::new(Config {
        sail: "sail".into(),
        store_dir: None,
        cwd: ".".into(),
    });
    let server = Server {
        ctx: Arc::new(context),
        out: Arc::clone(&out),
        inflight: Arc::clone(&inflight),
        workers: vec![],
    };
    server.notification(
        "notifications/cancelled",
        &Map::from_iter([("requestId".to_owned(), json!(1))]),
    );
    assert!(cancel.is_cancelled());
    send(
        &out,
        &jsonrpc::ok(&json!(2), &json!({"resultType":"complete"})),
    );
    assert!(worker.finish());
    assert_eq!(messages(&bytes).len(), 1);
    assert_eq!(messages(&bytes)[0]["id"], 2);
    assert!(inflight.lock().is_ok_and(|slots| slots.is_empty()));
}

#[test]
fn final_check_after_output_lock_wait_suppresses_cancelled_response() {
    let (out, bytes) = output();
    let held = out.lock().unwrap_or_else(|error| unreachable!("{error}"));
    let cancel = Arc::new(Cancel::default());
    let worker_cancel = Arc::clone(&cancel);
    let worker_out = Arc::clone(&out);
    let (paused, ready, release) = paused(large(1));
    std::thread::scope(|scope| {
        scope.spawn(move || send_checked(&worker_out, &paused, || worker_cancel.is_cancelled()));
        assert!(ready.recv_timeout(Duration::from_secs(2)).is_ok());
        assert!(release.send(()).is_ok());
        cancel.cancel();
        drop(held);
    });
    assert!(bytes.0.lock().is_ok_and(|bytes| bytes.is_empty()));
}

#[test]
fn cleanup_on_unwind_preserves_a_replacement_cancellation_slot() {
    let old = Arc::new(Cancel::default());
    let new = Arc::new(Cancel::default());
    let inflight = Arc::new(Mutex::new(HashMap::from([(
        "1".to_owned(),
        Arc::clone(&old),
    )])));
    let cleanup = InflightCleanup {
        inflight: Arc::clone(&inflight),
        key: "1".to_owned(),
        cancel: old,
    };
    assert!(
        inflight
            .lock()
            .is_ok_and(|mut slots| slots.insert("1".to_owned(), Arc::clone(&new)).is_some())
    );
    assert!(
        std::panic::catch_unwind(move || {
            let _cleanup = cleanup;
            assert!(std::thread::panicking(), "controlled worker unwind");
        })
        .is_err()
    );
    assert!(
        inflight
            .lock()
            .is_ok_and(|slots| slots.get("1").is_some_and(|slot| Arc::ptr_eq(slot, &new)))
    );
    drop(InflightCleanup {
        inflight: Arc::clone(&inflight),
        key: "1".to_owned(),
        cancel: new,
    });
    assert!(inflight.lock().is_ok_and(|slots| slots.is_empty()));
}

#[test]
fn concurrent_responses_are_complete_independent_newline_frames() {
    let (out, bytes) = output();
    let expected: Vec<_> = (0..4).map(large).collect();
    std::thread::scope(|scope| {
        for message in &expected {
            let out = Arc::clone(&out);
            scope.spawn(move || send(&out, message));
        }
    });
    let frames = messages(&bytes);
    assert_eq!(frames.len(), expected.len());
    for message in expected {
        assert_eq!(frames.iter().filter(|frame| **frame == message).count(), 1);
    }
}

#[test]
fn tool_results_carry_the_summary_and_the_complete_structured_payload() {
    for structured in [Value::Null, json!({}), large(1)] {
        for is_error in [false, true] {
            let result = tools::ToolResult {
                text: "summary λ\n".to_owned(),
                structured: structured.clone(),
                is_error,
            };
            let expected = json!({
                "resultType": "complete",
                "content": [
                    {"type": "text", "text": "summary λ\n"},
                    {"type": "text", "text": serde_json::to_string(&structured).unwrap_or_default()},
                ],
                "structuredContent": structured,
                "isError": is_error,
            });
            assert_eq!(result.into_json(), expected);
        }
    }
}
