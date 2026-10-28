//! Actual lexical cursor operations over original borrowed SMT bytes.
//! These endpoints describe lexical spans, not command or solver semantics.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
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
}

sail_code! {
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
}

sail_spec! {
    pub open spec fn trivia_stop(bytes:Seq<u8>,at:int,comment:bool) -> int
        decreases bytes.len()-at,
    {
        if at < 0 || at >= bytes.len() { at }
        else if comment { trivia_stop(bytes,at+1,bytes[at] != b'\n' && bytes[at] != b'\r') }
        else if crate::parser_grammar::frame_space(bytes[at]) { trivia_stop(bytes,at+1,false) }
        else if bytes[at] == b';' { trivia_stop(bytes,at+1,true) }
        else { at }
    }
    pub open spec fn word_stop(bytes:Seq<u8>,at:int) -> int
        decreases bytes.len()-at,
    {
        if at < 0 || at >= bytes.len() || crate::parser_grammar::word_delimiter(bytes[at]) { at }
        else { word_stop(bytes,at+1) }
    }
    pub open spec fn quoted_after(bytes:Seq<u8>,at:int) -> Option<int>
        decreases bytes.len()-at,
    {
        if at < 0 || at >= bytes.len() { None }
        else if bytes[at] == b'|' { Some(at+1) }
        else { quoted_after(bytes,at+1) }
    }
    pub open spec fn string_after(bytes:Seq<u8>,at:int) -> Option<int>
        decreases bytes.len()-at,
    {
        if at < 0 || at >= bytes.len() { None }
        else if bytes[at] == b'"' {
            if at+1 < bytes.len() && bytes[at+1] == b'"' { string_after(bytes,at+2) }
            else { Some(at+1) }
        } else { string_after(bytes,at+1) }
    }
}

sail_verified! {
    /// The actual lexer skips exactly whitespace and complete or trailing
    /// top-level comments, consuming CR/LF once without changing source bytes.
    pub fn skip_trivia(bytes:&[u8],start:usize) -> (end:usize)
        requires (start <= bytes.len())
        ensures (start <= end <= bytes.len(),end as int == trivia_stop(bytes@,start as int,false),
            crate::parser_grammar::trivia_after(bytes@.subrange(start as int,end as int),false).is_some(),
            end < bytes.len() ==> !crate::parser_grammar::frame_space(bytes@[end as int]) && bytes@[end as int] != b';'
                && crate::parser_grammar::trivia_after(bytes@.subrange(start as int,end as int),false) == Some(false))
    {
        let mut position=start;
        let mut comment=false;
        sail_while! { (position < bytes.len())
            invariant (start <= position <= bytes.len(),
                trivia_stop(bytes@,start as int,false) == trivia_stop(bytes@,position as int,comment),
                crate::parser_grammar::trivia_after(bytes@.subrange(start as int,position as int),false) == Some(comment))
            decreases (bytes.len()-position)
            {
                let byte=bytes[position];
                if comment {
                    comment=byte != b'\n' && byte != b'\r';
                } else if crate::parser_grammar::stream_space(byte) {
                } else if byte == b';' {
                    comment=true;
                } else { return position; }
                sail_proof! {
                    crate::parser_grammar::trivia_join(bytes@.subrange(start as int,position as int),seq![byte],false);
                    reveal_with_fuel(crate::parser_grammar::trivia_after,2);
                    assert(bytes@.subrange(start as int,position as int+1)
                        =~= bytes@.subrange(start as int,position as int)+seq![byte]);
                }
                position += 1;
            }
        }
        position
    }
}

