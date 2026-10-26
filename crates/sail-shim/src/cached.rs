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

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use sail_acceptance::query::{
    BoundPair, CorrelatedQueue, PublicationBatch, PublicationGate, ReplayCursor, response_frame,
};
use sail_api::client::Client;
use sail_api::protocol::{Effort, Identity, PutMeta};
use sail_key::{
    DeliveredInput, InputDelivery, Mode, ObservedPublication, QueryKey, SessionPrefix,
    SolverIdentity,
};
use sail_session::Session;
use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, Item, ItemKind, ResponseSplitter, is_single_response};

/// What the output side should do with the next reply.
enum Expect {
    /// Full input reached EOF; certify it only after a quiet, complete session.
    Terminal(DeliveredInput),
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
///
/// A reply must be exactly one complete response frame. Two concatenated
/// forms would add a reply to the client's stream and shift every later
/// pairing, and the replay after a fallback counts frames.
fn plausible(command: &[u8], reply: &[u8]) -> bool {
    use sail_smtlib::command::CommandKind;
    if !response_frame(reply) {
        return false;
    }
    let Ok(c) = classify(command) else {
        return false;
    };
    if !is_single_response(reply) {
        return false;
    }
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

/// A paired reply keeps its complete expectation and bytes together until PUT.
/// Silence certificates have a separate complete-session origin.
enum Entry {
    Paired {
        pair: BoundPair<Expect, Vec<u8>>,
        effort: Option<Effort>,
    },
    Silence {
        key: QueryKey,
    },
}

impl Entry {
    fn effort_mut(&mut self) -> Option<&mut Effort> {
        match self {
            Self::Paired { effort, .. } => effort.as_mut(),
            Self::Silence { .. } => None,
        }
    }

    fn wire_cost(&self) -> Option<usize> {
        match self {
            Self::Paired { pair, .. } => {
                let Expect::Store { command, text, .. } = pair.expectation() else {
                    return None;
                };
                pair.reply()
                    .len()
                    .checked_add(command.len())?
                    .checked_add(text.len())
            }
            Self::Silence { .. } => {
                Some(b"sail-session-silent/v1\n".len() + "sail-session-silence".len())
            }
        }
    }

    fn into_put(self) -> Option<([u8; 32], PutMeta, Vec<u8>)> {
        let (key, command, reply, effort) = match self {
            Self::Paired { pair, effort } => {
                let (expect, reply) = pair.into_parts();
                let Expect::Store { key, command, .. } = expect else {
                    return None;
                };
                (key, command, reply, effort)
            }
            Self::Silence { key } => (
                key,
                "sail-session-silence".to_owned(),
                b"sail-session-silent/v1\n".to_vec(),
                None,
            ),
        };
        Some((
            key.0,
            PutMeta {
                command,
                provenance: "pinned-solver".to_owned(),
                effort,
            },
            reply,
        ))
    }
}

/// A `check-sat` reply held back until its effort is known: the
/// `:all-statistics` reply after it gives the `rlimit-count` delta, and a
/// `reason-unknown` reply may come in between (ADR 0009).
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
struct Pairer {
    terminal: Option<DeliveredInput>,
    pending: CorrelatedQueue<Expect, Vec<u8>>,
    /// The latest `rlimit-count` seen in a statistics reply.
    last_count: Option<u64>,
    held: Option<HeldCheck>,
}

impl Default for Pairer {
    fn default() -> Self {
        Self {
            terminal: None,
            pending: CorrelatedQueue::empty(),
            last_count: None,
            held: None,
        }
    }
}

impl Pairer {
    fn reply(&mut self, reply: Vec<u8>) {
        // A stopped pairer never pairs again; retaining payloads only grows.
        if !self.pending.is_stopped() {
            self.pending.reply(reply);
        }
    }

    fn expect(&mut self, expect: Expect) {
        if self.terminal.is_some() {
            // The input owner seals once. Any later metadata violates that
            // terminal boundary and permanently withholds publication.
            self.pending.stop();
            return;
        }
        match expect {
            Expect::Terminal(key) => self.terminal = Some(key),
            _ if self.pending.is_stopped() => {}
            other => self.pending.expect(other),
        }
    }

    fn stop(&mut self) {
        self.pending.stop();
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
        while let Some(pair) = self.pending.take_pair() {
            let reply = pair.reply();
            let Expect::Store { text, limit, .. } = pair.expectation() else {
                self.pending.stop();
                break;
            };
            // Never store a reply that does not fit its command; stop
            // recording, since pairing can no longer be trusted.
            if reply.starts_with(b"(error") || !plausible(text, reply) {
                self.pending.stop();
                break;
            }
            let kind = classify(text).map(|c| c.kind).ok();
            match kind {
                Some(k) if k.is_check() => {
                    self.release(&mut out);
                    self.held = Some(HeldCheck {
                        entry: Entry::Paired {
                            effort: Some(Effort {
                                limit: *limit,
                                used: None,
                                reason: None,
                            }),
                            pair,
                        },
                        before: self.last_count,
                    });
                    continue;
                }
                Some(CommandKind::GetInfo {
                    keyword: b":reason-unknown",
                }) => {
                    if let Some(effort) = self.held.as_mut().and_then(|h| h.entry.effort_mut()) {
                        effort.reason =
                            Some(String::from_utf8_lossy(reply.trim_ascii()).into_owned());
                    }
                }
                Some(CommandKind::GetInfo {
                    keyword: b":all-statistics",
                }) => {
                    let count = rlimit_count(reply);
                    if let Some(h) = self.held.as_mut()
                        && let (Some(before), Some(after)) = (h.before, count)
                        && let Some(effort) = h.entry.effort_mut()
                    {
                        effort.used = Some(after.saturating_sub(before));
                    }
                    self.release(&mut out);
                    self.last_count = count.or(self.last_count);
                }
                _ => {}
            }
            out.push(Entry::Paired { pair, effort: None });
        }
        out
    }

    /// Releases a held check while preserving the actual frozen input token
    /// for the joint publication transition; a silence key cannot replace it.
    fn finish_observed(&mut self) -> (Vec<Entry>, Option<DeliveredInput>) {
        let mut out = Vec::new();
        let mut delivery = None;
        if !self.pending.is_stopped() {
            self.release(&mut out);
            if self.pending.is_empty()
                && let Some(key) = self.terminal.take()
            {
                out.push(Entry::Silence { key: key.key() });
                delivery = Some(key);
            }
        }
        (out, delivery)
    }

    #[cfg(test)]
    fn finish(&mut self) -> Vec<Entry> {
        self.finish_observed().0
    }
}

struct InputStatus {
    /// All producer expectations have been queued and the input side has finished.
    done: AtomicBool,
    /// Permanently false after any failed input delivery or completion observation.
    valid: AtomicBool,
}

impl InputStatus {
    fn send_expectation(&self, expects: &Sender<Expect>, expectation: Expect) {
        if expects.send(expectation).is_err() {
            self.valid.store(false, Ordering::SeqCst);
        }
    }
}

/// The actual input descriptor taken from the same owned solver child.
struct SolverPipe {
    stdin: Option<ChildStdin>,
}

impl sail_acceptance::query_delivery::InputPipe for SolverPipe {
    fn write(&mut self, bytes: &[u8]) -> bool {
        self.stdin
            .as_mut()
            .is_some_and(|stdin| stdin.write_all(bytes).and_then(|()| stdin.flush()).is_ok())
    }
    fn close(&mut self) {
        drop(self.stdin.take());
    }
}

struct Solving {
    stdin: InputDelivery<SolverPipe>,
    expects: Sender<Expect>,
    /// Input completion and permanently latched delivery/completion failures.
    input: Arc<InputStatus>,
    /// The output side; it owns the solver and returns its exit status.
    output: JoinHandle<u8>,
}

/// Sends paired replies to the daemon on its own thread, so forwarding never
/// waits on the daemon. Stops at the first daemon error.
fn spawn_storer(
    mut client: Client,
    entries: Receiver<Entry>,
    publication: Receiver<Option<ObservedPublication>>,
) -> Option<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("sail-z3-store".to_owned())
        .spawn(move || {
            // No entry is published until the entire solver session is known
            // to have emitted no stderr. Replies from older warning sessions
            // otherwise freeze a missing diagnostic on warm runs.
            let mut pending = PublicationBatch::empty();
            for entry in entries {
                let Some(cost) = entry.wire_cost() else {
                    return;
                };
                if !pending.push(entry, cost, 64 * 1024 * 1024) {
                    return;
                }
            }
            let Some(publication) = publication.recv().ok().flatten() else {
                return;
            };
            let Some(pending) = publication.release(pending) else {
                return;
            };
            // A daemon error drops the remaining stores.
            let _ = client.put_all(pending.into_iter().filter_map(Entry::into_put));
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

/// The storing thread and the channel to it.
struct Storing {
    tx: Sender<Entry>,
    publication: Sender<Option<ObservedPublication>>,
    storer: Option<JoinHandle<()>>,
}

impl Storing {
    /// Starts storing to `client`; without one, `pairer` stops recording.
    fn start(client: Option<Client>, pairer: &mut Pairer) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<Entry>();
        let (publication, observed) = std::sync::mpsc::channel();
        let storer = client.and_then(|client| spawn_storer(client, rx, observed));
        if storer.is_none() {
            // No daemon to store to.
            pairer.stop();
        }
        Self {
            tx,
            publication,
            storer,
        }
    }

    /// Ends the session's stores with the exact joint owned observation.
    fn close(self, observation: Option<ObservedPublication>) {
        let _ = self.publication.send(observation);
        drop(self.tx);
        if let Some(h) = self.storer {
            let _ = h.join();
        }
    }
}

/// The output side once a solver runs: forwards replies after the first
/// `served.len()`, records paired replies, and owns the solver process.
///
/// Each suppressed reply must reproduce the reply the cache already served
/// for that position: byte for byte, or for statistics, which carry no
/// verdict, structurally ([`ServedReply::reproduced_by`]). If the replay
/// differs, the client has seen a
/// stream the pinned solver does not produce: nothing more is forwarded,
/// the solver is killed, and the shim exits with an error (`SAIL2005`).
///
/// When the solver's output ends it waits for the solver to exit. The reply
/// in flight at that point is stored only if the solver exited cleanly: a
/// solver that died mid-reply leaves a partial reply, which is forwarded (as
/// pass-through would) but never stored. If the input side is still reading
/// when the solver exits, nothing will ever answer the commands the client
/// is waiting on, so the shim exits with the solver's status, as
/// pass-through does.
#[allow(
    clippy::too_many_lines,
    reason = "one forwarding loop; splitting it would scatter its offsets"
)]
fn output_side(
    mut from: impl Read,
    mut child: Child,
    served: &[ServedReply],
    expects: &Receiver<Expect>,
    client: Option<Client>,
    input: &InputStatus,
    stderr: Option<&crate::cached_stderr::Monitor>,
) -> u8 {
    let limit = buffer_limit();
    let mut all = sail_acceptance::query_output::Transcript::new(); // every output byte so far
    // Once forwarding is live and nothing more will be recorded, every later
    // byte is forwarded as read, exactly as pass-through forwards it.
    let mut streaming = false;
    let mut splitter = ResponseSplitter::new();
    let mut replay = ReplayCursor::new(u64::try_from(served.len()).unwrap_or(u64::MAX));
    let mut check = ReplayCheck::new(served);
    let mut forward_from: Option<usize> = replay.forwarding().then_some(0);
    let mut forwarded = 0usize;
    let mut forwarding_ok = true;
    // Whether the last complete response belongs to the fresh suffix, and its start.
    let mut prev: Option<(bool, usize)> = None;
    let mut buf = vec![0u8; 64 * 1024];

    // Pairs replies with expectations as either arrives. Stores go to a
    // separate thread, so forwarding never waits on the daemon.
    let mut pairer = Pairer::default();
    let storing = Storing::start(client, &mut pairer);

    loop {
        let n = match from.read(&mut buf) {
            Ok(0) => {
                all.end();
                break;
            }
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                all.refuse();
                forwarding_ok = false;
                break;
            }
        };
        if stderr.is_some_and(|stderr| !stderr.flush()) {
            forwarding_ok = false;
            break;
        }
        let chunk = buf.get(..n).unwrap_or_default();
        if streaming {
            if !write_out(chunk) {
                forwarding_ok = false;
                break;
            }
            continue;
        }
        all.append(chunk);
        let mut items: Vec<Item> = Vec::new();
        splitter.push(chunk, &mut items);
        for item in items {
            let start = usize::try_from(item.offset).unwrap_or(usize::MAX);
            if let Some((fresh, s)) = prev.take() {
                if all.frame(s, start).is_none() {
                    pairer.stop();
                }
                // Framing refusal stops publication. Replay comparison still
                // consumes the original bytes and keeps its original behavior.
                if !check.settle(
                    fresh,
                    all.bytes().get(s..start).unwrap_or_default(),
                    &mut pairer,
                ) {
                    break;
                }
            }
            let fresh = replay.advance();
            if fresh && forward_from.is_none() {
                forward_from = Some(start);
            }
            prev = Some((fresh, start));
        }
        if check.diverged {
            break;
        }
        if let Some(f) = forward_from
            && all.len() > forwarded.max(f)
        {
            if !write_out(all.bytes().get(forwarded.max(f)..).unwrap_or_default()) {
                forwarding_ok = false;
                break;
            }
            forwarded = all.len();
        }
        queue_stores(&mut pairer, expects, &storing.tx, false);
        // Streaming starts only after every suppressed replay reply has been
        // compared with the reply served in its place.
        if forward_from.is_some()
            && forwarded == all.len()
            && check.complete()
            && (pairer.pending.is_stopped()
                || all
                    .len()
                    .saturating_add(all.frames().saturating_mul(std::mem::size_of::<usize>()))
                    > limit)
        {
            // Recording has stopped, or the session outgrew the buffer: stop
            // recording for good and keep no more output.
            pairer.stop();
            streaming = true;
            prev = None;
            all.discard();
            forwarded = 0;
        }
    }
    // Stop reading first: a solver blocked writing to a closed pipe then
    // fails and exits instead of waiting forever.
    drop(from);
    if check.diverged {
        let _ = child.kill(); // it may wait for input that will never come
    }
    // End of output. Deliver what is left, as pass-through would.
    if !check.diverged && forward_from.is_some() && all.len() > forwarded {
        forwarding_ok &= write_out(all.bytes().get(forwarded..).unwrap_or_default());
    }
    let (_, mut code) = match child.wait() {
        Ok(status) => crate::status_code(status),
        Err(_) => (None, 1),
    };
    // The last reply runs to the end of the stream, and is complete only if
    // the solver exited cleanly.
    if !check.diverged
        && let Some((fresh, s)) = prev.take()
        && code == 0
    {
        let end = all.len();
        if all.frame(s, end).is_none() {
            pairer.stop();
        }
        check.settle(fresh, all.bytes().get(s..).unwrap_or_default(), &mut pairer);
    }
    // This consumes the actual stdout owner. It attests EOF and complete
    // response coverage, separately from the process/input/origin conditions.
    let output = all.seal();
    if output.is_none() {
        pairer.stop();
    }
    if check.finish(forwarding_ok && code == 0) {
        pairer.stop();
        code = 1;
    }
    let input_done = input.done.load(Ordering::SeqCst);
    // Once the input has ended, every expectation is queued or about to be.
    // Before that, more may never come, so take only what is there.
    queue_stores(&mut pairer, expects, &storing.tx, input_done);
    let (final_entries, delivery) = pairer.finish_observed();
    for entry in final_entries {
        let _ = storing.tx.send(entry);
    }
    let silent = stderr.is_some_and(crate::cached_stderr::Monitor::quiet);
    if stderr.is_some_and(crate::cached_stderr::Monitor::failed) || !forwarding_ok {
        code = 1;
    }
    let gate = PublicationGate {
        input_closed: input_done && input.valid.load(Ordering::SeqCst),
        pairing_active: !pairer.pending.is_stopped(),
        queues_empty: pairer.pending.is_empty(),
        normal_exit: code == 0,
        quiet: silent,
        forwarding_ok,
    };
    let publication = delivery.and_then(|delivery| delivery.observe_publication(output, gate));
    storing.close(publication);
    if !input_done {
        // The input side is blocked reading a client that waits for replies
        // the dead solver will never give.
        std::process::exit(i32::from(code));
    }
    code
}

