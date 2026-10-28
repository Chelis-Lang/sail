//! Content-addressed keys for solver queries.
//!
//! Two key versions exist:
//! - **v2, [`PrefixHasher`]: the default-mode key (ADR 0008).** It binds the
//!   mode, the solver identity (including the platform) and every raw byte
//!   the solver process received, up to and including the check command.
//! - **v1, [`query_key`]: stack only.** It is used for measurement, and is
//!   the candidate key for the named opt-in mode (ADR 0010). It is described
//!   below.
//!
//! A v1 [`QueryKey`] is a SHA-256 digest that binds:
//! - the verification mode, so opt-in modes can never collide with default
//!   mode (charter rule 8);
//! - the exact solver identity: name, version string and binary hash
//!   (rule 2);
//! - the complete modelled state at the check: every push level and its
//!   commands, every option in effect, `set-logic`/`set-info`, and the check
//!   command itself (rule 2).
//!
//! Inputs are length-prefixed before hashing, so distinct inputs cannot
//! produce the same byte stream. No normalization is applied (rule 5). A
//! snapshot the session model could not follow exactly has no key.
#![forbid(unsafe_code)]

mod prefix_factory;

use std::fmt;

use sail_acceptance::key_prefix::{HashHost, Prefix};
use sail_acceptance::query_prefix::{FramedCommand as CoreCommand, QueryPrefix as QueryState};
use sail_acceptance::query_raw::PrefixFeed;
use sail_session::Snapshot;
use sha2::{Digest, Sha256};

/// The exact identity of a solver binary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SolverIdentity {
    /// Solver family, for example `"z3"`.
    pub name: String,
    /// The version string the solver reports, for example `"4.16.0"`.
    pub version: String,
    /// SHA-256 of the solver executable.
    pub binary_sha256: [u8; 32],
    /// Platform class, for example `"macos-aarch64"` (ADR 0004 update,
    /// ADR 0010): the same binary can spend different effort on different
    /// platforms.
    pub platform: String,
}

impl SolverIdentity {
    /// The platform class of this machine: `<os>-<arch>`.
    #[must_use]
    pub fn host_platform() -> String {
        format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

/// The verification mode a result is produced under.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Default mode: observationally identical to plain Verus.
    Default,
    /// A named opt-in mode (recorded in `docs/divergences.md`).
    OptIn(String),
}

/// A content-addressed key for one solver query.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueryKey(pub [u8; 32]);

impl fmt::Display for QueryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for QueryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QueryKey({self})")
    }
}

fn field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Derives the key for `snapshot` under `mode` on `solver`, or `None` if the
/// session model was inexact at that point.
#[must_use]
pub fn query_key(snapshot: &Snapshot, solver: &SolverIdentity, mode: &Mode) -> Option<QueryKey> {
    if snapshot.inexact.is_some() {
        return None;
    }
    let mut hasher = Sha256::new();
    field(&mut hasher, b"sail-key/query/v1");
    match mode {
        Mode::Default => field(&mut hasher, b"mode:default"),
        Mode::OptIn(name) => {
            field(&mut hasher, b"mode:opt-in");
            field(&mut hasher, name.as_bytes());
        }
    }
    field(&mut hasher, solver.name.as_bytes());
    field(&mut hasher, solver.version.as_bytes());
    field(&mut hasher, &solver.binary_sha256);
    field(&mut hasher, solver.platform.as_bytes());
    let mut state = Vec::new();
    snapshot.encode_state(&mut state);
    field(&mut hasher, &state);
    Some(QueryKey(hasher.finalize().into()))
}

/// The running v2 key of one solver process (ADR 0008).
///
/// Feed it every byte sent to the process, exactly as sent, in order. At a
/// check, after feeding the check command's bytes, [`PrefixHasher::key`]
/// gives the key. Nothing is normalized: whitespace and comments between
/// commands count too.
pub struct PrefixHasher {
    prefix: Prefix<Sha256Host>,
}

