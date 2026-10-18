//! Original-pipe completion, refusal and genuine Cargo diagnostic controls.

use super::super::visible_output::{Receiver, send_test};
use super::*;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

struct Scratch(PathBuf, std::cell::Cell<bool>);
impl Scratch {
    fn new() -> std::io::Result<Self> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("sail-visible-test-{}-{unique}", std::process::id()));
        std::fs::create_dir(&path)?;
        Ok(Self(path, std::cell::Cell::new(false)))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.1.get() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
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
struct Sink(Arc<Mutex<Vec<u8>>>);
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("sink poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
}

// The original pipe carries every byte, including large complete Cargo JSON.
// Only the private ACK allows the sender to close its FIFO, after which this
// producer emits its causally later stderr. There is no cross-pipe order claim
// about the earlier status line or independent writers.
const PRODUCER: &str = r#"
import os,pathlib,sys
fifo,ready=map(pathlib.Path,sys.argv[1:])
os.mkfifo(fifo,0o600)
os.write(2,b'before\n')
os.write(1,b'{"reason":"compiler-artifact","padding":"'+b'x'*65536+b'"}\n')
ready.write_bytes(b'ready')
with fifo.open('rb',buffering=0) as source:
    while True:
        data=source.read(65536)
        if not data:break
        sys.stdout.buffer.write(data)
        sys.stdout.buffer.flush()
os.write(2,b'after\n')
"#;

#[derive(Clone, Copy)]
enum Destination {
    Combined,
    Split,
    Hidden,
    Broken,
}

#[allow(
    clippy::too_many_lines,
    reason = "One owned original-pipe/sender/relay lifecycle retains assertions and all cleanup."
)]
fn exercise(destination: Destination) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let root = Scratch::new()?;
    let ready = root.0.join("ready");
    let fifo = root.0.join("stdout.fifo");
    let receiver = Receiver::bind()?;
    let endpoint = receiver.path().to_owned();
    let directory = endpoint
        .parent()
        .ok_or("missing endpoint parent")?
        .to_owned();
    let mut child = Producer(
        Command::new("python3")
            .args(["-c", PRODUCER])
            .args([&fifo, &ready])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let shown = [
        b"verification results:: ".as_slice(),
        &[0, 255],
        &vec![b'v'; 7500],
        b"\n",
    ]
    .concat();
    let sent_bytes = shown.clone();
    let sender = std::thread::spawn(move || -> std::io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            if Instant::now() >= deadline {
                return Err(std::io::Error::other(
                    "original-pipe producer did not start",
                ));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut pipe = std::fs::OpenOptions::new().write(true).open(fifo)?;
        send_test(&endpoint, &sent_bytes, &mut pipe)
    });
    let output = Arc::new(Mutex::new(Vec::new()));
    let errors = if matches!(destination, Destination::Combined) {
        Arc::clone(&output)
    } else {
        Arc::new(Mutex::new(Vec::new()))
    };
    let mut public = Sink(Arc::clone(&output));
    let mut diagnostics = Sink(Arc::clone(&errors));
    let mut broken = Broken;
    let mut selected: Option<&mut dyn Write> = match destination {
        Destination::Hidden => None,
        Destination::Broken => Some(&mut broken),
        _ => Some(&mut public),
    };
    let observed = relay(
        &mut child.0,
        &mut selected,
        &mut diagnostics,
        &[],
        Some(receiver),
    );
    let sent = sender.join().map_err(|_| "sender thread failed")?;
    let exit = child.0.wait()?;
    assert!(!directory.exists(), "owned endpoint leaked");
    if matches!(destination, Destination::Broken) {
        assert!(observed.is_err());
        assert!(sent.is_err(), "failed delivery must receive NACK");
        return Ok(());
    }
    sent?;
    assert!(exit.success());
    let (captured_out, captured_err) = observed?;
    let mut json = br#"{"reason":"compiler-artifact","padding":""#.to_vec();
    json.extend(std::iter::repeat_n(b'x', 65536));
    json.extend_from_slice(b"\"}\n");
    assert_eq!(captured_out, [json, shown.clone()].concat());
    assert_eq!(captured_err, b"before\nafter\n");
    let actual = output.lock().map_err(|_| "stdout poisoned")?.clone();
    if matches!(destination, Destination::Combined) {
        let shown_at = actual
            .windows(shown.len())
            .position(|part| part == shown)
            .ok_or("shown stdout missing")?;
        let after_at = actual
            .windows(6)
            .position(|part| part == b"after\n")
            .ok_or("later stderr missing")?;
        assert!(
            shown_at + shown.len() <= after_at,
            "driver exited before stdout delivery"
        );
        assert_eq!(actual.len(), shown.len() + b"before\nafter\n".len());
    } else {
        assert_eq!(
            actual,
            if matches!(destination, Destination::Hidden) {
                Vec::new()
            } else {
                shown
            }
        );
        assert_eq!(*errors.lock().map_err(|_| "stderr poisoned")?, captured_err);
    }
    Ok(())
}