sail_verified! {
    /// Finds exactly the actual first word delimiter or input end.
    pub fn word_end(bytes:&[u8],start:usize) -> (end:usize)
        requires (start <= bytes.len())
        ensures (start <= end <= bytes.len(),end as int == word_stop(bytes@,start as int),
            forall|i:int| start <= i < end ==> !crate::parser_grammar::word_delimiter(bytes@[i]),
            end < bytes.len() ==> crate::parser_grammar::word_delimiter(bytes@[end as int]))
    {
        let mut end=start;
        sail_while! { (end < bytes.len())
            invariant (start <= end <= bytes.len(),word_stop(bytes@,start as int) == word_stop(bytes@,end as int),
                forall|i:int| start <= i < end ==> !crate::parser_grammar::word_delimiter(bytes@[i]))
            decreases (bytes.len()-end)
            {
                if crate::parser_grammar::delimiter(bytes[end]) { return end; }
                end += 1;
            }
        }
        end
    }
}

sail_verified! {
    /// The actual first bar closes a quoted symbol; no escape or normalization
    /// rule is introduced. A missing bar preserves the native lexical error.
    pub fn quoted_symbol_end(bytes:&[u8],start:usize) -> (end:Option<usize>)
        requires (start < bytes.len(),bytes@[start as int] == b'|')
        ensures (match end {
            Some(end) => start+1 < end <= bytes.len() && quoted_after(bytes@,start as int+1) == Some(end as int)
                && bytes@[end as int-1] == b'|'
                && forall|i:int| start < i < end-1 ==> bytes@[i] != b'|',
            None => quoted_after(bytes@,start as int+1).is_none()
                && forall|i:int| start < i < bytes.len() ==> bytes@[i] != b'|',
        })
    {
        let mut position=start+1;
        sail_while! { (position < bytes.len())
            invariant (start < position <= bytes.len(),
                quoted_after(bytes@,start as int+1) == quoted_after(bytes@,position as int),
                forall|i:int| start < i < position ==> bytes@[i] != b'|')
            decreases (bytes.len()-position)
            {
                if bytes[position] == b'|' { return Some(position+1); }
                position += 1;
            }
        }
        None
    }
}

sail_verified! {
    /// The actual SMT string scan consumes doubled quotes as a pair and
    /// returns the first unpaired closing quote, retaining its original span.
    pub fn string_end(bytes:&[u8],start:usize) -> (end:Option<usize>)
        requires (start < bytes.len(),bytes@[start as int] == b'"')
        ensures (match end {
            Some(end) => start+1 < end <= bytes.len() && string_after(bytes@,start as int+1) == Some(end as int)
                && bytes@[end as int-1] == b'"',
            None => string_after(bytes@,start as int+1).is_none(),
        })
    {
        let mut position=start+1;
        sail_while! { (position < bytes.len())
            invariant (start < position <= bytes.len(),
                string_after(bytes@,start as int+1) == string_after(bytes@,position as int))
            decreases (bytes.len()-position)
            {
                if bytes[position] == b'"' {
                    if position+1 < bytes.len() && bytes[position+1] == b'"' { position += 2; }
                    else { return Some(position+1); }
                } else { position += 1; }
            }
        }
        None
    }
}

sail_spec! {
    // The same ASCII classes as native u8::{is_ascii_digit,is_ascii_hexdigit}
    // and the original binary-literal match; no solver interpretation.
    pub open spec fn digit_class(byte:u8,kind:u8) -> bool {
        if kind == 2 { byte == b'0' || byte == b'1' }
        else { b'0' <= byte <= b'9' || kind == 1 && (b'a' <= byte <= b'f' || b'A' <= byte <= b'F') }
    }
    pub open spec fn digit_run(bytes:Seq<u8>,start:int,kind:u8) -> bool {
        0 <= start < bytes.len() && forall|i:int| start <= i < bytes.len() ==> digit_class(bytes[i],kind)
    }
    pub open spec fn first_dot(bytes:Seq<u8>,at:int) -> int
        decreases bytes.len()-at,
    {
        if at < 0 || at >= bytes.len() || bytes[at] == b'.' { at }
        else { first_dot(bytes,at+1) }
    }
    pub open spec fn classified_word(bytes:Seq<u8>) -> Option<TokenKind> {
        if bytes.len() == 0 { None }
        else if bytes[0] == b'#' {
            if bytes.len() >= 2 && bytes[1] == b'x' && digit_run(bytes,2,1) { Some(TokenKind::Hexadecimal) }
            else if bytes.len() >= 2 && bytes[1] == b'b' && digit_run(bytes,2,2) { Some(TokenKind::Binary) }
            else { None }
        } else if bytes[0] == b':' {
            if bytes.len()>1 { Some(TokenKind::Keyword) } else { None }
        } else if b'0' <= bytes[0] <= b'9' {
            let dot=first_dot(bytes,0);
            if dot < bytes.len() {
                if digit_run(bytes.take(dot),0,0) && digit_run(bytes,dot+1,0) { Some(TokenKind::Decimal) } else { None }
            } else { if digit_run(bytes,0,0) { Some(TokenKind::Numeral) } else { None } }
        } else { Some(TokenKind::Symbol) }
    }
}