// The source-bound hash primitive boundary: fresh, exact update, copied state,
// and SHA-256 finalization. No result or cache provenance is inferred here.
struct Sha256Host(Sha256);

impl HashHost for Sha256Host {
    fn append(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    fn duplicate(&self) -> Self {
        Self(self.0.clone())
    }
    fn empty(&self) -> Self {
        Self(Sha256::new())
    }
    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

impl Clone for PrefixHasher {
    fn clone(&self) -> Self {
        Self {
            prefix: self.prefix.duplicate(),
        }
    }
}

impl fmt::Debug for PrefixHasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrefixHasher({} bytes)", self.len())
    }
}

impl PrefixHasher {
    /// A hasher for a new process of `solver` under `mode`.
    #[must_use]
    pub fn new(solver: &SolverIdentity, mode: &Mode) -> Self {
        Self {
            prefix: prefix_factory::new_prefix(Sha256Host(Sha256::new()), solver, mode),
        }
    }

    /// Feeds the next bytes the process received without changing their order.
    /// Counter overflow permanently removes exact-length authority.
    pub fn update(&mut self, bytes: &[u8]) {
        self.prefix.update(bytes);
    }

    /// Feeds a checked raw interval only at its exact next byte position.
    /// Failure permanently removes exact-length authority without changing input forwarding.
    pub fn feed(&mut self, feed: &PrefixFeed<'_>) -> bool {
        self.prefix.feed(feed)
    }

    /// Bytes fed so far. Use [`Self::is_exact`] before relying on the counter.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.prefix.len()
    }

    /// Whether nothing has been fed yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the cumulative length still represents every supplied byte.
    #[must_use]
    pub fn is_exact(&self) -> bool {
        self.prefix.is_exact()
    }

    /// The unchanged v2 key, including the end marker and cumulative length.
    /// Cache users must also require [`Self::is_exact`].
    #[must_use]
    pub fn key(&self) -> QueryKey {
        QueryKey(self.prefix.key())
    }

    /// Key for complete clean session silence, distinct from normal reply keys.
    /// Feed all trailing bytes first and require [`Self::is_exact`].
    #[must_use]
    pub fn silence_key(&self) -> QueryKey {
        QueryKey(self.prefix.silence_key())
    }
}

/// Immutable borrow of one actual same-owner command transition.
/// Its key is computed only when requested, without copying the hash for no-output commands.
pub struct CommandFrame<'command, 'owner> {
    frame: CoreCommand<'command, 'owner, Sha256Host>,
}
impl CommandFrame<'_, '_> {
    /// The command's exact supported head bytes.
    #[must_use]
    pub fn head(&self) -> &[u8] {
        self.frame.head()
    }
    /// Computes the default successful-command response count from this exact accepted head.
    #[must_use]
    pub fn policy(&self) -> sail_acceptance::query_policy::ResponsePolicy {
        self.frame.policy()
    }
    /// The exact endpoint at this command transition.
    #[must_use]
    pub fn end(&self) -> usize {
        self.frame.end()
    }
    /// Derives the unchanged v2 key while this transition's hash state is immutably borrowed.
    #[must_use]
    pub fn key(&self) -> QueryKey {
        QueryKey(self.frame.key())
    }
}

/// One cache session's jointly owned exact raw transcript and incremental v2 key.
/// Unlike a detached hasher, this API cannot consume another transcript's feed tokens.
pub struct SessionPrefix {
    seed: Prefix<Sha256Host>,
    session: QueryState<Sha256Host>,
}

