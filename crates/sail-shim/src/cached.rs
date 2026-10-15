//! Cached sessions with a lazily started solver (ADR 0011).
//!
//! The shim answers each output-producing command from the daemon's cache,
//! keyed by the v2 prefix key, without starting a solver. On the first miss,
//! or anything unexpected, it starts the pinned Z3 and writes the *entire*
//! input received so far to it, so the solver's history is exactly what
//! plain Verus would have given it. It discards the solver's replies to the
//! commands already served from cache, and from then on forwards the
//! solver's output byte for byte. While the solver runs, replies are paired
//! with their commands and stored in the cache.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sail_api::client::Client;
use sail_api::protocol::{Effort, Identity, PutMeta};
use sail_key::{Mode, PrefixHasher, QueryKey, SolverIdentity};
use sail_session::Session;
use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, Item, ItemKind, ResponseSplitter};

/// What the output side should do with the next reply.
enum Expect {
    /// Full input reached EOF; certify it only after a quiet, complete session.
    Terminal(QueryKey),
    /// Store the reply under this key.
    Store {
        key: QueryKey,
        command: String,
        /// The command's exact bytes, for the shape check.
        text: Vec<u8>,
        /// For checks: the `:rlimit` in force (ADR 0009).
        limit: Option<u64>,
    },
    /// Pairing can no longer be trusted: stop recording.
    Stop,
}