sail_verified! {
    /// Checks the same nonempty original ASCII digit range used by the
    /// hexadecimal, binary, numeral and decimal token branches.
    #[allow(clippy::manual_range_contains, reason = "Explicit ASCII bounds are the actual shared operations checked by pinned Stock.")]
    fn all_digits(bytes:&[u8],start:usize,kind:u8) -> (accepted:bool)
        requires (start <= bytes.len(),kind <= 2)
        ensures (accepted == digit_run(bytes@,start as int,kind))
    {
        if start == bytes.len() { return false; }
        let mut position=start;
        sail_while! { (position < bytes.len())
            invariant (start < bytes.len(),start <= position <= bytes.len(),kind <= 2,
                forall|i:int| start <= i < position ==> digit_class(bytes@[i],kind))
            decreases (bytes.len()-position)
            {
                let byte=bytes[position];
                let accepted=if kind == 2 { byte == b'0' || byte == b'1' }
                    else { (b'0' <= byte && byte <= b'9')
                        || kind == 1 && ((b'a' <= byte && byte <= b'f') || (b'A' <= byte && byte <= b'F')) };
                if !accepted { return false; }
                position += 1;
            }
        }
        true
    }
}

sail_verified! {
    /// Actual native word classification over the original borrowed span.
    /// Invalid literals retain the same None/error path; permissive simple
    /// symbols, numeric spellings and non-ASCII bytes are unchanged.
    pub fn classify_word(bytes:&[u8]) -> (kind:Option<TokenKind>)
        requires (true) ensures (kind == classified_word(bytes@))
    {
        if bytes.is_empty() { return None; }
        if bytes[0] == b'#' {
            if bytes.len() >= 2 && bytes[1] == b'x' {
                if all_digits(bytes,2,1) { Some(TokenKind::Hexadecimal) } else { None }
            } else if bytes.len() >= 2 && bytes[1] == b'b' {
                if all_digits(bytes,2,2) { Some(TokenKind::Binary) } else { None }
            } else { None }
        } else if bytes[0] == b':' {
            if bytes.len()>1 { Some(TokenKind::Keyword) } else { None }
        } else if bytes[0] >= b'0' && bytes[0] <= b'9' {
            let mut dot=0usize;
            sail_while! { (dot < bytes.len() && bytes[dot] != b'.')
                invariant (dot <= bytes.len(),first_dot(bytes@,0) == first_dot(bytes@,dot as int),
                    forall|i:int| 0 <= i < dot ==> bytes@[i] != b'.')
                decreases (bytes.len()-dot)
                {
                    dot += 1;
                }
            }
            sail_proof! {
                assert(bytes@.subrange(0,dot as int) =~= bytes@.take(dot as int));
                if dot < bytes.len() {
                    assert(bytes@[dot as int] == b'.');
                    reveal_with_fuel(first_dot,1);
                } else {
                    assert(first_dot(bytes@,dot as int) == dot as int);
                }
                assert(first_dot(bytes@,0) == dot as int);
            }
            if dot < bytes.len() {
                if all_digits(&bytes[..dot],0,0) && all_digits(bytes,dot+1,0) { Some(TokenKind::Decimal) } else { None }
            } else {
                if all_digits(bytes,0,0) { Some(TokenKind::Numeral) } else { None }
            }
        } else { Some(TokenKind::Symbol) }
    }
}

