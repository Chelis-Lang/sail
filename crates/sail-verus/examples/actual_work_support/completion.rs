//! Explicit opt-in evidence admission; the default observer does not use this.

use super::{invalid, read};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::{Child, Command, Stdio};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

const MAX_AIR_BYTES: usize = 8 * 1024 * 1024;
#[cfg(target_os = "linux")]
const MAX_OUTPUT_BYTES: usize = 4096;
#[cfg(target_os = "linux")]
const TIME_LIMIT: Duration = Duration::from_secs(2);
const PREFIX: &[u8] = b";; SAIL-COMPLETE ";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Zero,
    Used,
}

pub(super) fn payload<'a>(bytes: &'a [u8], kind: &str) -> io::Result<(&'a [u8], Mode)> {
    let mut markers = bytes.windows(PREFIX.len()).filter(|part| *part == PREFIX);
    if markers.next().is_none() || markers.next().is_some() {
        return Err(invalid("missing or duplicate completion marker"));
    }
    for (name, mode) in [("zero-process", Mode::Zero), ("process-used", Mode::Used)] {
        let footer = format!(";; SAIL-COMPLETE 1 {kind} {name}\n");
        if let Some(body) = bytes.strip_suffix(footer.as_bytes()) {
            if body.is_empty() || body.ends_with(b"\n") {
                return Ok((body, mode));
            }
        }
    }
    Err(invalid("unsupported or misplaced completion marker"))
}

#[derive(Serialize)]
pub(super) struct Validator {
    path: PathBuf,
    binary_sha256: String,
    source_commit: String,
    image_admission: &'static str,
}

fn hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Validator {
    pub(super) fn new(path: &Path, binary_sha256: &str, source_commit: &str) -> io::Result<Self> {
        if !hex(binary_sha256, 64) || !hex(source_commit, 40) {
            return Err(invalid("invalid validator source or binary identity"));
        }
        // Refuse a leaf alias before resolving its executable name; a bare name
        // must never invoke a different executable through PATH lookup.
        if format!("{:x}", Sha256::digest(read(path)?)) != binary_sha256 {
            return Err(invalid("validator binary identity differs"));
        }
        let path = std::fs::canonicalize(path)?;
        if path.to_str().is_none() {
            return Err(invalid("non-UTF8 validator executable path"));
        }
        let validator = Self {
            path,
            binary_sha256: binary_sha256.to_owned(),
            source_commit: source_commit.to_owned(),
            image_admission: "held Linux proc exe digest before AIR input",
        };
        validator.check_identity()?;
        Ok(validator)
    }

    fn check_identity(&self) -> io::Result<()> {
        if format!("{:x}", Sha256::digest(read(&self.path)?)) != self.binary_sha256 {
            return Err(invalid("validator binary identity changed"));
        }
        Ok(())
    }

    pub(super) fn validate(&self, air: &[u8]) -> io::Result<()> {
        if air.len() > MAX_AIR_BYTES {
            return Err(invalid("completed zero-context AIR exceeds 8 MiB"));
        }
        self.check_identity()?;
        let outcome = invoke(&self.path, &self.binary_sha256, air);
        // An identity failure remains a refusal even if the child succeeded.
        self.check_identity()?;
        outcome
    }
}

#[cfg(target_os = "linux")]
#[derive(Default, Clone)]
struct Capture {
    bytes: Vec<u8>,
    done: bool,
    failed: bool,
}

#[cfg(target_os = "linux")]
fn capture(reader: impl Read + Send + 'static) -> Arc<Mutex<Capture>> {
    let state = Arc::new(Mutex::new(Capture::default()));
    let destination = state.clone();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buffer = [0; 512];
        loop {
            let result = reader.read(&mut buffer);
            let Ok(mut state) = destination.lock() else {
                return;
            };
            match result {
                Ok(0) => {
                    state.done = true;
                    return;
                }
                Ok(count) => {
                    let room = MAX_OUTPUT_BYTES.saturating_sub(state.bytes.len());
                    state.bytes.extend_from_slice(&buffer[..count.min(room)]);
                    if count > room {
                        state.failed = true;
                        state.done = true;
                        return;
                    }
                }
                Err(_) => {
                    state.failed = true;
                    state.done = true;
                    return;
                }
            }
        }
    });
    state
}

#[cfg(target_os = "linux")]
struct OwnedChild(Child);