fn parse_identity(id: &Identity) -> Option<SolverIdentity> {
    let mut sha = [0u8; 32];
    let hex = id.binary_sha256.as_bytes();
    if hex.len() != 64 {
        return None;
    }
    for (i, byte) in sha.iter_mut().enumerate() {
        let pair = std::str::from_utf8(hex.get(2 * i..2 * i + 2)?).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(SolverIdentity {
        name: id.name.clone(),
        version: id.version.clone(),
        binary_sha256: sha,
        platform: id.platform.clone(),
    })
}

fn write_out(bytes: &[u8]) -> bool {
    let mut out = std::io::stdout().lock();
    out.write_all(bytes).and_then(|()| out.flush()).is_ok()
}

/// Whether `reply` has the shape the solver gives for `command`. This is
/// defence in depth against any pairing or storage fault: a mismatch is
/// treated as a miss and the session falls back to the solver.
fn plausible(command: &[u8], reply: &[u8]) -> bool {
    use sail_smtlib::command::CommandKind;
    let Ok(c) = classify(command) else {
        return false;
    };
    let body = reply.trim_ascii();
    match c.kind {
        CommandKind::Echo => {
            echo_text(command).is_some_and(|t| reply == [t.as_slice(), b"\n"].concat())
        }
        k if k.is_check() => matches!(body, b"sat" | b"unsat" | b"unknown"),
        CommandKind::GetInfo {
            keyword: b":version",
        } => body.starts_with(b"(:version "),
        CommandKind::GetInfo {
            keyword: b":reason-unknown",
        } => body.starts_with(b"(:reason-unknown "),
        CommandKind::GetInfo {
            keyword: b":all-statistics",
        } => body.starts_with(b"(:") && body.ends_with(b")"),
        _ => !body.is_empty() && !body.starts_with(b"(error"),
    }
}

/// The text `(echo "...")` prints: the string literal's contents, with `""`
/// unescaped to `"`.
fn echo_text(command: &[u8]) -> Option<Vec<u8>> {
    let start = command.iter().position(|&b| b == b'"')? + 1;
    let end = command.iter().rposition(|&b| b == b'"')?;
    (end >= start).then(|| {
        let inner = command.get(start..end).unwrap_or_default();
        let mut out = Vec::with_capacity(inner.len());
        let mut i = 0;
        while let Some(&b) = inner.get(i) {
            out.push(b);
            i += if b == b'"' && inner.get(i + 1) == Some(&b'"') {
                2
            } else {
                1
            };
        }
        out
    })
}

/// A cached reply that must not be served: implausible for its command, or
/// a `check-sat` answer of `unknown` that
/// [`unknown_servable`](sail_api::policy::unknown_servable) rejects
/// (ADR 0009).
fn unservable(command: &[u8], reply: &[u8], effort: Option<&Effort>) -> bool {
    !plausible(command, reply)
        || (classify(command).is_ok_and(|c| c.kind.is_check())
            && reply.trim_ascii_start().starts_with(b"unknown")
            && !sail_api::policy::unknown_servable(effort))
}

/// A reply ready to store.
#[derive(Debug)]
struct Entry {
    key: QueryKey,
    command: String,
    reply: Vec<u8>,
    effort: Option<Effort>,
}

/// A `check-sat` reply held back until its effort is known: the
/// `:all-statistics` reply after it gives the `rlimit-count` delta, and a
/// `reason-unknown` reply may come in between (ADR 0009).
#[derive(Debug)]
struct HeldCheck {
    entry: Entry,
    before: Option<u64>,
}

/// The number after `:rlimit-count` in a statistics reply.
fn rlimit_count(reply: &[u8]) -> Option<u64> {
    const NEEDLE: &[u8] = b":rlimit-count";
    let at = reply.windows(NEEDLE.len()).position(|w| w == NEEDLE)?;
    let digits: Vec<u8> = reply
        .get(at + NEEDLE.len()..)?
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .take_while(u8::is_ascii_digit)
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

/// Pairs solver replies with the expectations the input side queues, in
/// order, whichever arrives first. Recording stops for good at an
/// `Expect::Stop`, a solver error reply, an implausible reply, or
/// [`Pairer::stop`].
#[derive(Default)]
struct Pairer {
    terminal: Option<QueryKey>,
    replies: VecDeque<Vec<u8>>,
    expects: VecDeque<Expect>,
    stopped: bool,
    /// The latest `rlimit-count` seen in a statistics reply.
    last_count: Option<u64>,
    held: Option<HeldCheck>,
}

impl Pairer {
    fn reply(&mut self, reply: Vec<u8>) {
        self.replies.push_back(reply);
    }

    fn expect(&mut self, expect: Expect) {
        match expect {
            Expect::Terminal(key) => self.terminal = Some(key),
            other => self.expects.push_back(other),
        }
    }

    fn stop(&mut self) {
        self.stopped = true;
    }

    /// Releases a held check, with whatever effort is known.
    fn release(&mut self, out: &mut Vec<Entry>) {
        if let Some(h) = self.held.take() {
            out.push(h.entry);
        }
    }

    /// Every entry that can be formed now and should be stored.
    fn drain(&mut self) -> Vec<Entry> {
        let mut out = Vec::new();
        // Pop only when both sides have an entry: popping one and finding the
        // other empty would drop it and shift every later pairing by one.
        while !self.stopped && !self.replies.is_empty() && !self.expects.is_empty() {
            let (Some(reply), Some(expect)) = (self.replies.pop_front(), self.expects.pop_front())
            else {
                break;
            };
            let Expect::Store {
                key,
                command,
                text,
                limit,
            } = expect
            else {
                self.stopped = true;
                break;
            };
            // Never store a reply that does not fit its command; stop
            // recording, since pairing can no longer be trusted.
            if reply.starts_with(b"(error") || !plausible(&text, &reply) {
                self.stopped = true;
                break;
            }
            let kind = classify(&text).map(|c| c.kind).ok();
            match kind {
                Some(k) if k.is_check() => {
                    self.release(&mut out);
                    self.held = Some(HeldCheck {
                        entry: Entry {
                            key,
                            command,
                            reply,
                            effort: Some(Effort {
                                limit,
                                used: None,
                                reason: None,
                            }),
                        },
                        before: self.last_count,
                    });
                    continue;
                }
                Some(CommandKind::GetInfo {
                    keyword: b":reason-unknown",
                }) => {
                    if let Some(effort) = self.held.as_mut().and_then(|h| h.entry.effort.as_mut()) {
                        effort.reason =
                            Some(String::from_utf8_lossy(reply.trim_ascii()).into_owned());
                    }
                }
                Some(CommandKind::GetInfo {
                    keyword: b":all-statistics",
                }) => {
                    let count = rlimit_count(&reply);
                    if let Some(h) = self.held.as_mut()
                        && let (Some(before), Some(after)) = (h.before, count)
                        && let Some(effort) = h.entry.effort.as_mut()
                    {
                        effort.used = Some(after.saturating_sub(before));
                    }
                    self.release(&mut out);
                    self.last_count = count.or(self.last_count);
                }
                _ => {}
            }
            out.push(Entry {
                key,
                command,
                reply,
                effort: None,
            });
        }
        out
    }

    /// At the end of the session: releases a held check.
    fn finish(&mut self) -> Vec<Entry> {
        let mut out = Vec::new();
        if !self.stopped {
            self.release(&mut out);
            if self.expects.is_empty()
                && self.replies.is_empty()
                && let Some(key) = self.terminal.take()
            {
                out.push(Entry {
                    key,
                    command: "sail-session-silence".to_owned(),
                    reply: b"sail-session-silent/v1\n".to_vec(),
                    effort: None,
                });
            }
        }
        out
    }
}

/// Where the solver's input goes.
enum Feed {
    Pipe(ChildStdin),
    /// A resumed snapshot's connection (prototype, ADR 0049 draft).
    Sock(UnixStream),
}

impl Feed {
    fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Pipe(p) => p.write_all(bytes).and_then(|()| p.flush()),
            Self::Sock(s) => s.write_all(bytes),
        }
    }

    /// Ends the solver's input.
    fn close(self) {
        if let Self::Sock(s) = self {
            let _ = s.shutdown(Shutdown::Write);
        }
    }
}

/// The solver process the output side waits for.
enum Proc {
    Child(Child),
    /// A resumed snapshot: its exit status arrives on this connection.
    Resumed(UnixStream),
}

impl Proc {
    fn wait(self) -> u8 {
        match self {
            Self::Child(mut c) => c.wait().map_or(1, |s| crate::status_code(s).1),
            Self::Resumed(s) => sail_api::snapshot::read_status(s)
                .and_then(|c| u8::try_from(c & 0xff).ok())
                .unwrap_or(1),
        }
    }
}

/// Whether snapshot markers may be sent to this solver.
enum Marking {
    Off,
    /// Until the library confirms it loaded (it creates `ready`).
    Pending {
        ready: PathBuf,
        deadline: Instant,
    },
    On,
}

/// A snapshot point seen while serving from the cache: a top-level `push`.
struct Candidate {
    /// Raw input offset of the `push`.
    offset: usize,
    /// Replies served before it.
    served: u64,
    /// The snapshot name for the input before it.
    name: String,
}

/// Snapshot state of a session (prototype, ADR 0049 draft).
struct Snap {
    cfg: sail_api::snapshot::Config,
    candidates: Vec<Candidate>,
}