sail_code! {
    /// Mechanical lexical/shallow-head failure, mapped to existing native
    /// `ErrorCode` values without granting command or solver authority.
    #[derive(Debug,Clone,Copy,PartialEq,Eq)]
    pub enum LexFailureKind {
        /// No initial token, including input containing only trivia.
        Empty,
        /// Opening command has no following head token.
        UnclosedList,
        /// Invalid word spelling or unsupported command head token kind.
        InvalidToken,
        /// No unpaired closing double quote.
        UnterminatedString,
        /// No closing vertical bar.
        UnterminatedQuotedSymbol,
    }
    /// Original byte offset and exact existing failure class.
    #[derive(Debug,Clone,Copy,PartialEq,Eq)]
    pub struct LexFailure {
        /// Mechanical error class.
        pub kind:LexFailureKind,
        /// Original byte offset used by the native diagnostic.
        pub offset:usize,
    }
    /// Actual original borrowed bytes, token cursor and sticky failure state.
    #[derive(Debug)]
    pub struct Lexer<'a> {
        src:&'a [u8],
        position:usize,
        failed:bool,
    }
    impl Token {
        sail_method! {
            /// Borrows exactly the original supplied span, retaining the
            /// public data API's empty default for arbitrary invalid fields.
            #[must_use]
            pub fn text<'a>(&self,src:&'a [u8]) -> (out:&'a [u8])
                requires (true)
                ensures (out@ == if self.start <= self.end && self.end <= src.len() {
                    src@.subrange(self.start as int,self.end as int) } else { Seq::<u8>::empty() })
            {
                if self.start <= self.end && self.end <= src.len() { &src[self.start..self.end] }
                else { &[] }
            }
        }
    }
}

sail_spec! {
    pub open spec fn lex_transition(bytes:Seq<u8>,position:int,failed:bool)
        -> (int,bool,Option<Result<Token,LexFailure>>) {
        if failed || position < 0 || position > bytes.len() { (position,failed,None) }
        else {
            let start=trivia_stop(bytes,position,false);
            if start >= bytes.len() { (start,false,None) }
            else if bytes[start] == b'(' || bytes[start] == b')' {
                (start+1,false,Some(Ok(Token { kind:if bytes[start] == b'(' {TokenKind::Open} else {TokenKind::Close},
                    start:start as usize,end:(start+1) as usize })))
            } else if bytes[start] == b'"' {
                match string_after(bytes,start+1) {
                    Some(end) => (end,false,Some(Ok(Token { kind:TokenKind::String,start:start as usize,end:end as usize }))),
                    None => (start,true,Some(Err(LexFailure { kind:LexFailureKind::UnterminatedString,offset:start as usize }))),
                }
            } else if bytes[start] == b'|' {
                match quoted_after(bytes,start+1) {
                    Some(end) => (end,false,Some(Ok(Token { kind:TokenKind::QuotedSymbol,start:start as usize,end:end as usize }))),
                    None => (start,true,Some(Err(LexFailure { kind:LexFailureKind::UnterminatedQuotedSymbol,offset:start as usize }))),
                }
            } else {
                let end=word_stop(bytes,start);
                match classified_word(bytes.subrange(start,end)) {
                    Some(kind) => (end,false,Some(Ok(Token { kind,start:start as usize,end:end as usize }))),
                    None => (end,true,Some(Err(LexFailure { kind:LexFailureKind::InvalidToken,offset:start as usize }))),
                }
            }
        }
    }
}

