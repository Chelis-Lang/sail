//! Kani conformance: Sail's Kani backend against direct stock Kani (ADR 0181,
//! ADR 0183).
//!
//! The oracle is the original Kani driver at the same pin, run without Sail,
//! with the same argv, environment and project bytes, at least twice per
//! harness. [`compare`] checks one pair of [`record::KaniRunRecord`]s field by
//! field: exit status, every property record (property id, class,
//! description, location, status), counts, the failed-check list, the backend
//! configuration, the rest of the export, and every stdout and stderr line.
//! A field may differ only if it is named in [`compare::VARIANCE`] and two
//! direct runs of the same project were seen to differ on it.
//!
//! [`compare::classify`] turns the comparisons of one harness into a
//! [`compare::KaniVerdict`]. Sail making no claim where direct Kani claims
//! one is a downgrade, counted on its own and never as `same`.
//!
//! The runner (`runner`, Linux only) drives both sides. The Sail side is an
//! adapter: today it is one `sail verify --backend kani` call per harness;
//! a batch executor replaces only that adapter. The fault arm (`fault`)
//! installs a stand-in `cbmc` and requires that no fault ever yields a
//! Sail success or a cached result.
pub mod compare;
pub mod corpus;
pub mod record;
mod strict_json;

#[cfg(target_os = "linux")]
pub mod fault;
#[cfg(target_os = "linux")]
pub mod runner;

pub use compare::{KaniComparison, KaniVerdict, classify, compare};
pub use record::{Arm, Claim, KaniRunRecord, NoClaim};

use sha2::{Digest as _, Sha256};

/// A Kani conformance harness error. Never a conformance result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    code: &'static str,
    message: String,
}

impl Error {
    pub(crate) fn corpus(message: impl Into<String>) -> Self {
        Self {
            code: "SAIL6320",
            message: message.into(),
        }
    }
    pub(crate) fn prepare(message: impl Into<String>) -> Self {
        Self {
            code: "SAIL6321",
            message: message.into(),
        }
    }
    pub(crate) fn arm(message: impl Into<String>) -> Self {
        Self {
            code: "SAIL6322",
            message: message.into(),
        }
    }
    /// The stable `SAILnnnn` code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// Lower-case hex sha256.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
