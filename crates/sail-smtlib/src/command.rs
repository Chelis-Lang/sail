//! Shallow classification of top-level SMT-LIB commands.
//!
//! [`classify`] reads only as many tokens as it needs: the command name and,
//! for a few commands, their small arguments. It never walks the body of an
//! `assert`, so it is cheap on the very large assertions Verus sends.

use crate::lex::{Lexer, Token, TokenKind};
use crate::{Error, ErrorCode};

/// A classified top-level command. Byte slices borrow from the command text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command<'a> {
    /// The command name, for example `b"check-sat"`.
    pub name: &'a [u8],
    /// What the command does, as far as session modelling is concerned.
    pub kind: CommandKind<'a>,
}

/// The session-relevant meaning of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandKind<'a> {
    /// `(push n)`, with `n` defaulting to 1.
    Push(u64),
    /// `(pop n)`, with `n` defaulting to 1.
    Pop(u64),
    /// `(assert …)`.
    Assert,
    /// `declare-fun`, `declare-const`, `declare-sort`, `declare-datatype(s)`.
    Declare,
    /// `define-fun`, `define-fun-rec`, `define-funs-rec`, `define-sort`,
    /// `define-const`.
    Define,
    /// `(set-option :keyword value)`. `value` is the exact text of the value.
    SetOption {
        /// The option keyword, including the colon.
        keyword: &'a [u8],
        /// The exact text of the value expression.
        value: &'a [u8],
    },
    /// `(set-info …)`.
    SetInfo,
    /// `(set-logic …)`.
    SetLogic,
    /// `(check-sat)`.
    CheckSat,
    /// `(check-sat-assuming …)`.
    CheckSatAssuming,
    /// Z3's `(check-sat-using tactic)`.
    CheckSatUsing,
    /// `(get-info :keyword)`.
    GetInfo {
        /// The requested keyword, including the colon.
        keyword: &'a [u8],
    },
    /// Any other `get-…` query (`get-model`, `get-value`, …).
    Get,
    /// `(echo "…")`.
    Echo,
    /// `(reset)`.
    Reset,
    /// `(reset-assertions)`.
    ResetAssertions,
    /// `(exit)`.
    Exit,
    /// A command this module does not model. Session models must treat it
    /// conservatively.
    Other,
}

/// Whether a command produces a response when `:print-success` is false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// No response unless the command fails.
    None,
    /// Exactly one response.
    One,
    /// Not known for this command.
    Unknown,
}

impl CommandKind<'_> {
    /// True for commands that ask the solver for a satisfiability answer.
    #[must_use]
    pub fn is_check(&self) -> bool {
        matches!(
            self,
            Self::CheckSat | Self::CheckSatAssuming | Self::CheckSatUsing
        )
    }

    /// The response this command produces when `:print-success` is false
    /// and the command succeeds.
    #[must_use]
    pub fn output(&self) -> Output {
        match self {
            Self::CheckSat
            | Self::CheckSatAssuming
            | Self::CheckSatUsing
            | Self::GetInfo { .. }
            | Self::Get
            | Self::Echo => Output::One,
            Self::Other => Output::Unknown,
            _ => Output::None,
        }
    }
}

/// Classifies one top-level command, given exactly its bytes (as produced by
/// [`crate::split::CommandSplitter`]).
///
/// # Errors
///
/// [`ErrorCode::InvalidToken`] if the text does not start with `(` and a
/// symbol, or a lexical error in the tokens it reads.
pub fn classify(text: &[u8]) -> Result<Command<'_>, Error> {
    let mut lexer = Lexer::new(text);
    let open = next(&mut lexer)?.ok_or(Error::new(ErrorCode::Empty, 0))?;
    if open.kind != TokenKind::Open {
        return Err(Error::new(ErrorCode::InvalidToken, open.start));
    }
    let head = next(&mut lexer)?.ok_or(Error::new(ErrorCode::UnclosedList, open.start))?;
    if head.kind != TokenKind::Symbol {
        return Err(Error::new(ErrorCode::InvalidToken, head.start));
    }
    let name = head.text(text);
    let kind = match name {
        b"push" | b"pop" => {
            let count = match next(&mut lexer)? {
                Some(t) if t.kind == TokenKind::Numeral => {
                    parse_u64(t.text(text)).ok_or(Error::new(ErrorCode::InvalidToken, t.start))?
                }
                Some(t) if t.kind == TokenKind::Close => 1,
                Some(t) => return Err(Error::new(ErrorCode::InvalidToken, t.start)),
                None => return Err(Error::new(ErrorCode::UnclosedList, open.start)),
            };
            if name == b"push" {
                CommandKind::Push(count)
            } else {
                CommandKind::Pop(count)
            }
        }
        b"assert" => CommandKind::Assert,
        b"declare-fun" | b"declare-const" | b"declare-sort" | b"declare-datatype"
        | b"declare-datatypes" => CommandKind::Declare,
        b"define-fun" | b"define-fun-rec" | b"define-funs-rec" | b"define-sort"
        | b"define-const" => CommandKind::Define,
        b"set-option" => {
            let keyword = keyword_after(next(&mut lexer)?, open)?;
            let value = value_span(text, &mut lexer, keyword.end)?;
            CommandKind::SetOption {
                keyword: keyword.text(text),
                value,
            }
        }
        b"set-info" => CommandKind::SetInfo,
        b"set-logic" => CommandKind::SetLogic,
        b"check-sat" => CommandKind::CheckSat,
        b"check-sat-assuming" => CommandKind::CheckSatAssuming,
        b"check-sat-using" => CommandKind::CheckSatUsing,
        b"get-info" => CommandKind::GetInfo {
            keyword: keyword_after(next(&mut lexer)?, open)?.text(text),
        },
        _ if name.starts_with(b"get-") => CommandKind::Get,
        b"echo" => CommandKind::Echo,
        b"reset" => CommandKind::Reset,
        b"reset-assertions" => CommandKind::ResetAssertions,
        b"exit" => CommandKind::Exit,
        _ => CommandKind::Other,
    };
    Ok(Command { name, kind })
}