/// Whether `item` is a top-level `push`: a snapshot point.
fn top_push(session: &Session, item: &Item) -> bool {
    item.kind == ItemKind::List
        && session.depth() == 0
        && matches!(
            classify(&item.bytes).map(|c| c.kind),
            Ok(CommandKind::Push(_))
        )
}

/// Records `item` as a snapshot point if it is one: a candidate to resume
/// from while serving, or a marker for a running solver.
fn note_point(
    snap: &mut Snap,
    solving: Option<&mut Solving>,
    session: &Session,
    item: &Item,
    (prefix, raw, served): (&PrefixHasher, &[u8], u64),
) {
    if !top_push(session, item) {
        return;
    }
    let offset = usize::try_from(item.offset).unwrap_or(usize::MAX);
    let name = point_name(prefix, raw, offset);
    match solving {
        Some(s) => s.marks.push_back((offset, name)),
        None => snap.candidates.push(Candidate {
            offset,
            served,
            name,
        }),
    }
}

/// The snapshot name for a process that has received `raw[..offset]`.
fn point_name(prefix: &PrefixHasher, raw: &[u8], offset: usize) -> String {
    let mut h = prefix.clone();
    let from = usize::try_from(h.len()).unwrap_or(usize::MAX);
    h.update(raw.get(from..offset).unwrap_or_default());
    sail_api::snapshot::name(&h.key())
}

struct Solving {
    stdin: Option<Feed>,
    expects: Sender<Expect>,
    /// Set once the input side has seen the end of its input.
    input_done: Arc<AtomicBool>,
    /// The output side; it owns the solver and returns its exit status.
    output: JoinHandle<u8>,
    /// Raw input bytes written to the solver so far.
    written: usize,
    /// Snapshot points not yet written: (raw offset, name).
    marks: VecDeque<(usize, String)>,
    marking: Marking,
    /// The raw offset a resumed snapshot took over at.
    resumed_at: Option<usize>,
}

/// Sends paired replies to the daemon on its own thread, so forwarding never
/// waits on the daemon. Stops at the first daemon error.
fn spawn_storer(
    mut client: Client,
    entries: Receiver<Entry>,
    quiet: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("sail-z3-store".to_owned())
        .spawn(move || {
            // No entry is published until the entire solver session is known
            // to have emitted no stderr. Replies from older warning sessions
            // otherwise freeze a missing diagnostic on warm runs.
            let mut pending = Vec::new();
            let mut bytes = 0usize;
            for entry in entries {
                bytes = bytes
                    .saturating_add(entry.reply.len())
                    .saturating_add(entry.command.len());
                if bytes > 64 * 1024 * 1024 {
                    return;
                }
                pending.push(entry);
            }
            if !quiet.load(Ordering::SeqCst) {
                return;
            }
            for entry in pending {
                let meta = PutMeta {
                    command: entry.command,
                    provenance: "pinned-solver".to_owned(),
                    effort: entry.effort,
                };
                if client.put(&entry.key.0, meta, entry.reply).is_err() {
                    // The daemon is gone: drop the remaining stores.
                    break;
                }
            }
        })
        .ok()
}

fn queue_stores(
    pairer: &mut Pairer,
    expects: &Receiver<Expect>,
    stores: &Sender<Entry>,
    wait: bool,
) {
    if wait {
        for e in expects {
            pairer.expect(e);
        }
    } else {
        for e in expects.try_iter() {
            pairer.expect(e);
        }
    }
    for entry in pairer.drain() {
        if stores.send(entry).is_err() {
            pairer.stop();
        }
    }
}

