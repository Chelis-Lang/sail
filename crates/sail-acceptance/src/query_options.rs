//! Exact scalar option fields and persistent native-session readback agreement.
//!
//! This records raw `:print-success` and `:rlimit` bytes. It does not establish
//! how a solver interprets options, aliases, resets or its initial defaults.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Untrusted byte intervals proposed by the native lexer.
    #[derive(Debug, Clone, Copy)]
    pub struct OptionSpans {
        /// The original command head interval.
        pub head_start: usize,
        /// One past the original head.
        pub head_end: usize,
        /// The option keyword interval.
        pub keyword_start: usize,
        /// One past the original keyword.
        pub keyword_end: usize,
        /// The proposed scalar value interval.
        pub value_start: usize,
        /// One past the original value.
        pub value_end: usize,
    }

    /// Raw option bytes independently retained through every accepted command.
    /// No deserializer or public field can create a matched state.
    pub struct OptionState {
        print_success: Option<Vec<u8>>,
        resource_limit: Option<Vec<u8>>,
        active: bool,
    }
}

sail_verified! {
    /// A nonempty accepted history exercises the whole state factory, not just
    /// its one-way refusal contract. It retains options across ordinary input
    /// and reset-assertions, then observes an actual empty-map reset readback.
    #[allow(clippy::byte_char_slices, reason = "Literal bytes expose the complete constructive raw-command witness.")]
    pub fn scalar_option_control() -> (accepted: bool)
        requires (true) ensures (accepted)
    {
        let print = [b'(',b's',b'e',b't',b'-',b'o',b'p',b't',b'i',b'o',b'n',b' ',b':',b'p',b'r',b'i',b'n',b't',b'-',b's',b'u',b'c',b'c',b'e',b's',b's',b' ',b'f',b'a',b'l',b's',b'e',b')'];
        let limit = [b'(',b's',b'e',b't',b'-',b'o',b'p',b't',b'i',b'o',b'n',b' ',b':',b'r',b'l',b'i',b'm',b'i',b't',b' ',b'1',b'0',b')'];
        let ordinary = [b'(',b'a',b's',b's',b'e',b'r',b't',b' ',b't',b'r',b'u',b'e',b')'];
        let retained = [b'(',b'r',b'e',b's',b'e',b't',b'-',b'a',b's',b's',b'e',b'r',b't',b'i',b'o',b'n',b's',b')'];
        let reset = [b'(',b'r',b'e',b's',b'e',b't',b')'];
        let print_spans = OptionSpans { head_start: 1, head_end: 11,
            keyword_start: 12, keyword_end: 26, value_start: 27, value_end: 32 };
        let limit_spans = OptionSpans { head_start: 1, head_end: 11,
            keyword_start: 12, keyword_end: 19, value_start: 20, value_end: 22 };
        let no = [b'f',b'a',b'l',b's',b'e'];
        let ten = [b'1',b'0'];
        sail_proof! {
            crate::query_prefix::flat_list_frame(print@.subrange(1,32));
            crate::query_prefix::flat_list_frame(limit@.subrange(1,22));
            crate::query_prefix::flat_list_frame(ordinary@.subrange(1,12));
            crate::query_prefix::flat_list_frame(retained@.subrange(1,17));
            crate::query_prefix::flat_list_frame(reset@.subrange(1,6));
            assert(print@ =~= seq![b'(']+print@.subrange(1,32)+seq![b')']);
            assert(limit@ =~= seq![b'(']+limit@.subrange(1,22)+seq![b')']);
            assert(ordinary@ =~= seq![b'(']+ordinary@.subrange(1,12)+seq![b')']);
            assert(retained@ =~= seq![b'(']+retained@.subrange(1,17)+seq![b')']);
            assert(reset@ =~= seq![b'(']+reset@.subrange(1,6)+seq![b')']);
            reveal(crate::query_policy::named);
            reveal_strlit("set-option");
            reveal_strlit(":print-success");
            reveal_strlit(":rlimit");
            reveal_strlit("reset");
            vstd::string::is_ascii_spec_bytes("set-option");
            vstd::string::is_ascii_spec_bytes(":print-success");
            vstd::string::is_ascii_spec_bytes(":rlimit");
            vstd::string::is_ascii_spec_bytes("reset");
            reveal_with_fuel(crate::query::trivia_after,2);
            assert(scalar_fields(print@,print_spans));
            assert(scalar_fields(limit@,limit_spans));
            assert(crate::query::head_span(ordinary@,1,7));
            assert(crate::query::head_span(retained@,1,17));
            assert(crate::query::head_span(reset@,1,6));
        }
        let mut state = OptionState::new();
        if !state.matches(None,None) { return false; }
        if !state.advance(&print,Some(print_spans),Some(&no),None) { return false; }
        if !state.advance(&limit,Some(limit_spans),Some(&no),Some(&ten)) { return false; }
        if !state.advance(&ordinary,None,Some(&no),Some(&ten)) { return false; }
        if !state.advance(&retained,None,Some(&no),Some(&ten)) { return false; }
        if !state.matches(Some(&no),Some(&ten)) { return false; }
        if !state.advance(&reset,None,None,None) { return false; }
        state.matches(None,None)
    }
}

