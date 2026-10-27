//! Original served replies and the existing mechanical replay comparison policy.
//!
//! A statistics shape exception is not a semantic equivalence or solver-origin
//! theorem. The native splitter remains an independent refusal check.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Untrusted native lexer intervals for one get-info keyword.
    #[derive(Debug, Clone, Copy)]
    pub struct KeywordSpans {
        /// Original head start.
        pub head_start: usize,
        /// Original head end.
        pub head_end: usize,
        /// Original keyword start.
        pub keyword_start: usize,
        /// Original keyword end.
        pub keyword_end: usize,
    }
    /// Owns the actual command and cache-hit bytes, with a privately minted
    /// mechanical comparison policy. No public Boolean grants an exception.
    pub struct ServedReply {
        command: Vec<u8>,
        bytes: Vec<u8>,
        statistics: bool,
    }
    /// Compares suppressed replies in their original served order.
    pub struct Replay<'a, Owner> {
        owner: Owner,
        served: &'a [ServedReply],
        checked: usize,
        diverged: bool,
    }
}

sail_verified! {
    /// A nonempty exact/statistics history exercises actual receipt minting,
    /// comparison, ordered remaining-count transitions and final completion.
    /// The differing list values witness mechanical policy, not solver meaning.
    #[allow(clippy::byte_char_slices, reason = "Literal bytes expose a constructive raw command and response history.")]
    pub fn replay_comparison_control() -> (accepted: bool)
        requires (true) ensures (accepted)
    {
        let command = [b'(',b'g',b'e',b't',b'-',b'i',b'n',b'f',b'o',b' ',b':',b'a',b'l',b'l',b'-',b's',b't',b'a',b't',b'i',b's',b't',b'i',b'c',b's',b')'];
        let spans = KeywordSpans {head_start:1,head_end:9,keyword_start:10,keyword_end:25};
        let exact = [b'u',b'n',b's',b'a',b't',b'\n'];
        let stored = [b'(',b':',b'x',b' ',b'1',b')',b'\n'];
        let actual = [b'(',b':',b'x',b' ',b'2',b')',b'\n'];
        sail_proof! {
            crate::query_prefix::flat_list_frame(command@.subrange(1,25));
            crate::query_prefix::flat_list_frame(actual@.subrange(1,5));
            assert(command@ =~= seq![b'(']+command@.subrange(1,25)+seq![b')']);
            assert(actual@.subrange(0,6) =~= seq![b'(']+actual@.subrange(1,5)+seq![b')']);
            reveal(crate::query_policy::named);
            reveal_strlit("get-info"); reveal_strlit(":all-statistics");
            vstd::string::is_ascii_spec_bytes("get-info");
            vstd::string::is_ascii_spec_bytes(":all-statistics");
            reveal_with_fuel(crate::query::trivia_after,2);
            assert(statistics_fields(command@,spans));
            assert(crate::query::response_span(actual@,0,6));
            assert(statistics_shape(actual@));
        }
        let first = ServedReply::new(b"(check-sat)",crate::inputs::copy_bytes(&exact),None);
        let second = ServedReply::new(&command,crate::inputs::copy_bytes(&stored),Some(spans));
        let served = [first,second];
        let mut replay = Replay::new(&served,7u64);
        if replay.remaining()!=2 || replay.next_statistics() { return false; }
        if !replay.compare(&exact) || replay.remaining()!=1 || !replay.next_statistics() { return false; }
        if !replay.compare(&actual) || replay.remaining()!=0 { return false; }
        replay.complete() && !replay.finish(true)
    }
}

sail_spec! {
    pub open spec fn statistics_fields(command: Seq<u8>, spans: KeywordSpans) -> bool {
        crate::query::single_list(command,true)
        && 1 <= spans.head_start < spans.head_end < spans.keyword_start < spans.keyword_end < command.len()
        && crate::query::trivia_after(command.subrange(1,spans.head_start as int),false) == Some(false)
        && crate::query_policy::named(command.subrange(spans.head_start as int,spans.head_end as int),"get-info"@)
        && crate::query::trivia_after(command.subrange(spans.head_end as int,spans.keyword_start as int),false) == Some(false)
        && crate::query_policy::named(command.subrange(spans.keyword_start as int,spans.keyword_end as int),":all-statistics"@)
        && crate::query::trivia_after(command.subrange(spans.keyword_end as int,command.len()-1),false) == Some(false)
    }
    /// Existing response-shape policy, not meaning of the enclosed fields.
    pub open spec fn statistics_shape(bytes: Seq<u8>) -> bool {
        bytes.len() >= 2 && bytes[0] == b'(' && bytes[1] == b':' && crate::query::response_shape(bytes)
    }
}

