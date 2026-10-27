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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use sail_acceptance::query::{
    BoundPair, CorrelatedQueue, PublicationBatch, PublicationGate, response_frame,
};
use sail_api::client::Client;
use sail_api::protocol::{Effort, Identity, PutMeta};
use sail_key::{DeliveredInput, InputDelivery, Mode, QueryKey, SessionPrefix, SolverIdentity};
use sail_session::Session as NativeSession;
use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, Item, ItemKind, ResponseSplitter, is_single_response};

/// Native Session plus independently retained raw policy-option bytes.
/// Map readback agreement does not prove the pinned solver's option semantics.
struct ObservedSession {
    native: NativeSession,
    options: sail_acceptance::query_options::OptionState,
}

impl ObservedSession {
    fn new() -> Self {
        Self {
            native: NativeSession::new(),
            options: sail_acceptance::query_options::OptionState::new(),
        }
    }

    fn option(&self, keyword: &[u8]) -> Option<&[u8]> {
        self.native.option(keyword)
    }

    fn readback_matches(&self) -> bool {
        self.options.matches(
            self.native.option(b":print-success"),
            self.native.option(b":rlimit"),
        )
    }

    fn expects_output(&self, command: &[u8]) -> Output {
        self.native.expects_output(command)
    }

    fn apply(&mut self, command: &[u8]) -> bool {
        let spans = option_spans(command);
        self.native.apply(command);
        self.options.advance(
            command,
            spans,
            self.native.option(b":print-success"),
            self.native.option(b":rlimit"),
        )
    }
}

/// Lexer offsets are candidates only; the shared operation checks original bytes.
fn option_spans(command: &[u8]) -> Option<sail_acceptance::query_options::OptionSpans> {
    let mut lexer = sail_smtlib::lex::Lexer::new(command);
    let _open = lexer.next()?.ok()?;
    let head = lexer.next()?.ok()?;
    if command.get(head.start..head.end)? != b"set-option" {
        return None;
    }
    let keyword = lexer.next()?.ok()?;
    let value = lexer.next()?.ok()?;
    Some(sail_acceptance::query_options::OptionSpans {
        head_start: head.start,
        head_end: head.end,
        keyword_start: keyword.start,
        keyword_end: keyword.end,
        value_start: value.start,
        value_end: value.end,
    })
}

/// Preserve native classification as a candidate, then check all original
/// head/keyword/remaining bytes in the shared statistics-command mint.
fn statistics_spans(command: &[u8]) -> Option<sail_acceptance::query_replay::KeywordSpans> {
    if classify(command).ok()?.kind
        != (CommandKind::GetInfo {
            keyword: b":all-statistics",
        })
    {
        return None;
    }
    let mut lexer = sail_smtlib::lex::Lexer::new(command);
    let _open = lexer.next()?.ok()?;
    let head = lexer.next()?.ok()?;
    let keyword = lexer.next()?.ok()?;
    Some(sail_acceptance::query_replay::KeywordSpans {
        head_start: head.start,
        head_end: head.end,
        keyword_start: keyword.start,
        keyword_end: keyword.end,
    })
}

/// A strongly retained allocation minted only beside the actual child/pipe
/// extraction below; PID and equal stream values cannot substitute for it.
type SolverLease = Arc<()>;
type SolverFrame = sail_acceptance::query_output_owner::OwnedFrame<SolverLease>;
type SolverPublication = sail_key::OwnedObservedPublication<SolverLease, SolverOwnerIdentity>;

/// Original pending-publication bound, also charged before retaining snapshots.
const PUBLICATION_LIMIT: usize = 64 * 1024 * 1024;
/// Fixed new retained/certificate state is charged before recording starts.
const PUBLICATION_FIXED_BYTES: usize = 5 * std::mem::size_of::<Vec<usize>>()
    + std::mem::size_of::<DeliveredInput>()
    + std::mem::size_of::<SolverPublication>()
    + std::mem::size_of::<sail_key::PrefixHasher>();

struct SolverOwnerIdentity;
impl sail_acceptance::inputs::OwnerIdentity<SolverLease> for SolverOwnerIdentity {
    fn same_owner(&self, original: &SolverLease, actual: &SolverLease) -> bool {
        Arc::ptr_eq(original, actual)
    }
}

/// The actual frozen input and its original private solver lease travel together.
struct TerminalInput {
    delivered: DeliveredInput,
    owner: SolverLease,
}
/// Created only alongside this exact spawn and extracted descriptors.
struct SolverProcess {
    child: Child,
    owner: SolverLease,
}

/// Production metadata has one private original checkpoint. Symbolic test
/// metadata exercises the same transitions but can never authorize publication.
enum StoreCommand {
    Original(sail_key::OwnedCommand),
    #[cfg(test)]
    Symbolic {
        key: QueryKey,
        command: String,
        text: Vec<u8>,
    },
}
impl StoreCommand {
    fn key(&self) -> QueryKey {
        match self {
            Self::Original(command) => command.key(),
            #[cfg(test)]
            Self::Symbolic { key, .. } => *key,
        }
    }
    fn text(&self) -> &[u8] {
        match self {
            Self::Original(command) => command.command(),
            #[cfg(test)]
            Self::Symbolic { text, .. } => text,
        }
    }
    fn retained_bytes(&self) -> Option<usize> {
        let command = match self {
            Self::Original(command) => command.retained_bytes()?,
            #[cfg(test)]
            Self::Symbolic { command, text, .. } => std::mem::size_of::<Self>()
                .checked_add(command.len())?
                .checked_add(text.len())?,
        };
        command.checked_add(std::mem::size_of::<SolverFrame>())
    }
    fn observed_by(&self, publication: &SolverPublication) -> bool {
        match self {
            Self::Original(command) => publication.same_command_session(command),
            #[cfg(test)]
            Self::Symbolic { .. } => false,
        }
    }
    fn into_metadata(self) -> (QueryKey, String) {
        match self {
            Self::Original(command) => {
                let (key, head, _command) = command.into_parts();
                let name = String::from_utf8(head)
                    .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
                (key, name)
            }
            #[cfg(test)]
            Self::Symbolic { key, command, .. } => (key, command),
        }
    }
}
impl TerminalInput {
    fn key(&self) -> QueryKey {
        self.delivered.key()
    }
}

