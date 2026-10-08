//! Content-addressed keys for solver queries.
//!
//! A [`QueryKey`] is a SHA-256 digest that binds:
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
    let mut state = Vec::new();
    snapshot.encode_state(&mut state);
    field(&mut hasher, &state);
    Some(QueryKey(hasher.finalize().into()))
}

/// SHA-256 of a byte string, for solver binary identity and trace digests.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
