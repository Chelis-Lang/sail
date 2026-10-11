//! Helpers for the fault-injection tests: a real daemon, a fault-injecting
//! proxy in front of it, shim sessions with deadlines, and checks against
//! direct runs of the pinned Z3.

use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sail_api::protocol::{Request, read_response};
use sail_cache::DiskCache;
use sail_key::{Mode, PrefixHasher, QueryKey, SolverIdentity};
use sail_session::Session;
use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, ItemKind, ResponseSplitter};

/// The recorded Verus session used throughout.
pub const TOY: &[u8] = include_bytes!("../../../sail-smtlib/tests/fixtures/verus-toy.in.smt2");

/// How long any single session may take before it counts as hung.
pub const DEADLINE: Duration = Duration::from_secs(60);

/// The pinned Z3 named by `SAIL_TEST_Z3`.
pub fn z3() -> PathBuf {
    match std::env::var("SAIL_TEST_Z3") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => unreachable!("set SAIL_TEST_Z3 to the pinned z3"),
    }
}

/// A scratch directory, removed on drop. Short, because Unix socket paths
/// are limited to about 100 bytes.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> Self {
        let d = PathBuf::from(format!("/tmp/sail-f{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(std::fs::create_dir_all(&d).is_ok());
        Self(d)
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_script(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    assert!(std::fs::write(path, format!("#!/bin/sh\n{body}")).is_ok());
    assert!(std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).is_ok());
}

/// A solver wrapper that logs `<pid> <args>` for each start, then execs the
/// real solver (so the logged pid is the solver's).
pub fn logging_solver(dir: &Path, real: &Path) -> PathBuf {
    let path = dir.join("z3");
    write_script(
        &path,
        &format!(
            "echo \"$$ $*\" >> '{}'\nexec '{}' \"$@\"\n",
            dir.join("starts.log").display(),
            real.display()
        ),
    );
    path
}

/// A fake solver that answers `--version` like `real`, and otherwise prints
/// the first `cut` bytes of `canned` and dies by SIGKILL, mid-reply.
pub fn dying_solver(dir: &Path, real: &Path, canned: &[u8], cut: usize) -> PathBuf {
    let out = dir.join("canned.out");
    assert!(std::fs::write(&out, canned).is_ok());
    let path = dir.join("z3-dying");
    write_script(
        &path,
        &format!(
            "echo \"$$ $*\" >> '{log}'\n\
             if [ \"$1\" = --version ]; then exec '{real}' --version; fi\n\
             head -c {cut} '{out}'\n\
             kill -9 $$\n",
            log = dir.join("starts.log").display(),
            real = real.display(),
            out = out.display(),
        ),
    );
    path
}

/// Pids of solver sessions (`-smt2 -in`) started through a wrapper.
pub fn solver_pids(dir: &Path) -> Vec<u32> {
    std::fs::read_to_string(dir.join("starts.log"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.strip_suffix(" -smt2 -in"))
        .filter_map(|pid| pid.trim().parse().ok())
        .collect()
}

/// Waits until `cond` holds, or panics after `limit`.
pub fn wait_until(limit: Duration, what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < limit, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The socket of the daemon serving each cache root, for [`drain`].
static DAEMONS: Mutex<Vec<(PathBuf, PathBuf)>> = Mutex::new(Vec::new());

/// Waits until the daemon serving `cache` (if this test started one) has
/// written every queued entry to disk. `PUT`s are written behind
/// (ADR 0015), so a session can end before its entries reach disk.
pub fn drain(cache: &Path) {
    let socket = DAEMONS.lock().ok().and_then(|d| {
        d.iter()
            .find(|(c, _)| c.as_path() == cache)
            .map(|(_, s)| s.clone())
    });
    let Some(socket) = socket else { return };
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        let queued = sail_api::client::Client::connect(&socket)
            .and_then(|mut c| c.info())
            .map(|i| i.queue.entries);
        match queued {
            Ok(0) | Err(_) => return,
            Ok(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

/// Starts the real daemon in-process on `socket`, serving `cache`.
pub fn start_daemon(socket: &Path, cache: &Path) {
    start_daemon_with(socket, cache, sail_daemon::Config::default());
}

/// [`start_daemon`] with `config`.
pub fn start_daemon_with(socket: &Path, cache: &Path, config: sail_daemon::Config) {
    if let Ok(mut d) = DAEMONS.lock() {
        d.push((cache.to_path_buf(), socket.to_path_buf()));
    }
    let (s, c) = (socket.to_path_buf(), cache.to_path_buf());
    let counters = Arc::new(sail_daemon::Stats::default());
    std::thread::spawn(move || sail_daemon::serve_with(&s, &c, counters, &config));
    wait_until(Duration::from_secs(5), "the daemon socket", || {
        UnixStream::connect(socket).is_ok()
    });
}

/// Which protocol operation a fault applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Hello,
    Get,
    Put,
    Stats,
    Info,
}

impl Op {
    fn of(r: &Request) -> Self {
        match r {
            Request::Hello(_) => Self::Hello,
            Request::Get(_) => Self::Get,
            Request::Put { .. } => Self::Put,
            Request::Stats => Self::Stats,
            Request::Info => Self::Info,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// What the proxy does when the `nth` request (1-based, counted across all
/// connections) of operation `op` arrives.
#[derive(Debug, Clone, Copy)]
pub enum Fault {
    /// The daemon dies: that request and every later one get no reply (the
    /// connection is closed), and the socket disappears.
    Die { op: Op, nth: u64 },
    /// The daemon dies halfway through writing that request's reply.
    DieMidReply { op: Op, nth: u64 },
    /// The daemon hangs: that request and every later one are never
    /// answered, but connections stay open.
    Hang { op: Op, nth: u64 },
    /// That one reply is delayed by `by` (longer than the client timeout),
    /// then sent.
    Delay { op: Op, nth: u64, by: Duration },
}

impl Fault {
    fn hits(&self, op: Op, n: u64) -> bool {
        match *self {
            Self::Die { op: o, nth }
            | Self::DieMidReply { op: o, nth }
            | Self::Hang { op: o, nth }
            | Self::Delay { op: o, nth, .. } => o == op && n == nth,
        }
    }
}

/// A fault-injecting proxy in front of the real daemon.
pub struct Proxy {
    counts: [AtomicU64; 5],
    dead: AtomicBool,
    hung: AtomicBool,
    triggered: AtomicBool,
    fault: Fault,
    listen: PathBuf,
    upstream: PathBuf,
}

impl Proxy {
    pub fn start(listen: &Path, upstream: &Path, fault: Fault) -> Arc<Self> {
        let _ = std::fs::remove_file(listen);
        let listener = match UnixListener::bind(listen) {
            Ok(l) => l,
            Err(e) => unreachable!("cannot bind {}: {e}", listen.display()),
        };
        let proxy = Arc::new(Self {
            counts: Default::default(),
            dead: AtomicBool::new(false),
            hung: AtomicBool::new(false),
            triggered: AtomicBool::new(false),
            fault,
            listen: listen.to_path_buf(),
            upstream: upstream.to_path_buf(),
        });
        let p = Arc::clone(&proxy);
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(conn) = conn else { continue };
                if p.dead.load(Ordering::SeqCst) {
                    continue; // dropped: the peer sees end of stream
                }
                let p = Arc::clone(&p);
                std::thread::spawn(move || p.relay(conn));
            }
        });
        proxy
    }

    /// Whether the fault has fired.
    pub fn triggered(&self) -> bool {
        self.triggered.load(Ordering::SeqCst)
    }

    fn die(&self) {
        self.dead.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_file(&self.listen);
    }

    fn hang_forever() {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    }

    fn relay(&self, mut conn: UnixStream) {
        let Ok(mut up) = UnixStream::connect(&self.upstream) else {
            return;
        };
        loop {
            let Ok(Some(req)) = Request::read(&mut conn) else {
                return;
            };
            if self.dead.load(Ordering::SeqCst) {
                return;
            }
            if self.hung.load(Ordering::SeqCst) {
                Self::hang_forever();
            }
            let op = Op::of(&req);
            let n = self.counts[op.index()].fetch_add(1, Ordering::SeqCst) + 1;
            let fire = self.fault.hits(op, n);
            if fire {
                self.triggered.store(true, Ordering::SeqCst);
                match self.fault {
                    Fault::Die { .. } => {
                        self.die();
                        return;
                    }
                    Fault::Hang { .. } => {
                        self.hung.store(true, Ordering::SeqCst);
                        Self::hang_forever();
                    }
                    Fault::DieMidReply { .. } | Fault::Delay { .. } => {}
                }
            }
            if req.write(&mut up).is_err() {
                return;
            }
            let Ok(body) = read_response(&mut up) else {
                return;
            };
            let Ok(len) = u32::try_from(body.len()) else {
                return;
            };
            let mut frame = len.to_be_bytes().to_vec();
            frame.extend_from_slice(&body);
            match self.fault {
                Fault::DieMidReply { .. } if fire => {
                    let half = frame.len() / 2;
                    let _ = conn.write_all(frame.get(..half).unwrap_or_default());
                    self.die();
                    return;
                }
                Fault::Delay { by, .. } if fire => std::thread::sleep(by),
                _ => {}
            }
            if conn.write_all(&frame).is_err() {
                return;
            }
        }
    }
}

/// Report cleanup metadata only after the child has been killed/reaped.
/// A reaped PID cannot authorize a later proc census: it may be reused.
fn deadline_snapshot(owner: u32, bytes: Option<usize>, reader_finished: bool, child_reaped: bool) {
    let metadata = format!(
        "fault-session cleanup: owner={owner}, child_reaped={child_reaped}, stdout_bytes={bytes:?}, reader_finished={reader_finished}\n"
    );
    // Bypass libtest capture, after child cleanup has already completed.
    let _ = std::io::stderr().write_all(metadata.as_bytes());
}

/// A running `sail-z3 -smt2 -in`, with its output collected as it arrives.
pub struct Shim {
    child: Child,
    stdin: Option<ChildStdin>,
    out: Arc<Mutex<Vec<u8>>>,
    reader: Option<JoinHandle<bool>>,
    started: Instant,
}

/// How a session ended.
pub struct Ended {
    /// `None` on a deadline or incomplete output/reader cleanup.
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    /// False when the reader could not retire or the buffer was unavailable.
    /// Such a session must never be accepted as a complete reply stream.
    pub output_complete: bool,
    pub elapsed: Duration,
}

impl Shim {
    pub fn spawn(solver: &Path, socket: Option<&Path>, env: &[(&str, &str)]) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_sail-z3"));
        cmd.args(["-smt2", "-in"])
            .env("SAIL_Z3_PATH", solver)
            .env_remove("SAIL_TRACE_DIR")
            .env_remove("SAIL_DAEMON_SOCKET")
            .env_remove("SAIL_DAEMON_TIMEOUT_MS")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(s) = socket {
            cmd.env("SAIL_DAEMON_SOCKET", s);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => unreachable!("cannot start sail-z3: {e}"),
        };
        Self::from_child(child)
    }

    fn from_child(mut child: Child) -> Self {
        let stdin = child.stdin.take();
        let mut stdout = child.stdout.take();
        let out = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&out);
        let reader = std::thread::spawn(move || {
            let Some(s) = stdout.as_mut() else {
                return false;
            };
            let mut buf = [0u8; 8192];
            loop {
                match s.read(&mut buf) {
                    Ok(0) => return true,
                    Ok(n) => {
                        let Ok(mut out) = sink.lock() else {
                            return false;
                        };
                        out.extend_from_slice(buf.get(..n).unwrap_or_default());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => return false,
                }
            }
        });
        Self {
            child,
            stdin,
            out,
            reader: Some(reader),
            started: Instant::now(),
        }
    }

    /// Writes `input` on a helper thread (the shim may stop reading).
    pub fn feed(&mut self, input: &[u8], close: bool) {
        let Some(mut stdin) = self.stdin.take() else {
            return;
        };
        let input = input.to_vec();
        let h = std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
            let _ = stdin.flush();
            (!close).then_some(stdin)
        });
        if !close {
            // Keep stdin open, as Verus does while it waits for a reply.
            self.stdin = h.join().ok().flatten();
        }
    }

    pub fn output(&self) -> Vec<u8> {
        self.out.lock().map(|o| o.clone()).unwrap_or_default()
    }

    /// Waits for the shim to exit, killing it at `limit` (a hang).
    pub fn wait(mut self, limit: Duration) -> Ended {
        let mut status = None;
        while self.started.elapsed() < limit {
            if let Ok(Some(s)) = self.child.try_wait() {
                status = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        // A clean child exit still requires EOF and complete reader delivery
        // within the original session deadline. Normal sessions keep their
        // full remaining deadline rather than a shorter new drain budget.
        while status.is_some()
            && self.reader.as_ref().is_some_and(|r| !r.is_finished())
            && self.started.elapsed() < limit
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut child_reaped = status.is_some();
        let timed_out = status.is_none() || self.reader.as_ref().is_some_and(|r| !r.is_finished());
        if timed_out {
            status = None;
            // Cleanup must precede diagnostics: the reader can be stalled
            // while owning the sink, and a blocking snapshot would prevent
            // the original deadline from ever killing/reaping the child.
            let _ = self.child.kill();
            child_reaped = self.child.wait().is_ok();
        }
        drop(self.stdin.take());
        // A descendant may retain stdout, or the reader may be stuck. This
        // is cleanup grace, not extra solver time; the original `limit` is
        // unchanged. Only join a finished reader, and never accept an
        // incomplete stream as a successful session.
        let retirement_started = Instant::now();
        while self.reader.as_ref().is_some_and(|r| !r.is_finished())
            && retirement_started.elapsed() < Duration::from_secs(1)
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        let reader_finished = self.reader.as_ref().is_none_or(JoinHandle::is_finished);
        let reader_joined = self.reader.take().is_none_or(|r| {
            if reader_finished {
                r.join().is_ok_and(|eof| eof)
            } else {
                // Rust cannot cancel a blocked thread. Dropping the handle
                // detaches it; report incomplete cleanup rather than claiming
                // it was joined. The test process owns its remaining lifetime.
                false
            }
        });
        let stdout = self.out.try_lock().ok().map(|out| out.clone());
        let output_complete = child_reaped && reader_joined && stdout.is_some();
        if timed_out || !output_complete {
            deadline_snapshot(
                self.child.id(),
                stdout.as_ref().map(Vec::len),
                reader_finished,
                child_reaped,
            );
        }
        Ended {
            status: if output_complete { status } else { None },
            stdout: stdout.unwrap_or_default(),
            output_complete,
            elapsed: self.started.elapsed(),
        }
    }
}

/// Runs one whole session (stdin closed after `input`).
pub fn session(solver: &Path, socket: Option<&Path>, input: &[u8], env: &[(&str, &str)]) -> Ended {
    let mut shim = Shim::spawn(solver, socket, env);
    shim.feed(input, true);
    shim.wait(DEADLINE)
}

/// Runs a whole session and requires success.
pub fn ok_session(solver: &Path, socket: Option<&Path>, input: &[u8]) -> Vec<u8> {
    let e = session(solver, socket, input, &[]);
    assert!(
        e.output_complete,
        "session output or reader cleanup incomplete"
    );
    assert!(
        e.status.is_some_and(|s| s.success()),
        "session failed: {:?}",
        e.status
    );
    e.stdout
}

/// The pinned Z3's own output for `input`, with no shim involved.
pub fn direct(input: &[u8]) -> Vec<u8> {
    let mut child = match Command::new(z3())
        .args(["-smt2", "-in"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => unreachable!("cannot start z3: {e}"),
    };
    let mut stdin = child.stdin.take();
    let input = input.to_vec();
    let w = std::thread::spawn(move || {
        if let Some(s) = stdin.as_mut() {
            let _ = s.write_all(&input);
        }
    });
    let out = child.wait_with_output();
    let _ = w.join();
    match out {
        Ok(o) => {
            assert!(o.stderr.is_empty(), "fault fixture must be solver-silent");
            o.stdout
        }
        Err(e) => unreachable!("{e}"),
    }
}

/// One output-producing command of a session.
#[derive(Debug, Clone)]
pub struct Want {
    /// Complete silent fixture input, independently checked by direct Z3.
    pub silence_key: Option<QueryKey>,
    /// The v2 key the shim files the reply under.
    pub key: QueryKey,
    /// The command's bytes.
    pub command: Vec<u8>,
    /// Statistics replies are compared by `rlimit-count` only.
    pub stats: bool,
    /// What the pinned Z3 says, run directly.
    pub reply: Vec<u8>,
}

/// Every output-producing command of `input`, keyed exactly as the shim
/// keys it, paired with the direct-Z3 reply. Commands past the end of
/// `output` (a session cut short) get an empty reply.
pub fn replies(identity: &SolverIdentity, input: &[u8], output: &[u8]) -> Vec<Want> {
    let mut items = Vec::new();
    let mut splitter = CommandSplitter::new();
    splitter.push(input, &mut items);
    let mut prefix = PrefixHasher::new(identity, &Mode::Default);
    let mut session = Session::new();
    let mut out = Vec::new();
    for item in items {
        let end = usize::try_from(item.offset + item.bytes.len() as u64).unwrap_or(usize::MAX);
        let from = usize::try_from(prefix.len()).unwrap_or(usize::MAX);
        prefix.update(input.get(from..end).unwrap_or_default());
        let expects = if item.kind == ItemKind::List {
            session.expects_output(&item.bytes)
        } else {
            Output::Unknown
        };
        session.apply(&item.bytes);
        assert!(
            expects != Output::Unknown,
            "fixtures have no unknown commands"
        );
        if expects == Output::One {
            let stats = classify(&item.bytes).is_ok_and(|c| {
                matches!(
                    c.kind,
                    CommandKind::GetInfo {
                        keyword: b":all-statistics"
                    }
                )
            });
            out.push(Want {
                silence_key: None,
                key: prefix.key(),
                command: item.bytes,
                stats,
                reply: Vec::new(),
            });
        }
    }
    for (r, bytes) in out.iter_mut().zip(split(output)) {
        r.reply = bytes;
    }
    if !out.is_empty()
        && out
            .iter()
            .all(|r| !r.reply.is_empty() && !r.reply.starts_with(b"(error"))
        && split(output).len() == out.len()
    {
        let from = usize::try_from(prefix.len()).unwrap_or(usize::MAX);
        prefix.update(input.get(from..).unwrap_or_default());
        if let Some(last) = out.last_mut() {
            last.silence_key = Some(prefix.silence_key());
        }
    }
    out
}

/// The replies in an output stream. A trailing incomplete reply is included.
pub fn split(output: &[u8]) -> Vec<Vec<u8>> {
    let mut items = Vec::new();
    let mut s = ResponseSplitter::new();
    s.push(output, &mut items);
    items.extend(s.finish());
    let starts: Vec<usize> = items
        .iter()
        .map(|i| usize::try_from(i.offset).unwrap_or(usize::MAX))
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let e = starts.get(i + 1).copied().unwrap_or(output.len());
            output.get(s..e).unwrap_or_default().to_vec()
        })
        .collect()
}

fn rlimit(reply: &[u8]) -> Option<&[u8]> {
    let line = reply
        .split(|&b| b == b'\n')
        .find(|l| l.windows(13).any(|w| w == b":rlimit-count"))?;
    line.split(u8::is_ascii_whitespace)
        .rfind(|w| !w.is_empty())
        .map(|w| w.strip_suffix(b")").unwrap_or(w))
}

/// Whether two replies to the same command agree: exactly, or by
/// `rlimit-count` for statistics (time and memory figures vary).
pub fn same(stats: bool, a: &[u8], b: &[u8]) -> bool {
    if stats {
        rlimit(a).is_some() && rlimit(a) == rlimit(b)
    } else {
        a == b
    }
}

fn show(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    s.chars().take(200).collect()
}

/// `got` gives exactly the replies `want` (from [`replies`]) lists.
pub fn assert_equivalent(want: &[Want], got: &[u8]) {
    let got = split(got);
    assert_eq!(got.len(), want.len(), "reply count");
    for (i, (w, g)) in want.iter().zip(&got).enumerate() {
        assert!(
            same(w.stats, &w.reply, g),
            "reply {i} to {}: want {:?}, got {:?}",
            show(&w.command),
            show(&w.reply),
            show(g)
        );
    }
}

/// `got` is a prefix of the correct output: every complete reply is right
/// and a trailing partial reply is a prefix of the right one.
pub fn assert_prefix(want: &[Want], got: &[u8]) {
    let got = split(got);
    assert!(
        got.len() <= want.len(),
        "more replies than the solver gives"
    );
    let last = got.len().saturating_sub(1);
    for (i, (w, g)) in want.iter().zip(&got).enumerate() {
        let ok = same(w.stats, &w.reply, g) || (i == last && (w.stats || w.reply.starts_with(g)));
        assert!(
            ok,
            "reply {i} to {}: want {:?}, got {:?}",
            show(&w.command),
            show(&w.reply),
            show(g)
        );
    }
}

/// The solver identity the daemon reports for `solver`, as the shim sees it.
pub fn identity(socket: &Path, solver: &Path) -> SolverIdentity {
    let canonical = solver
        .canonicalize()
        .unwrap_or_else(|_| solver.to_path_buf());
    let id = match sail_api::client::Client::connect(socket)
        .and_then(|mut c| c.hello(&canonical.to_string_lossy()))
    {
        Ok(id) => id,
        Err(e) => unreachable!("hello failed: {e}"),
    };
    let mut sha = [0u8; 32];
    for (i, byte) in sha.iter_mut().enumerate() {
        *byte = id
            .binary_sha256
            .get(2 * i..2 * i + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .unwrap_or_default();
    }
    SolverIdentity {
        name: id.name,
        version: id.version,
        binary_sha256: sha,
        platform: id.platform,
    }
}

/// Every entry file in the cache, with the key its path names.
pub fn entry_files(cache: &Path) -> Vec<(PathBuf, Option<QueryKey>)> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(cache.join("v2")) else {
        return out;
    };
    for d in dirs.flatten() {
        let Ok(files) = std::fs::read_dir(d.path()) else {
            continue;
        };
        for f in files.flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            out.push((f.path(), parse_key(&name)));
        }
    }
    out.sort();
    out
}

fn parse_key(hex: &str) -> Option<QueryKey> {
    if hex.len() != 64 {
        return None;
    }
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(QueryKey(k))
}

/// The path of `key`'s entry.
pub fn entry_path(cache: &Path, key: &QueryKey) -> PathBuf {
    let k = key.to_string();
    cache.join("v2").join(k.get(..2).unwrap_or("xx")).join(k)
}

/// Replay check: every entry the cache would serve is the pinned solver's
/// reply for its key, according to `known` (every session run against this
/// cache). Returns the number of valid entries.
pub fn assert_cache_correct(cache: &Path, known: &[Want]) -> usize {
    assert_cache_correct_except(cache, known, &[])
}

/// [`assert_cache_correct`], skipping the entries at `skip`.
pub fn assert_cache_correct_except(cache: &Path, known: &[Want], skip: &[QueryKey]) -> usize {
    drain(cache);
    let by_key: HashMap<[u8; 32], &Want> = known.iter().map(|r| (r.key.0, r)).collect();
    // Read through a copy: `DiskCache::get` removes entries it rejects.
    let copy = cache.with_extension("check");
    let _ = std::fs::remove_dir_all(&copy);
    copy_dir(cache, &copy);
    let disk = DiskCache::new(&copy);
    let mut valid = 0;
    for (path, key) in entry_files(&copy) {
        let Some(key) = key else { continue };
        if skip.contains(&key) {
            continue;
        }
        let Some((meta, reply)) = disk.get(&key) else {
            continue;
        };
        if meta.command == "sail-session-silence" {
            assert!(
                known.iter().any(|r| r.silence_key == Some(key)),
                "unobserved silence certificate"
            );
            assert_eq!(reply, b"sail-session-silent/v1\n");
            continue;
        }
        let Some(want) = by_key.get(&key.0) else {
            unreachable!("{} is valid but no session asked for it", path.display())
        };
        assert!(
            same(want.stats, &want.reply, &reply),
            "cached reply for {} is wrong: want {:?}, got {:?}",
            show(&want.command),
            show(&want.reply),
            show(&reply)
        );
        valid += 1;
    }
    let _ = std::fs::remove_dir_all(&copy);
    valid
}

/// Copies a directory tree.
pub fn copy_dir(from: &Path, to: &Path) {
    assert!(std::fs::create_dir_all(to).is_ok());
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for e in entries.flatten() {
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            assert!(std::fs::copy(e.path(), &target).is_ok());
        }
    }
}

/// Exit status of death by SIGKILL, as the shim reports it.
pub fn killed(status: Option<ExitStatus>) -> bool {
    status.and_then(|s| s.code()) == Some(128 + 9)
}

/// Sends SIGKILL to `pid` with the `kill` command.
pub fn kill9(pid: u32) {
    let ok = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .is_ok_and(|s| s.success());
    assert!(ok, "kill -9 {pid} failed");
}

#[cfg(test)]
mod wait_controls {
    use super::*;

    fn scripted_solver(dir: &Scratch, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.path("solver");
        assert!(std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).is_ok());
        assert!(std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).is_ok());
        path
    }

    fn controlled_child(solver: &Path) -> Shim {
        let child = Command::new(solver)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| unreachable!("cannot start lifecycle control: {e}"));
        // Same real reader and wait implementation as Shim::spawn, without
        // a solver descendant that the control would otherwise need to own.
        Shim::from_child(child)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn timeout_reaps_child_while_actual_reader_sink_is_paused() {
        let dir = Scratch::new("wait-paused-sink");
        // Trigger output only after the sink is paused, then leave the
        // actual child alive beyond our short
        // control deadline. This changes no real solver fixture deadline.
        let solver = scripted_solver(&dir, "IFS= read -r line; printf 'ready\n'; exec sleep 60");
        let mut shim = controlled_child(&solver);
        let pid = shim.child.id();
        let sink = Arc::clone(&shim.out);
        let held_sink = sink
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        shim.feed(b"trigger\n", true);
        let (finished, outcome) = std::sync::mpsc::sync_channel(1);
        let waiter = std::thread::spawn(move || {
            let ended = shim.wait(Duration::from_millis(100));
            let _ = finished.send(ended);
        });
        // The old snapshot-before-kill path cannot send while held_sink is
        // held. Always release the pause before asserting/joining so failure
        // cannot strand this control's waiter.
        let result = outcome.recv_timeout(Duration::from_secs(3));
        let child_reaped = !PathBuf::from(format!("/proc/{pid}")).exists();
        drop(held_sink);
        assert!(waiter.join().is_ok());
        wait_until(Duration::from_secs(1), "paused reader retirement", || {
            Arc::strong_count(&sink) == 1
        });
        assert_eq!(Arc::strong_count(&sink), 1);
        let ended = result.unwrap_or_else(|e| unreachable!("cleanup blocked by sink: {e}"));
        assert!(
            child_reaped,
            "deadline must reap child before observing sink"
        );
        assert!(ended.status.is_none());
        assert!(!ended.output_complete);
        assert_eq!(
            ended.stdout,
            Vec::<u8>::new(),
            "busy buffer must be reported unavailable"
        );
    }

    #[test]
    fn clean_exit_joins_reader_and_preserves_all_output() {
        let dir = Scratch::new("wait-complete");
        let solver = scripted_solver(&dir, "printf 'complete reply\n'");
        let shim = controlled_child(&solver);
        let ended = shim.wait(Duration::from_secs(5));
        assert!(ended.status.is_some_and(|status| status.success()));
        assert!(ended.output_complete);
        assert_eq!(ended.stdout, b"complete reply\n");
    }
}