/// What the output side should do with the next reply.
enum Expect {
    /// Full input reached EOF; certify it only after a quiet, complete session.
    Terminal(TerminalInput),
    /// Store the reply under this key.
    Store {
        command: StoreCommand,
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
        } => {
            sail_acceptance::query_replay::statistics_command(command, statistics_spans(command))
                && body.starts_with(b"(:")
                && body.ends_with(b")")
        }
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
        pair: BoundPair<Expect, SolverFrame>,
        effort: Option<Effort>,
    },
    Silence {
        key: QueryKey,
    },
}

impl Entry {
    fn observed_by(&self, publication: &SolverPublication) -> bool {
        match self {
            Self::Paired { pair, .. } => {
                let Expect::Store { command, .. } = pair.expectation() else {
                    return false;
                };
                publication.contains_frame(pair.reply()) && command.observed_by(publication)
            }
            Self::Silence { key } => *key == publication.input_key(),
        }
    }

    fn wire_cost(&self) -> Option<usize> {
        match self {
            Self::Paired { pair, .. } => {
                let Expect::Store { command, .. } = pair.expectation() else {
                    return None;
                };
                pair.reply()
                    .bytes()
                    .len()
                    .checked_add(command.retained_bytes()?)
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
                let Expect::Store { command, .. } = expect else {
                    return None;
                };
                let (key, command) = command.into_metadata();
                (key, command, reply.into_bytes(), effort)
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

fn batch_observed(pending: &PublicationBatch<Entry>, publication: &SolverPublication) -> bool {
    if !pending
        .entries()
        .iter()
        .all(|entry| entry.observed_by(publication))
    {
        return false;
    }
    let mut pairs = Vec::new();
    for entry in pending.entries() {
        if let Entry::Paired { pair, .. } = entry {
            let Expect::Store {
                command: StoreCommand::Original(command),
                ..
            } = pair.expectation()
            else {
                return false;
            };
            pairs.push((command, pair.reply()));
        }
    }
    publication.validate_pairs(&pairs)
}

/// A `check-sat` reply held back until its effort is known: the
/// `:all-statistics` reply after it gives the `rlimit-count` delta, and a
/// `reason-unknown` reply may come in between (ADR 0009).
struct HeldMetadata {
    before: Option<u64>,
    effort: Effort,
}

type HeldPayload = BoundPair<Expect, SolverFrame>;
type HeldCheck = sail_acceptance::query_holding::Holding<HeldPayload, HeldMetadata>;

fn held_entry(pair: HeldPayload, metadata: Option<HeldMetadata>) -> Entry {
    Entry::Paired {
        pair,
        effort: metadata.map(|metadata| metadata.effort),
    }
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
    terminal: Option<TerminalInput>,
    pending: CorrelatedQueue<Expect, SolverFrame>,
    /// The latest `rlimit-count` seen in a statistics reply.
    last_count: Option<u64>,
    held: HeldCheck,
}

impl Default for Pairer {
    fn default() -> Self {
        Self {
            terminal: None,
            pending: CorrelatedQueue::empty(),
            last_count: None,
            held: HeldCheck::new(),
        }
    }
}

impl Pairer {
    fn reply_frame(&mut self, reply: SolverFrame) {
        // A stopped pairer never pairs again; retaining payloads only grows.
        if !self.pending.is_stopped() {
            self.pending.reply(reply);
        }
    }

    #[cfg(test)]
    fn reply(&mut self, reply: Vec<u8>) {
        // Legacy value-domain fixtures each declare their own original stream;
        // they do not qualify actual producer ownership or full publication.
        let owner = Arc::new(());
        let mut output = sail_acceptance::query_output_owner::StreamOutput::new(Arc::clone(&owner));
        output.append(&reply);
        let frame = output
            .frame(0, reply.len())
            .and_then(|frame| frame.into_owned(owner, &SolverOwnerIdentity));
        if let Some(frame) = frame {
            self.reply_frame(frame);
        } else {
            self.stop();
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
        if let Some((pair, metadata)) = self.held.take() {
            out.push(held_entry(pair, metadata));
        }
    }

    /// Every entry that can be formed now and should be stored.
    fn drain(&mut self) -> Vec<Entry> {
        let mut out = Vec::new();
        // Pop only when both sides have an entry: popping one and finding the
        // other empty would drop it and shift every later pairing by one.
        while let Some(pair) = self.pending.take_pair() {
            let reply = pair.reply().bytes();
            let Expect::Store { command, limit, .. } = pair.expectation() else {
                self.pending.stop();
                break;
            };
            let text = command.text();
            // Never store a reply that does not fit its command; stop
            // recording, since pairing can no longer be trusted.
            if reply.starts_with(b"(error") || !plausible(text, reply) {
                self.pending.stop();
                break;
            }
            let kind = classify(text).map(|c| c.kind).ok();
            match kind {
                Some(k) if k.is_check() => {
                    let metadata = HeldMetadata {
                        before: self.last_count,
                        effort: Effort {
                            limit: *limit,
                            used: None,
                            reason: None,
                        },
                    };
                    if let Some((previous, metadata)) = self.held.replace(pair, metadata) {
                        out.push(held_entry(previous, metadata));
                    }
                    continue;
                }
                Some(CommandKind::GetInfo {
                    keyword: b":reason-unknown",
                }) => {
                    if let Some(mut metadata) = self.held.take_metadata() {
                        metadata.effort.reason =
                            Some(String::from_utf8_lossy(reply.trim_ascii()).into_owned());
                        if !self.held.put_metadata(metadata) {
                            self.pending.stop();
                        }
                    }
                }
                Some(CommandKind::GetInfo {
                    keyword: b":all-statistics",
                }) => {
                    let count = rlimit_count(reply);
                    if let Some(mut metadata) = self.held.take_metadata() {
                        if let (Some(before), Some(after)) = (metadata.before, count) {
                            metadata.effort.used = Some(after.saturating_sub(before));
                        }
                        if !self.held.put_metadata(metadata) {
                            self.pending.stop();
                        }
                    }
                    self.last_count = count.or(self.last_count);
                    for (pair, metadata) in self.held.release_before(pair) {
                        out.push(held_entry(pair, metadata));
                    }
                    continue;
                }
                _ => {}
            }
            out.push(Entry::Paired { pair, effort: None });
        }
        out
    }

    /// Releases a held check while preserving the actual frozen input token
    /// for the joint publication transition; a silence key cannot replace it.
    fn finish_observed(&mut self) -> (Vec<Entry>, Option<TerminalInput>) {
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
    /// Permanently false after failed delivery/completion or recording capacity.
    valid: AtomicBool,
    /// Monotone shared charge for checkpoints, input tape and fresh reply copies.
    /// Already released snapshots remain charged, so no live snapshot escapes it.
    recorded_bytes: AtomicUsize,
}

impl InputStatus {
    fn charge(&self, cost: usize) -> bool {
        let accepted = self.valid.load(Ordering::SeqCst)
            && self
                .recorded_bytes
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                    used.checked_add(cost)
                        .filter(|next| *next <= PUBLICATION_LIMIT)
                })
                .is_ok();
        if !accepted {
            self.valid.store(false, Ordering::SeqCst);
        }
        accepted
    }
    fn send_expectation(&self, expects: &Sender<Expect>, expectation: Expect) {
        let expectation = if let Expect::Store { command, .. } = &expectation {
            let charge = command.retained_bytes();
            let accepted = charge.is_some_and(|charge| self.charge(charge));
            if accepted {
                expectation
            } else {
                self.valid.store(false, Ordering::SeqCst);
                Expect::Stop
            }
        } else {
            expectation
        };
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
    /// Original child lease shared only with this child's actual output owner.
    owner: SolverLease,
}

/// Sends paired replies to the daemon on its own thread, so forwarding never
/// waits on the daemon. Stops at the first daemon error.
fn spawn_storer(
    mut client: Client,
    entries: Receiver<Entry>,
    publication: Receiver<Option<SolverPublication>>,
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
                if !pending.push(entry, cost, PUBLICATION_LIMIT) {
                    return;
                }
            }
            let Some(publication) = publication.recv().ok().flatten() else {
                return;
            };
            if !batch_observed(&pending, &publication) {
                return;
            }
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
    publication: Sender<Option<SolverPublication>>,
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
    fn close(self, observation: Option<SolverPublication>) {
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
    mut process: SolverProcess,
    served: &[ServedReply],
    expects: &Receiver<Expect>,
    client: Option<Client>,
    input: &InputStatus,
    stderr: Option<&crate::cached_stderr::Monitor>,
) -> u8 {
    let owner = Arc::clone(&process.owner);
    let limit = buffer_limit();
    let mut all = sail_acceptance::query_output_owner::StreamOutput::new_replayed(
        Arc::clone(&owner),
        u64::try_from(served.len()).unwrap_or(u64::MAX),
    );
    // Once forwarding is live and nothing more will be recorded, every later
    // byte is forwarded as read, exactly as pass-through forwards it.
    let mut streaming = false;
    let mut splitter = ResponseSplitter::new();
    let mut check = ReplayCheck::new(served, Arc::clone(&owner));
    let mut forward_from: Option<usize> = all.forwarding().then_some(0);
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
                let frame = all.frame(s, start);
                if frame.is_none() {
                    pairer.stop();
                }
                let owned = if fresh && input.charge(start.saturating_sub(s)) {
                    frame.and_then(|frame| {
                        frame.into_owned(Arc::clone(&owner), &SolverOwnerIdentity)
                    })
                } else {
                    if fresh {
                        pairer.stop();
                    }
                    None
                };
                // Framing refusal stops publication. Replay comparison still
                // consumes the original bytes and keeps its original behavior.
                if !check.settle_observed(
                    fresh,
                    all.bytes().get(s..start).unwrap_or_default(),
                    &mut pairer,
                    owned,
                ) {
                    break;
                }
            }
            let fresh = all.advance_replay();
            if fresh && forward_from.is_none() {
                forward_from = Some(start);
            }
            prev = Some((fresh, start));
        }
        if check.diverged() {
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
    if check.diverged() {
        let _ = process.child.kill(); // it may wait for input that will never come
    }
    // End of output. Deliver what is left, as pass-through would.
    if !check.diverged() && forward_from.is_some() && all.len() > forwarded {
        forwarding_ok &= write_out(all.bytes().get(forwarded..).unwrap_or_default());
    }
    let (_, mut code) = match process.child.wait() {
        Ok(status) => crate::status_code(status),
        Err(_) => (None, 1),
    };
    // The last reply runs to the end of the stream, and is complete only if
    // the solver exited cleanly.
    if !check.diverged()
        && let Some((fresh, s)) = prev.take()
        && code == 0
    {
        let end = all.len();
        let frame = all.frame(s, end);
        if frame.is_none() {
            pairer.stop();
        }
        let owned = if fresh && input.charge(end.saturating_sub(s)) {
            frame.and_then(|frame| frame.into_owned(Arc::clone(&owner), &SolverOwnerIdentity))
        } else {
            if fresh {
                pairer.stop();
            }
            None
        };
        check.settle_observed(
            fresh,
            all.bytes().get(s..).unwrap_or_default(),
            &mut pairer,
            owned,
        );
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
    // Completion of the actual stdout suppression cursor is distinct from
    // completing comparisons against every original served payload.
    let output =
        output.and_then(|output| output.with_replay(&check.comparison, &SolverOwnerIdentity));
    if output.is_none() {
        pairer.stop();
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
    let publication = delivery.and_then(|input| {
        input
            .delivered
            .observe_owned_publication(input.owner, output, gate, SolverOwnerIdentity)
    });
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
    // One opaque lease is allocated beside this exact owned spawn and both
    // extracted descriptors. Strong references survive until publication ends.
    let owner = Arc::new(());
    let process = SolverProcess {
        child,
        owner: Arc::clone(&owner),
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let input = Arc::new(InputStatus {
        done: AtomicBool::new(false),
        valid: AtomicBool::new(true),
        recorded_bytes: AtomicUsize::new(
            PUBLICATION_FIXED_BYTES.saturating_add(served_storage_bytes(&served)),
        ),
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
                process,
                &served,
                &rx,
                client,
                &observed_input,
                stderr.as_ref(),
            )
        })
        .ok()?;
    let mut s = Solving {
        stdin: prefix.recorded_delivery(SolverPipe { stdin: Some(stdin) }, PUBLICATION_LIMIT),
        expects: tx,
        input,
        output,
        owner,
    };
    s.write(prefix.bytes());
    Some(s)
}

impl Solving {
    fn expect(&self, expectation: Expect) {
        self.input.send_expectation(&self.expects, expectation);
    }

    fn write(&mut self, bytes: &[u8]) {
        if !self.input.charge(bytes.len()) {
            self.stdin.forget_capture();
        }
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
                Expect::Terminal(TerminalInput {
                    delivered,
                    owner: Arc::clone(&self.owner),
                })
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
    session: &mut ObservedSession,
    item: &Item,
    prefix: &mut SessionPrefix,
) -> (Output, String, Expect) {
    // A prior native mutation cannot silently change the next command's policy.
    if !session.readback_matches() {
        prefix.refuse();
        return (Output::Unknown, String::new(), Expect::Stop);
    }
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
    if !session.apply(&item.bytes) {
        prefix.refuse();
        return (Output::Unknown, String::new(), Expect::Stop);
    }
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
            command: StoreCommand::Original(frame.freeze()),
            limit,
        },
        _ => Expect::Stop,
    };
    (output, name, expect)
}

/// The replies a session served from the cache before any solver started.
struct Served {
    /// The replies, until a solver takes them to check its replay.
    replies: Vec<ServedReply>,
    /// How many there were.
    count: u64,
    /// Retained allocation capacities and fixed receipt/controller carriers.
    bytes: usize,
}

impl Default for Served {
    fn default() -> Self {
        let replies = Vec::new();
        let bytes = served_storage_bytes(&replies);
        Self {
            replies,
            count: 0,
            bytes,
        }
    }
}

type ServedReply = sail_acceptance::query_replay::ServedReply;

/// Actual retained command/payload capacities and fixed receipt/vector/controller
/// carriers. Allocator overhead and runtime cost are not RSS measurements.
#[allow(
    clippy::ptr_arg,
    reason = "The actual receipt-vector capacity is part of the retained allocation charge."
)]
fn served_storage_bytes(replies: &Vec<ServedReply>) -> usize {
    replies.iter().fold(
        (std::mem::size_of::<Served>() + std::mem::size_of::<ReplayCheck<'_>>()).saturating_add(
            replies
                .capacity()
                .saturating_mul(std::mem::size_of::<ServedReply>()),
        ),
        |cost, reply| {
            cost.saturating_add(reply.command_storage().capacity())
                .saturating_add(reply.reply_storage().capacity())
        },
    )
}

impl Served {
    fn push(&mut self, command: &[u8], reply: Vec<u8>) {
        let reply = ServedReply::new(command, reply, statistics_spans(command));
        self.bytes = self
            .bytes
            .saturating_add(reply.command_storage().capacity())
            .saturating_add(reply.reply_storage().capacity());
        let prior_capacity = self.replies.capacity();
        self.replies.push(reply);
        self.bytes = self.bytes.saturating_add(
            self.replies
                .capacity()
                .saturating_sub(prior_capacity)
                .saturating_mul(std::mem::size_of::<ServedReply>()),
        );
        self.count += 1;
    }

    fn take(&mut self) -> Vec<ServedReply> {
        let replies = std::mem::take(&mut self.replies);
        self.bytes = served_storage_bytes(&self.replies);
        replies
    }
}

/// Checks a replay's suppressed replies against those already served.
struct ReplayCheck<'a> {
    comparison: sail_acceptance::query_replay::Replay<'a, SolverLease>,
}

impl<'a> ReplayCheck<'a> {
    fn new(served: &'a [ServedReply], owner: SolverLease) -> Self {
        Self {
            comparison: sail_acceptance::query_replay::Replay::new(served, owner),
        }
    }

    /// Whether every served reply has been compared with its replay.
    fn complete(&self) -> bool {
        self.comparison.complete()
    }

    fn diverged(&self) -> bool {
        self.comparison.diverged()
    }

    /// At the end of the replay: whether it diverged, including a `clean`
    /// replay that gave fewer replies than were served. A divergence is
    /// reported on stderr.
    fn finish(&mut self, clean: bool) -> bool {
        if self.comparison.finish(clean) {
            eprintln!(
                "error[SAIL2005]: sail-z3: the pinned solver's replay differs from reply {} \
                 served from the cache; stopping",
                self.comparison.checked() + 1
            );
        }
        self.comparison.diverged()
    }

    /// Settles one complete reply: a fresh one goes to the pairer, a
    /// suppressed one must equal the reply served in its place. Returns
    /// false once the replay has diverged.
    fn settle_observed(
        &mut self,
        fresh: bool,
        bytes: &[u8],
        pairer: &mut Pairer,
        frame: Option<SolverFrame>,
    ) -> bool {
        if fresh {
            if let Some(frame) = frame {
                pairer.reply_frame(frame);
            } else {
                pairer.stop();
            }
        } else {
            // Keep the native splitter's offset-zero/exact-one-frame refusal
            // independently of the checked mechanical comparison policy.
            if self.comparison.next_statistics() && !is_single_response(bytes) {
                self.comparison.refuse();
            } else {
                self.comparison.compare(bytes);
            }
        }
        !self.comparison.diverged()
    }

    #[cfg(test)]
    fn settle(&mut self, fresh: bool, bytes: &[u8], pairer: &mut Pairer) -> bool {
        if fresh {
            pairer.reply(bytes.to_vec());
            !self.comparison.diverged()
        } else {
            self.settle_observed(false, bytes, pairer, None)
        }
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
    let mut session = ObservedSession::new();
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
                (Output::One, Expect::Store { command, .. }) => {
                    let key = command.key();
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
    use super::{Entry, Expect, Pairer, TerminalInput};
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
    fn scalar_option_history_matches_actual_session_readback() {
        assert!(sail_acceptance::query_options::scalar_option_control());
        let mut session = super::ObservedSession::new();
        for command in [
            &b"(set-option ;head\r\n :print-success ;key\n false)"[..],
            &b"(set-option :rlimit 00010)"[..],
            &b"(set-option :unrelated (a b))"[..],
            &b"(assert true)"[..],
            &b"(push 1)"[..],
            &b"(pop 1)"[..],
            &b"(reset-assertions)"[..],
            &b"(check-sat)"[..],
        ] {
            assert!(session.apply(command));
            assert!(session.readback_matches());
        }
        assert_eq!(session.option(b":print-success"), Some(&b"false"[..]));
        assert_eq!(session.option(b":rlimit"), Some(&b"00010"[..]));
        assert!(session.apply(b"(reset)"));
        assert_eq!(session.option(b":print-success"), None);
        assert_eq!(session.option(b":rlimit"), None);
    }

    #[test]
    fn sensitive_option_projection_refuses_omission_trailing_and_compound_values() {
        for command in [
            &b"(set-option :rlimit)"[..],
            &b"(set-option :rlimit 10 20)"[..],
            &b"(set-option :rlimit (10))"[..],
            &b"(set-option :print-success \"false\")"[..],
            &b"(set-option :print-success |false|)"[..],
        ] {
            let mut session = super::ObservedSession::new();
            assert!(!session.apply(command));
            assert!(!session.apply(b"(reset)"));
            assert!(!session.readback_matches());
        }
    }

    #[test]
    fn scalar_option_witness_cannot_swap_keyword_and_value_or_truncate_either() {
        let command = b"(set-option :rlimit 10)";
        let Some(original) = super::option_spans(command) else {
            unreachable!("literal set-option supplies candidate intervals");
        };
        let mut swapped = original;
        swapped.keyword_start = original.value_start;
        swapped.keyword_end = original.value_end;
        swapped.value_start = original.keyword_start;
        swapped.value_end = original.keyword_end;
        let mut omitted_keyword = original;
        omitted_keyword.keyword_end -= 1;
        let mut omitted_value = original;
        omitted_value.value_end -= 1;
        for spans in [swapped, omitted_keyword, omitted_value] {
            let mut state = sail_acceptance::query_options::OptionState::new();
            assert!(!state.advance(command, Some(spans), None, Some(b"10")));
            assert!(!state.matches(None, Some(b"10")));
        }
    }

    #[test]
    fn native_option_mutation_cannot_change_later_policy_without_checked_transition() {
        let mut session = super::ObservedSession::new();
        assert!(session.apply(b"(set-option :print-success false)"));
        assert!(session.apply(b"(set-option :rlimit 10)"));
        session.native.apply(b"(set-option :rlimit 20)");
        let mut prefix = super::SessionPrefix::new(&identity(), &super::Mode::Default);
        let bytes = b"(check-sat)";
        prefix.append(bytes);
        let item = super::Item {
            kind: super::ItemKind::List,
            offset: 0,
            bytes: bytes.to_vec(),
        };
        let (policy, _, expectation) = super::observe(&mut session, &item, &mut prefix);
        assert_eq!(policy, super::Output::Unknown);
        assert!(matches!(expectation, super::Expect::Stop));
        let mut state = sail_acceptance::query_options::OptionState::new();
        let command = b"(set-option :rlimit 10)";
        assert!(!state.advance(command, super::option_spans(command), None, Some(b"20")));
        assert!(!state.advance(b"(reset)", None, None, None));
    }

    #[test]
    fn checked_raw_prefixes_preserve_existing_keys_and_replay_bytes() {
        use super::{CommandSplitter, Mode, ObservedSession, SessionPrefix, observe};
        use sail_key::PrefixHasher;
        let input = b" ;head\r\n(set-option :print-success false)\n(echo \"a;()\")\t;gap\n(check-sat)\n;tail";
        for width in [1, 3, 17, input.len()] {
            for mode in [Mode::Default, Mode::OptIn("test-mode".to_owned())] {
                let mut parser = CommandSplitter::new();
                let mut prefix = SessionPrefix::new(&identity(), &mode);
                let mut session = ObservedSession::new();
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
        use super::{Item, ItemKind, ObservedSession, Output, SessionPrefix, observe};
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
            let (actual, _, metadata) = observe(&mut ObservedSession::new(), &item, &mut prefix);
            assert_eq!(actual, expected);
            assert_eq!(prefix.bytes(), command);
            if expected == Output::One {
                let Expect::Store {
                    command: checkpoint,
                    ..
                } = metadata
                else {
                    unreachable!("one response needs an exact Store expectation");
                };
                let key = checkpoint.key();
                let text = checkpoint.text();
                assert_eq!(text, command);
                assert_eq!(key, prefix.key());
            } else {
                assert!(matches!(metadata, Expect::Stop));
            }
        }
    }

    #[test]
    fn wrong_missing_or_overlapping_parser_items_disable_only_cache_interpretation() {
        use super::{Item, ItemKind, ObservedSession, Output, SessionPrefix, observe};
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
            let mut session = ObservedSession::new();
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
        use super::{Item, ItemKind, ObservedSession, Output, SessionPrefix, observe};
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
            let (output, name, expected) = observe(&mut ObservedSession::new(), &item, &mut prefix);
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
            recorded_bytes: std::sync::atomic::AtomicUsize::new(0),
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

    #[test]
    fn recording_costs_share_the_original_bound_and_cannot_resume() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let status = super::InputStatus {
            done: AtomicBool::new(false),
            valid: AtomicBool::new(true),
            recorded_bytes: AtomicUsize::new(super::PUBLICATION_LIMIT - 3),
        };
        assert!(status.charge(2));
        assert!(!status.charge(2));
        assert!(!status.valid.load(Ordering::SeqCst));
        assert!(!status.charge(0));
        assert_eq!(
            status.recorded_bytes.load(Ordering::SeqCst),
            super::PUBLICATION_LIMIT - 1
        );
    }

    struct TestPipe;
    impl sail_acceptance::query_delivery::InputPipe for TestPipe {
        fn write(&mut self, _: &[u8]) -> bool {
            true
        }
        fn close(&mut self) {}
    }
    fn terminal(bytes: &[u8]) -> Result<TerminalInput, &'static str> {
        let mut prefix = sail_key::SessionPrefix::new(&identity(), &sail_key::Mode::Default);
        let mut delivery = prefix.delivery(TestPipe);
        prefix.append(bytes);
        if !prefix.hash_tail() || !delivery.write(bytes) {
            return Err("script input must be complete");
        }
        delivery
            .seal(prefix.silence_key(), true)
            .map(|delivered| TerminalInput {
                delivered,
                owner: std::sync::Arc::new(()),
            })
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
        assert_eq!(input.as_ref().map(TerminalInput::key), Some(key));
        let (again, duplicate) = pairer.finish_observed();
        assert!(again.is_empty());
        assert!(duplicate.is_none());
        Ok(())
    }

    fn owned_output(
        owner: &super::SolverLease,
    ) -> Result<
        (
            super::SolverFrame,
            sail_acceptance::query_output_owner::CompletedStream<super::SolverLease>,
        ),
        &'static str,
    > {
        let mut output =
            sail_acceptance::query_output_owner::StreamOutput::new(std::sync::Arc::clone(owner));
        output.append(b"unsat\n");
        let frame = output
            .frame(0, 6)
            .ok_or("script frame")?
            .into_owned(std::sync::Arc::clone(owner), &super::SolverOwnerIdentity)
            .ok_or("retained original lease")?;
        output.end();
        Ok((frame, output.seal().ok_or("script EOF and coverage")?))
    }

    fn terminal_checkpoint(
        bytes: &[u8],
    ) -> Result<(TerminalInput, sail_key::OwnedCommand), &'static str> {
        let mut prefix = sail_key::SessionPrefix::new(&identity(), &sail_key::Mode::Default);
        let mut delivery = prefix.recorded_delivery(TestPipe, super::PUBLICATION_LIMIT);
        prefix.append(bytes);
        let command = prefix
            .command(0, bytes.trim_ascii_end())
            .ok_or("original script command")?
            .freeze();
        if !prefix.hash_tail() || !delivery.write(bytes) {
            return Err("script complete input");
        }
        let delivered = delivery
            .seal(prefix.silence_key(), true)
            .ok_or("script frozen input")?;
        Ok((
            TerminalInput {
                delivered,
                owner: std::sync::Arc::new(()),
            },
            command,
        ))
    }

    fn recorded_expectations(
        original: &[u8],
        acknowledged: &[u8],
        capacity: usize,
    ) -> Result<(TerminalInput, Vec<Expect>), &'static str> {
        let mut prefix = sail_key::SessionPrefix::new(&identity(), &sail_key::Mode::Default);
        let mut delivery = prefix.recorded_delivery(TestPipe, capacity);
        prefix.append(original);
        let mut splitter = super::CommandSplitter::new();
        let mut items = Vec::new();
        splitter.push(original, &mut items);
        if splitter.finish().is_some() {
            return Err("complete original parser input");
        }
        let mut session = super::ObservedSession::new();
        let mut expectations = Vec::new();
        for item in items {
            match super::observe(&mut session, &item, &mut prefix) {
                (super::Output::One, _, expectation) => expectations.push(expectation),
                (super::Output::None, _, _) => {}
                _ => return Err("supported original response policy"),
            }
        }
        // Tests may intentionally declare a different acknowledged tape under
        // this same parser seed. Its actual independent terminal key does not
        // authorize the original per-command checkpoints.
        let mut actual = sail_key::SessionPrefix::new(&identity(), &sail_key::Mode::Default);
        actual.append(acknowledged);
        if !actual.hash_tail() || !delivery.write(acknowledged) {
            return Err("complete scripted actual delivery");
        }
        let delivered = delivery
            .seal(actual.silence_key(), true)
            .ok_or("actual frozen tape")?;
        Ok((
            TerminalInput {
                delivered,
                owner: std::sync::Arc::new(()),
            },
            expectations,
        ))
    }

    fn framed_replies(
        owner: &super::SolverLease,
        skipped: u64,
        replies: &[&[u8]],
    ) -> Result<
        (
            Vec<super::SolverFrame>,
            sail_acceptance::query_output_owner::CompletedStream<super::SolverLease>,
        ),
        &'static str,
    > {
        let mut output = sail_acceptance::query_output_owner::StreamOutput::new_replayed(
            std::sync::Arc::clone(owner),
            skipped,
        );
        let mut frames = Vec::new();
        for reply in replies {
            let start = output.len();
            output.append(reply);
            let end = output.len();
            let fresh = output.advance_replay();
            let frame = output
                .frame(start, end)
                .ok_or("original complete response")?;
            if fresh {
                frames.push(
                    frame
                        .into_owned(std::sync::Arc::clone(owner), &super::SolverOwnerIdentity)
                        .ok_or("original retained response owner")?,
                );
            }
        }
        output.end();
        Ok((
            frames,
            output.seal().ok_or("actual original EOF and coverage")?,
        ))
    }

    fn publication_gate() -> super::PublicationGate {
        super::PublicationGate {
            input_closed: true,
            pairing_active: true,
            queues_empty: true,
            normal_exit: true,
            quiet: true,
            forwarding_ok: true,
        }
    }

    fn pair_batch(
        expectations: Vec<Expect>,
        frames: Vec<super::SolverFrame>,
    ) -> Result<super::PublicationBatch<Entry>, &'static str> {
        if expectations.len() != frames.len() {
            return Err("complete declared fixture pair domain");
        }
        let mut pairer = Pairer::default();
        let mut entries = Vec::new();
        for (expectation, frame) in expectations.into_iter().zip(frames) {
            pairer.expect(expectation);
            pairer.reply_frame(frame);
            entries.extend(pairer.drain());
        }
        entries.extend(pairer.finish());
        let mut pending = super::PublicationBatch::empty();
        for entry in entries {
            let cost = entry.wire_cost().ok_or("bounded original entry")?;
            if !pending.push(entry, cost, super::PUBLICATION_LIMIT) {
                return Err("original fixture publication bound");
            }
        }
        Ok(pending)
    }

    fn batch_pairs(
        pending: &super::PublicationBatch<Entry>,
    ) -> Result<Vec<(&sail_key::OwnedCommand, &super::SolverFrame)>, &'static str> {
        pending
            .entries()
            .iter()
            .map(|entry| {
                let Entry::Paired { pair, .. } = entry else {
                    return Err("original paired fixture entry");
                };
                let Expect::Store {
                    command: super::StoreCommand::Original(command),
                    ..
                } = pair.expectation()
                else {
                    return Err("original actual checkpoint");
                };
                Ok((command, pair.reply()))
            })
            .collect()
    }

