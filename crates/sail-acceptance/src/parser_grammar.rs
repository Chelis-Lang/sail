//! Exact byte grammar shared by the actual streaming splitter and token lexer.
//! These structural/lexical relations do not establish solver meaning or EOF.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Lexical contexts that hide structural parentheses in one solver form.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FrameLex {
        /// Parentheses delimit lists.
        Plain,
        /// Inside a double-quoted string.
        String,
        /// A quote may close the string or begin a doubled-quote escape.
        Quote,
        /// Inside a bar-quoted symbol.
        Symbol,
        /// Inside an input semicolon comment.
        Comment,
    }
}

sail_spec! {
pub open spec fn structural(lex: FrameLex, byte: u8) -> bool {
        lex is Plain || lex is Quote && byte != b'"'
    }
}

sail_spec! {
pub open spec fn frame_lex(lex: FrameLex, byte: u8, comments: bool) -> FrameLex {
        if lex is String { if byte == b'"' { FrameLex::Quote } else { FrameLex::String } }
        else if lex is Symbol { if byte == b'|' { FrameLex::Plain } else { FrameLex::Symbol } }
        else if lex is Comment {
            if byte == b'\n' || byte == b'\r' { FrameLex::Plain } else { FrameLex::Comment }
        } else if lex is Quote && byte == b'"' { FrameLex::String }
        else if byte == b'"' { FrameLex::String }
        else if byte == b'|' { FrameLex::Symbol }
        else if comments && byte == b';' { FrameLex::Comment }
        else { FrameLex::Plain }
    }
}

sail_spec! {
pub open spec fn frame_scan(bytes: Seq<u8>, comments: bool) -> (int, FrameLex)
        decreases bytes.len(),
    {
        if bytes.len() == 0 { (0, FrameLex::Plain) }
        else {
            let previous = frame_scan(bytes.drop_last(), comments);
            let byte = bytes.last();
            (previous.0 + if structural(previous.1, byte) && byte == b'(' { 1int }
                else if structural(previous.1, byte) && byte == b')' { -1int } else { 0int },
                frame_lex(previous.1, byte, comments))
        }
    }
}

sail_spec! {
pub open spec fn single_list(bytes: Seq<u8>, comments: bool) -> bool {
        bytes.len() >= 2 && bytes[0] == b'(' && bytes.last() == b')'
            && frame_scan(bytes, comments) == (0, FrameLex::Plain)
            && forall|i: int| 1 <= i < bytes.len() ==> frame_scan(bytes.take(i), comments).0 > 0
    }
}

sail_spec! {
pub open spec fn word_delimiter(byte: u8) -> bool {
        byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n'
            || byte == b'(' || byte == b')' || byte == b'"' || byte == b'|' || byte == b';'
    }
}

sail_spec! {
pub open spec fn symbol_first(byte: u8) -> bool {
        !word_delimiter(byte) && !(b'0' <= byte <= b'9') && byte != b'#' && byte != b':'
    }
}

sail_spec! {
pub open spec fn frame_space(byte: u8) -> bool {
        byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n'
    }
}

sail_spec! {
pub open spec fn trivia_after(bytes: Seq<u8>, comment: bool) -> Option<bool>
        decreases bytes.len(),
    {
        if bytes.len() == 0 { Some(comment) }
        else if comment {
            trivia_after(bytes.drop_first(), bytes[0] != b'\n' && bytes[0] != b'\r')
        } else if bytes[0] == b' ' || bytes[0] == b'\t' || bytes[0] == b'\n' || bytes[0] == b'\r' {
            trivia_after(bytes.drop_first(), false)
        } else if bytes[0] == b';' { trivia_after(bytes.drop_first(), true) }
        else { None }
    }
}

sail_spec! {
pub proof fn trivia_join(left: Seq<u8>, right: Seq<u8>, comment: bool)
        ensures trivia_after(left + right, comment) == match trivia_after(left, comment) {
            Some(next) => trivia_after(right, next), None => None,
        },
        decreases left.len(),
    {
        if left.len() > 0 {
            assert((left + right).drop_first() =~= left.drop_first() + right);
            if comment {
                trivia_join(left.drop_first(), right, left[0] != b'\n' && left[0] != b'\r');
            } else if left[0] == b' ' || left[0] == b'\t' || left[0] == b'\n' || left[0] == b'\r' {
                trivia_join(left.drop_first(), right, false);
            } else if left[0] == b';' {
                trivia_join(left.drop_first(), right, true);
            }
        } else { assert(left + right =~= right); }
    }
}