sail_spec! {
    pub open spec fn atom_bytes(bytes: Seq<u8>) -> bool {
        bytes.len() > 0 && forall|i: int| 0 <= i < bytes.len() ==>
            !crate::query::word_delimiter(bytes[i])
    }

    pub open spec fn keyword_fields(command: Seq<u8>, spans: OptionSpans) -> bool {
        crate::query::single_list(command, true)
        && 1 <= spans.head_start < spans.head_end < spans.keyword_start
        && spans.keyword_start + 1 < spans.keyword_end < command.len()
        && crate::query::trivia_after(command.subrange(1, spans.head_start as int), false) == Some(false)
        && crate::query_policy::named(command.subrange(spans.head_start as int, spans.head_end as int), "set-option"@)
        && crate::query::trivia_after(command.subrange(spans.head_end as int, spans.keyword_start as int), false) == Some(false)
        && command[spans.keyword_start as int] == b':'
        && atom_bytes(command.subrange(spans.keyword_start as int, spans.keyword_end as int))
        && crate::query::word_delimiter(command[spans.keyword_end as int])
    }

    pub open spec fn scalar_fields(command: Seq<u8>, spans: OptionSpans) -> bool {
        keyword_fields(command, spans)
        && spans.keyword_end < spans.value_start < spans.value_end < command.len()
        && crate::query::trivia_after(command.subrange(spans.keyword_end as int, spans.value_start as int), false) == Some(false)
        && atom_bytes(command.subrange(spans.value_start as int, spans.value_end as int))
        && crate::query::trivia_after(command.subrange(spans.value_end as int, command.len() - 1), false) == Some(false)
    }

    pub open spec fn observed_value(value: Option<&[u8]>) -> Option<Seq<u8>> {
        match value { Some(bytes) => Some(bytes@), None => None }
    }

    pub open spec fn next_options(command: Seq<u8>, spans: Option<OptionSpans>,
        before_print: Option<Seq<u8>>, before_limit: Option<Seq<u8>>,
        after_print: Option<Seq<u8>>, after_limit: Option<Seq<u8>>) -> bool {
        exists|head: Seq<u8>| (exists|start: int, end: int|
            crate::query::head_span(command, start, end) && head == command.subrange(start, end))
        && if crate::query_policy::named(head, "reset"@) {
            after_print.is_none() && after_limit.is_none()
        } else if crate::query_policy::named(head, "set-option"@) {
            spans.is_some() && keyword_fields(command, spans.unwrap()) && {
                let s = spans.unwrap();
                let key = command.subrange(s.keyword_start as int, s.keyword_end as int);
                if crate::query_policy::named(key, ":print-success"@) {
                    scalar_fields(command, s)
                        && after_print == Some(command.subrange(s.value_start as int, s.value_end as int))
                        && after_limit == before_limit
                } else if crate::query_policy::named(key, ":rlimit"@) {
                    scalar_fields(command, s)
                        && after_limit == Some(command.subrange(s.value_start as int, s.value_end as int))
                        && after_print == before_print
                } else { after_print == before_print && after_limit == before_limit }
            }
        } else { after_print == before_print && after_limit == before_limit }
    }
}

sail_verified! {
    /// A scalar field cannot hide a delimiter, string, quoted symbol or comment.
    pub fn scalar_atom(bytes: &[u8]) -> (accepted: bool)
        requires (true) ensures (accepted == atom_bytes(bytes@))
    {
        if bytes.is_empty() { return false; }
        let mut i = 0usize;
        sail_while! { (i < bytes.len())
            invariant (i <= bytes.len(), bytes.len() > 0,
                forall|j: int| 0 <= j < i ==> !crate::query::word_delimiter(bytes@[j]))
            decreases (bytes.len() - i)
            {
                if crate::query::delimiter(bytes[i]) { return false; }
                i += 1;
            }
        }
        true
    }
}

sail_verified! {
    /// Validate the original set-option head and keyword without trusting lexer tags.
    pub fn option_keyword(command: &[u8], spans: OptionSpans) -> (accepted: bool)
        requires (true) ensures (accepted == keyword_fields(command@, spans))
    {
        if command.len() < 2 || spans.head_start < 1
            || spans.head_start >= spans.head_end || spans.head_end >= spans.keyword_start
            || spans.keyword_start >= spans.keyword_end || spans.keyword_end >= command.len()
            || spans.keyword_end - spans.keyword_start < 2 { return false; }
        crate::query::complete_list(command, true)
            && crate::query::trivia(&command[1..spans.head_start], false)
            && crate::query_policy::word_eq(&command[spans.head_start..spans.head_end], "set-option")
            && crate::query::trivia(&command[spans.head_end..spans.keyword_start], false)
            && command[spans.keyword_start] == b':'
            && scalar_atom(&command[spans.keyword_start..spans.keyword_end])
            && crate::query::delimiter(command[spans.keyword_end])
    }
}

