//! SMT-LIB 2.6 tokens with byte spans.
//!
//! Whitespace and `;` comments separate tokens and are skipped. String
//! literals use `""` to escape a quote and may span lines. Quoted symbols run
//! from `|` to the next `|`. Any other run of non-delimiter bytes is a
//! "word", classified as a numeral, decimal, `#x`/`#b` literal, keyword or
//! simple symbol. Bytes at or above `0x80` are accepted inside simple
//! symbols, which is more lenient than the standard; the solver remains the
//! authority on what it accepts.

use crate::{Error, ErrorCode};

use sail_acceptance::query_lex::{LexFailure, LexFailureKind};
pub use sail_acceptance::query_lex::{Token, TokenKind};

/// An iterator over the tokens of `src`. After the first error it yields
/// nothing more.
#[derive(Debug)]
pub struct Lexer<'a>(sail_acceptance::query_lex::Lexer<'a>);

impl Clone for Lexer<'_> {
    fn clone(&self) -> Self {
        Self(self.0.duplicate())
    }
}

impl<'a> Lexer<'a> {
    /// Lexes `src` from the start.
    #[must_use]
    pub fn new(src: &'a [u8]) -> Self {
        Self(sail_acceptance::query_lex::Lexer::new(src))
    }
    pub(crate) fn command_head(&mut self) -> Result<(usize, &'a [u8]), Error> {
        self.0.command_head().map_err(native_failure)
    }
}

fn native_failure(failure: LexFailure) -> Error {
    let code = match failure.kind {
        LexFailureKind::Empty => ErrorCode::Empty,
        LexFailureKind::UnclosedList => ErrorCode::UnclosedList,
        LexFailureKind::InvalidToken => ErrorCode::InvalidToken,
        LexFailureKind::UnterminatedString => ErrorCode::UnterminatedString,
        LexFailureKind::UnterminatedQuotedSymbol => ErrorCode::UnterminatedQuotedSymbol,
    };
    Error::new(code, failure.offset)
}

impl Iterator for Lexer<'_> {
    type Item = Result<Token, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next_token()
            .map(|result| result.map_err(native_failure))
    }
}

/// Lexes all of `src`.
///
/// # Errors
///
/// Returns the first lexical error.
pub fn tokenize(src: &[u8]) -> Result<Vec<Token>, Error> {
    Lexer::new(src).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        match tokenize(src.as_bytes()) {
            Ok(tokens) => tokens.iter().map(|t| t.kind).collect(),
            Err(e) => unreachable!("{src}: {e}"),
        }
    }

    #[test]
    fn shared_lexer_and_head_controls_execute_on_original_bytes() {
        assert!(sail_acceptance::query_lex::lexer_head_constructive_control());
        assert!(sail_acceptance::query_lex::lexer_literal_spans_control());
        assert!(sail_acceptance::query_lex::lexer_sticky_error_control());
    }

    #[test]
    fn classifies_tokens() {
        use TokenKind::*;
        assert_eq!(
            kinds(r#"(set-option :rlimit 30000000) 100.0 #xFF #b01 |a b| "x""y" %%Function%%"#),
            vec![
                Open,
                Symbol,
                Keyword,
                Numeral,
                Close,
                Decimal,
                Hexadecimal,
                Binary,
                QuotedSymbol,
                String,
                Symbol
            ]
        );
    }

    #[test]
    fn skips_comments_and_whitespace() {
        assert_eq!(
            kinds(";; Prelude\n(push)\r\n; c\n"),
            vec![TokenKind::Open, TokenKind::Symbol, TokenKind::Close]
        );
    }

    #[test]
    fn string_with_escaped_quote_spans_lines() {
        let src = b"\"a\"\"b\nc\" x";
        let tokens = tokenize(src).unwrap_or_default();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text(src), b"\"a\"\"b\nc\"");
    }

    #[test]
    fn reports_errors_with_offsets() {
        for (src, code, offset) in [
            ("(echo \"abc", ErrorCode::UnterminatedString, 6),
            ("(a |b", ErrorCode::UnterminatedQuotedSymbol, 3),
            ("(a #q)", ErrorCode::InvalidToken, 3),
            ("(a 12ab)", ErrorCode::InvalidToken, 3),
            ("(a :)", ErrorCode::InvalidToken, 3),
        ] {
            assert_eq!(
                tokenize(src.as_bytes()),
                Err(Error::new(code, offset)),
                "{src}"
            );
        }
    }
}