#[test]
fn original_pipe_keeps_binary_bytes_large_json_and_causal_exit_order()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    exercise(Destination::Combined)?;
    exercise(Destination::Split)
}

#[test]
fn hidden_stdout_still_completes_and_broken_sink_nacks()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    exercise(Destination::Hidden)?;
    exercise(Destination::Broken)
}

struct Notify(mpsc::Sender<Vec<u8>>);
impl Write for Notify {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .send(bytes.to_vec())
            .map_err(|_| std::io::Error::other("observer closed"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn nack_releases_complete_emit_lock_and_next_sender_can_finish()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let receiver = Receiver::bind()?;
    let endpoint = receiver.path().to_owned();
    let directory = endpoint
        .parent()
        .ok_or("missing endpoint parent")?
        .to_owned();
    let (tx, rx) = mpsc::channel();
    let first_endpoint = endpoint.clone();
    let first_tx = tx.clone();
    let first =
        std::thread::spawn(move || send_test(&first_endpoint, b"first\n", &mut Notify(first_tx)));
    assert_eq!(rx.recv_timeout(Duration::from_secs(5))?, b"first\n");
    let first_marker = rx.recv_timeout(Duration::from_secs(5))?;
    let second = std::thread::spawn(move || send_test(&endpoint, b"second\n", &mut Notify(tx)));
    let held = matches!(
        rx.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    drop(receiver.decode(&first_marker)?.completion); // Production failure/NACK path.
    let first_result = first.join().map_err(|_| "first sender failed")?;
    let second_bytes = rx.recv_timeout(Duration::from_secs(5))?;
    let second_marker = rx.recv_timeout(Duration::from_secs(5))?;
    receiver
        .decode(&second_marker)?
        .completion
        .ok_or("missing completion")?
        .acknowledge()?;
    let second_result = second.join().map_err(|_| "second sender failed")?;
    receiver.finish()?;
    assert!(held, "another emit bypassed the held lock");
    assert!(first_result.is_err(), "NACK accepted as success");
    assert_eq!(second_bytes, b"second\n");
    second_result?;
    assert!(!directory.exists());
    Ok(())
}

#[test]
fn malformed_capability_marker_refuses_and_foreign_text_stays_literal()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let receiver = Receiver::bind()?;
    let parent = receiver.path().parent().ok_or("missing endpoint parent")?;
    let capability = std::fs::read_to_string(parent.join("capability"))?;
    let prefix = format!("[[sail-stdout:{capability}:");
    let malformed = format!("{prefix}1-0]]"); // Complete marker newline withheld.
    assert!(receiver.decode(malformed.as_bytes()).is_err());
    let foreign = b"[[sail-stdout:foreign:1-0]]\n";
    assert_eq!(receiver.decode(foreign)?.bytes, foreign);
    let mut untouched = Vec::new();
    assert!(send_test(receiver.path(), prefix.as_bytes(), &mut untouched).is_err());
    assert_eq!(
        untouched,
        Vec::<u8>::new(),
        "marker collision wrote partial bytes"
    );
    receiver.finish()?;
    Ok(())
}

#[test]
fn sender_refuses_symlink_lock_without_writing_output()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let receiver = Receiver::bind()?;
    let parent = receiver.path().parent().ok_or("missing endpoint parent")?;
    let lock = parent.join("writer.lock");
    let saved = parent.join("saved.lock");
    std::fs::rename(&lock, &saved)?;
    std::os::unix::fs::symlink(&saved, &lock)?;
    let mut output = Vec::new();
    let result = send_test(receiver.path(), b"must not write", &mut output);
    std::fs::remove_file(&lock)?;
    std::fs::rename(saved, lock)?;
    receiver.finish()?;
    assert!(result.is_err());
    assert_eq!(output, Vec::<u8>::new());
    Ok(())
}

#[test]
fn absent_private_ack_has_a_bounded_timeout() -> std::result::Result<(), Box<dyn std::error::Error>>
{
    let receiver = Receiver::bind()?;
    let directory = receiver
        .path()
        .parent()
        .ok_or("missing endpoint parent")?
        .to_owned();
    let started = Instant::now();
    let result = send_test(
        receiver.path(),
        b"delivered but not acknowledged\n",
        &mut Vec::new(),
    );
    let elapsed = started.elapsed();
    receiver.finish()?;
    let error = result.err().ok_or("missing ACK accepted")?;
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    assert!((Duration::from_secs(29)..Duration::from_secs(36)).contains(&elapsed));
    assert!(!directory.exists());
    Ok(())
}

// A subprocess of this same genuine test binary exercises the production sender
// without changing the parent test process's environment or output descriptors.
#[test]
fn actual_cargo_sender_child() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Some(config) = std::env::var_os("SAIL_VISIBLE_TEST_CONFIG") else {
        return Ok(());
    };
    let config: Value = serde_json::from_slice(&std::fs::read(config)?)?;
    let endpoint = PathBuf::from(config["endpoint"].as_str().ok_or("missing endpoint")?);
    let fifo = PathBuf::from(config["fifo"].as_str().ok_or("missing original pipe")?);
    let payload = std::fs::read(config["payload"].as_str().ok_or("missing payload")?)?;
    let mut writer = std::fs::OpenOptions::new().write(true).open(fifo)?;
    send_test(&endpoint, &payload, &mut writer)?;
    std::fs::write(
        config["completed"]
            .as_str()
            .ok_or("missing completion record")?,
        b"ACK received",
    )?;
    Ok(())
}