/// The output side once a solver runs: forwards replies after the first
/// `skip`, records paired replies, and owns the solver process.
///
/// When the solver's output ends it waits for the solver to exit. The reply
/// in flight at that point is stored only if the solver exited cleanly: a
/// solver that died mid-reply leaves a partial reply, which is forwarded (as
/// pass-through would) but never stored. If the input side is still reading
/// when the solver exits, nothing will ever answer the commands the client
/// is waiting on, so the shim exits with the solver's status, as
/// pass-through does.
fn output_side(
    mut from: impl Read,
    proc: Proc,
    skip: u64,
    expects: &Receiver<Expect>,
    client: Option<Client>,
    input_done: &AtomicBool,
    stderr: Option<&crate::cached_stderr::Monitor>,
) -> u8 {
    let mut all = Vec::new(); // every output byte so far
    let mut splitter = ResponseSplitter::new();
    let mut forward_from: Option<usize> = (skip == 0).then_some(0);
    let mut forwarded = 0usize;
    let mut forwarding_ok = true;
    let mut seen = 0u64; // replies completed so far
    let mut prev: Option<(u64, usize)> = None; // (index, start) of the last completed reply
    let mut buf = vec![0u8; 64 * 1024];

    // Pairs replies with expectations as either arrives. Stores go to a
    // separate thread, so forwarding never waits on the daemon.
    let mut pairer = Pairer::default();
    let (store_tx, store_rx) = std::sync::mpsc::channel::<Entry>();
    let quiet = Arc::new(AtomicBool::new(false));
    let storer = client.and_then(|client| spawn_storer(client, store_rx, Arc::clone(&quiet)));
    if storer.is_none() {
        // No daemon to store to.
        pairer.stop();
    }

    loop {
        let n = match from.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                forwarding_ok = false;
                break;
            }
        };
        if stderr.is_some_and(|stderr| !stderr.flush()) {
            forwarding_ok = false;
            break;
        }
        let chunk = buf.get(..n).unwrap_or_default();
        all.extend_from_slice(chunk);
        let mut items: Vec<Item> = Vec::new();
        splitter.push(chunk, &mut items);
        for item in items {
            let start = usize::try_from(item.offset).unwrap_or(usize::MAX);
            if let Some((k, s)) = prev.take()
                && k >= skip
            {
                pairer.reply(all.get(s..start).unwrap_or_default().to_vec());
            }
            if seen == skip && forward_from.is_none() {
                forward_from = Some(start);
            }
            prev = Some((seen, start));
            seen += 1;
        }
        if let Some(f) = forward_from
            && all.len() > forwarded.max(f)
        {
            if !write_out(all.get(forwarded.max(f)..).unwrap_or_default()) {
                forwarding_ok = false;
                break;
            }
            forwarded = all.len();
        }
        queue_stores(&mut pairer, expects, &store_tx, false);
    }
    // Stop reading first: a solver blocked writing to a closed pipe then
    // fails and exits instead of waiting forever.
    drop(from);
    // End of output. Deliver what is left, as pass-through would.
    if forward_from.is_some() && all.len() > forwarded {
        forwarding_ok &= write_out(all.get(forwarded..).unwrap_or_default());
    }
    let mut code = proc.wait();
    // The last reply runs to the end of the stream, and is complete only if
    // the solver exited cleanly.
    if let Some((k, s)) = prev.take()
        && k >= skip
        && code == 0
    {
        pairer.reply(all.get(s..).unwrap_or_default().to_vec());
    }
    let input_done = input_done.load(Ordering::SeqCst);
    // Once the input has ended, every expectation is queued or about to be.
    // Before that, more may never come, so take only what is there.
    queue_stores(&mut pairer, expects, &store_tx, input_done);
    for entry in pairer.finish() {
        let _ = store_tx.send(entry);
    }
    let silent = stderr.is_some_and(crate::cached_stderr::Monitor::quiet);
    if stderr.is_some_and(crate::cached_stderr::Monitor::failed) || !forwarding_ok {
        code = 1;
    }
    quiet.store(code == 0 && silent, Ordering::SeqCst);
    drop(store_tx);
    if let Some(h) = storer {
        let _ = h.join();
    }
    if !input_done {
        // The input side is blocked reading a client that waits for replies
        // the dead solver will never give.
        std::process::exit(i32::from(code));
    }
    code
}

/// Starts the solver and queues `first` (the expectation for the command
/// that caused the start). `client`, if any, records. Nothing is written
/// yet: the caller writes the input with [`Solving::write_to`]. With `snap`,
/// the solver runs with the snapshot library and takes snapshots at `marks`.
fn start_solver(
    solver: &Path,
    args: &[std::ffi::OsString],
    skip: u64,
    first: Expect,
    client: Option<Client>,
    snap: Option<(&sail_api::snapshot::Config, VecDeque<(usize, String)>)>,
) -> Option<Solving> {
    // Observation failure must not prevent replay: inherit solver stderr and
    // disable publication when its silence cannot be established.
    let (stderr, child_stderr) = crate::cached_stderr::Monitor::new()
        .map_or_else(|_| (None, Stdio::inherit()), |(m, io)| (Some(m), io));
    let client = stderr.as_ref().and(client);
    let mut cmd = Command::new(solver);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(child_stderr);
    let (marking, marks) = match snap {
        Some((cfg, marks)) => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let ready = cfg
                .dir
                .join(format!(".ready-{}-{nanos}", std::process::id()));
            cmd.envs(cfg.solver_env(&ready));
            (
                Marking::Pending {
                    ready,
                    deadline: Instant::now() + Duration::from_millis(300),
                },
                marks,
            )
        }
        None => (Marking::Off, VecDeque::new()),
    };
    let mut child = cmd.spawn().ok()?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    // Queued before the solver sees any input, so it precedes its reply.
    let _ = tx.send(first);
    let input_done = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&input_done);
    // If the thread cannot start, the solver is dropped with it; it exits
    // when `stdin` is dropped below.
    let output = std::thread::Builder::new()
        .name("sail-z3-cached-out".to_owned())
        .spawn(move || {
            output_side(
                stdout,
                Proc::Child(child),
                skip,
                &rx,
                client,
                &done,
                stderr.as_ref(),
            )
        })
        .ok()?;
    Some(Solving {
        stdin: Some(Feed::Pipe(stdin)),
        expects: tx,
        input_done,
        output,
        written: 0,
        marks,
        marking,
        resumed_at: None,
    })
}

/// Takes over a resumed snapshot that has received exactly `raw[..at]`.
/// On failure before anything was sent, hands `first` and `client` back
/// for a fresh start.
fn resume_solver(
    resumed: sail_api::snapshot::Resumed,
    at: usize,
    skip: u64,
    first: Expect,
    client: Option<Client>,
    marks: VecDeque<(usize, String)>,
) -> Result<Solving, (Expect, Option<Client>)> {
    let sail_api::snapshot::Resumed { io, err, status } = resumed;
    let Ok(reader) = io.try_clone() else {
        return Err((first, client));
    };
    let stderr = crate::cached_stderr::Monitor::from_stream(err).ok();
    let client = stderr.as_ref().and(client);
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = tx.send(first);
    let input_done = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&input_done);
    let output = std::thread::Builder::new()
        .name("sail-z3-cached-out".to_owned())
        .spawn(move || {
            output_side(
                reader,
                Proc::Resumed(status),
                skip,
                &rx,
                client,
                &done,
                stderr.as_ref(),
            )
        });
    let Ok(output) = output else {
        // The expectation went with the channel: a fresh start records nothing.
        return Err((Expect::Stop, None));
    };
    Ok(Solving {
        stdin: Some(Feed::Sock(io)),
        expects: tx,
        input_done,
        output,
        written: at,
        marks,
        marking: Marking::On,
        resumed_at: Some(at),
    })
}