/// Starts the solver, queues `first` (the expectation for the command that
/// caused the start) and writes `raw` to it. `served` holds the replies
/// already given from the cache, which the replay must reproduce. `client`,
/// if any, records.
fn start_solver(
    solver: &Path,
    args: &[std::ffi::OsString],
    prefix: &SessionPrefix,
    served: Vec<ServedReply>,
    first: Expect,
    client: Option<Client>,
) -> Option<Solving> {
    // Observation failure must not prevent replay: inherit solver stderr and
    // disable publication when its silence cannot be established.
    let (stderr, child_stderr) = crate::cached_stderr::Monitor::new()
        .map_or_else(|_| (None, Stdio::inherit()), |(m, io)| (Some(m), io));
    let client = stderr.as_ref().and(client);
    let mut child = Command::new(solver)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(child_stderr)
        .spawn()
        .ok()?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let input = Arc::new(InputStatus {
        done: AtomicBool::new(false),
        valid: AtomicBool::new(true),
    });
    // Queued before the solver sees any input, so it precedes its reply.
    input.send_expectation(&tx, first);
    let observed_input = Arc::clone(&input);
    // If the thread cannot start, the solver is dropped with it; it exits
    // when `stdin` is dropped below.
    let output = std::thread::Builder::new()
        .name("sail-z3-cached-out".to_owned())
        .spawn(move || {
            output_side(
                stdout,
                child,
                &served,
                &rx,
                client,
                &observed_input,
                stderr.as_ref(),
            )
        })
        .ok()?;
    let mut s = Solving {
        stdin: prefix.delivery(SolverPipe { stdin: Some(stdin) }),
        expects: tx,
        input,
        output,
    };
    s.write(prefix.bytes());
    Some(s)
}

