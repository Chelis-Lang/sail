//! Incremental splitting of solver streams into top-level items.
//!
//! Both splitters accept input in arbitrary chunks and produce the same items
//! regardless of where the chunk boundaries fall. They track only nesting
//! depth and the lexical states that can hide parentheses (strings, quoted
//! symbols and, on the input side, comments), so they have no depth limit
//! and never fail. Malformed input becomes [`ItemKind::Stray`] items rather
//! than errors: the solver, not this module, decides what is valid.

/// What a split item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// A complete parenthesised top-level command (input) or response
    /// (output).
    List,
    /// A bare top-level atom. On the output side this is a response line
    /// such as `unsat` or `<<DONE>>`. On the input side it is not valid
    /// SMT-LIB, but it is still forwarded to the solver.
    Atom,
    /// A `)` with no matching `(` (input side only).
    Stray,
}

/// One top-level item and where it starts in the whole stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What kind of item this is.
    pub kind: ItemKind,
    /// Byte offset of the item's first byte in the whole stream.
    pub offset: u64,
    /// The item's exact bytes.
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Between items, at depth 0.
    Idle,
    /// Inside a list, outside any string, quoted symbol or comment.
    List,
    /// Inside a string literal.
    Str,
    /// Just saw `"` inside a string: either an escaped `""` or the end.
    StrQuote,
    /// Inside a `|quoted symbol|`.
    Quoted,
    /// Inside a `;` comment (input side only). `in_list` says where to
    /// return.
    Comment { in_list: bool },
    /// Inside a bare top-level atom.
    Atom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Input,
    Output,
}

#[derive(Debug, Clone)]
struct Splitter {
    side: Side,
    state: State,
    depth: u64,
    /// Bytes of the item in progress.
    current: Vec<u8>,
    /// Stream offset of `current[0]`.
    current_offset: u64,
    /// Stream offset of the next byte to be pushed.
    offset: u64,
}

impl Splitter {
    fn new(side: Side) -> Self {
        Self {
            side,
            state: State::Idle,
            depth: 0,
            current: Vec::new(),
            current_offset: 0,
            offset: 0,
        }
    }

    fn emit(&mut self, kind: ItemKind, out: &mut Vec<Item>) {
        out.push(Item {
            kind,
            offset: self.current_offset,
            bytes: std::mem::take(&mut self.current),
        });
        self.state = State::Idle;
    }

    fn begin(&mut self, b: u8) {
        self.current_offset = self.offset;
        self.current.push(b);
    }

    fn push(&mut self, data: &[u8], out: &mut Vec<Item>) {
        for &b in data {
            self.step(b, out);
            self.offset += 1;
        }
    }

    fn step(&mut self, b: u8, out: &mut Vec<Item>) {
        match self.state {
            State::Idle => match b {
                b' ' | b'\t' | b'\n' | b'\r' => {}
                b';' if self.side == Side::Input => self.state = State::Comment { in_list: false },
                b'(' => {
                    self.begin(b);
                    self.depth = 1;
                    self.state = State::List;
                }
                b')' if self.side == Side::Input => {
                    self.begin(b);
                    self.emit(ItemKind::Stray, out);
                }
                _ => {
                    self.begin(b);
                    self.state = State::Atom;
                }
            },
            State::Atom => {
                let ends = match self.side {
                    // Output atoms are whole lines (echo output may contain
                    // spaces); input atoms end at any delimiter.
                    Side::Output => b == b'\n',
                    Side::Input => crate::lex::is_delimiter(b),
                };
                if ends {
                    if self.current.last() == Some(&b'\r') {
                        self.current.pop();
                    }
                    self.emit(ItemKind::Atom, out);
                    // The delimiter starts whatever comes next.
                    if b != b'\n' {
                        self.step(b, out);
                    }
                } else {
                    self.current.push(b);
                }
            }
            State::List => {
                self.current.push(b);
                match b {
                    b'(' => self.depth += 1,
                    b')' => {
                        self.depth -= 1;
                        if self.depth == 0 {
                            self.emit(ItemKind::List, out);
                        }
                    }
                    b'"' => self.state = State::Str,
                    b'|' => self.state = State::Quoted,
                    b';' if self.side == Side::Input => {
                        self.state = State::Comment { in_list: true }
                    }
                    _ => {}
                }
            }
            State::Str => {
                self.current.push(b);
                if b == b'"' {
                    self.state = State::StrQuote;
                }
            }
            State::StrQuote => {
                if b == b'"' {
                    // Escaped quote: still inside the string.
                    self.current.push(b);
                    self.state = State::Str;
                } else {
                    self.state = State::List;
                    self.step(b, out);
                }
            }
            State::Quoted => {
                self.current.push(b);
                if b == b'|' {
                    self.state = State::List;
                }
            }
            State::Comment { in_list } => {
                if in_list {
                    self.current.push(b);
                }
                if b == b'\n' || b == b'\r' {
                    self.state = if in_list { State::List } else { State::Idle };
                }
            }
        }
    }

    fn finish(mut self) -> Option<Item> {
        match self.state {
            State::Atom => {
                let mut out = Vec::new();
                self.emit(ItemKind::Atom, &mut out);
                out.pop()
            }
            _ if self.current.is_empty() => None,
            // An incomplete list: report what there is, as a stray.
            _ => Some(Item {
                kind: ItemKind::Stray,
                offset: self.current_offset,
                bytes: self.current,
            }),
        }
    }
}