/// Starts a solver for a session that has received `raw` and been served
/// `served` replies: from the latest parked snapshot of a prefix of `raw` if
/// there is one, otherwise a fresh solver (taking snapshots, with `snap`).
/// Writes `raw[..upto]` to it.
fn start(
    solver: &Path,
    args: &[std::ffi::OsString],
    (raw, upto): (&[u8], usize),
    served: u64,
    first: Expect,
    client: Option<Client>,
    snap: Option<&Snap>,
) -> Option<Solving> {
    let mut first = first;
    let mut client = client;
    let mut s = None;
    if let Some(snap) = snap {
        for (i, c) in snap.candidates.iter().enumerate().rev() {
            let Ok(r) = sail_api::snapshot::resume(&snap.cfg.dir, &c.name) else {
                continue;
            };
            let marks = snap
                .candidates
                .get(i + 1..)
                .unwrap_or_default()
                .iter()
                .map(|c| (c.offset, c.name.clone()))
                .collect();
            let skip = served.saturating_sub(c.served);
            match resume_solver(r, c.offset, skip, first, client, marks) {
                Ok(solving) => {
                    s = Some(solving);
                    first = Expect::Stop;
                    client = None;
                }
                Err((f, cl)) => {
                    first = f;
                    client = cl;
                }
            }
            break;
        }
    }
    let mut s = match (s, snap) {
        (Some(s), _) => s,
        (None, Some(snap)) => {
            snap.cfg.evict();
            let marks = snap
                .candidates
                .iter()
                .map(|c| (c.offset, c.name.clone()))
                .collect();
            start_solver(
                solver,
                args,
                served,
                first,
                client,
                Some((&snap.cfg, marks)),
            )?
        }
        (None, None) => start_solver(solver, args, served, first, client, None)?,
    };
    s.write_to(raw, upto);
    Some(s)
}

impl Solving {
    fn send(&mut self, bytes: &[u8]) {
        if let Some(stdin) = &mut self.stdin
            && stdin.send(bytes).is_err()
        {
            self.stdin = None;
        }
    }

