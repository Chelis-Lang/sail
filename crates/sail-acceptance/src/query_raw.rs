//! Owned exact input intervals feeding one incremental query key.
//! Current parser grammar and native lexer remain the framing/head authority.
pub use crate::parser_grammar::*;
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_verified! {
    /// Checks actual inter-item bytes, including complete semicolon comments.
    /// At EOF an unfinished line comment is permitted; a command cannot begin in one.
    pub fn trivia(bytes: &[u8], allow_open_comment: bool) -> (accepted: bool)
        requires (true)
        ensures (accepted == (trivia_after(bytes@, false) == Some(false)
            || allow_open_comment && trivia_after(bytes@, false) == Some(true)))
    {
        let mut i = 0;
        let mut comment = false;
        sail_proof! { assert(bytes@.subrange(0, bytes.len() as int) =~= bytes@); }
        sail_while! { (i < bytes.len())
            invariant (i <= bytes.len(),
                trivia_after(bytes@, false) == trivia_after(bytes@.subrange(i as int, bytes.len() as int), comment))
            decreases (bytes.len() - i)
            {
                sail_proof! {
                    assert(bytes@.subrange(i as int, bytes.len() as int).drop_first()
                        =~= bytes@.subrange(i as int + 1, bytes.len() as int));
                    reveal_with_fuel(trivia_after, 1);
                }
                let byte = bytes[i];
                if comment {
                    comment = byte != b'\n' && byte != b'\r';
                } else if byte == b';' {
                    comment = true;
                } else if byte != b' ' && byte != b'\t' && byte != b'\n' && byte != b'\r' {
                    return false;
                }
                i += 1;
            }
        }
        allow_open_comment || !comment
    }
}

sail_code! {
    /// Exact append-only solver input and the portion already fed to key derivation.
    /// Parser output is a candidate interval, never authority over different bytes.
    pub struct RawTranscript {
        bytes: Vec<u8>,
        fed: usize,
        valid: bool,
    }

    /// Borrow of the actual next contiguous bytes supplied to prefix hashing.
    pub struct PrefixFeed<'a> {
        bytes: &'a [u8],
        start: usize,
        end: usize,
    }
}