sail_verified! {
    /// Validates the whole original statistics command and exactly one keyword.
    pub fn statistics_command(command: &[u8], spans: Option<KeywordSpans>) -> (accepted: bool)
        requires (true)
        ensures (accepted == (spans.is_some() && statistics_fields(command@,spans.unwrap())))
    {
        let Some(s) = spans else { return false; };
        if command.len() < 2 || s.head_start < 1 || s.head_start >= s.head_end
            || s.head_end >= s.keyword_start || s.keyword_start >= s.keyword_end
            || s.keyword_end >= command.len() { return false; }
        crate::query::complete_list(command,true)
            && crate::query::trivia(&command[1..s.head_start],false)
            && crate::query_policy::word_eq(&command[s.head_start..s.head_end],"get-info")
            && crate::query::trivia(&command[s.head_end..s.keyword_start],false)
            && crate::query_policy::word_eq(&command[s.keyword_start..s.keyword_end],":all-statistics")
            && crate::query::trivia(&command[s.keyword_end..command.len()-1],false)
    }
}

sail_verified! {
    /// Checks the unchanged broad list shape. Native single-response checking
    /// still runs independently before the statistics comparison is accepted.
    pub fn statistics_reply(bytes: &[u8]) -> (accepted: bool)
        requires (true) ensures (accepted == statistics_shape(bytes@))
    {
        bytes.len() >= 2 && bytes[0] == b'(' && bytes[1] == b':'
            && crate::query::response_frame(bytes)
    }
}