const CARGO_WRAPPER: &str = r#"#!/usr/bin/env python3
import json,os,pathlib,subprocess,sys
args=sys.argv[1:];compiler=args.pop(0)
if '--crate-name'not in args or args[args.index('--crate-name')+1]!='sail_visible_cargo':os.execv(compiler,[compiler,*args])
code=subprocess.call([compiler,*args],close_fds=False)
config=json.loads(pathlib.Path(os.environ['SAIL_VISIBLE_TEST_CONFIG']).read_bytes())
if config['mode']=='plain':
    with open(config['payload'],'rb') as payload:sys.stdout.buffer.write(payload.read());sys.stdout.buffer.flush()
    pathlib.Path(config['completed']).write_bytes(b'plain stdout written')
    raise SystemExit(code)
fifo=pathlib.Path(config['fifo']);os.mkfifo(fifo,0o600)
with open(config['child_log'],'wb') as log:
    child=subprocess.Popen([config['executable'],'--exact',config['test'],'--nocapture'],stdout=log,stderr=log,close_fds=False)
    try:
        with fifo.open('rb',buffering=0) as source:
            while True:
                data=source.read(65536)
                if not data:break
                sys.stdout.buffer.write(data);sys.stdout.buffer.flush()
        child_code=child.wait(timeout=35)
    finally:
        if child.poll()is None:child.kill();child.wait()
if child_code!=0:raise SystemExit(child_code)
# This exit permits Cargo's final error only AFTER the real sender's ACK.
raise SystemExit(code)
"#;