    /// Whether markers may be sent; waits briefly for the library to confirm
    /// that it loaded, since an unstripped marker would reach the solver.
    fn marking_on(&mut self) -> bool {
        loop {
            match &self.marking {
                Marking::On => return true,
                Marking::Off => return false,
                Marking::Pending { ready, deadline } => {
                    if ready.exists() {
                        let _ = std::fs::remove_file(ready);
                        self.marking = Marking::On;
                    } else if Instant::now() >= *deadline {
                        self.marking = Marking::Off;
                    } else {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
            }
        }
    }

    /// Writes `raw[written..end]`, with a snapshot marker before each point.
    fn write_to(&mut self, raw: &[u8], end: usize) {
        loop {
            while self.marks.front().is_some_and(|(o, _)| *o < self.written) {
                self.marks.pop_front();
            }
            let stop = match self.marks.front() {
                Some((o, _)) if *o < end => *o,
                _ => end,
            };
            if stop > self.written {
                let bytes = raw.get(self.written..stop).unwrap_or_default().to_vec();
                self.send(&bytes);
                self.written = stop;
            }
            match self.marks.front() {
                Some((o, _)) if *o == self.written && *o < end => {
                    if let Some((_, name)) = self.marks.pop_front()
                        && self.marking_on()
                    {
                        self.send(&sail_api::snapshot::marker(&name));
                    }
                }
                _ => break,
            }
        }
    }

    fn finish(mut self, terminal: QueryKey) -> ExitCode {
        self.input_done.store(true, Ordering::SeqCst);
        let _ = self.expects.send(Expect::Terminal(terminal));
        if let Some(feed) = self.stdin.take() {
            feed.close();
        }
        if let Marking::Pending { ready, .. } = &self.marking {
            let _ = std::fs::remove_file(ready);
        }
        drop(self.expects);
        ExitCode::from(self.output.join().unwrap_or(1))
    }
}

/// The client timeout: `SAIL_DAEMON_TIMEOUT_MS` if set to a positive number
/// of milliseconds, otherwise [`sail_api::client::TIMEOUT`].
fn daemon_timeout() -> Duration {
    std::env::var(crate::ENV_DAEMON_TIMEOUT_MS)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&ms| ms > 0)
        .map_or(sail_api::client::TIMEOUT, Duration::from_millis)
}

/// Connects without consuming input and binds the explicit verification mode.
fn connect(socket: &Path, solver: &Path) -> Option<(Client, PrefixHasher)> {
    let mode = match std::env::var("SAIL_SOLVER_MODE").as_deref() {
        Ok("accept-first-proof") => sail_api::protocol::VerificationMode::AcceptFirstProof,
        Ok("default") | Err(std::env::VarError::NotPresent) => {
            sail_api::protocol::VerificationMode::Default
        }
        _ => return None,
    };
    let mut client = Client::connect_with_timeout(socket, daemon_timeout()).ok()?;
    let identity = parse_identity(&client.hello_in_mode(&solver.to_string_lossy(), mode).ok()?)?;
    let key_mode = match mode {
        sail_api::protocol::VerificationMode::Default => Mode::Default,
        sail_api::protocol::VerificationMode::AcceptFirstProof => {
            Mode::OptIn(mode.name().to_owned())
        }
    };
    let prefix = PrefixHasher::new(&identity, &key_mode);
    Some((client, prefix))
}

fn observe(
    session: &mut Session,
    item: &Item,
    prefix: &mut PrefixHasher,
    raw: &[u8],
) -> (Output, String, Expect) {
    let end = usize::try_from(item.offset + item.bytes.len() as u64).unwrap_or(usize::MAX);
    let from = usize::try_from(prefix.len()).unwrap_or(usize::MAX);
    prefix.update(raw.get(from..end).unwrap_or_default());
    let mut output = if item.kind == ItemKind::List {
        session.expects_output(&item.bytes)
    } else {
        Output::Unknown
    };
    session.apply(&item.bytes);
    // print-success changes the option command's own output as well as later
    // replies. Its non-default protocol always replays live, without recording.
    if session
        .option(b":print-success")
        .is_some_and(|v| v != b"false")
    {
        output = Output::Unknown;
    }
    let name = classify(&item.bytes)
        .map(|c| String::from_utf8_lossy(c.name).into_owned())
        .unwrap_or_default();
    let limit = session
        .option(b":rlimit")
        .and_then(|v| std::str::from_utf8(v).ok())
        .and_then(|v| v.parse::<u64>().ok());
    let expect = match output {
        Output::One => Expect::Store {
            key: prefix.key(),
            command: name.clone(),
            text: item.bytes.clone(),
            limit,
        },
        _ => Expect::Stop,
    };
    (output, name, expect)
}

/// The servable cached reply for `command` under `key`, if any. A daemon
/// error drops the client for the rest of the session.
fn lookup(client: &mut Option<Client>, key: &QueryKey, command: &[u8]) -> Option<Vec<u8>> {
    match client.as_mut().map(|c| c.get_with_effort(&key.0)) {
        Some(Ok(found)) => found
            .filter(|(r, e)| !unservable(command, r, e.as_ref()))
            .map(|(r, _)| r),
        Some(Err(_)) => {
            *client = None;
            None
        }
        None => None,
    }
}

/// Runs a cached session. Returns `None`, having consumed no input, if the
/// daemon cannot be reached or cannot identify the solver; the caller then
/// runs plain pass-through.
///
/// Any daemon error after that drops the connection for the rest of the
/// session: the command becomes a miss, and the session continues on the
/// solver without recording. A connection that failed mid-exchange is never
/// reused, so a late reply can never be read as the answer to a later
/// request.
pub(crate) fn run(socket: &Path, solver: &Path, args: &[std::ffi::OsString]) -> Option<ExitCode> {
    let (client, mut prefix) = connect(socket, solver)?;
    let mut session = Session::new();
    let mut splitter = CommandSplitter::new();
    let mut raw: Vec<u8> = Vec::new();
    let mut served = 0u64;
    let mut solving: Option<Solving> = None;
    let mut client = Some(client);
    let mut snap = sail_api::snapshot::Config::from_env().map(|cfg| Snap {
        cfg,
        candidates: Vec::new(),
    });
    let mut stdin = std::io::stdin().lock();
    let mut buf = vec![0u8; 64 * 1024];

    loop {
        let n = match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let chunk = buf.get(..n).unwrap_or_default().to_vec();
        raw.extend_from_slice(&chunk);
        let mut items = Vec::new();
        splitter.push(&chunk, &mut items);
        // Expectations for commands in this chunk are queued before the
        // chunk reaches the solver.
        for item in items {
            if let Some(snap) = &mut snap {
                note_point(
                    snap,
                    solving.as_mut(),
                    &session,
                    &item,
                    (&prefix, &raw, served),
                );
            }
            let (expects, name, expect) = observe(&mut session, &item, &mut prefix, &raw);
            if name == "exit" && closes_session(&item.bytes) {
                if let Some(s) = &solving
                    && expects != Output::None
                {
                    let _ = s.expects.send(expect);
                }
                if let Some(s) = &mut solving {
                    s.write_to(&raw, raw.len());
                }
                return finish_run(
                    solving,
                    client,
                    solver,
                    args,
                    (&raw, served),
                    &mut prefix,
                    (true, snap.as_ref()),
                );
            }
            if let Some(s) = &solving {
                if expects != Output::None {
                    let _ = s.expects.send(expect);
                }
                continue;
            }
            let hit = match expects {
                Output::None => continue,
                Output::One => lookup(&mut client, &prefix.key(), &item.bytes),
                Output::Unknown => None,
            };
            if let Some(reply) = hit {
                if !write_out(&reply) {
                    return Some(ExitCode::SUCCESS);
                }
                served += 1;
                continue;
            }
            // Miss: start the solver (or resume a snapshot) with the input
            // so far; the rest of the chunk follows below.
            let end = usize::try_from(item.offset + item.bytes.len() as u64).unwrap_or(raw.len());
            let Some(s) = start(
                solver,
                args,
                (&raw, end.min(raw.len())),
                served,
                expect,
                client.take(),
                snap.as_ref(),
            ) else {
                return Some(ExitCode::from(1));
            };
            solving = Some(s);
        }
        if let Some(s) = &mut solving {
            s.write_to(&raw, raw.len());
        }
    }
    // At the end of input while serving, incomplete trailing input goes to
    // the solver, which decides what it means.
    finish_run(
        solving,
        client,
        solver,
        args,
        (&raw, served),
        &mut prefix,
        (splitter.finish().is_none(), snap.as_ref()),
    )
}

fn closes_session(command: &[u8]) -> bool {
    use sail_smtlib::lex::{Lexer, TokenKind};
    let mut tokens = Lexer::new(command);
    let _ = tokens.next();
    let _ = tokens.next();
    matches!(tokens.next(), Some(Ok(t)) if t.kind == TokenKind::Close) && tokens.next().is_none()
}

fn finish_run(
    solving: Option<Solving>,
    client: Option<Client>,
    solver: &Path,
    args: &[std::ffi::OsString],
    (raw, served): (&[u8], u64),
    prefix: &mut PrefixHasher,
    (complete, snap): (bool, Option<&Snap>),
) -> Option<ExitCode> {
    let from = usize::try_from(prefix.len()).unwrap_or(usize::MAX);
    prefix.update(raw.get(from..).unwrap_or_default());
    let end = SessionEnd {
        raw,
        served,
        terminal: prefix.silence_key(),
        complete,
    };
    let (code, started, resumed_at) = finish_input(solving, client, solver, args, &end, snap);
    record_provenance(raw, end.served, started, resumed_at);
    code
}

struct SessionEnd<'a> {
    raw: &'a [u8],
    served: u64,
    terminal: QueryKey,
    complete: bool,
}