impl Solving {
    fn expect(&self, expectation: Expect) {
        self.input.send_expectation(&self.expects, expectation);
    }

    fn write(&mut self, bytes: &[u8]) {
        if !self.stdin.write(bytes) {
            self.stdin.close();
            self.input.valid.store(false, Ordering::SeqCst);
        }
    }

    fn finish(mut self, terminal: QueryKey, complete: bool) -> ExitCode {
        // Only the frozen actual delivery can supply a complete-input token.
        // It cannot be reconstructed from a terminal key or a success Boolean.
        let delivered = self.stdin.seal(terminal, complete);
        let final_expectation = match delivered {
            Some(delivered) if self.input.valid.load(Ordering::SeqCst) => {
                Expect::Terminal(delivered)
            }
            _ => {
                self.input.valid.store(false, Ordering::SeqCst);
                Expect::Stop
            }
        };
        self.expect(final_expectation);
        // All metadata sends, including their failure latch, precede done.
        self.input.done.store(true, Ordering::SeqCst);
        self.stdin.close();
        drop(self.expects);
        ExitCode::from(self.output.join().unwrap_or(1))
    }
}

/// Default bound on the input and output a cached session retains for replay
/// and recording.
const BUFFER_LIMIT: usize = 256 * 1024 * 1024;