sail_code! {
    impl<'a> PrefixFeed<'a> {
        sail_impl_spec! {
            /// Exact original raw bytes of this feed interval.
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            /// First raw-byte position in this feed.
            pub closed spec fn from(&self) -> nat { self.start as nat }
            /// Exclusive raw-byte endpoint of this feed.
            pub closed spec fn to(&self) -> nat { self.end as nat }
        }
        sail_method! {
            /// Borrows the unmodified feed bytes.
            pub fn bytes(&self) -> (bytes: &'a [u8])
                requires (true)
                ensures (bytes@ == self.view())
            { self.bytes }
        }
        sail_method! {
            /// First raw-byte position in this feed.
            pub fn start(&self) -> (start: usize)
                requires (true)
                ensures (start == self.from())
            { self.start }
        }
        sail_method! {
            /// Exclusive raw-byte endpoint in this feed.
            pub fn end(&self) -> (end: usize)
                requires (true)
                ensures (end == self.to())
            { self.end }
        }
    }
}

sail_code! {
    impl RawTranscript {
        sail_impl_spec! {
            /// Complete received input bytes, including comments and separators.
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            /// Number of bytes already returned in contiguous prefix feeds.
            pub closed spec fn fed_view(&self) -> nat { self.fed as nat }
            /// Whether interval interpretation has never failed.
            pub closed spec fn active(&self) -> bool { self.valid }
        }
        sail_method! {
            /// Creates an empty valid transcript.
            pub fn empty() -> (transcript: Self)
                requires (true)
                ensures (transcript.view() == Seq::<u8>::empty(), transcript.fed_view() == 0,
                    transcript.active())
            { Self { bytes: Vec::new(), fed: 0, valid: true } }
        }
        sail_method! {
            /// Appends every received byte exactly once without moving the feed endpoint.
            pub fn append(&mut self, bytes: &[u8]) -> (result: ())
                requires (true)
                ensures (final(self).view() == old(self).view() + bytes@,
                    final(self).fed_view() == old(self).fed_view(),
                    final(self).active() == old(self).active())
            { self.bytes.extend_from_slice(bytes); }
        }
        sail_method! {
            /// Borrows the complete original input for solver replay.
            pub fn bytes(&self) -> (bytes: &[u8])
                requires (true)
                ensures (bytes@ == self.view())
            { self.bytes.as_slice() }
        }
        sail_method! {
            /// Number of raw bytes returned for prefix hashing.
            pub fn fed(&self) -> (fed: usize)
                requires (true)
                ensures (fed == self.fed_view())
            { self.fed }
        }
        sail_method! {
            /// Whether every interpreted interval has passed its byte checks.
            pub fn valid(&self) -> (valid: bool)
                requires (true)
                ensures (valid == self.active())
            { self.valid }
        }
        sail_method! {
            /// Permanently refuses further cache interpretation, preserving all raw bytes.
            pub fn refuse(&mut self) -> (result: ())
                requires (true)
                ensures (!final(self).active(), final(self).view() == old(self).view(),
                    final(self).fed_view() == old(self).fed_view())
            { self.valid = false; }
        }
        sail_method! {
            /// Whether the still-unfed suffix is only whitespace or line comments.
            pub fn trailing_trivia(&self) -> (valid: bool)
                requires (true)
                ensures (valid == (self.fed_view() <= self.view().len()
                    && (trivia_after(self.view().subrange(self.fed_view() as int, self.view().len() as int), false) == Some(false)
                    || trivia_after(self.view().subrange(self.fed_view() as int, self.view().len() as int), false) == Some(true))))
            {
                if self.fed > self.bytes.len() { return false; }
                trivia(&self.bytes[self.fed..], true)
            }
        }
        sail_method! {
            /// Validates one parser interval and returns every raw byte since the last feed.
            /// A changed, overlapping, omitted or out-of-range item permanently refuses caching.
            pub fn command<'a>(&'a mut self, offset: usize, command: &[u8]) -> (feed: Option<PrefixFeed<'a>>)
                requires (true)
                ensures (final(self).view() == old(self).view(),
                    feed.is_some() == (old(self).active() && command@.len() > 0
                        && old(self).fed_view() <= offset
                        && offset as nat + command@.len() <= old(self).view().len()
                        && old(self).view().subrange(offset as int, offset as int + command@.len() as int) == command@
                        && trivia_after(old(self).view().subrange(old(self).fed_view() as int, offset as int), false) == Some(false)),
                    feed.is_some() ==> (
                        old(self).active() && final(self).active()
                        && old(self).fed_view() <= offset
                        && offset < final(self).fed_view() <= old(self).view().len()
                        && final(self).fed_view() == offset as nat + command@.len()
                        && old(self).view().subrange(offset as int, final(self).fed_view() as int) == command@
                        && feed.unwrap().from() == old(self).fed_view()
                        && feed.unwrap().to() == final(self).fed_view()
                        && feed.unwrap().view() == old(self).view().subrange(
                            old(self).fed_view() as int, final(self).fed_view() as int)
                        && trivia_after(old(self).view().subrange(old(self).fed_view() as int, offset as int), false) == Some(false)
                    ),
                    feed.is_none() ==> (!final(self).active()
                        && final(self).fed_view() == old(self).fed_view()))
            {
                if !self.valid || command.is_empty() || self.fed > offset {
                    self.valid = false;
                    return None;
                }
                let raw_length = self.bytes.len();
                let Some(end) = offset.checked_add(command.len()) else {
                    self.valid = false;
                    return None;
                };
                if end > raw_length {
                    self.valid = false;
                    return None;
                }
                if !super::inputs::byte_eq(&self.bytes[offset..end], command)
                    || !trivia(&self.bytes[self.fed..offset], false)
                {
                    self.valid = false;
                    return None;
                }
                let start = self.fed;
                self.fed = end;
                Some(PrefixFeed { bytes: &self.bytes[start..end], start, end })
            }
        }
        sail_method! {
            /// Returns the entire remaining raw suffix without interpreting its syntax.
            /// EOF/parser or explicit-exit completion is checked separately by the adapter.
            pub fn tail<'a>(&'a mut self) -> (feed: Option<PrefixFeed<'a>>)
                requires (true)
                ensures (final(self).view() == old(self).view(),
                    feed.is_some() == (old(self).active() && old(self).fed_view() <= old(self).view().len()),
                    feed.is_some() ==> (old(self).active() && final(self).active()
                        && old(self).fed_view() <= old(self).view().len()
                        && final(self).fed_view() == old(self).view().len()
                        && feed.unwrap().from() == old(self).fed_view()
                        && feed.unwrap().to() == old(self).view().len()
                        && feed.unwrap().view() == old(self).view().subrange(
                            old(self).fed_view() as int, old(self).view().len() as int)),
                    feed.is_none() ==> (!final(self).active()
                        && final(self).fed_view() == old(self).fed_view()))
            {
                if !self.valid || self.fed > self.bytes.len() {
                    self.valid = false;
                    return None;
                }
                let start = self.fed;
                let end = self.bytes.len();
                self.fed = end;
                Some(PrefixFeed { bytes: &self.bytes[start..end], start, end })
            }
        }
    }
}

sail_verified! {
    /// Accepts exactly one complete parenthesized frame, without trusting offsets.
    /// Strings use doubled quotes; symbols use bars; only input permits comments.
    pub fn complete_list(bytes: &[u8], comments: bool) -> (accepted: bool)
        requires (true)
        ensures (accepted == single_list(bytes@, comments))
    {
        if bytes.len() < 2 || bytes[0] != b'(' { return false; }
        let mut i = 1usize;
        let mut depth = 1usize;
        let mut lex = FrameLex::Plain;
        sail_proof! {
            assert(bytes@.take(1) =~= seq![b'(']);
            reveal_with_fuel(frame_scan, 2);
        }
        sail_while! { (i < bytes.len())
            invariant (1 <= i <= bytes.len(), 0 < depth <= i,
                bytes.len() >= 2, bytes@[0] == b'(',
                frame_scan(bytes@.take(i as int), comments) == (depth as int, lex),
                forall|j: int| 1 <= j <= i ==> frame_scan(bytes@.take(j), comments).0 > 0)
            decreases (bytes.len() - i)
            {
                let byte = bytes[i];
                let next = frame_step(depth, lex, byte, comments);
                sail_proof! {
                    assert(bytes@.take(i as int + 1).drop_last() =~= bytes@.take(i as int));
                    assert(bytes@.take(i as int + 1).last() == byte);
                    reveal_with_fuel(frame_scan, 1);
                }
                i += 1;
                depth = next.0;
                lex = next.1;
                if depth == 0 { return i == bytes.len(); }
                sail_proof! {
                    assert forall|j: int| 1 <= j <= i implies frame_scan(bytes@.take(j), comments).0 > 0 by {
                        if j == i {} else { assert(j <= i - 1); }
                    }
                }
            }
        }
        sail_proof! { assert(bytes@.take(bytes.len() as int) =~= bytes@); }
        false
    }
}

sail_spec! {
    /// A delimited word has the same stop in the actual lexical specification.
    pub proof fn word_stop_boundary(bytes: Seq<u8>, position: int, limit: int)
        requires 0 <= position <= limit < bytes.len(), word_delimiter(bytes[limit])
        ensures position <= crate::query_lex::word_stop(bytes, position) <= limit,
            word_delimiter(bytes[crate::query_lex::word_stop(bytes, position)]),
            forall|i: int| position <= i < crate::query_lex::word_stop(bytes, position)
                ==> !word_delimiter(bytes[i]),
        decreases limit - position,
    {
        reveal_with_fuel(crate::query_lex::word_stop, 1);
        if !word_delimiter(bytes[position]) {
            assert(position < limit);
            word_stop_boundary(bytes, position + 1, limit);
        }
    }

    /// A complete trivia interval ends at the native lexer's same next byte.
    pub proof fn trivia_stop_at_span(bytes: Seq<u8>, position: int, end: int, comment: bool)
        requires 0 <= position <= end < bytes.len(),
            trivia_after(bytes.subrange(position, end), comment) == Some(false),
            !frame_space(bytes[end]), bytes[end] != b';',
        ensures crate::query_lex::trivia_stop(bytes, position, comment) == end,
        decreases end - position,
    {
        reveal_with_fuel(crate::query_lex::trivia_stop, 1);
        reveal_with_fuel(trivia_after, 1);
        if position < end {
            assert(bytes.subrange(position, end).drop_first() =~= bytes.subrange(position + 1, end));
            if comment {
                trivia_stop_at_span(bytes, position + 1, end, bytes[position] != b'\n' && bytes[position] != b'\r');
            } else if frame_space(bytes[position]) {
                trivia_stop_at_span(bytes, position + 1, end, false);
            } else if bytes[position] == b';' {
                trivia_stop_at_span(bytes, position + 1, end, true);
            } else { assert(false); }
        } else { assert(!comment); }
    }

    /// The native head transition is complete for the original
    /// exact head-span contract, including leading comment trivia.
    pub proof fn head_span_transition(bytes: Seq<u8>, start: int, end: int)
        requires bytes.len() <= usize::MAX, head_span(bytes, start, end),
        ensures crate::query_lex::head_transition(bytes)
            == (end, false, Ok((0usize, bytes.subrange(start, end)))),
    {
        trivia_stop_at_span(bytes, 1, start, false);
        word_stop_boundary(bytes, start, end);
        let stopped = crate::query_lex::word_stop(bytes, start);
        if stopped < end { assert(!word_delimiter(bytes[stopped])); assert(false); }
        assert(stopped == end);
        assert(bytes.subrange(start, end).len() > 0);
        assert(bytes.subrange(start, end)[0] == bytes[start]);
        reveal_with_fuel(crate::query_lex::trivia_stop, 1);
        reveal(crate::query_lex::classified_word);
        reveal(crate::query_lex::lex_transition);
        reveal(crate::query_lex::head_transition);
    }

    /// A first bare symbol in a complete list has an original head span.
    pub proof fn first_symbol_head_span(bytes: Seq<u8>)
        requires bytes.len() <= usize::MAX, single_list(bytes, true), symbol_first(bytes[1]),
        ensures exists|start: int, end: int| head_span(bytes, start, end),
    {
        word_stop_boundary(bytes, 1, bytes.len() - 1);
        let end = crate::query_lex::word_stop(bytes, 1);
        reveal_with_fuel(crate::query_lex::word_stop, 1);
        assert(1 < end);
        assert(bytes.subrange(1, 1) =~= Seq::<u8>::empty());
        assert(head_span(bytes, 1, end));
    }
}

sail_verified! {
    /// Extracts the actual first bare-head name only after full framing.
    /// This does not validate the command's arity, types or solver semantics.
    pub fn command_head(bytes: &[u8]) -> (head: Option<&[u8]>)
        requires (true)
        ensures (head.is_some() ==> exists|start: int, end: int|
            head_span(bytes@, start, end) && head.unwrap()@ == bytes@.subrange(start, end),
            (exists|start: int, end: int| head_span(bytes@,start,end)) ==> head.is_some(),
            forall|start: int,end: int| head_span(bytes@,start,end) ==> head.is_some() && head.unwrap()@ == bytes@.subrange(start,end),
            single_list(bytes@, true) && symbol_first(bytes@[1]) ==> head.is_some(),
            single_list(bytes@, true) && !frame_space(bytes@[1]) && bytes@[1] != b';'
                && !symbol_first(bytes@[1]) ==> head.is_none())
    {
        if !complete_list(bytes, true) { return None; }
        let mut lexer = crate::query_lex::Lexer::new(bytes);
        let observed = lexer.command_head();
        sail_proof! {
            assert(bytes@.len() == bytes.len());
            assert(bytes@.len() <= usize::MAX);
            assert forall|start: int, end: int| head_span(bytes@, start, end)
                implies match observed { Ok((_open, name)) => name@ == bytes@.subrange(start, end), Err(_) => false } by {
                head_span_transition(bytes@, start, end);
            }
            if symbol_first(bytes@[1]) { first_symbol_head_span(bytes@); }
            if !frame_space(bytes@[1]) && bytes@[1] != b';' && !symbol_first(bytes@[1]) {
                if let Ok((_open, name)) = observed {
                    let (start, end) = choose|start: int, end: int|
                        head_span(bytes@, start, end) && name@ == bytes@.subrange(start, end);
                    trivia_stop_at_span(bytes@, 1, start, false);
                    reveal_with_fuel(crate::query_lex::trivia_stop, 1);
                    assert(start == 1);
                    assert(false);
                }
            }
        }
        match observed { Ok((_open, head)) => Some(head), Err(_) => None }
    }
}

sail_verified! {
    /// Checks command framing and exact first-head bytes before deriving its prefix binding.
    /// Any refusal permanently disables cache interpretation while preserving raw input.
    pub fn framed_command<'a, 'b>(raw: &'a mut RawTranscript, offset: usize, command: &'b [u8])
        -> (accepted: Option<(PrefixFeed<'a>, &'b [u8])>)
        requires (true)
        ensures (final(raw).view() == old(raw).view(),
            accepted.is_some() ==> (
                final(raw).active() && old(raw).active() && single_list(command@, true)
                && exists|start: int, end: int| head_span(command@, start, end)
                    && accepted.unwrap().1@ == command@.subrange(start, end)),
            accepted.is_some() ==> (
                old(raw).fed_view() <= offset
                && final(raw).fed_view() == offset as nat + command@.len()
                && final(raw).fed_view() <= old(raw).view().len()
                && old(raw).view().subrange(offset as int, final(raw).fed_view() as int) == command@
                && accepted.unwrap().0.from() == old(raw).fed_view()
                && accepted.unwrap().0.to() == final(raw).fed_view()
                && accepted.unwrap().0.view() == old(raw).view().subrange(old(raw).fed_view() as int, final(raw).fed_view() as int)
                && trivia_after(old(raw).view().subrange(old(raw).fed_view() as int, offset as int), false) == Some(false)),
            accepted.is_none() ==> (!final(raw).active() && final(raw).fed_view() == old(raw).fed_view()),
            old(raw).active() && (exists|start: int,end: int| head_span(command@,start,end))
                && old(raw).fed_view() <= offset
                && offset as nat + command@.len() <= old(raw).view().len()
                && old(raw).view().subrange(offset as int, offset as int + command@.len() as int) == command@
                && trivia_after(old(raw).view().subrange(old(raw).fed_view() as int, offset as int), false) == Some(false)
                ==> accepted.is_some(),
            old(raw).active() && single_list(command@, true) && symbol_first(command@[1])
                && old(raw).fed_view() <= offset
                && offset as nat + command@.len() <= old(raw).view().len()
                && old(raw).view().subrange(offset as int, offset as int + command@.len() as int) == command@
                && trivia_after(old(raw).view().subrange(old(raw).fed_view() as int, offset as int), false) == Some(false)
                ==> accepted.is_some())
    {
        let head = command_head(command);
        let Some(head) = head else { raw.refuse(); return None; };
        let feed = raw.command(offset, command);
        match feed { Some(feed) => Some((feed, head)), None => None }
    }
}

sail_verified! {
    /// A nonempty command head following whitespace and a completed comment is accepted exactly.
    #[allow(clippy::byte_char_slices, reason = "Explicit byte values expose the constructive grammar witness to stock verification.")]
    pub fn commented_head_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let bytes = [b'(',b' ',b';',b'x',b'\n',b'c',b')'];
        sail_proof! {
            reveal_with_fuel(frame_scan,8);
            reveal_with_fuel(trivia_after,6);
            assert(head_span(bytes@,5,6));
        }
        let expected = [b'c'];
        match command_head(&bytes) {
            Some(head) => {
                sail_proof! { assert(head@ == bytes@.subrange(5,6)); assert(head@ =~= expected@); }
                crate::inputs::byte_eq(head,&expected)
            }
            None => false,
        }
    }
}