fn finish_input(
    solving: Option<Solving>,
    mut client: Option<Client>,
    solver: &Path,
    args: &[std::ffi::OsString],
    end: &SessionEnd<'_>,
    snap: Option<&Snap>,
) -> (Option<ExitCode>, bool, Option<usize>) {
    let certified = solving.is_none()
        && end.complete
        && client.as_mut().is_some_and(|c| {
            c.get(&end.terminal.0)
                .ok()
                .flatten()
                .is_some_and(|r| r == b"sail-session-silent/v1\n")
        });
    let solving = match solving {
        Some(s) => Some(s),
        None if !certified => start(
            solver,
            args,
            (end.raw, end.raw.len()),
            end.served,
            Expect::Stop,
            client.take(),
            snap,
        ),
        None => return (Some(ExitCode::SUCCESS), false, None),
    };
    match solving {
        Some(s) => {
            let at = s.resumed_at;
            (Some(s.finish(end.terminal)), true, at)
        }
        None => (None, true, None),
    }
}

/// When `SAIL_PROVENANCE_DIR` is set, writes `<dir>/<pid>-<nanos>.json`
/// saying how many replies the cache served before a solver started, keyed
/// by the SHA-256 of the session's whole input (ADR 0014). `sail verify
/// --output json` joins it to Verus's transcript of the same process.
///
/// Best effort, and only after every reply has been forwarded and the
/// solver has exited: nothing here can affect what the client receives.
fn record_provenance(input: &[u8], served: u64, solver_started: bool, resumed_at: Option<usize>) {
    let Some(dir) = std::env::var_os(crate::ENV_PROVENANCE_DIR).filter(|v| !v.is_empty()) else {
        return;
    };
    let digest = sail_key::sha256(input);
    let hex = digest.iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    });
    let record = serde_json::json!({
        "version": 1,
        "input_sha256": hex,
        "replies_served": served,
        "solver_started": solver_started,
        "snapshot_resumed_at": resumed_at,
    });
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = Path::new(&dir);
    let name = format!("{}-{nanos}", std::process::id());
    let tmp = dir.join(format!("{name}.tmp"));
    if std::fs::write(&tmp, record.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join(format!("{name}.json")));
    }
}

#[cfg(test)]
mod tests {
    use super::{Expect, Pairer};
    use proptest::prelude::*;
    use sail_key::QueryKey;

    fn key(i: u8) -> QueryKey {
        QueryKey([i; 32])
    }

    fn store(i: u8, text: &str) -> Expect {
        let command = text
            .trim_start_matches('(')
            .split([' ', ')'])
            .next()
            .unwrap_or_default()
            .to_owned();
        Expect::Store {
            key: key(i),
            command,
            text: text.as_bytes().to_vec(),
            limit: None,
        }
    }

    fn model(i: u8) -> Expect {
        store(i, "(get-model)")
    }

    proptest! {
        /// Whatever order replies and expectations arrive in, and however
        /// often pairs are drained, reply `i` is stored under key `i`.
        #[test]
        fn pairing_is_order_independent(order in prop::collection::vec(any::<bool>(), 0..80), drains in prop::collection::vec(any::<bool>(), 0..80)) {
            let n = order.iter().filter(|b| **b).count().min(order.len() - order.iter().filter(|b| **b).count());
            let mut p = Pairer::default();
            let (mut r, mut e) = (0u8, 0u8);
            let mut stored = Vec::new();
            for (i, is_reply) in order.iter().enumerate() {
                if *is_reply && usize::from(r) < n {
                    p.reply(vec![b'm', r]);
                    r += 1;
                } else if usize::from(e) < n {
                    p.expect(model(e));
                    e += 1;
                }
                if drains.get(i).copied().unwrap_or(false) {
                    stored.extend(p.drain());
                }
            }
            while usize::from(r) < n { p.reply(vec![b'm', r]); r += 1; }
            while usize::from(e) < n { p.expect(model(e)); e += 1; }
            stored.extend(p.drain());
            prop_assert_eq!(stored.len(), n);
            for entry in &stored {
                prop_assert_eq!(entry.key.0[0], entry.reply[1]);
            }
        }
    }