/// The bound on retained session bytes: `SAIL_SHIM_BUFFER_BYTES` if set to a
/// positive number, otherwise [`BUFFER_LIMIT`]. A session that exceeds it
/// stops using the cache and continues as pass-through.
fn buffer_limit() -> usize {
    std::env::var(crate::ENV_BUFFER_BYTES)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&bytes| bytes > 0)
        .unwrap_or(BUFFER_LIMIT)
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
fn connect(socket: &Path, solver: &Path) -> Option<(Client, SessionPrefix)> {
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
    let prefix = SessionPrefix::new(&identity, &key_mode);
    Some((client, prefix))
}

fn observe(
    session: &mut Session,
    item: &Item,
    prefix: &mut SessionPrefix,
) -> (Output, String, Expect) {
    // The splitter proposes an interval. Exact bytes, monotonicity and complete
    // inter-item trivia are checked before the proposal can drive a cache key.
    let Some(offset) = usize::try_from(item.offset).ok() else {
        prefix.refuse();
        return (Output::Unknown, String::new(), Expect::Stop);
    };
    let Some(frame) = prefix.command(offset, &item.bytes) else {
        return (Output::Unknown, String::new(), Expect::Stop);
    };
    let head = frame.head();
    let name = String::from_utf8_lossy(head).into_owned();
    if !classify(&item.bytes).is_ok_and(|command| command.name == head) {
        prefix.refuse();
        return (Output::Unknown, String::new(), Expect::Stop);
    }
    let policy = match frame.policy() {
        sail_acceptance::query_policy::ResponsePolicy::None => Output::None,
        sail_acceptance::query_policy::ResponsePolicy::One => Output::One,
        sail_acceptance::query_policy::ResponsePolicy::Unknown => Output::Unknown,
    };
    let mut output = if item.kind == ItemKind::List && session.expects_output(&item.bytes) == policy
    {
        policy
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
    let limit = session
        .option(b":rlimit")
        .and_then(|v| std::str::from_utf8(v).ok())
        .and_then(|v| v.parse::<u64>().ok());
    let expect = match output {
        Output::One => Expect::Store {
            key: frame.key(),
            command: name.clone(),
            text: item.bytes.clone(),
            limit,
        },
        _ => Expect::Stop,
    };
    (output, name, expect)
}

/// The replies a session served from the cache before any solver started.
#[derive(Default)]
struct Served {
    /// The replies, until a solver takes them to check its replay.
    replies: Vec<ServedReply>,
    /// How many there were.
    count: u64,
    /// The retained replies' total length.
    bytes: usize,
}

impl Served {
    fn push(&mut self, command: &[u8], reply: Vec<u8>) {
        let statistics = classify(command).is_ok_and(|c| {
            c.kind
                == CommandKind::GetInfo {
                    keyword: b":all-statistics",
                }
        });
        self.bytes = self.bytes.saturating_add(reply.len());
        self.replies.push(ServedReply {
            bytes: reply,
            statistics,
        });
        self.count += 1;
    }

    fn take(&mut self) -> Vec<ServedReply> {
        self.bytes = 0;
        std::mem::take(&mut self.replies)
    }
}

/// One reply served from the cache.
struct ServedReply {
    bytes: Vec<u8>,
    /// Whether it answers `(get-info :all-statistics)`.
    statistics: bool,
}

impl ServedReply {
    /// Whether the replay's reply in this position reproduces it: the same
    /// bytes, or for statistics, one well-formed statistics response. Their
    /// times, memory and even resource counts vary from run to run on the
    /// same input (`corpus/repros/z3-resource-count-variation`), and they
    /// carry no verdict.
    fn reproduced_by(&self, replay: &[u8]) -> bool {
        if self.statistics {
            plausible(b"(get-info :all-statistics)", replay)
        } else {
            self.bytes == replay
        }
    }
}

/// Checks a replay's suppressed replies against those already served.
struct ReplayCheck<'a> {
    served: &'a [ServedReply],
    checked: usize,
    diverged: bool,
}

