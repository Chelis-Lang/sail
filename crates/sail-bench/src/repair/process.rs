//! Process plumbing for the repair workload: owned daemons, the arms'
//! environments, invocations with deadlines and the public proof-journal
//! observer.

use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use sail_api::jobs::{Request, Response};
use sail_api::proof::{Cursor, Observation, Phase as ProofPhase};
use serde::Serialize;
use serde_json::Value;

use super::Phase;
use crate::daemon::MAX_SOCKET_PATH;

/// How often the proof-journal observer runs.
const POLL: Duration = Duration::from_millis(100);

/// Writes `value` as pretty JSON through a synced temporary file and an
/// atomic rename, so a reader sees the previous or the new file.
///
/// Returns the SHA-256 of the bytes written.
///
/// # Errors
///
/// A message if any step fails; the previous file is then untouched.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<String, String> {
    let mut text = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    text.push(b'\n');
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let tmp = dir.join(format!(".{name}.tmp"));
    let result = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(&text)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)?;
        File::open(dir)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
        .map(|()| super::sha256_hex(&text))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn signal_group(pid: u32, signal: &str) {
    let _ = Command::new("kill")
        .args([signal, "--", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn wait_for(child: &mut Child, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// Signals a process group this harness started, then kills it after
/// `grace`.
fn stop_group(child: &mut Child, first: &str, grace: Duration) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    signal_group(child.id(), first);
    if !wait_for(child, grace) {
        signal_group(child.id(), "-KILL");
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn socket_alive(path: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}

/// One arm's private environment.
#[derive(Debug, Clone)]
pub struct Env {
    /// `SAIL_CACHE_DIR`.
    pub cache: PathBuf,
    /// `SAIL_DAEMON_SOCKET`.
    pub socket: PathBuf,
    /// `RUSTUP_TOOLCHAIN` for Verus.
    pub rust_toolchain: String,
}

impl Env {
    /// Sets this arm's variables and removes every other inherited `SAIL_*`
    /// selector except `SAIL_VERUS_HOME`.
    pub fn apply(&self, cmd: &mut Command) {
        for (key, _) in std::env::vars_os() {
            let k = key.to_string_lossy();
            if k.starts_with("SAIL_") && k != "SAIL_VERUS_HOME" {
                cmd.env_remove(&key);
            }
        }
        cmd.env("SAIL_CACHE_DIR", &self.cache)
            .env("SAIL_DAEMON_SOCKET", &self.socket)
            .env("RUSTUP_TOOLCHAIN", &self.rust_toolchain)
            // Fresh verification: never reuse a proof receipt.
            .env("SAIL_MANAGED_FORCE", "1");
    }
}

/// A `sail serve` this harness started and alone stops. It never adopts a
/// live pre-existing socket; a stale one at its own path is removed.
pub struct OwnedDaemon {
    child: Child,
    /// Cache socket (`SAIL_DAEMON_SOCKET`).
    pub socket: PathBuf,
}

impl OwnedDaemon {
    /// Starts `binary serve` and waits for it to answer `STATS`.
    ///
    /// # Errors
    ///
    /// A message if a live service already owns the socket, or the daemon
    /// does not start.
    pub fn start(binary: &Path, env: &Env, log: &Path) -> Result<Self, String> {
        let socket = env.socket.clone();
        if socket.as_os_str().len() >= MAX_SOCKET_PATH {
            return Err(format!("socket path {} is too long", socket.display()));
        }
        for path in [socket.clone(), sail_api::jobs::socket_path(&socket)] {
            if socket_alive(&path) {
                return Err(format!(
                    "refusing to adopt the live service at {}",
                    path.display()
                ));
            }
            let _ = std::fs::remove_file(&path);
        }
        let out = File::options()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|e| format!("{}: {e}", log.display()))?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(binary);
        cmd.arg("serve")
            .arg("--cache")
            .arg(&env.cache)
            .arg("--socket")
            .arg(&socket)
            .args(["--idle-timeout", "0"])
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err);
        env.apply(&mut cmd);
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        let child = cmd
            .spawn()
            .map_err(|e| format!("cannot start {} serve: {e}", binary.display()))?;
        let mut daemon = Self { child, socket };
        let deadline = Instant::now() + Duration::from_secs(20);
        while daemon.stats().is_none() {
            if !daemon.alive() || Instant::now() >= deadline {
                return Err(format!("{} serve did not become ready", binary.display()));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(daemon)
    }

    /// Whether the daemon process is still running.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Cache counters, if it answers.
    #[must_use]
    pub fn stats(&self) -> Option<sail_api::protocol::StatsSnapshot> {
        sail_api::client::Client::connect(&self.socket)
            .ok()?
            .stats()
            .ok()
    }
}

impl Drop for OwnedDaemon {
    fn drop(&mut self) {
        // SIGTERM runs the daemon's owned-job shutdown before it exits.
        stop_group(&mut self.child, "-TERM", Duration::from_secs(10));
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Whether a proof observation is actionable for the repair author: a
/// focused diagnostic, function or obligation result, or the focused phase
/// finishing. A started query alone is not.
#[must_use]
pub fn actionable(event: &sail_api::proof::Event) -> bool {
    event.input.phase == ProofPhase::Focus
        && matches!(
            event.input.observation,
            Observation::Diagnostic(_)
                | Observation::FunctionObserved(_)
                | Observation::ObligationObserved(_)
                | Observation::PhaseFinished
        )
}

/// Reads the public proof journal of the job the foreground command
/// reports on stderr.
struct Observer {
    client: sail_api::jobs::Client,
    stderr: PathBuf,
    dir: PathBuf,
    job: Option<String>,
    cursor: Cursor,
    pages: u64,
    error: Option<String>,
    found: bool,
}

impl Observer {
    fn new(socket: &Path, dir: &Path) -> Self {
        Self {
            client: sail_api::jobs::Client::new(sail_api::jobs::socket_path(socket)),
            stderr: dir.join("stderr.txt"),
            dir: dir.to_path_buf(),
            job: None,
            cursor: Cursor::default(),
            pages: 0,
            error: None,
            found: false,
        }
    }

    /// One observation step; true once an actionable event was read.
    fn poll(&mut self) -> bool {
        if self.found || self.error.is_some() {
            return self.found;
        }
        if self.job.is_none() {
            let text = std::fs::read_to_string(&self.stderr).unwrap_or_default();
            self.job = text
                .lines()
                .find_map(|l| l.strip_prefix("Repair validation job "))
                .map(|s| s.trim().to_owned());
            if let Some(job) = &self.job {
                self.cursor.stream_id.clone_from(job);
            }
        }
        let Some(job) = self.job.clone() else {
            return false;
        };
        let request = Request::ProofEvents {
            id: job,
            cursor: self.cursor.clone(),
            limit: 64,
        };
        match self.client.call(&request) {
            Ok(Response::ProofEvents(page)) => {
                self.pages += 1;
                if !page.events.is_empty() || page.gap.is_some() || page.terminal || self.pages == 1
                {
                    let _ = write_json(
                        &self.dir.join(format!("proof-page-{:06}.json", self.pages)),
                        &page,
                    );
                }
                self.found = page.events.iter().any(actionable);
                self.cursor = page.next;
                if page.gap.is_some() && !self.found {
                    self.error = page.gap;
                }
            }
            Ok(Response::Error(e)) => self.error = Some(e),
            Ok(_) => self.error = Some("unexpected proof-events response".to_owned()),
            Err(e) => self.error = Some(e.to_string()),
        }
        self.found
    }

    fn cancel(&self) {
        if let Some(id) = &self.job {
            let _ = self.client.call(&Request::Cancel {
                id: id.clone(),
                superseded: false,
            });
        }
    }
}

/// What one invocation produced.
#[derive(Debug, Default)]
pub struct Invoked {
    /// The recorded phase.
    pub phase: Phase,
    /// The command's JSON document, if it printed one.
    pub document: Option<Value>,
    /// First actionable proof-journal evidence, if observed.
    pub first_actionable_ms: Option<f64>,
}

/// One process invocation of an arm's binary.
pub struct Call<'a> {
    /// Phase name.
    pub name: &'a str,
    /// The `sail` binary.
    pub binary: &'a Path,
    /// Its arguments.
    pub args: Vec<String>,
    /// Working directory.
    pub cwd: &'a Path,
    /// The arm's environment.
    pub env: &'a Env,
    /// Raw evidence directory.
    pub dir: PathBuf,
    /// Deadline.
    pub timeout: Duration,
    /// Read the proof journal for first actionable evidence (M7).
    pub observe: bool,
}

/// The last `chars` characters of `text`.
fn tail(text: &str, chars: usize) -> &str {
    let skip = text.chars().count().saturating_sub(chars);
    text.char_indices()
        .nth(skip)
        .and_then(|(i, _)| text.get(i..))
        .unwrap_or("")
}

/// The command's JSON document: the whole of stdout, or else its last
/// non-empty line.
fn document(stdout: &str) -> Option<Value> {
    serde_json::from_str(stdout).ok().or_else(|| {
        stdout
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .and_then(|l| serde_json::from_str(l).ok())
    })
}

impl Call<'_> {
    /// The command, with output redirected into the evidence directory.
    fn command(&self) -> Result<Command, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        write_json(
            &self.dir.join("command.json"),
            &serde_json::json!({
                "binary": self.binary, "args": self.args, "cwd": self.cwd,
                "env": {"SAIL_CACHE_DIR": self.env.cache, "SAIL_DAEMON_SOCKET": self.env.socket,
                        "RUSTUP_TOOLCHAIN": self.env.rust_toolchain, "SAIL_MANAGED_FORCE": "1"},
            }),
        )?;
        let stdout = File::create(self.dir.join("stdout.json")).map_err(|e| e.to_string())?;
        let stderr = File::create(self.dir.join("stderr.txt")).map_err(|e| e.to_string())?;
        let mut cmd = Command::new(self.binary);
        cmd.args(&self.args)
            .current_dir(self.cwd)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr);
        self.env.apply(&mut cmd);
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        Ok(cmd)
    }

    /// Runs the call. Never fails: every problem is recorded in the phase.
    #[must_use]
    pub fn invoke(&self) -> Invoked {
        let mut invoked = Invoked {
            phase: Phase {
                name: self.name.to_owned(),
                args: self.args.clone(),
                ..Phase::default()
            },
            ..Invoked::default()
        };
        let mut cmd = match self.command() {
            Ok(c) => c,
            Err(e) => {
                invoked.phase.error = Some(format!("cannot prepare {}: {e}", self.dir.display()));
                return invoked;
            }
        };
        let started = Instant::now();
        match cmd.spawn() {
            Ok(child) => self.supervise(child, started, &mut invoked),
            Err(e) => {
                invoked.phase.error = Some(format!("cannot start {}: {e}", self.binary.display()));
                return invoked;
            }
        }
        let phase = &mut invoked.phase;
        phase.wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        let stdout = std::fs::read_to_string(self.dir.join("stdout.json")).unwrap_or_default();
        invoked.document = document(&stdout);
        if invoked.document.is_none() && phase.error.is_none() && !phase.timed_out {
            let stderr = std::fs::read_to_string(self.dir.join("stderr.txt")).unwrap_or_default();
            phase.error = Some(format!(
                "no JSON document on stdout (exit {:?}): {}",
                phase.exit_code,
                tail(&stderr, 400).trim()
            ));
        }
        invoked
    }

    /// Waits for `child`, polling the proof journal, and stops its
    /// process group at the deadline.
    fn supervise(&self, mut child: Child, started: Instant, invoked: &mut Invoked) {
        let mut observer = self
            .observe
            .then(|| Observer::new(&self.env.socket, &self.dir));
        let mut next_poll = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(e) => {
                    invoked.phase.error = Some(format!("cannot wait for the command: {e}"));
                    stop_group(&mut child, "-INT", Duration::from_secs(5));
                    break None;
                }
            }
            if started.elapsed() > self.timeout {
                invoked.phase.timed_out = true;
                // The CLI's interrupt handler cancels its daemon job; the exact
                // owned job id is cancelled too, in case the CLI is unresponsive.
                stop_group(&mut child, "-INT", Duration::from_secs(5));
                if let Some(o) = &observer {
                    o.cancel();
                }
                break child.try_wait().ok().flatten();
            }
            if Instant::now() >= next_poll {
                next_poll = Instant::now() + POLL;
                if let Some(o) = observer.as_mut()
                    && invoked.first_actionable_ms.is_none()
                    && o.poll()
                {
                    invoked.first_actionable_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        invoked.phase.exit_code = status.and_then(|s| s.code());
        if let Some(o) = &observer {
            invoked.phase.job.clone_from(&o.job);
            invoked.phase.proof_pages = o.pages;
            invoked.phase.observation_error.clone_from(&o.error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_and_tails() {
        assert_eq!(
            document("{\n  \"a\": 1\n}\n"),
            Some(serde_json::json!({"a": 1}))
        );
        assert_eq!(
            document("noise\n{\"a\": 2}\n"),
            Some(serde_json::json!({"a": 2}))
        );
        assert_eq!(document("error: no"), None);
        assert_eq!(tail("abcdef", 2), "ef");
        assert_eq!(tail("ab", 5), "ab");
    }

    #[test]
    fn atomic_json_write_replaces_whole_file() {
        let dir = std::env::temp_dir().join(format!("sail-bench-repair-{}", std::process::id()));
        let path = dir.join("r.json");
        assert!(write_json(&path, &serde_json::json!({"a": 1})).is_ok());
        assert!(write_json(&path, &serde_json::json!({"a": 2})).is_ok());
        let back = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(back, "{\n  \"a\": 2\n}\n");
        assert!(!dir.join(".r.json.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn actionable_events_are_focused_results() {
        let event = |phase: &str, kind: &str| -> Option<sail_api::proof::Event> {
            serde_json::from_value(serde_json::json!({
                "version": 1, "stream_id": "j", "sequence": 1,
                "input": {"version": 1, "event_id": "e", "run_id": "r", "context_sha256": "c",
                    "input_revision": "i", "target": {"package_id": "p", "name": "n", "kind": ["lib"]},
                    "phase": phase, "function": null, "obligation": null,
                    "observation": {"kind": kind}}
            }))
            .ok()
        };
        assert!(event("focus", "phase_finished").is_some_and(|e| actionable(&e)));
        assert!(event("full", "phase_finished").is_some_and(|e| !actionable(&e)));
        assert!(event("focus", "phase_started").is_some_and(|e| !actionable(&e)));
    }

    #[test]
    fn a_hung_command_is_stopped_at_its_deadline() {
        let dir = std::env::temp_dir().join(format!("sail-bench-hang-{}", std::process::id()));
        let env = Env {
            cache: dir.join("cache"),
            socket: dir.join("s.sock"),
            rust_toolchain: "none".to_owned(),
        };
        let call = Call {
            name: "hang",
            binary: Path::new("/bin/sleep"),
            args: vec!["30".to_owned()],
            cwd: Path::new("/"),
            env: &env,
            dir: dir.join("hang"),
            timeout: Duration::from_millis(300),
            observe: false,
        };
        let started = Instant::now();
        let r = call.invoke();
        assert!(r.phase.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(r.document.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
