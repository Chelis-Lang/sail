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

/// The kind of a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// `(`
    Open,
    /// `)`
    Close,
    /// A simple symbol such as `assert` or `%%Function%%`.
    Symbol,
    /// A `|quoted symbol|`, including the bars.
    QuotedSymbol,
    /// A keyword such as `:rlimit`, including the colon.
    Keyword,
    /// A numeral such as `42`.
    Numeral,
    /// A decimal such as `100.0`.
    Decimal,
    /// A `#x` hexadecimal literal.
    Hexadecimal,
    /// A `#b` binary literal.
    Binary,
    /// A `"string literal"`, including the quotes.
    String,
}

/// A token and its byte span `start..end` in the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token {
    /// What kind of token this is.
    pub kind: TokenKind,
    /// Byte offset of the first byte.
    pub start: usize,
    /// Byte offset one past the last byte.
    pub end: usize,
}

impl Token {
    /// The token's bytes in `src`, which must be the input it was lexed from.
    #[must_use]
    pub fn text<'a>(&self, src: &'a [u8]) -> &'a [u8] {
        src.get(self.start..self.end).unwrap_or_default()
    }
}

/// True for bytes that end a word: whitespace, parentheses, and the bytes
/// that start strings, quoted symbols and comments.
pub(crate) fn is_delimiter(b: u8) -> bool {
    sail_acceptance::query::delimiter(b)
}

/// An iterator over the tokens of `src`. After the first error it yields
/// nothing more.
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    failed: bool,
}

impl<'a> Lexer<'a> {
    /// Lexes `src` from the start.
    #[must_use]
    pub fn new(src: &'a [u8]) -> Self {
        Self {
            src,
            pos: 0,
            failed: false,
        }
    }

    fn byte(&self, i: usize) -> Option<u8> {
        self.src.get(i).copied()
    }

    fn skip_trivia(&mut self) {
        while let Some(b) = self.byte(self.pos) {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                b';' => {
                    while let Some(c) = self.byte(self.pos) {
                        self.pos += 1;
                        if c == b'\n' || c == b'\r' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn string(&mut self, start: usize) -> Result<Token, Error> {
        let mut i = start + 1;
        loop {
            match self.byte(i) {
                None => return Err(Error::new(ErrorCode::UnterminatedString, start)),
                Some(b'"') if self.byte(i + 1) == Some(b'"') => i += 2,
                Some(b'"') => {
                    self.pos = i + 1;
                    return Ok(self.token(TokenKind::String, start));
                }
                Some(_) => i += 1,
            }
        }
    }

    fn quoted_symbol(&mut self, start: usize) -> Result<Token, Error> {
        let rest = self.src.get(start + 1..).unwrap_or_default();
        match rest.iter().position(|&b| b == b'|') {
            Some(n) => {
                self.pos = start + 1 + n + 1;
                Ok(self.token(TokenKind::QuotedSymbol, start))
            }
            None => Err(Error::new(ErrorCode::UnterminatedQuotedSymbol, start)),
        }
    }

    fn word(&mut self, start: usize) -> Result<Token, Error> {
        let mut end = start;
        while let Some(b) = self.byte(end) {
            if is_delimiter(b) {
                break;
            }
            end += 1;
        }
        self.pos = end;
        let text = self.src.get(start..end).unwrap_or_default();
        match classify_word(text) {
            Some(kind) => Ok(self.token(kind, start)),
            None => Err(Error::new(ErrorCode::InvalidToken, start)),
        }
    }

    fn token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            start,
            end: self.pos,
        }
    }
}

fn classify_word(text: &[u8]) -> Option<TokenKind> {
    let all = |s: &[u8], f: fn(&u8) -> bool| !s.is_empty() && s.iter().all(f);
    match text {
        [b'#', b'x', digits @ ..] => {
            all(digits, u8::is_ascii_hexdigit).then_some(TokenKind::Hexadecimal)
        }
        [b'#', b'b', digits @ ..] => {
            all(digits, |b| matches!(b, b'0' | b'1')).then_some(TokenKind::Binary)
        }
        [] | [b'#', ..] => None,
        [b':', rest @ ..] => (!rest.is_empty()).then_some(TokenKind::Keyword),
        [first, ..] if first.is_ascii_digit() => {
            let mut parts = text.splitn(2, |&b| b == b'.');
            let int = parts.next().unwrap_or_default();
            match parts.next() {
                None => all(int, u8::is_ascii_digit).then_some(TokenKind::Numeral),
                Some(frac) => (all(int, u8::is_ascii_digit) && all(frac, u8::is_ascii_digit))
                    .then_some(TokenKind::Decimal),
            }
        }
        _ => Some(TokenKind::Symbol),
    }
}

impl Iterator for Lexer<'_> {
    type Item = Result<Token, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        self.skip_trivia();
        let start = self.pos;
        let b = self.byte(start)?;
        let result = match b {
            b'(' => {
                self.pos += 1;
                Ok(self.token(TokenKind::Open, start))
            }
            b')' => {
                self.pos += 1;
                Ok(self.token(TokenKind::Close, start))
            }
            b'"' => self.string(start),
            b'|' => self.quoted_symbol(start),
            _ => self.word(start),
        };
        if result.is_err() {
            self.failed = true;
        }
        Some(result)
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