sail_verified! {
    /// Require one exact scalar value and no omitted or trailing argument bytes.
    pub fn option_scalar(command: &[u8], spans: OptionSpans) -> (accepted: bool)
        requires (true) ensures (accepted == scalar_fields(command@, spans))
    {
        if !option_keyword(command, spans) || spans.keyword_end >= spans.value_start
            || spans.value_start >= spans.value_end || spans.value_end >= command.len() { return false; }
        crate::query::trivia(&command[spans.keyword_end..spans.value_start], false)
            && scalar_atom(&command[spans.value_start..spans.value_end])
            && crate::query::trivia(&command[spans.value_end..command.len() - 1], false)
    }
}

sail_verified! {
    /// Compare actual optional readback with the independently retained raw bytes.
    pub fn option_readback(expected: &Option<Vec<u8>>, actual: Option<&[u8]>) -> (same: bool)
        requires (true)
        ensures (same == (match expected { Some(bytes) => Some(bytes@), None => None } == observed_value(actual)))
    {
        match (expected, actual) {
            (Some(before), Some(after)) => crate::inputs::byte_eq(before, after),
            (None, None) => true,
            _ => false,
        }
    }
}

sail_code! {
    impl OptionState {
        sail_impl_spec! {
            pub closed spec fn print_view(&self) -> Option<Seq<u8>> {
                match self.print_success { Some(bytes) => Some(bytes@), None => None }
            }
            pub closed spec fn limit_view(&self) -> Option<Seq<u8>> {
                match self.resource_limit { Some(bytes) => Some(bytes@), None => None }
            }
            pub closed spec fn active_view(&self) -> bool { self.active }
        }

        sail_method! {
            /// A fresh native map has neither explicit setting; solver defaults are separate.
            #[allow(clippy::new_without_default, reason = "The explicit initial option observation is checked in the shared constructor.")]
            pub fn new() -> (state: Self)
                requires (true)
                ensures (state.active_view(), state.print_view().is_none(), state.limit_view().is_none())
            { Self { print_success: None, resource_limit: None, active: true } }
        }

        sail_method! {
            /// Recheck the actual native map at every policy use, including ordinary commands.
            pub fn matches(&self, print: Option<&[u8]>, limit: Option<&[u8]>) -> (accepted: bool)
                requires (true)
                ensures (accepted == (self.active_view()
                    && self.print_view() == observed_value(print) && self.limit_view() == observed_value(limit)))
            {
                self.active && option_readback(&self.print_success, print)
                    && option_readback(&self.resource_limit, limit)
            }
        }

        sail_method! {
            /// Advance from the full original command, then require actual post-apply readback.
            /// Refusal is sticky; native map preservation is never an assumed premise.
            pub fn advance(&mut self, command: &[u8], spans: Option<OptionSpans>,
                print: Option<&[u8]>, limit: Option<&[u8]>) -> (accepted: bool)
                requires (true)
                ensures (accepted == (old(self).active_view()
                        && next_options(command@, spans, old(self).print_view(), old(self).limit_view(),
                            observed_value(print), observed_value(limit))),
                    final(self).active_view() == accepted,
                    accepted ==> old(self).active_view()
                        && next_options(command@, spans, old(self).print_view(), old(self).limit_view(),
                            final(self).print_view(), final(self).limit_view())
                        && final(self).print_view() == observed_value(print)
                        && final(self).limit_view() == observed_value(limit))
            {
                if !self.active { return false; }
                self.active = false;
                let head = crate::query::command_head(command);
                let Some(head) = head else { return false; };
                if crate::query_policy::word_eq(head, "reset") {
                    self.print_success = None;
                    self.resource_limit = None;
                } else if crate::query_policy::word_eq(head, "set-option") {
                    let Some(spans) = spans else { return false; };
                    if !option_keyword(command, spans) { return false; }
                    let key = &command[spans.keyword_start..spans.keyword_end];
                    let print_setting = crate::query_policy::word_eq(key, ":print-success");
                    let limit_setting = crate::query_policy::word_eq(key, ":rlimit");
                    if print_setting || limit_setting {
                        if !option_scalar(command, spans) { return false; }
                        let value = crate::inputs::copy_bytes(&command[spans.value_start..spans.value_end]);
                        if print_setting { self.print_success = Some(value); }
                        else { self.resource_limit = Some(value); }
                    }
                }
                if !option_readback(&self.print_success, print)
                    || !option_readback(&self.resource_limit, limit) { return false; }
                self.active = true;
                true
            }
        }
    }
}
