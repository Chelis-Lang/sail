//! Concurrent production senders, the real relay, and a real Cargo-like pipe.

use super::super::visible_output::{Receiver, Stream, send_test};
use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Producer(std::process::Child);
impl Drop for Producer {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[derive(Clone)]
struct Sink {
    bytes: Arc<Mutex<Vec<u8>>>,
    triggered: Arc<AtomicBool>,
    trigger: PathBuf,
    committed: PathBuf,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes
            .lock()
            .map_err(|_| std::io::Error::other("output poisoned"))?
            .extend_from_slice(bytes);
        // Commit raw Cargo stderr while a first visible chunk is being written.
        // Draining it before the next chunk would split this logical emit.
        if bytes.starts_with(b"VIS-") && !self.triggered.swap(true, Ordering::AcqRel) {
            std::fs::write(&self.trigger, b"first-visible-chunk")?;
            let end = Instant::now() + std::time::Duration::from_secs(5);
            while !self.committed.exists() {
                if Instant::now() >= end {
                    return Err(std::io::Error::other("Cargo diagnostic was not committed"));
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

const PRODUCER: &str = r#"
import os,pathlib,sys,time
ready,trigger,committed,done=map(pathlib.Path,sys.argv[1:])
def await_file(path):
    end=time.monotonic()+15
    while not path.exists():
        if time.monotonic()>=end:raise SystemExit(9)
        time.sleep(.001)
os.write(2,b'before\n')
sys.stdout.buffer.write(b'{"reason":"compiler-artifact","padding":"'+b'x'*65536+b'"}\n')
sys.stdout.buffer.flush()
ready.write_bytes(b'ready')
await_file(trigger)
os.write(2,b'cargo-mid\n')
os.write(1,b'cargo-stdout-mid\n')
committed.write_bytes(b'committed')
await_file(done)
os.write(2,b'after\n')
"#;

fn payload(name: u8, size: usize) -> Vec<u8> {
    let mut bytes = b"VIS-".to_vec();
    bytes.extend_from_slice(&[name, 0, 255, b'\n']);
    bytes.extend(std::iter::repeat_n(name, size));
    bytes.extend_from_slice(&[0, 255, b'-', name, b'\n']);
    bytes
}

fn occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep the real producer and concurrent writer lifecycle together with exact-byte assertions."
)]
fn exercise(combined: bool) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = Scratch(std::env::temp_dir().join(format!(
        "sail-visible-concurrent-{}-{unique}",
        std::process::id()
    )));
    std::fs::create_dir(&root.0)?;
    let ready = root.0.join("ready");
    let trigger = root.0.join("trigger");
    let committed = root.0.join("committed");
    let done = root.0.join("done");
    let receiver = Receiver::bind()?;
    let endpoint = receiver.path().to_owned();
    let directory = endpoint
        .parent()
        .ok_or("missing endpoint directory")?
        .to_owned();
    let mut child = Producer(
        Command::new("python3")
            .args(["-c", PRODUCER])
            .args([&ready, &trigger, &committed, &done])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let first = payload(b'A', 6100);
    let second = payload(b'B', 7300);
    let pending = Arc::new(AtomicUsize::new(2));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let mut senders = Vec::new();
    for (kind, bytes) in [
        (Stream::Stdout, first.clone()),
        (Stream::Stderr, second.clone()),
    ] {
        let endpoint = endpoint.clone();
        let ready = ready.clone();
        let done = done.clone();
        let pending = Arc::clone(&pending);
        let barrier = Arc::clone(&barrier);
        senders.push(std::thread::spawn(move || -> std::io::Result<()> {
            let end = Instant::now() + std::time::Duration::from_secs(15);
            while !ready.exists() {
                if Instant::now() >= end {
                    return Err(std::io::Error::other("Cargo producer did not start"));
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            barrier.wait();
            let result = send_test(&endpoint, kind, &bytes);
            if pending.fetch_sub(1, Ordering::AcqRel) == 1 {
                std::fs::write(done, b"both-senders-completed")?;
            }
            result
        }));
    }
    let output = Arc::new(Mutex::new(Vec::new()));
    let errors = if combined {
        Arc::clone(&output)
    } else {
        Arc::new(Mutex::new(Vec::new()))
    };
    let triggered = Arc::new(AtomicBool::new(false));
    let mut out = Sink {
        bytes: Arc::clone(&output),
        triggered: Arc::clone(&triggered),
        trigger: trigger.clone(),
        committed: committed.clone(),
    };
    let mut err = Sink {
        bytes: Arc::clone(&errors),
        triggered,
        trigger,
        committed,
    };
    let observed = relay(
        &mut child.0,
        &mut Some(&mut out),
        &mut err,
        &[],
        Some(receiver),
    );
    for sender in senders {
        sender.join().map_err(|_| "visible sender panicked")??;
    }
    assert!(child.0.wait()?.success(), "Cargo-like producer failed");
    let (captured_out, captured_err) = observed?;
    assert!(!directory.exists(), "owned endpoint leaked");
    let mut expected_json = br#"{"reason":"compiler-artifact","padding":""#.to_vec();
    expected_json.extend(std::iter::repeat_n(b'x', 65536));
    expected_json.extend_from_slice(b"\"}\n");
    expected_json.extend_from_slice(b"cargo-stdout-mid\n");
    assert_eq!(captured_out, expected_json, "Cargo JSON must remain exact");
    let actual_out = output.lock().map_err(|_| "stdout poisoned")?.clone();
    let actual_err = errors.lock().map_err(|_| "stderr poisoned")?.clone();
    assert_eq!(
        occurrences(&captured_err, &second),
        1,
        "raw stderr split an emit"
    );
    assert_eq!(occurrences(&captured_err, b"cargo-mid\n"), 1);
    assert!(captured_err.starts_with(b"before\n") && captured_err.ends_with(b"after\n"));
    if combined {
        assert_eq!(
            occurrences(&actual_out, &first),
            1,
            "concurrent writer split stdout"
        );
        assert_eq!(
            occurrences(&actual_out, &second),
            1,
            "concurrent writer split stderr"
        );
        assert_eq!(
            actual_out.len(),
            first.len() + second.len() + b"before\ncargo-mid\nafter\ncargo-stdout-mid\n".len()
        );
        assert!(actual_out.starts_with(b"before\n") && actual_out.ends_with(b"after\n"));
    } else {
        assert_eq!(occurrences(&actual_out, &first), 1);
        assert_eq!(occurrences(&actual_out, b"cargo-stdout-mid\n"), 1);
        assert_eq!(actual_out.len(), first.len() + b"cargo-stdout-mid\n".len());
        assert_eq!(actual_err, captured_err);
        assert_eq!(
            actual_err.len(),
            second.len() + b"before\ncargo-mid\nafter\n".len()
        );
    }
    Ok(())
}

#[test]
fn concurrent_multichunk_emits_and_raw_stderr_stay_contiguous_shared()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    exercise(true)
}

#[test]
fn concurrent_multichunk_emits_and_raw_stderr_preserve_split_bytes()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    exercise(false)
}

#[test]
fn refused_emit_releases_lock_for_another_production_sender()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut receiver = Receiver::bind()?;
    let endpoint = receiver.path().to_owned();
    let directory = endpoint
        .parent()
        .ok_or("missing endpoint directory")?
        .to_owned();
    let (seen_tx, seen_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first_frame = AtomicBool::new(true);
    receiver.start(move |frame| {
        let Ok(frame) = frame else {
            return false;
        };
        if first_frame.swap(false, Ordering::AcqRel) {
            let _ = seen_tx.send(());
            if release_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .is_err()
            {
                return false;
            }
            drop(frame); // The same NACK sent by a broken relay/public sink.
            true
        } else {
            frame.acknowledge().is_ok()
        }
    })?;
    let first_endpoint = endpoint.clone();
    let first = std::thread::spawn(move || {
        send_test(&first_endpoint, Stream::Stdout, &payload(b'A', 5000))
    });
    seen_rx.recv_timeout(std::time::Duration::from_secs(5))?;
    let (done_tx, done_rx) = mpsc::channel();
    let second = std::thread::spawn(move || {
        let result = send_test(&endpoint, Stream::Stderr, &payload(b'B', 5000));
        let _ = done_tx.send(result);
    });
    // A sender cannot finish while the current emit has not been acknowledged.
    let held = matches!(
        done_rx.recv_timeout(std::time::Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    release_tx.send(())?;
    let first_result = first.join().map_err(|_| "first sender panicked")?;
    let second_result = done_rx.recv_timeout(std::time::Duration::from_secs(5))?;
    second.join().map_err(|_| "second sender panicked")?;
    receiver.finish()?;
    assert!(
        held,
        "second emit finished while the first was awaiting delivery"
    );
    assert!(first_result.is_err(), "NACK must fail the first emit");
    second_result?;
    assert!(
        !directory.exists(),
        "owned endpoint leaked after refused emit"
    );
    Ok(())
}