impl SessionPrefix {
    /// Starts exact framing for the actual solver identity and mode.
    #[must_use]
    pub fn new(solver: &SolverIdentity, mode: &Mode) -> Self {
        let (session, seed) =
            prefix_factory::new_session_seed(Sha256Host(Sha256::new()), solver, mode);
        Self { seed, session }
    }
    /// Transfers a fresh actual solver input pipe into an independent delivery
    /// checker seeded with this session's exact solver and mode header.
    #[must_use]
    pub fn delivery<Pipe: sail_acceptance::query_delivery::InputPipe>(
        &self,
        pipe: Pipe,
    ) -> InputDelivery<Pipe> {
        InputDelivery {
            delivery: sail_acceptance::query_delivery::Delivery::new(pipe, self.seed.duplicate()),
        }
    }
    /// Appends exactly the bytes received by this session.
    pub fn append(&mut self, bytes: &[u8]) {
        self.session.append(bytes);
    }
    /// Interprets only an exact command interval of this owner's bytes.
    pub fn command<'a, 'b>(
        &'b mut self,
        offset: usize,
        command: &'a [u8],
    ) -> Option<CommandFrame<'a, 'b>> {
        self.session
            .command(offset, command)
            .map(|frame| CommandFrame { frame })
    }

    /// Borrows all original bytes for pinned-solver replay.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.session.bytes()
    }
    /// Permanently refuses cache interpretation without changing the original bytes.
    pub fn refuse(&mut self) {
        self.session.refuse();
    }
    /// Whether interpretation remains valid under the checked same-owner invariant.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.session.valid()
    }
    /// Number of original bytes already included in this owner's hash.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.session.len()
    }
    /// Whether no input byte has entered this owner's hash.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.session.is_empty()
    }
    /// Whether the remaining original input is only whitespace/comments.
    #[must_use]
    pub fn trailing_trivia(&self) -> bool {
        self.session.trailing_trivia()
    }
    /// Hashes the owned remainder; this does not claim EOF or a clean solver run.
    pub fn hash_tail(&mut self) -> bool {
        self.session.hash_tail()
    }
    /// The current same-owner normal-key candidate.
    #[must_use]
    pub fn key(&self) -> QueryKey {
        QueryKey(self.session.key())
    }
    /// The current same-owner silence-key candidate; clean publication remains separate.
    #[must_use]
    pub fn silence_key(&self) -> QueryKey {
        QueryKey(self.session.silence_key())
    }
}

/// SHA-256 of a byte string, for solver binary identity and trace digests.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Independent full-write accounting for one actual solver input pipe.
/// Its closure check binds delivered bytes to the parser session terminal key.
pub struct InputDelivery<Pipe> {
    delivery: sail_acceptance::query_delivery::Delivery<Pipe, Sha256Host>,
}
impl<Pipe: sail_acceptance::query_delivery::InputPipe> InputDelivery<Pipe> {
    /// Calls the owned pipe's full write and flush before acknowledging bytes.
    pub fn write(&mut self, bytes: &[u8]) -> bool {
        self.delivery.write(bytes)
    }
    /// Whether a delivery failure has permanently withheld eligibility.
    #[must_use]
    pub fn active(&self) -> bool {
        self.delivery.active()
    }
    /// Checks acknowledged input against this session's parser terminal key.
    #[must_use]
    pub fn matches(&self, expected: QueryKey, complete: bool) -> bool {
        self.delivery.matches(expected.0, complete)
    }
    /// Freezes this input once; no later writes can qualify or modify the token.
    pub fn seal(&mut self, expected: QueryKey, complete: bool) -> Option<DeliveredInput> {
        self.delivery
            .seal(expected.0, complete)
            .map(|delivered| DeliveredInput { delivered })
    }
    /// Closes the exact owned input pipe.
    pub fn close(&mut self) {
        self.delivery.close();
    }
}

/// Frozen complete input for one actual solver delivery. This is not yet a
/// clean solver-output or executable-image certificate.
pub struct DeliveredInput {
    delivered: sail_acceptance::query_delivery::Delivered<Sha256Host>,
}
impl DeliveredInput {
    /// The complete-input key retained by the private delivery token.
    #[must_use]
    pub fn key(&self) -> QueryKey {
        QueryKey(self.delivered.key())
    }
}