    #[test]
    fn actual_checkpoint_binding_preserves_held_release_permutation() -> Result<(), &'static str> {
        let bytes=b"(get-info :all-statistics)\n(check-sat)\n(get-info :reason-unknown)\n(get-info :all-statistics)\n";
        let (input, expectations) = recorded_expectations(bytes, bytes, super::PUBLICATION_LIMIT)?;
        let (frames, output) = framed_replies(
            &input.owner,
            0,
            &[
                b"(:rlimit-count 10)\n",
                b"unknown\n",
                b"(:reason-unknown \"canceled\")\n",
                b"(:rlimit-count 13)\n",
            ],
        )?;
        let publication = input
            .delivered
            .observe_owned_publication(
                input.owner,
                Some(output),
                publication_gate(),
                super::SolverOwnerIdentity,
            )
            .ok_or("original complete fixture publication")?;
        let pending = pair_batch(expectations, frames)?;
        let pairs = batch_pairs(&pending)?;
        assert_eq!(
            pairs
                .iter()
                .map(|(_, frame)| frame.payload().ordinal())
                .collect::<Vec<_>>(),
            vec![0, 2, 1, 3]
        );
        assert!(super::batch_observed(&pending, &publication));
        assert!(publication.validate_pairs(&pairs));
        assert!(!publication.validate_pairs(&pairs[..3]));
        let wrong: Vec<_> = pairs
            .iter()
            .map(|(command, _)| (*command, pairs[0].1))
            .collect();
        assert!(!publication.validate_pairs(&wrong));
        let expected: Vec<_> = pairs
            .iter()
            .map(|(command, frame)| {
                (
                    command.key(),
                    command.head().to_vec(),
                    frame.bytes().to_vec(),
                )
            })
            .collect();
        let released = publication
            .release(pending)
            .ok_or("same original payload release")?;
        assert_eq!(released.len(), expected.len());
        for (entry, (key, head, reply)) in released.into_iter().zip(expected) {
            let (actual_key, meta, actual_reply) =
                entry.into_put().ok_or("original wire conversion")?;
            assert_eq!(QueryKey(actual_key), key);
            assert_eq!(meta.command.as_bytes(), head);
            assert_eq!(meta.provenance, "pinned-solver");
            assert_eq!(actual_reply, reply);
        }
        Ok(())
    }

    #[test]
    fn identical_reply_bytes_cannot_swap_original_command_associations() -> Result<(), &'static str>
    {
        let bytes = b"(get-info :name)\n(get-info :name)\n";
        let (input, expectations) = recorded_expectations(bytes, bytes, super::PUBLICATION_LIMIT)?;
        let (frames, output) =
            framed_replies(&input.owner, 0, &[b"(:name \"z3\")\n", b"(:name \"z3\")\n"])?;
        let publication = input
            .delivered
            .observe_owned_publication(
                input.owner,
                Some(output),
                publication_gate(),
                super::SolverOwnerIdentity,
            )
            .ok_or("original complete fixture publication")?;
        let pending = pair_batch(expectations, frames)?;
        let pairs = batch_pairs(&pending)?;
        assert_eq!(pairs[0].1.bytes(), pairs[1].1.bytes());
        assert_ne!(pairs[0].0.key(), pairs[1].0.key());
        assert!(publication.validate_pairs(&pairs));
        assert!(!publication.validate_pairs(&[(pairs[0].0, pairs[1].1), (pairs[1].0, pairs[0].1)]));
        Ok(())
    }

    #[test]
    fn parser_lineage_and_terminal_key_do_not_replace_actual_prefix_bytes()
    -> Result<(), &'static str> {
        for (actual, capacity) in [
            (b";b\n(check-sat)\n".as_slice(), super::PUBLICATION_LIMIT),
            (b";a\n(check-sat)\n".as_slice(), 1),
        ] {
            let (input, expectations) =
                recorded_expectations(b";a\n(check-sat)\n", actual, capacity)?;
            let (frames, output) = framed_replies(&input.owner, 0, &[b"unsat\n"])?;
            let publication = input
                .delivered
                .observe_owned_publication(
                    input.owner,
                    Some(output),
                    publication_gate(),
                    super::SolverOwnerIdentity,
                )
                .ok_or("original fixture stream/child owners")?;
            let pending = pair_batch(expectations, frames)?;
            let pairs = batch_pairs(&pending)?;
            assert!(publication.same_command_session(pairs[0].0));
            assert!(!publication.validate_pairs(&pairs));
            assert!(!super::batch_observed(&pending, &publication));
        }
        Ok(())
    }

    #[test]
    fn fresh_bindings_require_this_output_owners_complete_replay_prefix() -> Result<(), &'static str>
    {
        let bytes = b"(get-info :name)\n(get-info :name)\n";
        for skipped in [1, 0, 3] {
            let (input, mut expectations) =
                recorded_expectations(bytes, bytes, super::PUBLICATION_LIMIT)?;
            let Expect::Store {
                command: super::StoreCommand::Original(command),
                ..
            } = expectations.pop().ok_or("fresh original expectation")?
            else {
                return Err("fresh original checkpoint");
            };
            let (mut frames, output) = framed_replies(
                &input.owner,
                skipped,
                &[b"(:name \"z3\")\n", b"(:name \"z3\")\n"],
            )?;
            let publication = input
                .delivered
                .observe_owned_publication(
                    input.owner,
                    Some(output),
                    publication_gate(),
                    super::SolverOwnerIdentity,
                )
                .ok_or("original output owner and complete coverage")?;
            if let Some(frame) = frames.pop() {
                assert_eq!(frame.payload().ordinal(), 1);
                assert_eq!(
                    publication.validate_pairs(&[(&command, &frame)]),
                    skipped == 1
                );
            } else {
                assert_eq!(skipped, 3);
                assert!(!publication.validate_pairs(&[]));
            }
        }
        Ok(())
    }

    /// Native Arc identity and scripted streams only; this is not an actual
    /// trusted solver or physical launch/pipe qualification.
    #[test]
    fn equal_byte_streams_cannot_swap_original_owners_or_child_publication()
    -> Result<(), &'static str> {
        let (original, first_command) = terminal_checkpoint(b"(check-sat)\n")?;
        let (other, second_command) = terminal_checkpoint(b"(check-sat)\n")?;
        assert_eq!(first_command.key(), second_command.key());
        let (first, output) = owned_output(&original.owner)?;
        let (second, other_output) = owned_output(&other.owner)?;
        assert_eq!(first.bytes(), second.bytes());
        assert_eq!(first.payload().ordinal(), second.payload().ordinal());
        assert_eq!(first.payload().start(), second.payload().start());
        assert_eq!(first.payload().end(), second.payload().end());
        assert!(output.contains_frame(&first, &super::SolverOwnerIdentity));
        assert!(!other_output.contains_frame(&first, &super::SolverOwnerIdentity));
        let gate = publication_gate();
        let publication = original
            .delivered
            .observe_owned_publication(
                original.owner,
                Some(output),
                gate,
                super::SolverOwnerIdentity,
            )
            .ok_or("original stream/child leases")?;
        assert!(publication.contains_frame(&first));
        assert!(!publication.contains_frame(&second));
        assert!(publication.same_command_session(&first_command));
        assert!(!publication.same_command_session(&second_command));
        let mut pairs = super::CorrelatedQueue::empty();
        pairs.expect(Expect::Store {
            command: super::StoreCommand::Original(first_command),
            limit: None,
        });
        pairs.reply(first);
        let mut original_batch = super::PublicationBatch::empty();
        assert!(original_batch.push(
            Entry::Paired {
                pair: pairs.take_pair().ok_or("original pair")?,
                effort: None
            },
            6,
            6
        ));
        assert!(super::batch_observed(&original_batch, &publication));
        pairs.expect(Expect::Store {
            command: super::StoreCommand::Original(second_command),
            limit: None,
        });
        pairs.reply(second);
        let mut swapped_batch = super::PublicationBatch::empty();
        assert!(swapped_batch.push(
            Entry::Paired {
                pair: pairs.take_pair().ok_or("swapped pair")?,
                effort: None
            },
            6,
            6
        ));
        assert!(!super::batch_observed(&swapped_batch, &publication));
        if let Expect::Store { command, .. } = store(1, "(check-sat)") {
            assert!(
                !command.observed_by(&publication),
                "symbolic metadata never supplies live publication authority"
            );
        }
        assert!(
            Entry::Silence {
                key: publication.input_key()
            }
            .observed_by(&publication)
        );
        assert!(!Entry::Silence { key: key(255) }.observed_by(&publication));
        let third = terminal(b"(check-sat)\n")?;
        let gate = publication_gate();
        assert!(
            third
                .delivered
                .observe_owned_publication(
                    third.owner,
                    Some(other_output),
                    gate,
                    super::SolverOwnerIdentity
                )
                .is_none()
        );
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
            command: super::StoreCommand::Symbolic {
                key: key(i),
                command,
                text: text.as_bytes().to_vec(),
            },
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
            Expect::Store { command, .. } => Expect::Store {
                command,
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
    fn consecutive_checks_release_original_payloads_and_annotations() {
        let mut pairer = Pairer::default();
        pairer.expect(check(1, 10));
        pairer.reply(b"unsat\n".to_vec());
        pairer.expect(check(2, 20));
        pairer.reply(b"sat\n".to_vec());
        let displaced = stored_entries(pairer.drain());
        assert_eq!(displaced.len(), 1);
        assert_eq!(displaced[0].key, key(1));
        assert_eq!(displaced[0].reply, b"unsat\n");
        assert_eq!(
            displaced[0].effort.as_ref().and_then(|effort| effort.limit),
            Some(10)
        );
        let last = stored_entries(pairer.finish());
        assert_eq!(last.len(), 1);
        assert_eq!(last[0].key, key(2));
        assert_eq!(last[0].reply, b"sat\n");
        assert_eq!(
            last[0].effort.as_ref().and_then(|effort| effort.limit),
            Some(20)
        );
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
        let mut check = super::ReplayCheck::new(&replies, std::sync::Arc::new(()));
        let mut pairer = Pairer::default();
        assert!(check.settle(false, b"(model)\n", &mut pairer));
        assert!(!check.finish(false), "an unclean end is not judged");
        assert!(
            check.finish(true),
            "a clean replay one reply short diverged"
        );
        let mut check = super::ReplayCheck::new(&replies, std::sync::Arc::new(()));
        assert!(!check.settle(false, b"(model 1)\n", &mut pairer));
        assert!(check.diverged());
    }

    #[test]
    fn raw_statistics_mint_requires_the_original_complete_keyword_command() {
        use sail_acceptance::query_replay::{ServedReply, statistics_command};
        for command in [
            &b"(get-info :all-statistics)"[..],
            &b"( ;head\nget-info;gap\r\n :all-statistics ;tail\n)"[..],
        ] {
            assert!(statistics_command(
                command,
                super::statistics_spans(command)
            ));
        }
        for command in [
            &b"(get-info :all-statistics extra)"[..],
            &b"(get-info :all-statistics-more)"[..],
            &b"(get-info |:all-statistics|)"[..],
            &b"(get-model :all-statistics)"[..],
        ] {
            let receipt = ServedReply::new(
                command,
                b"(:x 1)\n".to_vec(),
                super::statistics_spans(command),
            );
            assert!(!receipt.statistics());
            assert!(!receipt.reproduced_by(b"(:x 2)\n"));
        }
        assert!(!super::plausible(
            b"(get-info :all-statistics extra)",
            b"(:x 1)\n"
        ));
        assert!(sail_acceptance::query_replay::replay_comparison_control());
    }

    #[test]
    fn replay_join_requires_complete_comparisons_original_count_and_same_owner() {
        use sail_acceptance::query_output_owner::StreamOutput;
        use sail_acceptance::query_replay::{Replay, ServedReply};
        let owner = std::sync::Arc::new(());
        let served = [ServedReply::new(b"(check-sat)", b"unsat\n".to_vec(), None)];
        for (count, actual_owner, compare, accepted) in [
            (1, std::sync::Arc::clone(&owner), true, true),
            (1, std::sync::Arc::clone(&owner), false, false),
            (0, std::sync::Arc::clone(&owner), true, false),
            (1, std::sync::Arc::new(()), true, false),
        ] {
            let mut replay = Replay::new(&served, std::sync::Arc::clone(&owner));
            let mut output = StreamOutput::new_replayed(actual_owner, count);
            output.append(b"unsat\n");
            assert!(output.frame(0, 6).is_some());
            assert_eq!(output.advance_replay(), count == 0);
            output.end();
            let Some(output) = output.seal() else {
                unreachable!("complete literal output frame");
            };
            if compare {
                assert!(replay.compare(b"unsat\n"));
            }
            assert_eq!(
                output
                    .with_replay(&replay, &super::SolverOwnerIdentity)
                    .is_some(),
                accepted
            );
        }
    }

    #[test]
    fn served_receipt_budget_counts_original_commands_payloads_and_carriers() {
        let mut served = super::Served::default();
        assert_eq!(served.bytes, super::served_storage_bytes(&served.replies));
        served.push(b"(check-sat)", b"unsat\n".to_vec());
        served.push(b"(get-info :all-statistics)", b"(:x 1)\n".to_vec());
        assert_eq!(served.count, 2);
        assert_eq!(served.bytes, super::served_storage_bytes(&served.replies));
        let retained = served.take();
        assert_eq!(served.bytes, super::served_storage_bytes(&Vec::new()));
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0].command(), b"(check-sat)");
        assert_eq!(retained[1].bytes(), b"(:x 1)\n");
    }

    #[test]
    fn replay_join_refuses_same_owner_count_with_different_retained_stdout() {
        use sail_acceptance::query_output_owner::StreamOutput;
        use sail_acceptance::query_replay::{Replay, ServedReply};
        let owner = std::sync::Arc::new(());
        let served = [ServedReply::new(b"(check-sat)", b"unsat\n".to_vec(), None)];
        let mut replay = Replay::new(&served, std::sync::Arc::clone(&owner));
        assert!(replay.compare(b"unsat\n"));
        assert!(replay.complete());
        let mut output = StreamOutput::new_replayed(std::sync::Arc::clone(&owner), 1);
        output.append(b"sat\n");
        assert!(output.frame(0, 4).is_some());
        assert!(!output.advance_replay());
        output.end();
        let Some(output) = output.seal() else {
            unreachable!("complete literal actual stdout frame");
        };
        assert!(
            output
                .with_replay(&replay, &super::SolverOwnerIdentity)
                .is_none()
        );
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