sail_verified! {
    /// Advances actual lexical state and nesting using the framing grammar.
    pub fn frame_step(depth: usize, lex: FrameLex, byte: u8, comments: bool)
        -> (next: (usize, FrameLex))
        requires (0 < depth < usize::MAX)
        ensures (next.0 as int == depth as int
                + if structural(lex, byte) && byte == b'(' { 1int }
                else if structural(lex, byte) && byte == b')' { -1int } else { 0int },
            next.1 == frame_lex(lex, byte, comments),
            next.0 == 0 ==> byte == b')' && next.1 is Plain)
    {
        if matches!(lex, FrameLex::String) {
            return (depth, if byte == b'"' { FrameLex::Quote } else { FrameLex::String });
        }
        if matches!(lex, FrameLex::Symbol) {
            return (depth, if byte == b'|' { FrameLex::Plain } else { FrameLex::Symbol });
        }
        if matches!(lex, FrameLex::Comment) {
            return (depth, if byte == b'\n' || byte == b'\r' { FrameLex::Plain } else { FrameLex::Comment });
        }
        if matches!(lex, FrameLex::Quote) && byte == b'"' {
            return (depth, FrameLex::String);
        }
        let next = if byte == b'(' { depth + 1 }
            else if byte == b')' { depth - 1 } else { depth };
        let lex = if byte == b'"' { FrameLex::String }
            else if byte == b'|' { FrameLex::Symbol }
            else if comments && byte == b';' { FrameLex::Comment }
            else { FrameLex::Plain };
        (next, lex)
    }
}

sail_verified! {
    /// Checks the same exact delimiter byte set used by the native SMT lexer.
    pub fn delimiter(byte: u8) -> (is_delimiter: bool)
        requires (true)
        ensures (is_delimiter == word_delimiter(byte))
    { matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b'(' | b')' | b'"' | b'|' | b';') }
}

sail_verified! {
    /// Checks stream whitespace without interpreting locale or Unicode.
    pub fn stream_space(byte: u8) -> (space: bool)
        requires (true)
        ensures (space == frame_space(byte))
    { matches!(byte, b' ' | b'\t' | b'\r' | b'\n') }
}

sail_spec! {
pub open spec fn flat_byte(byte: u8) -> bool {
        byte != b'(' && byte != b')' && byte != b'"' && byte != b'|' && byte != b';'
    }
}

sail_spec! {
pub proof fn flat_contents_prefix(contents: Seq<u8>)
        requires forall|i: int| 0 <= i < contents.len() ==> #[trigger] flat_byte(contents[i])
        ensures crate::parser_grammar::frame_scan(seq![b'('] + contents,true) == (1int,crate::parser_grammar::FrameLex::Plain)
        decreases contents.len()
    {
        let framed = seq![b'('] + contents;
        if contents.len() > 0 {
            flat_contents_prefix(contents.drop_last());
            assert(framed.drop_last() =~= seq![b'('] + contents.drop_last());
            assert(framed.last() == contents[contents.len() - 1]);
            assert(flat_byte(framed.last()));
            reveal_with_fuel(crate::parser_grammar::frame_scan,1);
        } else {
            assert(framed =~= seq![b'(']);
            reveal_with_fuel(crate::parser_grammar::frame_scan,2);
        }
    }
}

sail_spec! {
pub proof fn flat_list_frame(contents: Seq<u8>)
        requires forall|i: int| 0 <= i < contents.len() ==> #[trigger] flat_byte(contents[i])
        ensures crate::parser_grammar::single_list(seq![b'('] + contents + seq![b')'],true)
    {
        let full = seq![b'('] + contents + seq![b')'];
        flat_contents_prefix(contents);
        assert(full.drop_last() =~= seq![b'('] + contents);
        reveal_with_fuel(crate::parser_grammar::frame_scan,1);
        assert forall|i: int| 1 <= i < full.len() implies crate::parser_grammar::frame_scan(full.take(i),true).0 > 0 by {
            let prefix = contents.take(i - 1);
            assert(full.take(i) =~= seq![b'('] + prefix);
            flat_contents_prefix(prefix);
        }
    }
}