#[cfg(target_os = "linux")]
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Never signal a process group after its leader has been reaped and
        // the numeric ID could have been reused. The admitted genuine helper
        // starts no children. The enclosing native controller separately owns
        // its container and retires it on any failed observation.
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            #[cfg(target_os = "linux")]
            {
                let _ = Command::new("/bin/kill")
                    .args(["-s", "KILL", "--", &format!("-{}", self.0.id())])
                    .status();
            }
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[cfg(target_os = "linux")]
fn admit_image(child: &mut Child, expected: &str) -> io::Result<std::fs::File> {
    if child.try_wait()?.is_some() {
        return Err(invalid("validator exited before image admission"));
    }
    let mut image = std::fs::File::open(format!("/proc/{}/exe", child.id()))?;
    let metadata = image.metadata()?;
    if !metadata.is_file() || metadata.len() > super::MAX_FILE_BYTES {
        return Err(invalid("unsupported mapped validator image"));
    }
    let mut bytes = Vec::new();
    (&mut image)
        .take(super::MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > super::MAX_FILE_BYTES {
        return Err(invalid("mapped validator image exceeded the limit"));
    }
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != expected {
        return Err(invalid(format!(
            "mapped validator image differs: expected={expected}; actual={actual}; no AIR delivered"
        )));
    }
    if child.try_wait()?.is_some() {
        return Err(invalid("validator exited during image admission"));
    }
    Ok(image)
}

#[cfg(not(target_os = "linux"))]
fn invoke(_program: &Path, _expected: &str, _air: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "paired-completion validator requires Linux mapped-image admission",
    ))
}

