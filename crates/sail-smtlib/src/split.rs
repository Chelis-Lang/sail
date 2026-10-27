//! Incremental splitting of solver streams into top-level items.
//!
//! Both splitters accept input in arbitrary chunks and produce the same items
//! regardless of where the chunk boundaries fall. They track only nesting
//! depth and the lexical states that can hide parentheses (strings, quoted
//! symbols and, on the input side, comments), so they have no depth limit
//! and never fail. Malformed input becomes [`ItemKind::Stray`] items rather
//! than errors: the solver, not this module, decides what is valid.

// The public data types retain their existing fields and constructors. They
// carry no cache authority; the shared machine proves only emitted-byte facts.
use sail_acceptance::query_split::StreamSplitter;
pub use sail_acceptance::query_split::{Item, ItemKind};

/// Splits solver input (what Verus sends) into top-level commands.
///
/// Comments and whitespace between commands are dropped; comments inside a
/// command are kept in its bytes.
#[derive(Debug)]
pub struct CommandSplitter(StreamSplitter);

impl Clone for CommandSplitter {
    fn clone(&self) -> Self {
        Self(self.0.duplicate())
    }
}

impl Default for CommandSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandSplitter {
    /// A splitter at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self(StreamSplitter::start(true))
    }

    /// Feeds the next chunk of the stream, appending completed items to
    /// `out`.
    pub fn push(&mut self, data: &[u8], out: &mut Vec<Item>) {
        let current = std::mem::replace(&mut self.0, StreamSplitter::start(true));
        let (next, items) = current.push_items(data, std::mem::take(out));
        self.0 = next;
        *out = items;
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
#[derive(Debug)]
pub struct ResponseSplitter(StreamSplitter);

impl Clone for ResponseSplitter {
    fn clone(&self) -> Self {
        Self(self.0.duplicate())
    }
}

impl Default for ResponseSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseSplitter {
    /// A splitter at the start of a stream.
    #[must_use]
    pub fn new() -> Self {
        Self(StreamSplitter::start(false))
    }

    /// Feeds the next chunk of the stream, appending completed items to
    /// `out`.
    pub fn push(&mut self, data: &[u8], out: &mut Vec<Item>) {
        let current = std::mem::replace(&mut self.0, StreamSplitter::start(false));
        let (next, items) = current.push_items(data, std::mem::take(out));
        self.0 = next;
        *out = items;
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
    fn shared_owned_stdout_parser_controls_execute() {
        assert!(sail_acceptance::query_output_split::parsed_output_constructive_control());
        assert!(sail_acceptance::query_output_split::parsed_output_partial_refused());
    }

    #[test]
    fn shared_list_frames_preserve_chunks_and_incomplete_finish() {
        for (source, kind) in [
            (b"(echo \"a\"\"(b)\")".as_slice(), ItemKind::List),
            (b"(assert |x)y|)", ItemKind::List),
            (b"(x; hidden )\r\n)", ItemKind::List),
            (b"(echo \"x\"", ItemKind::Stray),
            (b"(|x)", ItemKind::Stray),
        ] {
            for width in 1..=source.len() {
                let mut splitter = CommandSplitter::new();
                let mut items = Vec::new();
                for chunk in source.chunks(width) {
                    splitter.push(chunk, &mut items);
                }
                items.extend(splitter.finish());
                assert_eq!(
                    items,
                    vec![Item {
                        kind,
                        offset: 0,
                        bytes: source.to_vec()
                    }]
                );
            }
        }
        let source = b"echo text\r\n()";
        for width in 1..=source.len() {
            let mut splitter = ResponseSplitter::new();
            let mut items = Vec::new();
            for chunk in source.chunks(width) {
                splitter.push(chunk, &mut items);
            }
            assert!(splitter.finish().is_none());
            assert_eq!(
                items,
                vec![
                    Item {
                        kind: ItemKind::Atom,
                        offset: 0,
                        bytes: b"echo text".to_vec()
                    },
                    Item {
                        kind: ItemKind::List,
                        offset: 11,
                        bytes: b"()".to_vec()
                    },
                ]
            );
        }
    }

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
