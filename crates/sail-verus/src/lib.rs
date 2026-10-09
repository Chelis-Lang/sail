//! Verus integration: version pinning, the corpus manifest, and running
//! Verus on corpus units with or without the shim.
//!
//! Status (M3): pins, the corpus, and invocation of single-file and
//! single-crate (`build = "verus"`) and Cargo package (`build =
//! "cargo-verus"`, with optional Verus dependency patching) units.
//! Query-to-function mapping and diagnostic handling come in a later wave.
#![forbid(unsafe_code)]

pub mod adapter;
pub mod attribution;
pub mod corpus;
pub mod diagnostics;
pub mod managed;
pub mod patch;
pub mod report;
mod report_scope;
pub mod run;
pub mod rvt;
pub mod source_build;
pub mod toolchain;

/// An error with a stable code (`SAIL3xxx`) and a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The stable code, for example `"SAIL3001"`.
    pub code: &'static str,
    /// What went wrong, with enough context to act on it.
    pub message: String,
}

impl Error {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// Result type for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// The root of sail's cache: `$SAIL_CACHE_DIR`, else `$XDG_CACHE_HOME/sail`,
/// else `~/.cache/sail`.
///
/// # Errors
///
/// `SAIL3001` if none of those variables is set.
pub fn cache_dir() -> Result<std::path::PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(dir) = var("SAIL_CACHE_DIR") {
        return Ok(dir.into());
    }
    var("XDG_CACHE_HOME")
        .map(|d| std::path::PathBuf::from(d).join("sail"))
        .or_else(|| var("HOME").map(|h| std::path::PathBuf::from(h).join(".cache").join("sail")))
        .ok_or_else(|| {
            Error::new(
                "SAIL3001",
                "none of SAIL_CACHE_DIR, XDG_CACHE_HOME or HOME is set",
            )
        })
}
