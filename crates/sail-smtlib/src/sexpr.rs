//! S-expressions over the original bytes.
//!
//! Parsing is iterative and limited to [`MAX_DEPTH`] levels of nesting, so
//! neither parsing nor dropping a tree can overflow the stack on any input.

use crate::lex::{Lexer, Token, TokenKind};
use crate::{Error, ErrorCode};

/// Maximum list nesting accepted by [`parse_one`] and [`parse_all`].
///
/// Verus queries observed so far nest far less deeply. Callers that only
/// need command boundaries use [`crate::split`], which has no depth limit.
pub const MAX_DEPTH: usize = 2048;

/// An s-expression with its byte span `start..end` in the input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SExpr {
    /// An atom or a list.
    pub kind: SExprKind,
    /// Byte offset of the first byte (the atom, or the `(`).
    pub start: usize,
    /// Byte offset one past the last byte (the atom, or the `)`).
    pub end: usize,
}

/// The shape of an [`SExpr`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SExprKind {
    /// A single token (never [`TokenKind::Open`] or [`TokenKind::Close`]).
    Atom(TokenKind),
    /// A parenthesised list.
    List(Vec<SExpr>),
}

impl SExpr {
    /// The exact bytes of this expression in `src`, the input it was parsed
    /// from. This is the faithful printer: it never reformats.
    #[must_use]
    pub fn text<'a>(&self, src: &'a [u8]) -> &'a [u8] {
        src.get(self.start..self.end).unwrap_or_default()
    }

    /// The list elements, if this is a list.
    #[must_use]
    pub fn as_list(&self) -> Option<&[SExpr]> {
        match &self.kind {
            SExprKind::List(items) => Some(items),
            SExprKind::Atom(_) => None,
        }
    }

    /// Writes a canonical rendering: atoms verbatim, list elements separated
    /// by one space, no comments. Two expressions are structurally equal
    /// exactly when their canonical renderings are equal.
    pub fn write_canonical(&self, src: &[u8], out: &mut Vec<u8>) {
        match &self.kind {
            SExprKind::Atom(_) => out.extend_from_slice(self.text(src)),
            SExprKind::List(items) => {
                out.push(b'(');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(b' ');
                    }
                    item.write_canonical(src, out);
                }
                out.push(b')');
            }
        }
    }

    /// The canonical rendering as a new buffer; see [`SExpr::write_canonical`].
    #[must_use]
    pub fn canonical(&self, src: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_canonical(src, &mut out);
        out
    }
}

impl Drop for SExpr {
    fn drop(&mut self) {
        // Flatten iteratively so dropping a deep tree cannot recurse deeply.
        let SExprKind::List(items) = &mut self.kind else {
            return;
        };
        let mut stack = std::mem::take(items);
        while let Some(mut node) = stack.pop() {
            if let SExprKind::List(children) = &mut node.kind {
                stack.append(children);
            }
        }
    }
}

/// Parses every top-level s-expression in `src`.
///
/// # Errors
///
/// Returns the first lexical or structural error.
pub fn parse_all(src: &[u8]) -> Result<Vec<SExpr>, Error> {
    let mut top = Vec::new();
    // Each frame is an open list: its `(` offset and the items so far.
    let mut stack: Vec<(usize, Vec<SExpr>)> = Vec::new();
    for token in Lexer::new(src) {
        let Token { kind, start, end } = token?;
        let finished = match kind {
            TokenKind::Open => {
                if stack.len() >= MAX_DEPTH {
                    return Err(Error::new(ErrorCode::DepthLimit, start));
                }
                stack.push((start, Vec::new()));
                continue;
            }
            TokenKind::Close => {
                let Some((open, items)) = stack.pop() else {
                    return Err(Error::new(ErrorCode::UnexpectedClose, start));
                };
                SExpr {
                    kind: SExprKind::List(items),
                    start: open,
                    end,
                }
            }
            atom => SExpr {
                kind: SExprKind::Atom(atom),
                start,
                end,
            },
        };
        match stack.last_mut() {
            Some((_, items)) => items.push(finished),
            None => top.push(finished),
        }
    }
    match stack.last() {
        Some((open, _)) => Err(Error::new(ErrorCode::UnclosedList, *open)),
        None => Ok(top),
    }
}

/// Parses exactly one s-expression; whitespace and comments around it are
/// allowed.
///
/// # Errors
///
/// [`ErrorCode::Empty`] if there is none, [`ErrorCode::TrailingInput`] if
/// there is more than one, or any error from [`parse_all`].
pub fn parse_one(src: &[u8]) -> Result<SExpr, Error> {
    let mut all = parse_all(src)?;
    match all.len() {
        0 => Err(Error::new(ErrorCode::Empty, src.len())),
        1 => all.pop().ok_or(Error::new(ErrorCode::Empty, 0)),
        _ => Err(Error::new(
            ErrorCode::TrailingInput,
            all.get(1).map_or(0, |e| e.start),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_slice_the_original_bytes() {
        let src = b"(assert  (=> a ; note\n b))";
        let expr = parse_one(src).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(expr.text(src), &src[..]);
        assert_eq!(expr.canonical(src), b"(assert (=> a b))");
        let inner = expr.as_list().and_then(|l| l.get(1)).map(|e| e.text(src));
        assert_eq!(inner, Some(&b"(=> a ; note\n b)"[..]));
    }

    #[test]
    fn structural_errors() {
        assert_eq!(
            parse_all(b"(a))"),
            Err(Error::new(ErrorCode::UnexpectedClose, 3))
        );
        assert_eq!(
            parse_all(b" ((a)"),
            Err(Error::new(ErrorCode::UnclosedList, 1))
        );
        assert_eq!(parse_one(b"  ; c\n"), Err(Error::new(ErrorCode::Empty, 6)));
        assert_eq!(
            parse_one(b"a b"),
            Err(Error::new(ErrorCode::TrailingInput, 2))
        );
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_crash() {
        let deep = "(".repeat(MAX_DEPTH + 1);
        assert_eq!(
            parse_all(deep.as_bytes()).map_err(|e| e.code),
            Err(ErrorCode::DepthLimit)
        );
        let ok = format!("{}x{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
        assert!(parse_one(ok.as_bytes()).is_ok());
    }
}