sail_code! {
    impl<'a> Lexer<'a> {
        sail_impl_spec! {
            pub closed spec fn input_view(&self) -> Seq<u8> { self.src@ }
            pub closed spec fn position_view(&self) -> int { self.position as int }
            pub closed spec fn failed_view(&self) -> bool { self.failed }
            pub closed spec fn well_formed(&self) -> bool { self.position <= self.src@.len() }
        }
        sail_method! {
            /// Starts before the first token of these exact borrowed bytes.
            pub fn new(src:&'a [u8]) -> (out:Self)
                requires (true)
                ensures (out.input_view() == src@,out.position_view() == 0,!out.failed_view(),out.well_formed())
            { Self { src,position:0,failed:false } }
        }
        sail_method! {
            /// Borrows exactly the lexer's original source for actual argument
            /// spans; callers cannot substitute a separately supplied buffer.
            pub fn source(&self) -> (out:&'a [u8])
                requires (self.well_formed()) ensures (out@ == self.input_view(),0<=self.position_view()<=out.len())
            {self.src}
        }
        sail_method! {
            /// Actual cursor used to bound the shared argument-token walk.
            pub fn position(&self) -> (out:usize)
                requires (self.well_formed()) ensures (out as int == self.position_view())
            {self.position}
        }
        sail_method! {
            /// Retains original input, cursor and sticky failure under Clone.
            #[must_use]
            pub fn duplicate(&self) -> (out:Self)
                requires (self.well_formed())
                ensures (out.well_formed(),out.input_view() == self.input_view(),
                    out.position_view() == self.position_view(),out.failed_view() == self.failed_view())
            { Self { src:self.src,position:self.position,failed:self.failed } }
        }
        sail_method! {
            /// The actual next-token transition and exact error/EOF policy.
            /// Exhausting these finite bytes does not assert physical EOF.
            pub fn next_token(&mut self) -> (out:Option<Result<Token,LexFailure>>)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),final(self).input_view() == old(self).input_view(),
                    (final(self).position_view(),final(self).failed_view(),out)
                        == lex_transition(old(self).input_view(),old(self).position_view(),old(self).failed_view()),
                    match out { Some(Ok(token)) => old(self).position_view() <= token.start < token.end <= old(self).input_view().len()
                        && final(self).position_view() == token.end
                        && crate::parser_grammar::trivia_after(old(self).input_view().subrange(old(self).position_view(),token.start as int),false) == Some(false)
                        && if token.kind is Symbol {
                            crate::parser_grammar::symbol_first(old(self).input_view()[token.start as int])
                            && (forall|i:int| token.start <= i < token.end ==> !crate::parser_grammar::word_delimiter(old(self).input_view()[i]))
                            && (token.end < old(self).input_view().len() ==> crate::parser_grammar::word_delimiter(old(self).input_view()[token.end as int]))
                        } else { true },_ => true })
            {
                if self.failed { return None; }
                self.position=skip_trivia(self.src,self.position);
                let start=self.position;
                if start == self.src.len() { return None; }
                let byte=self.src[start];
                let result=match byte {
                    b'(' | b')' => {
                        self.position += 1;
                        Ok(Token { kind:if byte == b'(' { TokenKind::Open } else { TokenKind::Close },start,end:self.position })
                    }
                    b'"' => match string_end(self.src,start) {
                        Some(end) => { self.position=end; Ok(Token { kind:TokenKind::String,start,end }) }
                        None => Err(LexFailure { kind:LexFailureKind::UnterminatedString,offset:start }),
                    },
                    b'|' => match quoted_symbol_end(self.src,start) {
                        Some(end) => { self.position=end; Ok(Token { kind:TokenKind::QuotedSymbol,start,end }) }
                        None => Err(LexFailure { kind:LexFailureKind::UnterminatedQuotedSymbol,offset:start }),
                    },
                    _ => {
                        let end=word_end(self.src,start);
                        self.position=end;
                        match classify_word(&self.src[start..end]) {
                            Some(kind) => Ok(Token { kind,start,end }),
                            None => Err(LexFailure { kind:LexFailureKind::InvalidToken,offset:start }),
                        }
                    }
                };
                if result.is_err() { self.failed=true; }
                Some(result)
            }
        }
    }
}