sail_code! {
    impl ServedReply {
        sail_impl_spec! {
            pub closed spec fn command_view(&self) -> Seq<u8> { self.command@ }
            pub closed spec fn bytes_view(&self) -> Seq<u8> { self.bytes@ }
            pub closed spec fn statistics_view(&self) -> bool { self.statistics }
            pub open spec fn reproduces(&self, actual: Seq<u8>) -> bool {
                if self.statistics_view() { statistics_shape(actual) } else { self.bytes_view() == actual }
            }
        }
        sail_method! {
            /// Retains the actual served bytes and copies the complete original
            /// command through the checked copy operation before minting policy.
            pub fn new(command: &[u8], bytes: Vec<u8>, spans: Option<KeywordSpans>) -> (out: Self)
                requires (true)
                ensures (out.command_view() == command@, out.bytes_view() == bytes@,
                    out.statistics_view() == (spans.is_some() && statistics_fields(command@,spans.unwrap())))
            {
                let statistics = statistics_command(command,spans);
                let command = crate::inputs::copy_bytes(command);
                Self { command,bytes,statistics }
            }
        }
        sail_method! {
            /// Original cache-hit bytes, never a reserialized substitute.
            pub fn bytes(&self) -> (out: &[u8]) requires (true) ensures (out@ == self.bytes_view()) { &self.bytes }
        }
        sail_method! {
            /// Original complete command bytes retained with this served reply.
            pub fn command(&self) -> (out: &[u8]) requires (true) ensures (out@ == self.command_view()) { &self.command }
        }
        sail_method! {
            /// Immutable original allocation, allowing native capacity accounting.
            pub fn command_storage(&self) -> (out: &Vec<u8>) requires (true)
                ensures (out@ == self.command_view()) { &self.command }
        }
        sail_method! {
            /// Immutable served allocation, allowing native capacity accounting.
            pub fn reply_storage(&self) -> (out: &Vec<u8>) requires (true)
                ensures (out@ == self.bytes_view()) { &self.bytes }
        }
        sail_method! {
            /// Whether the native splitter refusal check is also required.
            pub fn statistics(&self) -> (out: bool) requires (true) ensures (out == self.statistics_view()) { self.statistics }
        }
        sail_method! {
            /// Actual mechanical decision over these retained and fresh bytes.
            pub fn reproduced_by(&self, actual: &[u8]) -> (accepted: bool)
                requires (true) ensures (accepted == self.reproduces(actual@))
            {
                if self.statistics { statistics_reply(actual) }
                else { crate::inputs::byte_eq(&self.bytes,actual) }
            }
        }
    }
    impl<'a, Owner> Replay<'a, Owner> {
        sail_impl_spec! {
            pub closed spec fn owner_view(&self) -> Owner { self.owner }
            pub closed spec fn served_view(&self) -> Seq<ServedReply> { self.served@ }
            pub closed spec fn checked_view(&self) -> nat { self.checked as nat }
            pub closed spec fn diverged_view(&self) -> bool { self.diverged }
            pub open spec fn well_formed(&self) -> bool { self.checked_view() <= self.served_view().len() }
            pub open spec fn complete_view(&self) -> bool {
                !self.diverged_view() && self.checked_view() == self.served_view().len()
            }
        }
        sail_method! {
            /// Original opaque output owner retained for the completion join.
            pub fn owner(&self) -> (out: &Owner) requires (true)
                ensures (*out == self.owner_view()) { &self.owner }
        }
        sail_method! {
            /// Begins at the first actual served payload, including an empty prefix.
            pub fn new(served: &'a [ServedReply], owner: Owner) -> (out: Self)
                requires (true)
                ensures (out.well_formed(),out.owner_view() == owner,out.served_view() == served@,
                    out.checked_view() == 0,!out.diverged_view())
            { Self { owner,served,checked:0,diverged:false } }
        }
        sail_method! {
            /// Remaining original served positions; no separate supplied count.
            pub fn remaining(&self) -> (out: usize) requires (self.well_formed())
                ensures (out == self.served_view().len()-self.checked_view())
            { self.served.len()-self.checked }
        }
        sail_method! {
            /// Original served prefix cardinality, for the actual stdout join.
            pub fn total(&self) -> (out: usize) requires (true)
                ensures (out == self.served_view().len()) { self.served.len() }
        }
        sail_method! {
            /// Original compared position, retained for the unchanged diagnostic.
            pub fn checked(&self) -> (out: usize) requires (true)
                ensures (out == self.checked_view()) { self.checked }
        }
        sail_method! {
            /// Recheck a retained stdout prefix slice against its original
            /// served ordinal, independently of earlier compare arguments.
            pub fn reproduces_at(&self, ordinal: usize, actual: &[u8]) -> (accepted: bool)
                requires (true)
                ensures (accepted == (ordinal < self.served_view().len()
                    && self.served_view()[ordinal as int].reproduces(actual@)))
            {
                ordinal < self.served.len() && self.served[ordinal].reproduced_by(actual)
            }
        }
        sail_method! {
            /// Sticky divergence observed by the native kill/forwarding path.
            pub fn diverged(&self) -> (out: bool) requires (true)
                ensures (out == self.diverged_view()) { self.diverged }
        }
        sail_method! {
            /// Completion requires every original served position and no refusal.
            pub fn complete(&self) -> (out: bool) requires (true)
                ensures (out == self.complete_view())
            { !self.diverged && self.checked == self.served.len() }
        }
        sail_method! {
            /// The next retained reply needs the independent native framing check.
            pub fn next_statistics(&self) -> (out: bool) requires (self.well_formed())
                ensures (out == (self.checked_view() < self.served_view().len()
                    && self.served_view()[self.checked_view() as int].statistics_view()))
            {
                if self.checked == self.served.len() { false }
                else { self.served[self.checked].statistics() }
            }
        }
        sail_method! {
            /// An independent framing refusal cannot be healed by later comparisons.
            pub fn refuse(&mut self) -> (out: ()) requires (true)
                ensures (final(self).owner_view() == old(self).owner_view(),final(self).served_view() == old(self).served_view(),
                    final(self).checked_view() == old(self).checked_view(),final(self).diverged_view())
            { self.diverged = true; }
        }
        sail_method! {
            /// Compares the actual next suppressed slice; acceptance advances
            /// exactly one original position and all refusals latch divergence.
            pub fn compare(&mut self, actual: &[u8]) -> (accepted: bool)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),final(self).owner_view() == old(self).owner_view(),final(self).served_view() == old(self).served_view(),
                    accepted == (!old(self).diverged_view() && old(self).checked_view() < old(self).served_view().len()
                        && old(self).served_view()[old(self).checked_view() as int].reproduces(actual@)),
                    final(self).checked_view() == old(self).checked_view() + if accepted {1nat} else {0nat},
                    final(self).diverged_view() == !accepted)
            {
                if self.diverged || self.checked == self.served.len()
                    || !self.served[self.checked].reproduced_by(actual) {
                    self.diverged = true; return false;
                }
                self.checked += 1;
                true
            }
        }
        sail_method! {
            /// Preserve the existing clean-short-replay rule without accepting
            /// a short prefix as complete on an unclean process exit.
            pub fn finish(&mut self, clean: bool) -> (diverged: bool)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),final(self).owner_view() == old(self).owner_view(),final(self).served_view() == old(self).served_view(),
                    final(self).checked_view() == old(self).checked_view(),
                    diverged == (old(self).diverged_view() || clean && old(self).checked_view() < old(self).served_view().len()),
                    final(self).diverged_view() == diverged)
            {
                self.diverged |= clean && self.checked < self.served.len();
                self.diverged
            }
        }
    }
}