#[cfg(target_os = "linux")]
fn invoke(program: &Path, expected: &str, air: &[u8]) -> io::Result<()> {
    let mut command = Command::new(program);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = OwnedChild(command.spawn()?);
    // Do not deliver even one AIR byte before binding the executable actually
    // mapped in this unreaped owned child. The genuine helper blocks on stdin
    // and contains no fork/exec path; hold its image FD for the entire call.
    let _mapped_image = admit_image(&mut child.0, expected)?;
    let Some(mut stdin) = child.0.stdin.take() else {
        return Err(invalid("missing validator stdin"));
    };
    let Some(stdout) = child.0.stdout.take() else {
        return Err(invalid("missing validator stdout"));
    };
    let Some(stderr) = child.0.stderr.take() else {
        return Err(invalid("missing validator stderr"));
    };
    let stdout = capture(stdout);
    let stderr = capture(stderr);
    let input = air.to_vec();
    let written = Arc::new(Mutex::new(None));
    let destination = written.clone();
    std::thread::spawn(move || {
        let outcome = stdin.write_all(&input).and_then(|()| stdin.flush()).is_ok();
        drop(stdin);
        if let Ok(mut state) = destination.lock() {
            *state = Some(outcome);
        }
    });
    let deadline = Instant::now() + TIME_LIMIT;
    loop {
        let out = stdout.lock().map_err(invalid)?.clone();
        let err = stderr.lock().map_err(invalid)?.clone();
        let input = *written.lock().map_err(invalid)?;
        if out.failed || err.failed || input == Some(false) {
            return Err(invalid(format!(
                "validator I/O failed or exceeded output limit; stdout={:?}; stderr={:?}",
                out.bytes, err.bytes
            )));
        }
        if let Some(status) = child.0.try_wait()? {
            if out.done && err.done && input.is_some() {
                return if status.success()
                    && out.bytes == b"SAIL-AIR-NO-QUERY 1\n"
                    && err.bytes.is_empty()
                {
                    Ok(())
                } else {
                    Err(invalid(format!(
                        "validator refused AIR: exit={status}; stdout={:?}; stderr={:?}",
                        out.bytes, err.bytes
                    )))
                };
            }
        }
        if Instant::now() >= deadline {
            return Err(invalid(format!(
                "validator exceeded 2 seconds; stdout={:?}; stderr={:?}",
                out.bytes, err.bytes
            )));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_unique_eof_markers_only() -> io::Result<()> {
        let (body, mode) = payload(
            b";; SAIL-COMPLETE 1 transcript zero-process\n",
            "transcript",
        )?;
        assert!(body.is_empty());
        assert!(mode == Mode::Zero);
        let body = b";;;>>> QUERY\n(check-sat)\n;;;<<<\n;;;>>> RESPONSE\nunsat\n;;;<<<\n";
        let mut complete = body.to_vec();
        complete.extend_from_slice(b";; SAIL-COMPLETE 1 transcript process-used\n");
        let (actual, mode) = payload(&complete, "transcript")?;
        assert_eq!(actual, body);
        assert!(mode == Mode::Used);
        for bad in [
            b"".as_slice(),
            b";; SAIL-COMPLETE 2 transcript zero-process\n",
            b";; SAIL-COMPLETE 1 transcript zero-process",
            b";; SAIL-COMPLETE 1 air zero-process\n",
            b";; SAIL-COMPLETE 1 transcript zero-process\n\n",
            b";; SAIL-COMPLETE 1 transcript zero-process\n; trailing\n",
            b"prefix;; SAIL-COMPLETE 1 transcript zero-process\n",
            b";; SAIL-COMPLETE 1 transcript zero-process\n;; SAIL-COMPLETE 1 transcript zero-process\n",
        ] {
            assert!(payload(bad, "transcript").is_err());
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    struct Fixture(PathBuf);

    #[cfg(target_os = "linux")]
    impl Fixture {
        fn new() -> io::Result<Self> {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "sail-air-validator-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&root)?;
            Ok(Self(root))
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn unsupported_platform_never_launches_a_helper() -> io::Result<()> {
        let Err(error) = invoke(Path::new("/never-executed"), &"a".repeat(64), b"AIR") else {
            return Err(invalid("unsupported image admission accepted"));
        };
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn program(fixture: &Fixture, text: &str) -> io::Result<PathBuf> {
        use std::os::unix::fs::PermissionsExt as _;
        let path = fixture.0.join("validator");
        std::fs::write(&path, format!("#!/bin/sh\n{text}\n"))?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(path)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn explicit_identity_and_exact_success_contract() -> io::Result<()> {
        let path = std::fs::canonicalize("/bin/cat")?;
        let digest = format!("{:x}", Sha256::digest(read(&path)?));
        let validator = Validator::new(&path, &digest, &"a".repeat(40))?;
        // Actual ELF/input transport fixture only; native qualification must
        // separately execute the genuine source-bound AIR grammar helper.
        validator.validate(b"SAIL-AIR-NO-QUERY 1\n")?;
        assert!(Validator::new(&path, &"0".repeat(64), &"a".repeat(40)).is_err());
        assert!(Validator::new(&path, &digest, "invalid").is_err());
        assert!(validator.validate(b"not the success contract\n").is_err());
        assert!(
            validator
                .validate(&vec![b'x'; MAX_OUTPUT_BYTES + 1])
                .is_err()
        );
        assert!(validator.validate(&vec![b' '; MAX_AIR_BYTES + 1]).is_err());
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn interpreter_or_changed_path_refuses_before_input() -> io::Result<()> {
        let fixture = Fixture::new()?;
        let captured = fixture.0.join("delivered-air");
        let quoted = captured.to_string_lossy().replace('\'', "'\\''");
        let path = program(&fixture, &format!("cat > '{quoted}'"))?;
        let digest = format!("{:x}", Sha256::digest(read(&path)?));
        let validator = Validator::new(&path, &digest, &"a".repeat(40))?;
        let error = validator
            .validate(b"AIR MUST NOT BE DELIVERED")
            .err()
            .ok_or_else(|| invalid("foreign interpreter accepted"))?;
        assert!(error.to_string().contains("mapped validator image differs"));
        match std::fs::read(&captured) {
            Ok(bytes) => assert!(bytes.is_empty()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        std::fs::write(&path, b"changed executable")?;
        assert!(validator.validate(b"AIR MUST NOT BE DELIVERED").is_err());
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn restored_expected_path_cannot_admit_a_foreign_mapped_image() -> io::Result<()> {
        let fixture = Fixture::new()?;
        let cat = std::fs::canonicalize("/bin/cat")?;
        let expected = format!("{:x}", Sha256::digest(read(&cat)?));
        let restored = fixture.0.join("expected-image");
        std::fs::copy(&cat, &restored)?;
        let path = fixture.0.join("validator");
        let delivered = fixture.0.join("delivered-air");
        let quote = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"));
        program(
            &fixture,
            &format!(
                "mv {} {}\nprintf 'restored\\n'\ncat > {}",
                quote(&restored),
                quote(&path),
                quote(&delivered)
            ),
        )?;
        let mut command = Command::new(&path);
        use std::os::unix::process::CommandExt as _;
        command
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = OwnedChild(command.spawn()?);
        let output = capture(
            child
                .0
                .stdout
                .take()
                .ok_or_else(|| invalid("missing fixture stdout"))?,
        );
        let deadline = Instant::now() + TIME_LIMIT;
        loop {
            let state = output.lock().map_err(invalid)?.clone();
            if state.bytes == b"restored\n" {
                break;
            }
            if state.failed || state.done || Instant::now() >= deadline {
                return Err(invalid(
                    "foreign-image fixture failed to restore expected path",
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(format!("{:x}", Sha256::digest(read(&path)?)), expected);
        let error = admit_image(&mut child.0, &expected)
            .err()
            .ok_or_else(|| invalid("foreign image admitted after pathname restore"))?;
        assert!(error.to_string().contains("mapped validator image differs"));
        // No AIR is provided before image admission. The same production guard
        // refuses although the restored pathname now has the expected bytes.
        match std::fs::read(&delivered) {
            Ok(bytes) => assert!(bytes.is_empty()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_attested_process_that_blocks_is_retired_on_timeout() -> io::Result<()> {
        // Actual ELF/timeout transport fixture only, not an AIR validator or
        // proof. The real source-bound helper is separately qualified natively.
        let path = std::fs::canonicalize("/usr/bin/python3")?;
        let digest = format!("{:x}", Sha256::digest(read(&path)?));
        let validator = Validator::new(&path, &digest, &"a".repeat(40))?;
        let started = Instant::now();
        let error = validator
            .validate(b"import time\ntime.sleep(30)\n")
            .err()
            .ok_or_else(|| invalid("blocking fixture accepted"))?;
        assert!(error.to_string().contains("exceeded 2 seconds"));
        assert!(started.elapsed() < Duration::from_secs(5));
        Ok(())
    }
}
