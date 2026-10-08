//! Errors with stable codes (`SAIL1xxx`).

use std::fmt;

/// What went wrong. Each variant has a stable code; see [`ErrorCode::code`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// A string literal has no closing `"`.
    UnterminatedString,
    /// A quoted symbol has no closing `|`.
    UnterminatedQuotedSymbol,
    /// A `)` with no matching `(`.
    UnexpectedClose,
    /// Input ended inside a list.
    UnclosedList,
    /// A token that is not valid SMT-LIB (for example `#q` or `12ab`).
    InvalidToken,
    /// No s-expression where one was required.
    Empty,
    /// More input after the single s-expression that was expected.
    TrailingInput,
    /// Lists nested deeper than [`crate::sexpr::MAX_DEPTH`].
    DepthLimit,
}

impl ErrorCode {
    /// The stable, user-facing code for this error.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::UnterminatedString => "SAIL1001",
            Self::UnterminatedQuotedSymbol => "SAIL1002",
            Self::UnexpectedClose => "SAIL1003",
            Self::UnclosedList => "SAIL1004",
            Self::InvalidToken => "SAIL1005",
            Self::Empty => "SAIL1006",
            Self::TrailingInput => "SAIL1007",
            Self::DepthLimit => "SAIL1008",
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Self::UnterminatedString => "unterminated string literal",
            Self::UnterminatedQuotedSymbol => "unterminated quoted symbol",
            Self::UnexpectedClose => "unexpected `)`",
            Self::UnclosedList => "unclosed `(`",
            Self::InvalidToken => "invalid token",
            Self::Empty => "expected an s-expression",
            Self::TrailingInput => "unexpected input after s-expression",
            Self::DepthLimit => "lists nested too deeply",
        }
    }
}

/// An SMT-LIB syntax error at a byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Error {
    /// What went wrong.
    pub code: ErrorCode,
    /// Byte offset into the input where the problem starts.
    pub offset: usize,
}

impl Error {
    pub(crate) fn new(code: ErrorCode, offset: usize) -> Self {
        Self { code, offset }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error[{}]: {} at byte {}",
            self.code.code(),
            self.code.describe(),
            self.offset
        )
    }
}

impl std::error::Error for Error {}
