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

use std::fmt;

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
#[derive(Clone)]
pub struct PrefixHasher {
    hasher: Sha256,
    bytes: u64,
}

impl fmt::Debug for PrefixHasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrefixHasher({} bytes)", self.bytes)
    }
}

impl PrefixHasher {
    /// A hasher for a new process of `solver` under `mode`.
    #[must_use]
    pub fn new(solver: &SolverIdentity, mode: &Mode) -> Self {
        let mut hasher = Sha256::new();
        field(&mut hasher, b"sail-key/prefix/v2");
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
        Self { hasher, bytes: 0 }
    }

    /// Feeds the next bytes the process received.
    pub fn update(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
        self.bytes += bytes.len() as u64;
    }

    /// Bytes fed so far.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.bytes
    }

    /// Whether nothing has been fed yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes == 0
    }

    /// The key for a check whose command ends exactly at the bytes fed so
    /// far. The total length is bound, so a prefix and its extension never
    /// share a key.
    #[must_use]
    pub fn key(&self) -> QueryKey {
        let mut h = self.hasher.clone();
        h.update(b"\x00sail-key/prefix/v2/end");
        h.update(self.bytes.to_le_bytes());
        QueryKey(h.finalize().into())
    }

    /// Key for evidence that the complete input ended cleanly without solver
    /// diagnostics or unexpected replies. Feed every input byte, including
    /// trailing whitespace, before calling. This domain cannot be used as a
    /// normal command-reply key.
    #[must_use]
    pub fn silence_key(&self) -> QueryKey {
        let mut h = Sha256::new();
        field(&mut h, b"sail-key/session-silence/v1");
        field(&mut h, &self.key().0);
        QueryKey(h.finalize().into())
    }
}

/// SHA-256 of a byte string, for solver binary identity and trace digests.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