    #[test]
    fn errors_stop_recording() {
        let mut p = Pairer::default();
        p.expect(store(1, "(check-sat)"));
        p.reply(b"unknown\n".to_vec());
        p.expect(store(2, "(get-model)"));
        p.reply(b"(model)\n".to_vec());
        p.expect(store(3, "(echo \"<<DONE>>\")"));
        p.reply(b"(error \"x\")\n".to_vec());
        p.expect(store(4, "(echo \"<<DONE>>\")"));
        p.reply(b"<<DONE>>\n".to_vec());
        // The `unknown` check is held for its effort when recording stops,
        // so only the model is stored.
        let stored = p.drain();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].key, key(2));
        assert!(p.finish().is_empty());
    }

    fn check(i: u8, limit: u64) -> Expect {
        match store(i, "(check-sat)") {
            Expect::Store {
                key, command, text, ..
            } => Expect::Store {
                key,
                command,
                text,
                limit: Some(limit),
            },
            other => other,
        }
    }

    #[test]
    fn checks_carry_effort_and_reason() {
        let mut p = Pairer::default();
        p.expect(store(1, "(get-info :all-statistics)"));
        p.reply(b"(:rlimit-count 100)\n".to_vec());
        p.expect(check(2, 30_000_000));
        p.reply(b"unknown\n".to_vec());
        p.expect(store(3, "(get-info :reason-unknown)"));
        p.reply(b"(:reason-unknown \"incomplete quantifiers\")\n".to_vec());
        p.expect(store(4, "(get-info :all-statistics)"));
        p.reply(b"(:rlimit-count 160)\n".to_vec());
        let stored = p.drain();
        let keys: Vec<u8> = stored.iter().map(|e| e.key.0[0]).collect();
        assert_eq!(keys, [1, 3, 2, 4]);
        let effort = stored
            .iter()
            .find(|e| e.key == key(2))
            .and_then(|e| e.effort.clone());
        assert_eq!(
            effort,
            Some(super::Effort {
                limit: Some(30_000_000),
                used: Some(60),
                reason: Some("(:reason-unknown \"incomplete quantifiers\")".to_owned()),
            })
        );
    }

    #[test]
    fn a_held_check_is_released_at_the_end() {
        let mut p = Pairer::default();
        p.expect(check(1, 10));
        p.reply(b"unsat\n".to_vec());
        assert!(p.drain().is_empty());
        let rest = p.finish();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].effort.as_ref().and_then(|e| e.used), None);
    }

    #[test]
    fn implausible_replies_stop_recording() {
        // The bug this guards against: a statistics reply paired with an echo.
        let mut p = Pairer::default();
        p.expect(store(1, "(echo \"<<DONE>>\")"));
        p.reply(b"(:rlimit-count 5)\n".to_vec());
        p.expect(store(2, "(get-model)"));
        p.reply(b"(model)\n".to_vec());
        assert_eq!(p.drain().len(), 0);
    }

    #[test]
    fn unknown_policy() {
        use super::{Effort, unservable};
        let e = |reason: &str, used: u64, limit: u64| Effort {
            limit: Some(limit),
            used: Some(used),
            reason: Some(format!("(:reason-unknown \"{reason}\")")),
        };
        let check = b"(check-sat)".as_slice();
        // Far from the limit, gave up for its own reasons: servable.
        assert!(!unservable(
            check,
            b"unknown\n",
            Some(&e("(incomplete quantifiers)", 2_000, 30_000_000))
        ));
        // Resource exhaustion, or near the limit: never served.
        assert!(unservable(
            check,
            b"unknown\n",
            Some(&e("canceled", 30_000_000, 30_000_000))
        ));
        assert!(unservable(
            check,
            b"unknown\n",
            Some(&e("(incomplete quantifiers)", 16_000_000, 30_000_000))
        ));
        // No effort recorded, or an unlimited check: never served.
        assert!(unservable(check, b"unknown\n", None));
        assert!(unservable(
            check,
            b"unknown\n",
            Some(&e("(incomplete quantifiers)", 5, 0))
        ));
        // Other answers are unaffected by effort.
        assert!(!unservable(check, b"unsat\n", None));
    }

    #[test]
    fn plausibility_by_command() {
        use super::plausible;
        assert!(plausible(b"(echo \"<<DONE>>\")", b"<<DONE>>\n"));
        assert!(plausible(b"(echo \"a\"\"b\")", b"a\"b\n"));
        assert!(!plausible(b"(echo \"<<DONE>>\")", b"(:x 1)\n"));
        assert!(plausible(b"(check-sat)", b"unsat\n"));
        assert!(!plausible(b"(check-sat)", b"<<DONE>>\n"));
        assert!(plausible(
            b"(get-info :version)",
            b"(:version \"4.16.0\")\n"
        ));
        assert!(!plausible(b"(get-info :version)", b"unsat\n"));
        assert!(plausible(
            b"(get-info :all-statistics)",
            b"(:rlimit-count 5)\n"
        ));
        assert!(!plausible(b"(get-model)", b"(error \"no model\")\n"));
    }
}
