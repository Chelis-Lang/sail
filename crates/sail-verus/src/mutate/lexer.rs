//! A small Rust/Verus lexer: enough to find functions, clauses, statements
//! and operators inside `verus! { ... }` with exact byte offsets. Comments
//! and whitespace are skipped; everything else becomes a token. It never
//! fails: an unterminated literal or comment runs to the end of the file.

/// Token kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An identifier or keyword (including raw identifiers).
    Ident,
    /// A lifetime such as `'a`.
    Lifetime,
    /// An integer literal, with any suffix.
    Int,
    /// A float literal.
    Float,
    /// A string, byte string or raw string literal.
    Str,
    /// A character or byte literal.
    Char,
    /// Punctuation or an operator.
    Punct,
}

/// One token: its kind and byte range in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tok {
    /// What it is.
    pub kind: Kind,
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

/// Multi-character operators, longest first.
const PUNCTS: &[&str] = &[
    "<==>", "=~~=", "!~~=", "<<=", ">>=", "...", "..=", "==>", "<==", "&&&", "|||", "===", "=~=",
    "!~=", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "%=",
    "^=", "&=", "|=", "<<", ">>", "..",
];

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

/// Tokenizes `src`.
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one lexer loop; b/c/i/j/k are the bytes, the current byte and cursors"
)]
pub fn lex(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    let at = |i: usize| b.get(i).copied().unwrap_or(0);
    while i < b.len() {
        let c = at(i);
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && at(i + 1) == b'/' {
            while i < b.len() && at(i) != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && at(i + 1) == b'*' {
            let mut depth = 0usize;
            while i < b.len() {
                if at(i) == b'/' && at(i + 1) == b'*' {
                    depth += 1;
                    i += 2;
                } else if at(i) == b'*' && at(i + 1) == b'/' {
                    depth = depth.saturating_sub(1);
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }
        let start = i;
        // Raw strings and byte strings: r"..", r#".."#, b"..", br"..", b'x'.
        if c == b'r' || c == b'b' {
            let mut j = i;
            if at(j) == b'b' {
                j += 1;
            }
            if at(j) == b'r' && (at(j + 1) == b'"' || at(j + 1) == b'#') {
                let mut k = j + 1;
                let mut hashes = 0;
                while at(k) == b'#' {
                    hashes += 1;
                    k += 1;
                }
                if at(k) == b'"' {
                    k += 1;
                    loop {
                        if k >= b.len() {
                            break;
                        }
                        if at(k) == b'"' && (1..=hashes).all(|h| at(k + h) == b'#') {
                            k += 1 + hashes;
                            break;
                        }
                        k += 1;
                    }
                    toks.push(Tok {
                        kind: Kind::Str,
                        start,
                        end: k.min(b.len()),
                    });
                    i = k;
                    continue;
                }
            }
            if c == b'b' && at(i + 1) == b'"' {
                i = string_end(b, i + 1);
                toks.push(Tok {
                    kind: Kind::Str,
                    start,
                    end: i,
                });
                continue;
            }
            if c == b'b' && at(i + 1) == b'\'' {
                i = char_end(b, i + 1);
                toks.push(Tok {
                    kind: Kind::Char,
                    start,
                    end: i,
                });
                continue;
            }
        }
        if is_ident_start(c) {
            if c == b'r' && at(i + 1) == b'#' && is_ident_start(at(i + 2)) {
                i += 2;
            }
            while i < b.len() && is_ident_continue(at(i)) {
                i += 1;
            }
            toks.push(Tok {
                kind: Kind::Ident,
                start,
                end: i,
            });
            continue;
        }
        if c.is_ascii_digit() {
            // After a `.`, a number is a tuple index (`x.0.1`): no fraction.
            let after_dot = toks.last().is_some_and(|t: &Tok| {
                t.kind == Kind::Punct && b.get(t.start) == Some(&b'.') && t.end == t.start + 1
            });
            let (end, float) = number_end(b, i, !after_dot);
            i = end;
            toks.push(Tok {
                kind: if float { Kind::Float } else { Kind::Int },
                start,
                end: i,
            });
            continue;
        }
        if c == b'"' {
            i = string_end(b, i);
            toks.push(Tok {
                kind: Kind::Str,
                start,
                end: i,
            });
            continue;
        }
        if c == b'\'' {
            // A char literal ('x', '\n', '\u{..}') or a lifetime ('a).
            let is_char = if at(i + 1) == b'\\' {
                true
            } else {
                // One (possibly multi-byte) character, then a quote.
                let len = utf8_len(at(i + 1));
                at(i + 1 + len) == b'\''
            };
            if is_char {
                i = char_end(b, i);
                toks.push(Tok {
                    kind: Kind::Char,
                    start,
                    end: i,
                });
            } else {
                i += 1;
                while i < b.len() && is_ident_continue(at(i)) {
                    i += 1;
                }
                toks.push(Tok {
                    kind: Kind::Lifetime,
                    start,
                    end: i,
                });
            }
            continue;
        }
        let rest = b.get(i..).unwrap_or_default();
        let len = PUNCTS
            .iter()
            .find(|p| rest.starts_with(p.as_bytes()))
            .map_or_else(|| utf8_len(c), |p| p.len());
        i += len;
        toks.push(Tok {
            kind: Kind::Punct,
            start,
            end: i.min(b.len()),
        });
    }
    toks
}

fn utf8_len(c: u8) -> usize {
    match c {
        0xF0..=0xFF => 4,
        0xE0..=0xEF => 3,
        0xC0..=0xDF => 2,
        _ => 1,
    }
}

/// The end of a `"`-delimited string starting at `i` (the opening quote).
fn string_end(b: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < b.len() {
        match b.get(i).copied().unwrap_or(0) {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    b.len()
}

/// The end of a char literal starting at `i` (the opening quote).
fn char_end(b: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < b.len() {
        match b.get(i).copied().unwrap_or(0) {
            b'\\' => i += 2,
            b'\'' => return i + 1,
            b'\n' => return i,
            _ => i += 1,
        }
    }
    b.len()
}

/// The end of a numeric literal starting at `i`, and whether it is a float.
fn number_end(b: &[u8], mut i: usize, fraction: bool) -> (usize, bool) {
    let at = |i: usize| b.get(i).copied().unwrap_or(0);
    let mut float = false;
    if at(i) == b'0' && matches!(at(i + 1), b'x' | b'o' | b'b') {
        i += 2;
        while is_ident_continue(at(i)) {
            i += 1;
        }
        return (i, false);
    }
    while at(i).is_ascii_digit() || at(i) == b'_' {
        i += 1;
    }
    // A fraction only when a digit follows the dot (`0..n` is a range,
    // `x.0.1` a tuple index, `1.max(2)` a method call).
    if fraction && at(i) == b'.' && at(i + 1).is_ascii_digit() {
        float = true;
        i += 1;
        while at(i).is_ascii_digit() || at(i) == b'_' {
            i += 1;
        }
    }
    if matches!(at(i), b'e' | b'E')
        && (at(i + 1).is_ascii_digit()
            || (matches!(at(i + 1), b'+' | b'-') && at(i + 2).is_ascii_digit()))
    {
        float = true;
        i += 2;
        while at(i).is_ascii_digit() || at(i) == b'_' {
            i += 1;
        }
    }
    // Suffix (u64, usize, f32, ...).
    if is_ident_start(at(i)) {
        if at(i) == b'f' {
            float = true;
        }
        while is_ident_continue(at(i)) {
            i += 1;
        }
    }
    (i, float)
}

/// For every opening bracket token, the index of its matching closing token
/// (and vice versa); `None` for other tokens and unbalanced brackets.
#[must_use]
pub fn match_brackets(src: &str, toks: &[Tok]) -> Vec<Option<usize>> {
    let mut out = vec![None; toks.len()];
    let mut stack: Vec<(u8, usize)> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if t.kind != Kind::Punct {
            continue;
        }
        let c = src.as_bytes().get(t.start).copied().unwrap_or(0);
        match c {
            b'(' | b'[' | b'{' => stack.push((c, i)),
            b')' | b']' | b'}' => {
                let open = match c {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                // Pop to the matching opener; mismatches are dropped.
                while let Some((o, j)) = stack.pop() {
                    if o == open {
                        if let Some(slot) = out.get_mut(i) {
                            *slot = Some(j);
                        }
                        if let Some(slot) = out.get_mut(j) {
                            *slot = Some(i);
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<&str> {
        lex(src)
            .iter()
            .map(|t| src.get(t.start..t.end).unwrap_or_default())
            .collect()
    }

    #[test]
    fn lexes_verus_operators_and_skips_comments() {
        assert_eq!(
            texts("a ==> b <==> c // x\n &&& d /* y /* z */ */ ||| e"),
            ["a", "==>", "b", "<==>", "c", "&&&", "d", "|||", "e"]
        );
        assert_eq!(
            texts("x.0.1 + 0..n"),
            ["x", ".", "0", ".", "1", "+", "0", "..", "n"]
        );
        assert_eq!(texts("1u64 1.5 0xFFu8"), ["1u64", "1.5", "0xFFu8"]);
    }

    #[test]
    fn lexes_literals_and_lifetimes() {
        let src = r##"'a 'x' '\n' "s\"t" r#"r"# b'c' b"d""##;
        let kinds: Vec<Kind> = lex(src).iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            [
                Kind::Lifetime,
                Kind::Char,
                Kind::Char,
                Kind::Str,
                Kind::Str,
                Kind::Char,
                Kind::Str
            ]
        );
    }

    #[test]
    fn matches_brackets() {
        let src = "f(a[1], { b })";
        let toks = lex(src);
        let m = match_brackets(src, &toks);
        assert_eq!(m.get(1).copied().flatten(), Some(toks.len() - 1));
        assert_eq!(m.get(3).copied().flatten(), Some(5));
    }
}
