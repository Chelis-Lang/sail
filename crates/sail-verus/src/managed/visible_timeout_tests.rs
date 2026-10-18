//! An incomplete real frame must time out while Cargo's other pipe stays busy.

use super::super::visible_output::Receiver;
use super::*;
use std::os::unix::net::UnixDatagram;
use std::sync::mpsc;
use std::time::Duration;

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Producer(std::process::Child);
impl Producer {
    fn stop(&mut self) -> std::io::Result<()> {
        if self.0.try_wait()?.is_none() {
            self.0.kill()?;
        }
        self.0.wait()?;
        Ok(())
    }
}
impl Drop for Producer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct SocketName(PathBuf);
impl Drop for SocketName {
    fn drop(&mut self) {
        // The receiver may already have removed this exact owned client name.
        let _ = std::fs::remove_file(&self.0);
    }
}

// Two small valid Cargo-like JSON records per millisecond, for at most 40s.
// The explicit record-count cap keeps even the complete stream below 2 MiB.
const PRODUCER: &str = r#"
import os,pathlib,sys,time
ready,progress=map(pathlib.Path,sys.argv[1:])
line=b'{"reason":"timeout"}\n'
end=time.monotonic()+40
count=0
while time.monotonic()<end and count+2<=90000:
    data=line*2
    if os.write(1,data)!=len(data):raise SystemExit(8)
    count+=2
    if count==2:ready.write_bytes(b'ready')
    if count%100==0:
        temp=progress.with_suffix('.tmp')
        temp.write_text(str(count))
        os.replace(temp,progress)
    time.sleep(.001)
"#;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One actual peer/pipe lifecycle keeps timeout and unconditional cleanup assertions together."
)]
fn incomplete_emit_times_out_even_while_cargo_json_remains_busy()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = Scratch(std::env::temp_dir().join(format!(
        "sail-visible-timeout-{}-{unique}",
        std::process::id()
    )));
    std::fs::create_dir(&root.0)?;
    let ready = root.0.join("ready");
    let progress = root.0.join("progress");
    let receiver = Receiver::bind()?;
    let endpoint = receiver.path().to_owned();
    let directory = endpoint
        .parent()
        .ok_or("missing output directory")?
        .to_owned();
    let mut child = Producer(
        Command::new("python3")
            .args(["-c", PRODUCER])
            .args([&ready, &progress])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let (ack_tx, ack_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let sender = std::thread::spawn(move || -> std::io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            if Instant::now() >= deadline {
                return Err(std::io::Error::other("Cargo producer did not start"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let name = SocketName(
            endpoint
                .parent()
                .ok_or_else(|| std::io::Error::other("missing output directory"))?
                .join("stalled-client"),
        );
        let socket = UnixDatagram::bind(&name.0)?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;
        socket.connect(&endpoint)?;
        // Fault injection deliberately bypasses the production complete sender:
        // tag 1 is stdout WITHOUT the final-chunk bit. Receive its real ACK,
        // then keep this endpoint alive while withholding every later chunk.
        let frame = b"\x01visible-prefix";
        if socket.send(frame)? != frame.len() {
            return Err(std::io::Error::other("partial injected frame"));
        }
        let mut ack = [0; 2];
        if socket.recv(&mut ack)? != 1 || ack[0] != 1 {
            return Err(std::io::Error::other("first chunk was not acknowledged"));
        }
        ack_tx
            .send(Instant::now())
            .map_err(|_| std::io::Error::other("ACK observer disappeared"))?;
        release_rx
            .recv_timeout(Duration::from_secs(45))
            .map_err(|_| std::io::Error::other("stalled sender was not released"))?;
        Ok(())
    });
    let mut visible = Vec::new();
    let mut diagnostics = Vec::new();
    let observed = relay(
        &mut child.0,
        &mut Some(&mut visible),
        &mut diagnostics,
        &[],
        Some(receiver),
    );
    let finished = Instant::now();
    // Perform all owned cleanup before interpreting any result or assertion.
    let was_running = child.0.try_wait();
    let stopped = child.stop();
    let released = release_tx.send(());
    let sent = sender.join();
    let acknowledged = ack_rx.try_recv();
    stopped?;
    released.map_err(|_| "stalled sender exited before cleanup")?;
    sent.map_err(|_| "stalled sender panicked")??;
    let acknowledged = acknowledged.map_err(|_| "initial frame lacked a real ACK")?;
    assert!(was_running?.is_none(), "Cargo reached its own 40s limit");
    let elapsed = finished.duration_since(acknowledged);
    assert!(
        elapsed >= Duration::from_secs(29),
        "incomplete emit refused early"
    );
    assert!(
        elapsed < Duration::from_secs(36),
        "busy Cargo bypassed the 30s deadline: {elapsed:?}"
    );
    let failure = observed
        .err()
        .ok_or("incomplete write unexpectedly succeeded")?;
    let text = failure.to_string();
    assert!(
        text.contains("SAIL3106") && text.contains("before its final chunk"),
        "{text}"
    );
    assert_eq!(visible, b"visible-prefix");
    assert!(diagnostics.is_empty(), "Cargo JSON reached stderr");
    let count: usize = std::fs::read_to_string(progress)?.parse()?;
    assert!(count >= 100, "Cargo did not keep emitting JSON");
    assert!(count * b"{\"reason\":\"timeout\"}\n".len() <= 2 * 1024 * 1024);
    assert!(!directory.exists(), "owned output endpoint leaked");
    Ok(())
}