impl<'a> ReplayCheck<'a> {
    fn new(served: &'a [ServedReply]) -> Self {
        Self {
            served,
            checked: 0,
            diverged: false,
        }
    }

    /// Whether every served reply has been compared with its replay.
    fn complete(&self) -> bool {
        !self.diverged && self.checked == self.served.len()
    }

    /// At the end of the replay: whether it diverged, including a `clean`
    /// replay that gave fewer replies than were served. A divergence is
    /// reported on stderr.
    fn finish(&mut self, clean: bool) -> bool {
        self.diverged |= clean && self.checked < self.served.len();
        if self.diverged {
            eprintln!(
                "error[SAIL2005]: sail-z3: the pinned solver's replay differs from reply {} \
                 served from the cache; stopping",
                self.checked + 1
            );
        }
        self.diverged
    }

    /// Settles one complete reply: a fresh one goes to the pairer, a
    /// suppressed one must equal the reply served in its place. Returns
    /// false once the replay has diverged.
    fn settle(&mut self, fresh: bool, bytes: &[u8], pairer: &mut Pairer) -> bool {
        if fresh {
            pairer.reply(bytes.to_vec());
        } else if self
            .served
            .get(self.checked)
            .is_some_and(|r| r.reproduced_by(bytes))
        {
            self.checked += 1;
        } else {
            self.diverged = true;
        }
        !self.diverged
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
#[allow(
    clippy::too_many_lines,
    reason = "one input loop; splitting it would scatter the session state"
)]
pub(crate) fn run(socket: &Path, solver: &Path, args: &[std::ffi::OsString]) -> Option<ExitCode> {
    let (client, mut prefix) = connect(socket, solver)?;
    let mut session = Session::new();
    let mut splitter = CommandSplitter::new();
    let mut served = Served::default();
    let mut solving: Option<Solving> = None;
    let mut client = Some(client);
    let mut stdin = std::io::stdin().lock();
    let mut input_read_ok = true;
    let mut buf = vec![0u8; 64 * 1024];
    let limit = buffer_limit();

    loop {
        let n = match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                input_read_ok = false;
                break;
            }
        };
        let chunk = buf.get(..n).unwrap_or_default().to_vec();
        prefix.append(&chunk);
        let mut items = Vec::new();
        splitter.push(&chunk, &mut items);
        // Expectations for commands in this chunk are queued before the
        // chunk reaches the solver.
        let mut fresh_solver = false;
        for item in items {
            let (expects, name, expect) = observe(&mut session, &item, &mut prefix);
            if name == "exit" && closes_session(&item.bytes) {
                if let Some(s) = &solving
                    && expects != Output::None
                {
                    s.expect(expect);
                }
                if let Some(s) = &mut solving
                    && !fresh_solver
                {
                    s.write(&chunk);
                }
                return finish_run(solving, client, solver, args, served, &mut prefix, true);
            }
            if let Some(s) = &solving {
                if expects != Output::None {
                    s.expect(expect);
                }
                continue;
            }
            let hit = match (expects, &expect) {
                (Output::None, _) => continue,
                (Output::One, Expect::Store { key, .. }) => {
                    match client.as_mut().map(|c| c.get_with_effort(&key.0)) {
                        Some(Ok(found)) => found
                            .filter(|(r, e)| !unservable(&item.bytes, r, e.as_ref()))
                            .map(|(r, _)| r),
                        Some(Err(_)) => {
                            client = None;
                            None
                        }
                        None => None,
                    }
                }
                _ => None,
            };
            if let Some(reply) = hit {
                if !write_out(&reply) {
                    // As when forwarding the solver's output fails.
                    return Some(ExitCode::from(1));
                }
                served.push(&item.bytes, reply);
                continue;
            }
            // Miss: start the solver with the whole input so far.
            let replies = served.take();
            let Some(s) = start_solver(solver, args, &prefix, replies, expect, client.take())
            else {
                return Some(ExitCode::from(1));
            };
            solving = Some(s);
            fresh_solver = true;
        }
        if let Some(s) = &mut solving
            && !fresh_solver
        {
            s.write(&chunk);
        }
        // Served replies are retained for the replay check, so they count.
        if prefix.bytes().len().saturating_add(served.bytes) > limit {
            let terminal = prefix.silence_key();
            return Some(overflow(
                solving,
                solver,
                args,
                prefix,
                served.take(),
                terminal,
                &mut stdin,
            ));
        }
    }
    // At the end of input while serving, incomplete trailing input goes to
    // the solver, which decides what it means.
    let complete = input_read_ok && splitter.finish().is_none() && prefix.trailing_trivia();
    finish_run(solving, client, solver, args, served, &mut prefix, complete)
}