// Specialized retention for this one actual-Cargo fixture. A failed capture
// keeps the original owned tree; it never deletes evidence to satisfy a bound.
struct CargoEvidence<'a> {
    root: &'a Scratch,
    destination: Option<PathBuf>,
    retained: std::cell::Cell<u64>,
    finished: bool,
}
impl<'a> CargoEvidence<'a> {
    fn new(root: &'a Scratch) -> std::io::Result<Self> {
        use std::os::unix::fs::DirBuilderExt as _;
        root.1.set(true);
        let destination = std::env::var_os("SAIL_VISIBLE_TEST_EVIDENCE")
            .map(|path| {
                let parent = PathBuf::from(path).canonicalize()?;
                let name = root
                    .0
                    .file_name()
                    .ok_or_else(|| std::io::Error::other("missing fixture name"))?;
                let destination = parent.join(name);
                if destination.starts_with(root.0.canonicalize()?) {
                    return Err(std::io::Error::other(
                        "fixture evidence is inside its input tree",
                    ));
                }
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&destination)?;
                Ok::<_, std::io::Error>(destination)
            })
            .transpose()?;
        Ok(Self {
            root,
            destination,
            retained: std::cell::Cell::new(0),
            finished: false,
        })
    }
    fn capture(&self, label: &str) -> std::io::Result<()> {
        use std::os::unix::fs::{
            DirBuilderExt as _, FileTypeExt as _, OpenOptionsExt as _, PermissionsExt as _,
        };
        let Some(destination) = &self.destination else {
            return Ok(());
        };
        let destination = destination.join(label);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination)?;
        let mut pending = vec![PathBuf::new()];
        let mut rows = Vec::new();
        while let Some(relative) = pending.pop() {
            let source = self.root.0.join(&relative);
            let metadata = std::fs::symlink_metadata(&source)?;
            let mode = metadata.permissions().mode() & 0o777;
            if metadata.is_dir() {
                if !relative.as_os_str().is_empty() {
                    std::fs::DirBuilder::new()
                        .mode(0o700)
                        .create(destination.join(&relative))?;
                }
                let mut children = std::fs::read_dir(&source)?
                    .map(|entry| entry.map(|entry| relative.join(entry.file_name())))
                    .collect::<std::io::Result<Vec<_>>>()?;
                children.sort();
                pending.extend(children.into_iter().rev());
                rows.push(serde_json::json!({"path":relative,"kind":"directory","mode":mode}));
            } else if metadata.is_file() {
                let reserved = self
                    .retained
                    .get()
                    .checked_add(metadata.len())
                    .ok_or_else(|| std::io::Error::other("fixture capture size overflow"))?;
                if reserved > 8 * 1024 * 1024 {
                    return Err(std::io::Error::other(
                        "complete fixture evidence exceeds 8MiB; original tree preserved",
                    ));
                }
                self.retained.set(reserved);
                let bytes = std::fs::read(&source)?;
                if u64::try_from(bytes.len()).ok() != Some(metadata.len()) {
                    return Err(std::io::Error::other("fixture file changed during capture"));
                }
                let path = destination.join(&relative);
                let mut output = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)?;
                output.write_all(&bytes)?;
                rows.push(serde_json::json!({"path":relative,"kind":"file","mode":mode,"bytes":bytes.len(),"sha256":digest(&bytes)}));
            } else if metadata.file_type().is_fifo() && relative == Path::new("stdout.fifo") {
                rows.push(
                    serde_json::json!({"path":relative,"kind":"owned stdout FIFO","mode":mode}),
                );
            } else {
                return Err(std::io::Error::other(
                    "unexpected fixture alias/special; original tree preserved",
                ));
            }
        }
        std::fs::write(
            destination.join("index.json"),
            serde_json::to_vec_pretty(&rows)?,
        )
    }
    fn finish(mut self) {
        self.finished = true;
        self.root.1.set(false);
    }
}
impl Drop for CargoEvidence<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let retained = self.capture("failure");
        self.root
            .1
            .set(self.destination.is_none() || retained.is_err());
        eprintln!(
            "actual Cargo fixture evidence: destination={:?}; original={:?}; preserved={}; capture={retained:?}",
            self.destination,
            self.root.0,
            self.root.1.get()
        );
    }
}