/// Splits solver input (what Verus sends) into top-level commands.
///
/// Comments and whitespace between commands are dropped; comments inside a
/// command are kept in its bytes.
#[derive(Debug, Clone)]
pub struct CommandSplitter(Splitter);

impl Default for CommandSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandSplitter {
    /// A splitter at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self(Splitter::new(Side::Input))
    }

    /// Feeds the next chunk of the stream, appending completed items to
    /// `out`.
    pub fn push(&mut self, data: &[u8], out: &mut Vec<Item>) {
        self.0.push(data, out);
    }

    /// Ends the stream, returning any incomplete trailing item.
    #[must_use]
    pub fn finish(self) -> Option<Item> {
        self.0.finish()
    }
}

/// Splits solver output (what Z3 answers) into top-level responses.
///
/// A response is either a balanced parenthesised expression, such as a model
/// or `(:version "4.16.0")`, or a single line, such as `unsat` or the text of
/// an `echo`. Echo text that itself starts with `(` is indistinguishable from
/// an expression at this level; callers pairing responses with commands must
/// allow for that.
#[derive(Debug, Clone)]
pub struct ResponseSplitter(Splitter);

impl Default for ResponseSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseSplitter {
    /// A splitter at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self(Splitter::new(Side::Output))
    }

    /// Feeds the next chunk of the stream, appending completed items to
    /// `out`.
    pub fn push(&mut self, data: &[u8], out: &mut Vec<Item>) {
        self.0.push(data, out);
    }

    /// Ends the stream, returning any incomplete trailing item.
    #[must_use]
    pub fn finish(self) -> Option<Item> {
        self.0.finish()
    }
}

/// Whether `reply` is exactly one complete solver response, starting at its
/// first byte and followed by nothing but whitespace.
///
/// "Complete" means the [`ResponseSplitter`] emits it while the stream is
/// still open: a balanced expression, or a line ended by `\n`. This is the
/// unit a live session counts responses in, so a stored reply that passes
/// stands for exactly one reply in the stream.
#[must_use]
pub fn is_single_response(reply: &[u8]) -> bool {
    let mut splitter = ResponseSplitter::new();
    let mut items = Vec::new();
    splitter.push(reply, &mut items);
    matches!(items.as_slice(), [one] if one.offset == 0) && splitter.finish().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_response_requires_exactly_one_complete_frame() {
        for ok in [
            b"sat\n".as_slice(),
            b"unsat\r\n",
            b"(:name \"Z3\")\n",
            b"(:name \"Z3\")",
            b"(model\n  (define-fun x () Int 1)\n)\n  \n",
            b"(:reason-unknown \"a ) in a string\")\n",
        ] {
            assert!(is_single_response(ok), "{:?}", String::from_utf8_lossy(ok));
        }
        for bad in [
            b"".as_slice(),
            b"\n",
            b" sat\n",
            b"sat",
            b"sat\nunsat\n",
            b"(:name \"Z3\")(:evil \"extra\")\n",
            b"(:name \"Z3\")\n(:evil \"extra\")\n",
            b"(:name \"Z3\") trailing\n",
            b"(:name \"Z3\"",
            b"(:name \"Z3\")\nsat",
        ] {
            assert!(
                !is_single_response(bad),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
    }

    fn commands(src: &[u8]) -> Vec<(ItemKind, u64, String)> {
        let mut s = CommandSplitter::new();
        let mut out = Vec::new();
        s.push(src, &mut out);
        out.extend(s.finish());
        out.into_iter()
            .map(|i| {
                (
                    i.kind,
                    i.offset,
                    String::from_utf8_lossy(&i.bytes).into_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn splits_commands_with_tricky_lexemes() {
        let src = b"; header (\n(echo \"a)\"\"(\") (assert |x)y|) (assert ; c )\n true)\n";
        assert_eq!(
            commands(src),
            vec![
                (ItemKind::List, 11, "(echo \"a)\"\"(\")".to_owned()),
                (ItemKind::List, 26, "(assert |x)y|)".to_owned()),
                (ItemKind::List, 41, "(assert ; c )\n true)".to_owned()),
            ]
        );
    }

    #[test]
    fn strays_and_incomplete_input() {
        assert_eq!(
            commands(b") foo (push"),
            vec![
                (ItemKind::Stray, 0, ")".to_owned()),
                (ItemKind::Atom, 2, "foo".to_owned()),
                (ItemKind::Stray, 6, "(push".to_owned()),
            ]
        );
    }

    #[test]
    fn splits_responses_into_lines_and_lists() {
        let mut s = ResponseSplitter::new();
        let mut out = Vec::new();
        s.push(
            b"unsat\n<<DONE>>\r\n(:version \"4.16.0\")\n(model\n (define-fun x () Int 1))\n",
            &mut out,
        );
        let texts: Vec<_> = out
            .iter()
            .map(|i| String::from_utf8_lossy(&i.bytes).into_owned())
            .collect();
        assert_eq!(
            texts,
            [
                "unsat",
                "<<DONE>>",
                "(:version \"4.16.0\")",
                "(model\n (define-fun x () Int 1))"
            ]
        );
        assert!(out.iter().all(|i| i.kind != ItemKind::Stray));
    }
}
