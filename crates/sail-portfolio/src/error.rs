//! Errors with stable codes (`SAIL51xx`).

use std::fmt;
use std::path::{Path, PathBuf};

/// Everything that can stop a fragility measurement.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PortfolioError {
    /// `SAIL5101`: a file or directory could not be read.
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error, as text.
        message: String,
    },
    /// `SAIL5102`: invalid options (for example zero configurations).
    Options(String),
}

impl PortfolioError {
    /// The stable code for this error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "SAIL5101",
            Self::Options(_) => "SAIL5102",
        }
    }

    /// A [`PortfolioError::Io`] for `path`.
    #[must_use]
    pub fn io(path: &Path, err: &std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            message: err.to_string(),
        }
    }
}

impl fmt::Display for PortfolioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: ", self.code())?;
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Options(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for PortfolioError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_carries_the_code() {
        let e = PortfolioError::Options("no configurations".to_owned());
        assert_eq!(e.to_string(), "error[SAIL5102]: no configurations");
        let e = PortfolioError::io(Path::new("x"), &std::io::Error::other("boom"));
        assert_eq!(e.code(), "SAIL5101");
    }
}