fn next(lexer: &mut Lexer<'_>) -> Result<Option<Token>, Error> {
    lexer.next().transpose()
}

fn keyword_after(token: Option<Token>, open: Token) -> Result<Token, Error> {
    match token {
        Some(t) if t.kind == TokenKind::Keyword => Ok(t),
        Some(t) => Err(Error::new(ErrorCode::InvalidToken, t.start)),
        None => Err(Error::new(ErrorCode::UnclosedList, open.start)),
    }
}

/// The exact text of the single expression following offset `after`.
fn value_span<'a>(text: &'a [u8], lexer: &mut Lexer<'_>, after: usize) -> Result<&'a [u8], Error> {
    let mut depth = 0usize;
    let mut start = None;
    for token in lexer.by_ref() {
        let token = token?;
        let begin = *start.get_or_insert(token.start);
        match token.kind {
            TokenKind::Open => depth += 1,
            TokenKind::Close if depth == 0 => {
                return Err(Error::new(ErrorCode::Empty, token.start));
            }
            TokenKind::Close => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Ok(text.get(begin..token.end).unwrap_or_default());
        }
    }
    Err(Error::new(ErrorCode::UnclosedList, after))
}

fn parse_u64(digits: &[u8]) -> Option<u64> {
    std::str::from_utf8(digits).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(src: &str) -> CommandKind<'_> {
        match classify(src.as_bytes()) {
            Ok(c) => c.kind,
            Err(e) => unreachable!("{src}: {e}"),
        }
    }

    #[test]
    fn classifies_verus_commands() {
        assert_eq!(kind("(push)"), CommandKind::Push(1));
        assert_eq!(kind("(pop 3)"), CommandKind::Pop(3));
        assert_eq!(kind("(assert (=> a b))"), CommandKind::Assert);
        assert_eq!(kind("(declare-fun succ (Fuel) Fuel)"), CommandKind::Declare);
        assert_eq!(
            kind("(set-option :rlimit 30000000)"),
            CommandKind::SetOption {
                keyword: b":rlimit",
                value: b"30000000"
            }
        );
        assert_eq!(
            kind("(set-option :x (a (b)) )"),
            CommandKind::SetOption {
                keyword: b":x",
                value: b"(a (b))"
            }
        );
        assert_eq!(
            kind("(get-info :all-statistics)"),
            CommandKind::GetInfo {
                keyword: b":all-statistics"
            }
        );
        assert_eq!(kind("(get-model)"), CommandKind::Get);
        assert_eq!(kind("(echo \"<<DONE>>\")"), CommandKind::Echo);
        assert_eq!(kind("(check-sat)"), CommandKind::CheckSat);
        assert_eq!(kind("(eval x)"), CommandKind::Other);
    }

    #[test]
    fn output_expectations() {
        assert_eq!(kind("(check-sat)").output(), Output::One);
        assert_eq!(kind("(check-sat-using smt)").output(), Output::One);
        assert_eq!(kind("(assert true)").output(), Output::None);
        assert_eq!(kind("(simplify x)").output(), Output::Unknown);
    }

    #[test]
    fn rejects_malformed_commands() {
        for src in [
            "x",
            "()",
            "(push x)",
            "(set-option rlimit 1)",
            "(set-option :rlimit)",
            "(get-info)",
            "(pop",
        ] {
            assert!(classify(src.as_bytes()).is_err(), "{src}");
        }
    }
}
