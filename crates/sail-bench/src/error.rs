//! Errors with stable codes (`SAIL7xxx`).

use std::fmt;
use std::path::{Path, PathBuf};

/// Everything that can stop a `sail-bench` command.
#[derive(Debug)]
#[non_exhaustive]
pub enum BenchError {
    /// `SAIL7001`: bad command-line usage.
    Usage(String),
    /// `SAIL7002`: a file or directory could not be read or written.
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error, as text.
        message: String,
    },
    /// `SAIL7003`: a trace line is not a valid trace record.
    TraceFormat {
        /// The `.jsonl` file.
        path: PathBuf,
        /// 1-based line number.
        line: usize,
        /// What is wrong.
        message: String,
    },
    /// `SAIL7004`: the solver could not be started or driven.
    Solver(String),
    /// `SAIL7005`: results could not be serialized.
    Json(String),
}

impl BenchError {
    /// The stable code for this error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Usage(_) => "SAIL7001",
            Self::Io { .. } => "SAIL7002",
            Self::TraceFormat { .. } => "SAIL7003",
            Self::Solver(_) => "SAIL7004",
            Self::Json(_) => "SAIL7005",
        }
    }

    /// An [`BenchError::Io`] for `path`.
    #[must_use]
    pub fn io(path: &Path, err: &std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            message: err.to_string(),
        }
    }
}

impl fmt::Display for BenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: ", self.code())?;
        match self {
            Self::Usage(m) => write!(f, "{m}"),
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::TraceFormat {
                path,
                line,
                message,
            } => write!(f, "{}:{line}: {message}", path.display()),
            Self::Solver(m) => write!(f, "solver: {m}"),
            Self::Json(m) => write!(f, "json: {m}"),
        }
    }
}

impl std::error::Error for BenchError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_carries_the_code() {
        let e = BenchError::Usage("no trace directory".to_owned());
        assert_eq!(e.to_string(), "error[SAIL7001]: no trace directory");
        let e = BenchError::TraceFormat {
            path: PathBuf::from("a.jsonl"),
            line: 3,
            message: "bad".to_owned(),
        };
        assert_eq!(e.to_string(), "error[SAIL7003]: a.jsonl:3: bad");
    }
}