struct Mirrored {
    separate: Sink,
    combined: Sink,
}
impl Write for Mirrored {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.separate.write_all(bytes)?;
        self.combined.write_all(bytes)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.separate.flush()?;
        self.combined.flush()
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Matched plain/barrier Cargo arms share exact source/paths and retain full stream assertions."
)]
fn genuine_cargo_matches_plain_full_streams_and_waits_for_final_stdout_delivery()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt as _;
    let root = Scratch::new()?;
    let evidence = CargoEvidence::new(&root)?;
    let project = &root.0;
    std::fs::create_dir(project.join("src"))?;
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname=\"sail_visible_cargo\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[workspace]\n",
    )?;
    let source = b"fn main() { let _ = MISSING_FOR_CARGO_DIAGNOSTIC; }\n";
    std::fs::write(project.join("src/main.rs"), source)?;
    let wrapper = project.join("wrapper.py");
    std::fs::write(&wrapper, CARGO_WRAPPER)?;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))?;
    let json_record = br#"{"reason":"compiler-artifact","fresh":true,"padding":"test"}"#.to_vec();
    let mut large = b"driver first line\n".to_vec();
    large.extend_from_slice(br#"{"reason":"compiler-artifact","padding":""#);
    large.extend(std::iter::repeat_n(b'x', 65536));
    large.extend_from_slice(b"\"}\ndriver final stdout");
    let cases = [
        (
            "partial-summary",
            b"verification results:: final driver stdout".to_vec(),
            b"verification results:: final driver stdout\n".to_vec(),
        ),
        ("partial-json", json_record.clone(), Vec::new()),
        (
            "complete-json",
            [json_record.as_slice(), b"\n"].concat(),
            Vec::new(),
        ),
        (
            "large-multiline",
            large,
            b"driver first line\ndriver final stdout\n".to_vec(),
        ),
    ];
    let config = project.join("config.json");
    let test = format!("{}::actual_cargo_sender_child", module_path!());
    let test = test
        .strip_prefix("sail_verus::")
        .ok_or("unexpected test module")?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    for (case, payload, expected_public) in cases {
        std::fs::write(project.join("payload"), &payload)?;
        std::fs::write(project.join(format!("{case}.payload")), &payload)?;
        let mut arms = Vec::new();
        for mode in ["plain", "barrier"] {
            let label = format!("{case}-{mode}");
            let receiver = if mode == "barrier" {
                Some(Receiver::bind()?)
            } else {
                None
            };
            let directory = receiver
                .as_ref()
                .and_then(|receiver| receiver.path().parent())
                .map(Path::to_owned);
            let completed = project.join(format!("{label}.completed"));
            let child_log = project.join(format!("{label}.child.log"));
            std::fs::write(
                &config,
                serde_json::to_vec(&serde_json::json!({
                    "mode": mode, "endpoint": receiver.as_ref().map(Receiver::path),
                    "fifo": project.join("stdout.fifo"), "payload": project.join("payload"),
                    "completed": completed, "child_log": child_log,
                    "executable": std::env::current_exe()?, "test": test,
                }))?,
            )?;
            let mut child = Producer(
                Command::new(&cargo)
                    .args([
                        "check",
                        "--offline",
                        "--message-format=json-render-diagnostics",
                    ])
                    .current_dir(project)
                    .env("CARGO_TARGET_DIR", project.join("target"))
                    .env("CARGO_INCREMENTAL", "0")
                    .env("CARGO_BUILD_JOBS", "1")
                    .env("CARGO_TERM_COLOR", "never")
                    .env("RUSTUP_AUTO_INSTALL", "0")
                    .env("RUSTC_WRAPPER", &wrapper)
                    .env_remove("RUSTC_WORKSPACE_WRAPPER")
                    .env_remove("RUSTFLAGS")
                    .env_remove("CARGO_ENCODED_RUSTFLAGS")
                    .env("SAIL_VISIBLE_TEST_CONFIG", &config)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?,
            );
            let combined = Arc::new(Mutex::new(Vec::new()));
            let public_out = Arc::new(Mutex::new(Vec::new()));
            let public_err = Arc::new(Mutex::new(Vec::new()));
            let mut out = Mirrored {
                separate: Sink(Arc::clone(&public_out)),
                combined: Sink(Arc::clone(&combined)),
            };
            let mut err = Mirrored {
                separate: Sink(Arc::clone(&public_err)),
                combined: Sink(Arc::clone(&combined)),
            };
            let observed = relay(&mut child.0, &mut Some(&mut out), &mut err, &[], receiver);
            let exit = child.0.wait()?;
            let separate_out = public_out.lock().map_err(|_| "stdout poisoned")?.clone();
            let separate_err = public_err.lock().map_err(|_| "stderr poisoned")?.clone();
            let combined_bytes = combined
                .lock()
                .map_err(|_| "combined output poisoned")?
                .clone();
            for (name, bytes) in [
                ("stdout", &separate_out),
                ("stderr", &separate_err),
                ("combined", &combined_bytes),
            ] {
                std::fs::write(project.join(format!("{label}.{name}")), bytes)?;
                eprintln!(
                    "actual Cargo {label} {name}: total={} bounded-prefix={:?}",
                    bytes.len(),
                    &bytes[..bytes.len().min(65536)]
                );
            }
            if let Ok((stdout, stderr)) = &observed {
                std::fs::write(project.join(format!("{label}.cargo.stdout")), stdout)?;
                std::fs::write(project.join(format!("{label}.cargo.stderr")), stderr)?;
            }
            if child_log.is_file() {
                let log = std::fs::read(&child_log)?;
                eprintln!(
                    "actual Cargo {label} child log: total={} bounded-prefix={:?}",
                    log.len(),
                    &log[..log.len().min(65536)]
                );
            }
            std::fs::write(
                project.join(format!("{label}.status.json")),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "exit":exit.code(), "relay_error":observed.as_ref().err().map(ToString::to_string),
                    "completed_exists":completed.is_file(), "child_log_exists":child_log.is_file(),
                }))?,
            )?;
            evidence.capture(&label)?; // Retain raw streams and every owned file BEFORE assertions.
            assert_eq!(exit.code(), Some(101), "{mode}");
            let captured = observed?;
            assert!(completed.is_file(), "{mode} producer did not finish");
            assert_eq!(std::fs::read(project.join("src/main.rs"))?, source);
            if let Some(directory) = directory {
                assert!(!directory.exists(), "owned ACK endpoint leaked");
                assert_eq!(std::fs::read(completed)?, b"ACK received");
                assert!(std::fs::metadata(child_log)?.len() < 16384);
            }
            assert_eq!(
                separate_out, expected_public,
                "{label}: original Cargo processing/newline must remain exact"
            );
            if mode == "barrier" && !expected_public.is_empty() {
                // Separate complete streams remain exact above and below. Only
                // the final line is the causal frontier; unrelated earlier
                // stderr may arrive between preceding stdout records.
                let shown = expected_public
                    .strip_suffix(b"\n")
                    .ok_or("expected Cargo newline missing")?
                    .rsplit(|byte| *byte == b'\n')
                    .next()
                    .ok_or("final stdout line missing")?;
                let shown = [shown, b"\n"].concat();
                let visible = combined
                    .lock()
                    .map_err(|_| "combined output poisoned")?
                    .clone();
                let shown_at = visible
                    .windows(shown.len())
                    .position(|part| part == shown.as_slice())
                    .ok_or("final stdout missing")?;
                let cargo_error = b"error: could not compile";
                let error_at = visible
                    .windows(cargo_error.len())
                    .position(|part| part == cargo_error)
                    .ok_or("Cargo final error missing")?;
                assert!(
                    shown_at + shown.len() <= error_at,
                    "Cargo error overtook acknowledged stdout"
                );
            }
            if mode == "barrier" {
                std::fs::remove_file(project.join("stdout.fifo"))?;
            }
            arms.push((captured, separate_out, separate_err));
        }
        assert_eq!(
            arms[1], arms[0],
            "{case}: every captured/public stdout and stderr byte must match plain Cargo"
        );
        let text = String::from_utf8(arms[0].2.clone())?;
        assert!(text.contains("error[E0425]") && text.contains("MISSING_FOR_CARGO_DIAGNOSTIC"));
        assert!(
            !text.contains("\"$message_type\""),
            "raw compiler JSON bypassed Cargo rendering"
        );
    }
    evidence.finish();
    Ok(())
}