/// Continues a session that outgrew [`buffer_limit`] as pass-through: no
/// more lookups, nothing recorded, and no retained input. A solver already
/// running stops recording; otherwise one starts now with the whole input,
/// and its replies to the commands already served are compared with them
/// and discarded as on any miss. No provenance record is written, so the session's checks are
/// reported as unrecorded.
fn overflow(
    solving: Option<Solving>,
    solver: &Path,
    args: &[std::ffi::OsString],
    raw: SessionPrefix,
    served: Vec<ServedReply>,
    terminal: QueryKey,
    stdin: &mut impl Read,
) -> ExitCode {
    let mut s = match solving {
        Some(s) => {
            s.expect(Expect::Stop);
            s
        }
        None => match start_solver(solver, args, &raw, served, Expect::Stop, None) {
            Some(s) => s,
            None => return ExitCode::from(1),
        },
    };
    drop(raw);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        s.write(buf.get(..n).unwrap_or_default());
        if !s.stdin.active() {
            break;
        }
    }
    // Never certified or published: the input was not retained.
    s.finish(terminal, false)
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
    served: Served,
    prefix: &mut SessionPrefix,
    complete: bool,
) -> Option<ExitCode> {
    if !prefix.hash_tail() || usize::try_from(prefix.len()).ok() != Some(prefix.bytes().len()) {
        prefix.refuse();
    }
    let mut end = SessionEnd {
        prefix,
        served,
        terminal: prefix.silence_key(),
        complete: complete && prefix.valid(),
    };
    let (code, started) = finish_input(solving, client, solver, args, &mut end);
    record_provenance(prefix.bytes(), end.served.count, started);
    code
}

struct SessionEnd<'a> {
    prefix: &'a SessionPrefix,
    served: Served,
    terminal: QueryKey,
    complete: bool,
}

fn finish_input(
    solving: Option<Solving>,
    mut client: Option<Client>,
    solver: &Path,
    args: &[std::ffi::OsString],
    end: &mut SessionEnd<'_>,
) -> (Option<ExitCode>, bool) {
    let certified = solving.is_none()
        && end.complete
        && client.as_mut().is_some_and(|c| {
            c.get(&end.terminal.0)
                .ok()
                .flatten()
                .is_some_and(|r| r == b"sail-session-silent/v1\n")
        });
    match solving {
        Some(s) => (Some(s.finish(end.terminal, end.complete)), true),
        None if !certified => (
            start_solver(
                solver,
                args,
                end.prefix,
                end.served.take(),
                Expect::Stop,
                client.take(),
            )
            .map(|s| s.finish(end.terminal, end.complete)),
            true,
        ),
        None => (Some(ExitCode::SUCCESS), false),
    }
}