sail_spec! {
    pub open spec fn head_transition(bytes:Seq<u8>) -> (int,bool,Result<(usize,Seq<u8>),LexFailure>) {
        let first=lex_transition(bytes,0,false);
        match first.2 {
            None => (first.0,first.1,Err(LexFailure { kind:LexFailureKind::Empty,offset:0 })),
            Some(Err(error)) => (first.0,first.1,Err(error)),
            Some(Ok(open)) => if !(open.kind is Open) {
                (first.0,first.1,Err(LexFailure { kind:LexFailureKind::InvalidToken,offset:open.start }))
            } else {
                let next=lex_transition(bytes,first.0,first.1);
                match next.2 {
                    None => (next.0,next.1,Err(LexFailure { kind:LexFailureKind::UnclosedList,offset:open.start })),
                    Some(Err(error)) => (next.0,next.1,Err(error)),
                    Some(Ok(head)) => if !(head.kind is Symbol) {
                        (next.0,next.1,Err(LexFailure { kind:LexFailureKind::InvalidToken,offset:head.start }))
                    } else { (next.0,next.1,Ok((open.start,bytes.subrange(head.start as int,head.end as int)))) },
                }
            },
        }
    }
    pub open spec fn borrowed_head_view(out:Result<(usize,&[u8]),LexFailure>) -> Result<(usize,Seq<u8>),LexFailure> {
        match out { Ok((open,name)) => Ok((open,name@)),Err(error) => Err(error) }
    }
}