// The real grandchild deliberately keeps both original pipe descriptors open.
// Parent owns and reaps it after the test requests shutdown; self-deadlines
// also make the old blocking-join implementation fail finitely, not hang CI.
const DESCENDANT_HOLDER: &str = r#"
import json,os,pathlib,subprocess,sys,time
root=pathlib.Path(sys.argv[1]);stop=root/'stop';ready=root/'grandchild.ready'
script="import os,pathlib,sys,time\nroot=pathlib.Path(sys.argv[1]);(root/'grandchild.ready').write_text(str(os.getpid()))\nend=time.monotonic()+8\nwhile not(root/'stop').exists()and time.monotonic()<end:time.sleep(.001)\n(root/'grandchild.done').write_text('exiting')\n"
child=subprocess.Popen([sys.executable,'-c',script,str(root)],close_fds=False)
end=time.monotonic()+8
try:
    while not ready.exists():
        if time.monotonic()>=end:raise RuntimeError('grandchild did not start')
        time.sleep(.001)
    os.write(1,b'force-public-write-failure\n')
    while not stop.exists()and time.monotonic()<end:time.sleep(.001)
finally:
    stop.write_text('stop exact owned grandchild')
    try:code=child.wait(timeout=3)
    except subprocess.TimeoutExpired:child.kill();code=child.wait(timeout=3)
    (root/'reaped.json').write_text(json.dumps({'pid':child.pid,'exit':code}))