/// When `SAIL_PROVENANCE_DIR` is set, writes `<dir>/<pid>-<nanos>.json`
/// saying how many replies the cache served before a solver started, keyed
/// by the SHA-256 of the session's whole input (ADR 0014). `sail verify
/// --output json` joins it to Verus's transcript of the same process.
///
/// Best effort, and only after every reply has been forwarded and the
/// solver has exited: nothing here can affect what the client receives.
fn record_provenance(input: &[u8], served: u64, solver_started: bool) {
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
    use super::{Entry, Expect, Pairer};
    use proptest::prelude::*;
    use sail_key::QueryKey;

    struct StoredEntry {
        key: QueryKey,
        reply: Vec<u8>,
        effort: Option<super::Effort>,
    }

    fn stored_entry(entry: super::Entry) -> StoredEntry {
        let Some((key, meta, reply)) = entry.into_put() else {
            unreachable!("a stored entry must retain a Store expectation");
        };
        StoredEntry {
            key: QueryKey(key),
            reply,
            effort: meta.effort,
        }
    }

    fn stored_entries(entries: Vec<super::Entry>) -> Vec<StoredEntry> {
        entries.into_iter().map(stored_entry).collect()
    }

    fn identity() -> super::SolverIdentity {
        super::SolverIdentity {
            name: "z3".to_owned(),
            version: "pinned-test".to_owned(),
            binary_sha256: [9; 32],
            platform: "test-platform".to_owned(),
        }
    }

    #[test]
    fn checked_raw_prefixes_preserve_existing_keys_and_replay_bytes() {
        use super::{CommandSplitter, Mode, Session, SessionPrefix, observe};
        use sail_key::PrefixHasher;
        let input = b" ;head\r\n(set-option :print-success false)\n(echo \"a;()\")\t;gap\n(check-sat)\n;tail";
        for width in [1, 3, 17, input.len()] {
            for mode in [Mode::Default, Mode::OptIn("test-mode".to_owned())] {
                let mut parser = CommandSplitter::new();
                let mut prefix = SessionPrefix::new(&identity(), &mode);
                let mut session = Session::new();
                let mut items = Vec::new();
                for chunk in input.chunks(width) {
                    prefix.append(chunk);
                    parser.push(chunk, &mut items);
                    for item in items.drain(..) {
                        let _ = observe(&mut session, &item, &mut prefix);
                        let endpoint =
                            usize::try_from(item.offset).unwrap_or_default() + item.bytes.len();
                        let mut plain = PrefixHasher::new(&identity(), &mode);
                        plain.update(&input[..endpoint]);
                        assert_eq!(prefix.key(), plain.key());
                        assert_eq!(usize::try_from(prefix.len()).unwrap_or_default(), endpoint);
                        assert!(prefix.valid());
                    }
                }
                assert!(parser.finish().is_none());
                assert!(prefix.trailing_trivia());
                assert!(prefix.hash_tail());
                let mut plain = PrefixHasher::new(&identity(), &mode);
                plain.update(input);
                assert_eq!(prefix.silence_key(), plain.silence_key());
                assert_eq!(prefix.bytes(), input);
            }
        }
    }

    #[test]
    fn output_expectations_use_the_exact_checked_frame_head() {
        use super::{Item, ItemKind, Output, Session, SessionPrefix, observe};
        for (command, expected) in [
            (b"( ;head\r\ncheck-sat-using smt)".as_slice(), Output::One),
            (
                b"(assert (str.contains \"a;()\" \"a\"))".as_slice(),
                Output::None,
            ),
            (b"(echo \"a;()\"\"b\")".as_slice(), Output::One),
            (b"(get-info :name)".as_slice(), Output::One),
            (b"(get-proof)".as_slice(), Output::One),
            (b"(reset)".as_slice(), Output::None),
            (b"(check-sat-usingbad)".as_slice(), Output::Unknown),
            (b"(|check-sat|)".as_slice(), Output::Unknown),
        ] {
            let mut prefix = SessionPrefix::new(&identity(), &super::Mode::Default);
            prefix.append(command);
            let item = Item {
                kind: ItemKind::List,
                offset: 0,
                bytes: command.to_vec(),
            };
            let (actual, _, metadata) = observe(&mut Session::new(), &item, &mut prefix);
            assert_eq!(actual, expected);
            assert_eq!(prefix.bytes(), command);
            if expected == Output::One {
                let Expect::Store { key, text, .. } = metadata else {
                    unreachable!("one response needs an exact Store expectation");
                };
                assert_eq!(text, command);
                assert_eq!(key, prefix.key());
            } else {
                assert!(matches!(metadata, Expect::Stop));
            }
        }
    }

    #[test]
    fn wrong_missing_or_overlapping_parser_items_disable_only_cache_interpretation() {
        use super::{Item, ItemKind, Output, Session, SessionPrefix, observe};
        use sail_acceptance::query::RawTranscript;
        let first = b"(set-option :print-success false)\n";
        let input = [first.as_slice(), b"(check-sat)"].concat();
        for item in [
            Item {
                kind: ItemKind::List,
                offset: first.len() as u64,
                bytes: b"(check-sat)".to_vec(),
            },
            Item {
                kind: ItemKind::List,
                offset: 0,
                bytes: b"(check-bad)".to_vec(),
            },
            Item {
                kind: ItemKind::List,
                offset: u64::MAX,
                bytes: b"(check-sat)".to_vec(),
            },
        ] {
            let mut prefix = SessionPrefix::new(&identity(), &super::Mode::Default);
            prefix.append(&input);
            let mut session = Session::new();
            let (output, name, expectation) = observe(&mut session, &item, &mut prefix);
            assert_eq!(output, Output::Unknown);
            assert_eq!(name, "");
            assert!(matches!(expectation, Expect::Stop));
            assert!(!prefix.valid());
            assert_eq!(prefix.bytes(), input);
        }
        let mut raw = RawTranscript::empty();
        raw.append(b"(check-sat)");
        assert!(raw.command(0, b"(check-sat)").is_some());
        assert!(raw.command(0, b"(check-sat)").is_none());
        assert!(!raw.valid());
        assert_eq!(raw.bytes(), b"(check-sat)");
    }

    #[test]
    fn native_frames_match_checked_lexical_grammar_across_fragments() {
        use sail_acceptance::query::{command_head, complete_list, response_frame};
        let input = b";header )\r\n( ;head\n echo \"a;()\"\"x\") (assert |a)b|) (assert ;hidden )\n true)\n";
        for width in 1..=input.len() {
            let mut splitter = super::CommandSplitter::new();
            let mut items = Vec::new();
            for chunk in input.chunks(width) {
                splitter.push(chunk, &mut items);
            }
            assert!(splitter.finish().is_none());
            assert_eq!(items.len(), 3);
            for item in items {
                assert!(complete_list(&item.bytes, true));
                let checked = command_head(&item.bytes);
                let native = super::classify(&item.bytes)
                    .ok()
                    .map(|command| command.name);
                assert_eq!(checked, native);
            }
        }
        let output = b"unsat\r\n(:name \"a;()\"\"x\")\n(model (define-fun |a)b| () Int 1))\n";
        for width in 1..=output.len() {
            let mut splitter = super::ResponseSplitter::new();
            let mut items = Vec::new();
            for chunk in output.chunks(width) {
                splitter.push(chunk, &mut items);
            }
            assert!(splitter.finish().is_none());
            assert_eq!(items.len(), 3);
            for (i, item) in items.iter().enumerate() {
                let start = usize::try_from(item.offset).unwrap_or_default();
                let end = items.get(i + 1).map_or(output.len(), |next| {
                    usize::try_from(next.offset).unwrap_or_default()
                });
                assert!(response_frame(&output[start..end]));
            }
        }
    }

    #[test]
    fn framing_refuses_quoted_heads_partial_forms_and_coalesced_replies() {
        use super::{Item, ItemKind, Output, Session, SessionPrefix, observe};
        for bytes in [
            b"(check-sat)(check-sat)".as_slice(),
            b"(|check-sat|)",
            b"(echo \"unfinished)",
            b"(assert true ;comment EOF",
        ] {
            let mut prefix = SessionPrefix::new(&identity(), &super::Mode::Default);
            prefix.append(bytes);
            let item = Item {
                kind: ItemKind::List,
                offset: 0,
                bytes: bytes.to_vec(),
            };
            let (output, name, expected) = observe(&mut Session::new(), &item, &mut prefix);
            assert_eq!(output, Output::Unknown);
            assert_eq!(name, "");
            assert!(matches!(expected, Expect::Stop));
            assert!(!prefix.valid());
            assert_eq!(prefix.bytes(), bytes);
        }
        assert!(!super::plausible(
            b"(get-info :version)",
            b"(:version \"x\")(:name \"coalesced\")\n"
        ));
        assert!(!super::plausible(
            b"(get-info :version)",
            b"(:version \"unfinished\""
        ));
        assert!(!super::plausible(b"(check-sat)", b"unsat"));
        assert!(super::plausible(
            b"(get-info :version)",
            b"(:version \"4.16.0\")\n"
        ));
    }

    #[test]
    fn lost_expectation_metadata_permanently_blocks_publication() -> Result<(), &'static str> {
        use std::sync::atomic::{AtomicBool, Ordering};
        let status = super::InputStatus {
            done: AtomicBool::new(false),
            valid: AtomicBool::new(true),
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        drop(receiver);
        status.send_expectation(&sender, Expect::Stop);
        assert!(!status.valid.load(Ordering::SeqCst));
        assert!(!status.done.load(Ordering::SeqCst));
        status.send_expectation(&sender, Expect::Terminal(terminal(b"(echo 1)")?));
        assert!(!status.valid.load(Ordering::SeqCst));
        assert!(!status.done.load(Ordering::SeqCst));
        Ok(())
    }

    struct TestPipe;
    impl sail_acceptance::query_delivery::InputPipe for TestPipe {
        fn write(&mut self, _: &[u8]) -> bool {
            true
        }
        fn close(&mut self) {}
    }
    fn terminal(bytes: &[u8]) -> Result<sail_key::DeliveredInput, &'static str> {
        let mut prefix = sail_key::SessionPrefix::new(&identity(), &sail_key::Mode::Default);
        let mut delivery = prefix.delivery(TestPipe);
        prefix.append(bytes);
        if !prefix.hash_tail() || !delivery.write(bytes) {
            return Err("script input must be complete");
        }
        delivery
            .seal(prefix.silence_key(), true)
            .ok_or("script delivery must seal")
    }

    #[test]
    fn duplicate_or_late_metadata_cannot_extend_a_frozen_delivery() -> Result<(), &'static str> {
        let mut late = Pairer::default();
        late.expect(Expect::Terminal(terminal(b"(echo 1)")?));
        late.expect(store(1, "(echo 1)"));
        assert!(late.pending.is_stopped());
        assert!(late.finish().is_empty());
        let mut duplicate = Pairer::default();
        duplicate.expect(Expect::Terminal(terminal(b"(echo 1)")?));
        duplicate.expect(Expect::Terminal(terminal(b"(echo 1)")?));
        assert!(duplicate.pending.is_stopped());
        assert!(duplicate.finish().is_empty());
        Ok(())
    }

    #[test]
    fn finalization_retains_original_delivery_once_beside_silence_entry() -> Result<(), &'static str>
    {
        let delivered = terminal(b"(echo 1)")?;
        let key = delivered.key();
        let mut pairer = Pairer::default();
        pairer.expect(Expect::Terminal(delivered));
        let (entries, input) = pairer.finish_observed();
        assert_eq!(entries.len(), 1);
        assert!(matches!(&entries[0], Entry::Silence { key: observed } if *observed == key));
        assert_eq!(input.as_ref().map(sail_key::DeliveredInput::key), Some(key));
        let (again, duplicate) = pairer.finish_observed();
        assert!(again.is_empty());
        assert!(duplicate.is_none());
        Ok(())
    }

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
                    p.reply(format!("(m {r})\n").into_bytes());
                    r += 1;
                } else if usize::from(e) < n {
                    p.expect(model(e));
                    e += 1;
                }
                if drains.get(i).copied().unwrap_or(false) {
                    stored.extend(p.drain());
                }
            }
            while usize::from(r) < n { p.reply(format!("(m {r})\n").into_bytes()); r += 1; }
            while usize::from(e) < n { p.expect(model(e)); e += 1; }
            stored.extend(p.drain());
            prop_assert_eq!(stored.len(), n);
            for entry in stored {
                let entry = stored_entry(entry);
                prop_assert_eq!(entry.reply, format!("(m {})\n", entry.key.0[0]).into_bytes());
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
        let stored = stored_entries(p.drain());
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
        let stored = stored_entries(p.drain());
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
        let rest = stored_entries(p.finish());
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
    fn replay_must_reproduce_served_replies() {
        let mut served = super::Served::default();
        served.push(b"(get-model)", b"(model)\n".to_vec());
        served.push(
            b"(get-info :all-statistics)",
            b"(:rlimit-count 5\n :time 0.01)\n".to_vec(),
        );
        let replies = served.take();
        assert_eq!(served.count, 2);
        assert!(replies[0].reproduced_by(b"(model)\n"));
        assert!(!replies[0].reproduced_by(b"(model )\n"));
        // Statistics, even resource counts, vary on identical input: only
        // their structure is checked.
        assert!(replies[1].reproduced_by(b"(:rlimit-count 5\n :time 0.02)\n"));
        assert!(replies[1].reproduced_by(b"(:rlimit-count 6\n :time 0.01)\n"));
        assert!(!replies[1].reproduced_by(b"(:rlimit-count 5)\n(:x 1)\n"));
        assert!(!replies[1].reproduced_by(b"unsat\n"));
        assert!(!replies[1].reproduced_by(b"(error \"x\")\n"));
        let mut check = super::ReplayCheck::new(&replies);
        let mut pairer = Pairer::default();
        assert!(check.settle(false, b"(model)\n", &mut pairer));
        assert!(!check.finish(false), "an unclean end is not judged");
        assert!(
            check.finish(true),
            "a clean replay one reply short diverged"
        );
        let mut check = super::ReplayCheck::new(&replies);
        assert!(!check.settle(false, b"(model 1)\n", &mut pairer));
        assert!(check.diverged);
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
        // Exactly one complete frame (#379).
        assert!(!plausible(
            b"(get-info :name)",
            b"(:name \"Z3\")(:evil \"extra\")\n"
        ));
        assert!(!plausible(b"(check-sat)", b"unsat\nsat\n"));
        assert!(!plausible(b"(get-model)", b"(model)\n(model)\n"));
        assert!(!plausible(b"(get-model)", b"(model"));
        assert!(!plausible(
            b"(get-info :all-statistics)",
            b"(:a 1)\n(:b 2)\n"
        ));
    }
}