sail_code! {
    impl<'a> Lexer<'a> {
        sail_method! {
            /// Executes the actual shallow two-token command-head read. The
            /// returned name borrows this lexer's own original input rather
            /// than interpreting a token against a separately supplied slice.
            /// Full framing implies the existing checked raw head-span relation;
            /// command arity/body and solver meaning remain separate.
            ///
            /// # Errors
            /// Retains the original empty/unclosed, invalid-token and
            /// unterminated string/quoted-symbol diagnostic classes/offsets.
            pub fn command_head(&mut self) -> (out:Result<(usize,&'a [u8]),LexFailure>)
                requires (old(self).well_formed(),old(self).position_view() == 0,!old(self).failed_view())
                ensures (final(self).well_formed(),final(self).input_view() == old(self).input_view(),
                    (final(self).position_view(),final(self).failed_view(),borrowed_head_view(out))
                        == head_transition(old(self).input_view()),
                    match out { Ok((_open,name)) => name@.len()>0
                        && (crate::parser_grammar::single_list(old(self).input_view(),true) ==>
                            exists|start:int,end:int| crate::parser_grammar::head_span(old(self).input_view(),start,end)
                                && name@ == old(self).input_view().subrange(start,end)),Err(_) => true })
            {
                let open=match self.next_token() {
                    None => return Err(LexFailure { kind:LexFailureKind::Empty,offset:0 }),
                    Some(Err(error)) => return Err(error),
                    Some(Ok(open)) => open,
                };
                if !matches!(open.kind,TokenKind::Open) {
                    return Err(LexFailure { kind:LexFailureKind::InvalidToken,offset:open.start });
                }
                let head=match self.next_token() {
                    None => return Err(LexFailure { kind:LexFailureKind::UnclosedList,offset:open.start }),
                    Some(Err(error)) => return Err(error),
                    Some(Ok(head)) => head,
                };
                if !matches!(head.kind,TokenKind::Symbol) {
                    return Err(LexFailure { kind:LexFailureKind::InvalidToken,offset:head.start });
                }
                Ok((open.start,&self.src[head.start..head.end]))
            }
        }
    }
}

sail_verified! {
    /// Nonempty actual command with CRLF comment trivia returns its original
    /// borrowed head and leaves the closing token for the existing classifier.
    #[allow(clippy::byte_char_slices, reason = "Explicit original bytes keep the whole-root lexical/head witness concrete.")]
    pub fn lexer_head_constructive_control() -> (passed:bool)
        requires (true) ensures (passed)
    {
        let src=[b'(',b';',b'c',b'\r',b'\n',b'c',b'h',b'e',b'c',b'k',b'-',b's',b'a',b't',b')'];
        let expected=[b'c',b'h',b'e',b'c',b'k',b'-',b's',b'a',b't'];
        sail_proof! {
            assert(src@.subrange(5,14) =~= expected@);
            assert(trivia_stop(src@,1,false) == 5) by {
                reveal_with_fuel(trivia_stop,7);
            }
            assert(word_stop(src@,5) == 14) by {
                reveal_with_fuel(word_stop,11);
            }
            assert(lex_transition(src@,0,false)
                == (1int,false,Some(Ok(Token { kind:TokenKind::Open,start:0,end:1 }))));
            assert(lex_transition(src@,1,false)
                == (14int,false,Some(Ok(Token { kind:TokenKind::Symbol,start:5,end:14 }))));
            assert(head_transition(src@) == (14int,false,Ok((0usize,expected@)))) by {
                reveal(head_transition);
            }
        }
        let mut lexer=Lexer::new(&src);
        let Ok((open,name))=lexer.command_head() else { return false; };
        if open != 0 || !crate::inputs::byte_eq(name,&expected) { return false; }
        let Some(Ok(close))=lexer.next_token() else { return false; };
        matches!(close.kind,TokenKind::Close) && close.start == 14 && close.end == 15
            && lexer.next_token().is_none()
    }
}

sail_verified! {
    /// Actual doubled-quote, quoted-symbol and binary token spans stay on the
    /// original byte carrier, including punctuation hidden inside quotes.
    #[allow(clippy::byte_char_slices, reason = "Keep each literal boundary visible in the actual nonempty Stock witness.")]
    pub fn lexer_literal_spans_control() -> (passed:bool)
        requires (true) ensures (passed)
    {
        let src=[b'"',b'a',b'"',b'"',b'b',b'"',b' ',b'|',b'x',b')',b'|',b' ',b'#',b'b',b'0',b'1'];
        sail_proof! {
            assert(lex_transition(src@,0,false).2 == Some(Ok(Token { kind:TokenKind::String,start:0,end:6 }))) by {
                reveal_with_fuel(string_after,6);
            }
            assert(lex_transition(src@,6,false).2 == Some(Ok(Token { kind:TokenKind::QuotedSymbol,start:7,end:11 }))) by {
                reveal_with_fuel(trivia_stop,3);
                reveal_with_fuel(quoted_after,4);
            }
            assert(lex_transition(src@,11,false).2 == Some(Ok(Token { kind:TokenKind::Binary,start:12,end:16 }))) by {
                reveal_with_fuel(trivia_stop,3);
                reveal_with_fuel(word_stop,5);
            }
        }
        let mut lexer=Lexer::new(&src);
        let Some(Ok(string))=lexer.next_token() else { return false; };
        let Some(Ok(symbol))=lexer.next_token() else { return false; };
        let Some(Ok(binary))=lexer.next_token() else { return false; };
        matches!(string.kind,TokenKind::String) && string.start == 0 && string.end == 6
            && matches!(symbol.kind,TokenKind::QuotedSymbol) && symbol.start == 7 && symbol.end == 11
            && matches!(binary.kind,TokenKind::Binary) && binary.start == 12 && binary.end == 16
            && crate::inputs::byte_eq(string.text(&src),&src[..6])
            && crate::inputs::byte_eq(symbol.text(&src),&src[7..11])
            && crate::inputs::byte_eq(binary.text(&src),&src[12..]) && lexer.next_token().is_none()
    }
}

sail_verified! {
    /// A real invalid word mints the original error; neither the original
    /// iterator nor its actual copy can resume at a later valid word.
    #[allow(clippy::byte_char_slices, reason = "Keep concrete invalid-then-valid bytes for the sticky-error/copy control.")]
    pub fn lexer_sticky_error_control() -> (passed:bool)
        requires (true) ensures (passed)
    {
        let src=[b'#',b'q',b' ',b'f',b'o',b'o'];
        let mut lexer=Lexer::new(&src);
        let Some(Err(error))=lexer.next_token() else { return false; };
        let mut copied=lexer.duplicate();
        matches!(error.kind,LexFailureKind::InvalidToken) && error.offset == 0
            && lexer.next_token().is_none() && copied.next_token().is_none()
    }
}