"#;

struct DescendantTree {
    child: std::process::Child,
    stop: PathBuf,
}
impl Drop for DescendantTree {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.stop, b"owned test shutdown");
        let deadline = Instant::now() + Duration::from_secs(3);
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[test]
fn relay_error_joins_readers_without_waiting_for_descendant_pipe_eof()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let root = Scratch::new()?;
    let mut tree = DescendantTree {
        child: Command::new("python3")
            .args(["-c", DESCENDANT_HOLDER])
            .arg(&root.0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
        stop: root.0.join("stop"),
    };
    let mut broken = Broken;
    let mut errors = Vec::new();
    let started = Instant::now();
    let observed = relay(
        &mut tree.child,
        &mut Some(&mut broken),
        &mut errors,
        &[],
        Some(Receiver::bind()?),
    );
    let elapsed = started.elapsed();
    let parent_running = tree.child.try_wait();
    let descendant_retained_descriptors =
        root.0.join("grandchild.ready").is_file() && !root.0.join("grandchild.done").exists();
    // Exact owned shutdown and parent wait/reap happen BEFORE interpreting results.
    std::fs::write(root.0.join("stop"), b"test cleanup")?;
    let exit = tree.child.wait()?;
    let reaped: Value = serde_json::from_slice(&std::fs::read(root.0.join("reaped.json"))?)?;
    assert!(exit.success());
    assert_eq!(reaped["exit"], 0);
    assert!(root.0.join("grandchild.done").is_file());
    assert!(observed.is_err(), "broken public sink accepted");
    assert!(parent_running?.is_none(), "relay waited for producer EOF");
    assert!(
        descendant_retained_descriptors,
        "fixture did not retain actual descendant FDs"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "reader shutdown awaited descendant EOF: {elapsed:?}"
    );
    Ok(())
}
