//! Actual shallow command data and response-category operations.
//! Raw lexical classification and native Session correspondence are refined here
//! without asserting the pinned solver's semantic interpretation.
use crate::query_lex::{LexFailure, LexFailureKind, Lexer, Token, TokenKind};
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
/// A classified top-level command. Byte slices borrow from the command text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command<'a> {
    /// The command name, for example `b"check-sat"`.
    pub name: &'a [u8],
    /// What the command does, as far as session modelling is concerned.
    pub kind: CommandKind<'a>,
}

/// The session-relevant meaning of a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandKind<'a> {
    /// `(push n)`, with `n` defaulting to 1.
    Push(u64),
    /// `(pop n)`, with `n` defaulting to 1.
    Pop(u64),
    /// `(assert …)`.
    Assert,
    /// `declare-fun`, `declare-const`, `declare-sort`, `declare-datatype(s)`.
    Declare,
    /// `define-fun`, `define-fun-rec`, `define-funs-rec`, `define-sort`,
    /// `define-const`.
    Define,
    /// `(set-option :keyword value)`. `value` is the exact text of the value.
    SetOption {
        /// The option keyword, including the colon.
        keyword: &'a [u8],
        /// The exact text of the value expression.
        value: &'a [u8],
    },
    /// `(set-info …)`.
    SetInfo,
    /// `(set-logic …)`.
    SetLogic,
    /// `(check-sat)`.
    CheckSat,
    /// `(check-sat-assuming …)`.
    CheckSatAssuming,
    /// Z3's `(check-sat-using tactic)`.
    CheckSatUsing,
    /// `(get-info :keyword)`.
    GetInfo {
        /// The requested keyword, including the colon.
        keyword: &'a [u8],
    },
    /// Any other `get-…` query (`get-model`, `get-value`, …).
    Get,
    /// `(echo "…")`.
    Echo,
    /// `(reset)`.
    Reset,
    /// `(reset-assertions)`.
    ResetAssertions,
    /// `(exit)`.
    Exit,
    /// A command this module does not model. Session models must treat it
    /// conservatively.
    Other,
}

/// Whether a command produces a response when `:print-success` is false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// No response unless the command fails.
    None,
    /// Exactly one response.
    One,
    /// Not known for this command.
    Unknown,
}

}

sail_spec! {
    /// Exact existing response category, not solver validity or success.
    pub open spec fn output_category(kind:CommandKind) -> Output {
        match kind {
            CommandKind::CheckSat | CommandKind::CheckSatAssuming | CommandKind::CheckSatUsing
                | CommandKind::GetInfo {..} | CommandKind::Get | CommandKind::Echo => Output::One,
            CommandKind::Other => Output::Unknown,
            _ => Output::None,
        }
    }
    pub open spec fn check_category(kind:CommandKind) -> bool {
        matches!(kind,CommandKind::CheckSat | CommandKind::CheckSatAssuming | CommandKind::CheckSatUsing)
    }
}

sail_code! {
    impl CommandKind<'_> {
        sail_method! {
            /// Whether this actual command variant is a satisfiability check.
            #[must_use]
            pub fn is_check(&self) -> (out:bool)
                requires (true) ensures (out == check_category(*self))
            {matches!(self,Self::CheckSat | Self::CheckSatAssuming | Self::CheckSatUsing)}
        }
        sail_method! {
            /// The actual existing response category with print-success false
            /// and successful command execution. Solver meaning stays separate.
            #[must_use]
            pub fn output(&self) -> (out:Output)
                requires (true) ensures (out == output_category(*self))
            {
                match self {
                    Self::CheckSat | Self::CheckSatAssuming | Self::CheckSatUsing
                        | Self::GetInfo {..} | Self::Get | Self::Echo => Output::One,
                    Self::Other => Output::Unknown,
                    _ => Output::None,
                }
            }
        }
    }
}

sail_spec! {
    pub open spec fn kind_fields(kind:CommandKind) -> (u8,u64,Seq<u8>,Seq<u8>) {
        match kind {
            CommandKind::Push(n)=>(0u8,n,seq![],seq![]),CommandKind::Pop(n)=>(1u8,n,seq![],seq![]),
            CommandKind::Assert=>(2u8,0u64,seq![],seq![]),CommandKind::Declare=>(3u8,0u64,seq![],seq![]),
            CommandKind::Define=>(4u8,0u64,seq![],seq![]),CommandKind::SetOption{keyword,value}=>(5u8,0u64,keyword@,value@),
            CommandKind::SetInfo=>(6u8,0u64,seq![],seq![]),CommandKind::SetLogic=>(7u8,0u64,seq![],seq![]),
            CommandKind::CheckSat=>(8u8,0u64,seq![],seq![]),CommandKind::CheckSatAssuming=>(9u8,0u64,seq![],seq![]),
            CommandKind::CheckSatUsing=>(10u8,0u64,seq![],seq![]),CommandKind::GetInfo{keyword}=>(11u8,0u64,keyword@,seq![]),
            CommandKind::Get=>(12u8,0u64,seq![],seq![]),CommandKind::Echo=>(13u8,0u64,seq![],seq![]),
            CommandKind::Reset=>(14u8,0u64,seq![],seq![]),CommandKind::ResetAssertions=>(15u8,0u64,seq![],seq![]),
            CommandKind::Exit=>(16u8,0u64,seq![],seq![]),CommandKind::Other=>(17u8,0u64,seq![],seq![]),
        }
    }
    pub open spec fn command_view(out:Result<Command,crate::query_lex::LexFailure>)
        -> Result<(Seq<u8>,(u8,u64,Seq<u8>,Seq<u8>)),crate::query_lex::LexFailure> {
        match out {Ok(c)=>Ok((c.name@,kind_fields(c.kind))),Err(e)=>Err(e)}
    }
    pub open spec fn head_tag(head:Seq<u8>) -> u8 {
        if crate::query_policy::named(head,"push"@) {0u8}
        else if crate::query_policy::named(head,"pop"@) {1u8}
        else if crate::query_policy::named(head,"assert"@) {2u8}
        else if crate::query_policy::named(head,"declare-fun"@) || crate::query_policy::named(head,"declare-const"@)
            || crate::query_policy::named(head,"declare-sort"@) || crate::query_policy::named(head,"declare-datatype"@)
            || crate::query_policy::named(head,"declare-datatypes"@) {3u8}
        else if crate::query_policy::named(head,"define-fun"@) || crate::query_policy::named(head,"define-fun-rec"@)
            || crate::query_policy::named(head,"define-funs-rec"@) || crate::query_policy::named(head,"define-sort"@)
            || crate::query_policy::named(head,"define-const"@) {4u8}
        else if crate::query_policy::named(head,"set-option"@) {5u8}
        else if crate::query_policy::named(head,"set-info"@) {6u8}
        else if crate::query_policy::named(head,"set-logic"@) {7u8}
        else if crate::query_policy::named(head,"check-sat"@) {8u8}
        else if crate::query_policy::named(head,"check-sat-assuming"@) {9u8}
        else if crate::query_policy::named(head,"check-sat-using"@) {10u8}
        else if crate::query_policy::named(head,"get-info"@) {11u8}
        else if crate::query_policy::prefixed(head,"get-"@) {12u8}
        else if crate::query_policy::named(head,"echo"@) {13u8}
        else if crate::query_policy::named(head,"reset"@) {14u8}
        else if crate::query_policy::named(head,"reset-assertions"@) {15u8}
        else if crate::query_policy::named(head,"exit"@) {16u8}
        else {17u8}
    }
    pub open spec fn decimal_from(bytes:Seq<u8>,at:int,value:u64) -> Option<u64>
        decreases bytes.len()-at,
    {
        if at<0 || at>bytes.len() {None}
        else if at==bytes.len() {Some(value)}
        else if bytes[at]<b'0' || bytes[at]>b'9' {None}
        else {
            let next=value as int*10+(bytes[at]-b'0') as int;
            if next>u64::MAX {None}else{decimal_from(bytes,at+1,next as u64)}
        }
    }
    pub open spec fn decimal_value(bytes:Seq<u8>) -> Option<u64> {
        let start=if bytes.len()>0 && bytes[0]==b'+' {1int}else{0int};
        if start==bytes.len() {None}else{decimal_from(bytes,start,0u64)}
    }
}

sail_verified! {
    /// Exact original head classification, including the existing get-prefix
    /// rule. Neither bytes nor keyword case are normalized.
    pub fn tag(head:&[u8]) -> (out:u8)
        requires (true) ensures (out==head_tag(head@),out<=17)
    {
        if crate::query_policy::word_eq(head,"push") {0}
        else if crate::query_policy::word_eq(head,"pop") {1}
        else if crate::query_policy::word_eq(head,"assert") {2}
        else if crate::query_policy::word_eq(head,"declare-fun") || crate::query_policy::word_eq(head,"declare-const")
            || crate::query_policy::word_eq(head,"declare-sort") || crate::query_policy::word_eq(head,"declare-datatype")
            || crate::query_policy::word_eq(head,"declare-datatypes") {3}
        else if crate::query_policy::word_eq(head,"define-fun") || crate::query_policy::word_eq(head,"define-fun-rec")
            || crate::query_policy::word_eq(head,"define-funs-rec") || crate::query_policy::word_eq(head,"define-sort")
            || crate::query_policy::word_eq(head,"define-const") {4}
        else if crate::query_policy::word_eq(head,"set-option") {5}
        else if crate::query_policy::word_eq(head,"set-info") {6}
        else if crate::query_policy::word_eq(head,"set-logic") {7}
        else if crate::query_policy::word_eq(head,"check-sat") {8}
        else if crate::query_policy::word_eq(head,"check-sat-assuming") {9}
        else if crate::query_policy::word_eq(head,"check-sat-using") {10}
        else if crate::query_policy::word_eq(head,"get-info") {11}
        else if crate::query_policy::word_prefix(head,"get-") {12}
        else if crate::query_policy::word_eq(head,"echo") {13}
        else if crate::query_policy::word_eq(head,"reset") {14}
        else if crate::query_policy::word_eq(head,"reset-assertions") {15}
        else if crate::query_policy::word_eq(head,"exit") {16}
        else {17}
    }
}

sail_verified! {
    /// Exact unsigned-decimal conversion, including Rust's optional leading
    /// plus and overflow refusal. No UTF-8 or spelling normalization is used.
    #[allow(clippy::manual_range_contains, reason = "Explicit ASCII bounds expose the exact digit arithmetic to the checked decimal recurrence.")]
    pub fn parse_u64(bytes:&[u8]) -> (out:Option<u64>)
        requires (true) ensures (out==decimal_value(bytes@))
    {
        let mut at=0usize;
        if !bytes.is_empty() && bytes[0]==b'+' {at+=1;}
        if at==bytes.len() {return None;}
        let mut value=0u64;
        sail_while! { (at<bytes.len())
            invariant (at<=bytes.len(),decimal_value(bytes@)==decimal_from(bytes@,at as int,value))
            decreases (bytes.len()-at)
            {
                let byte=bytes[at];
                if byte<b'0' || byte>b'9' {return None;}
                let digit=u64::from(byte-b'0');
                let ten=value.checked_mul(10)?;
                let next=ten.checked_add(digit)?;
                value=next;at+=1;
            }
        }
        Some(value)
    }
}

sail_spec! {
    pub open spec fn keyword_result(token:Option<Token>,open_start:usize) -> Result<Token,LexFailure> {
        match token {
            Some(t) => if t.kind is Keyword {Ok(t)} else {
                Err(LexFailure {kind:LexFailureKind::InvalidToken,offset:t.start})},
            None => Err(LexFailure {kind:LexFailureKind::UnclosedList,offset:open_start}),
        }
    }
    pub open spec fn span_view(out:Result<&[u8],LexFailure>) -> Result<Seq<u8>,LexFailure> {
        match out {Ok(bytes)=>Ok(bytes@),Err(e)=>Err(e)}
    }
    // The progress guard makes this total even on arbitrary mathematical
    // cursors. Actual successful next_token supplies strict progress/bounds.
    pub open spec fn value_scan(bytes:Seq<u8>,at:int,failed:bool,depth:int,start:Option<int>,after:usize)
        -> (int,bool,Result<Seq<u8>,LexFailure>)
        decreases bytes.len()-at,
    {
        let step=crate::query_lex::lex_transition(bytes,at,failed);
        match step.2 {
            None => (step.0,step.1,Err(LexFailure {kind:LexFailureKind::UnclosedList,offset:after})),
            Some(Err(e)) => (step.0,step.1,Err(e)),
            Some(Ok(t)) => {
                let begin=match start {Some(n)=>n,None=>t.start as int};
                let next_depth=match t.kind {TokenKind::Open=>depth+1,TokenKind::Close=>depth-1,_=>depth};
                if t.kind is Close && depth==0 {
                    (step.0,step.1,Err(LexFailure {kind:LexFailureKind::Empty,offset:t.start}))
                } else if next_depth==0 {
                    (step.0,step.1,Ok(bytes.subrange(begin,t.end as int)))
                } else if at<step.0<=bytes.len() {
                    value_scan(bytes,step.0,step.1,next_depth,Some(begin),after)
                } else {(step.0,step.1,Err(LexFailure {kind:LexFailureKind::UnclosedList,offset:after}))}
            }
        }
    }
}

sail_verified! {
    /// Exact keyword-token admission and original error offset. This does not
    /// assert the solver accepts the requested option/info keyword.
    ///
    /// # Errors
    /// Retains the original missing/non-keyword failure and byte offset.
    pub fn keyword_after(token:Option<Token>,open_start:usize) -> (out:Result<Token,LexFailure>)
        requires (true) ensures (out==keyword_result(token,open_start))
    {
        match token {
            Some(t) => if matches!(t.kind,TokenKind::Keyword) {Ok(t)} else {
                Err(LexFailure {kind:LexFailureKind::InvalidToken,offset:t.start})},
            None => Err(LexFailure {kind:LexFailureKind::UnclosedList,offset:open_start}),
        }
    }
}

sail_verified! {
    /// Walks the actual lexer and borrows precisely its first complete value
    /// expression. It retains the original shallow stop and error policy,
    /// including nested lists, comment gaps and doubled-quote strings.
    ///
    /// # Errors
    /// Returns the first lexical error, an unexpected closing delimiter, or
    /// the existing unclosed-value error at `after`.
    pub fn value_span<'a>(lexer:&mut Lexer<'a>,after:usize) -> (out:Result<&'a [u8],LexFailure>)
        requires (old(lexer).well_formed())
        ensures (final(lexer).well_formed(),final(lexer).input_view()==old(lexer).input_view(),
            (final(lexer).position_view(),final(lexer).failed_view(),span_view(out))
                ==value_scan(old(lexer).input_view(),old(lexer).position_view(),old(lexer).failed_view(),0,None,after))
    {
        let text=lexer.source();
        let mut depth=0usize;
        let mut start:Option<usize>=None;
        sail_while! { (lexer.position()<text.len())
            invariant (lexer.well_formed(),lexer.input_view()==text@,text@==old(lexer).input_view(),
                0<=lexer.position_view()<=text.len(),depth<=lexer.position_view(),
                match start {Some(begin)=>begin<=lexer.position_view(),None=>depth==0},
                value_scan(old(lexer).input_view(),old(lexer).position_view(),old(lexer).failed_view(),0,None,after)
                    ==value_scan(text@,lexer.position_view(),lexer.failed_view(),depth as int,
                        match start {Some(begin)=>Some(begin as int),None=>None},after))
            decreases (text.len()-lexer.position_view())
            {
                sail_proof! {reveal_with_fuel(value_scan,1);}
                let token=match lexer.next_token() {
                    Some(Ok(t))=>t,
                    Some(Err(e))=>return Err(e),
                    None=>return Err(LexFailure {kind:LexFailureKind::UnclosedList,offset:after}),
                };
                let begin=if let Some(begin)=start {begin} else {start=Some(token.start);token.start};
                match token.kind {
                    TokenKind::Open=>depth+=1,
                    TokenKind::Close=>{
                        if depth==0 {return Err(LexFailure {kind:LexFailureKind::Empty,offset:token.start});}
                        depth-=1;
                    }
                    _=>{}
                }
                if depth==0 {return Ok(&text[begin..token.end]);}
            }
        }
        sail_proof! {reveal_with_fuel(value_scan,1);}
        Err(LexFailure {kind:LexFailureKind::UnclosedList,offset:after})
    }
}

sail_spec! {
    pub open spec fn next_view(out:Option<Result<Token,LexFailure>>) -> Result<Option<Token>,LexFailure> {
        match out {None=>Ok(None),Some(Ok(t))=>Ok(Some(t)),Some(Err(e))=>Err(e)}
    }
    pub open spec fn kind_view(out:Result<CommandKind,LexFailure>)
        -> Result<(u8,u64,Seq<u8>,Seq<u8>),LexFailure> {
        match out {Ok(k)=>Ok(kind_fields(k)),Err(e)=>Err(e)}
    }
    pub open spec fn kind_transition(bytes:Seq<u8>,at:int,failed:bool,which:u8,open:usize)
        -> (int,bool,Result<(u8,u64,Seq<u8>,Seq<u8>),LexFailure>) {
        if which==0 || which==1 {
            let step=crate::query_lex::lex_transition(bytes,at,failed);
            let count=match next_view(step.2) {
                Err(e)=>Err(e),
                Ok(None)=>Err(LexFailure {kind:LexFailureKind::UnclosedList,offset:open}),
                Ok(Some(t))=>if t.kind is Close {Ok(1u64)} else if t.kind is Numeral {
                    match decimal_value(bytes.subrange(t.start as int,t.end as int)) {
                        Some(n)=>Ok(n),None=>Err(LexFailure {kind:LexFailureKind::InvalidToken,offset:t.start}),
                    }
                } else {Err(LexFailure {kind:LexFailureKind::InvalidToken,offset:t.start})},
            };
            (step.0,step.1,match count {Ok(n)=>Ok((which,n,seq![],seq![])),Err(e)=>Err(e)})
        } else if which==5 || which==11 {
            let step=crate::query_lex::lex_transition(bytes,at,failed);
            let keyword=match next_view(step.2) {Err(e)=>Err(e),Ok(t)=>keyword_result(t,open)};
            match keyword {
                Err(e)=>(step.0,step.1,Err(e)),
                Ok(t)=>{
                    let keyword=bytes.subrange(t.start as int,t.end as int);
                    if which==11 {(step.0,step.1,Ok((which,0u64,keyword,seq![])))}
                    else {
                        let value=value_scan(bytes,step.0,step.1,0,None,t.end);
                        (value.0,value.1,match value.2 {Ok(v)=>Ok((which,0u64,keyword,v)),Err(e)=>Err(e)})
                    }
                }
            }
        } else {(at,failed,Ok((which,0u64,seq![],seq![])))}
    }
    pub open spec fn classification(bytes:Seq<u8>) -> Result<(Seq<u8>,(u8,u64,Seq<u8>,Seq<u8>)),LexFailure> {
        let head=crate::query_lex::head_transition(bytes);
        match head.2 {
            Err(e)=>Err(e),
            Ok((open,name))=>match kind_transition(bytes,head.0,head.1,head_tag(name),open).2 {
                Err(e)=>Err(e),Ok(kind)=>Ok((name,kind)),
            },
        }
    }
}

sail_verified! {
    /// Transposes the actual lexical event without dropping a lexical error.
    ///
    /// # Errors
    /// Returns the actual next-token lexical error unchanged.
    pub fn next(lexer:&mut Lexer<'_>) -> (out:Result<Option<Token>,LexFailure>)
        requires (old(lexer).well_formed())
        ensures (final(lexer).well_formed(),final(lexer).input_view()==old(lexer).input_view(),
            (final(lexer).position_view(),final(lexer).failed_view(),out)
                == {let step=crate::query_lex::lex_transition(old(lexer).input_view(),old(lexer).position_view(),old(lexer).failed_view());
                    (step.0,step.1,next_view(step.2))},
            match out {Ok(Some(t))=>0<=old(lexer).position_view()<=t.start<t.end<=old(lexer).input_view().len() && final(lexer).position_view()==t.end,_=>true})
    {
        match lexer.next_token() {None=>Ok(None),Some(Ok(t))=>Ok(Some(t)),Some(Err(e))=>Err(e)}
    }
}

sail_verified! {
    /// Actual shallow argument reading for the already borrowed command head.
    /// Successful cases deliberately do not validate unused trailing tokens.
    ///
    /// # Errors
    /// Preserves the original lexical, argument-kind and numeric-overflow
    /// errors with their original command byte offsets.
    pub fn classify_kind<'a>(lexer:&mut Lexer<'a>,head:&[u8],open:usize) -> (out:Result<CommandKind<'a>,LexFailure>)
        requires (old(lexer).well_formed())
        ensures (final(lexer).well_formed(),final(lexer).input_view()==old(lexer).input_view(),
            (final(lexer).position_view(),final(lexer).failed_view(),kind_view(out))
                ==kind_transition(old(lexer).input_view(),old(lexer).position_view(),old(lexer).failed_view(),head_tag(head@),open))
    {
        let text=lexer.source();
        let which=tag(head);
        let kind=match which {
            0 | 1=>{
                let token=next(lexer)?;
                let count=match token {
                    Some(t)=>{
                        if matches!(t.kind,TokenKind::Numeral) {
                            match parse_u64(t.text(text)) {Some(n)=>n,None=>return Err(LexFailure {kind:LexFailureKind::InvalidToken,offset:t.start})}
                        } else if matches!(t.kind,TokenKind::Close) {1}
                        else {return Err(LexFailure {kind:LexFailureKind::InvalidToken,offset:t.start});}
                    }
                    None=>return Err(LexFailure {kind:LexFailureKind::UnclosedList,offset:open}),
                };
                if which==0 {CommandKind::Push(count)} else {CommandKind::Pop(count)}
            }
            2=>CommandKind::Assert,
            3=>CommandKind::Declare,
            4=>CommandKind::Define,
            5 | 11=>{
                let token=next(lexer)?;
                let keyword=keyword_after(token,open)?;
                if which==11 {CommandKind::GetInfo {keyword:keyword.text(text)}}
                else {
                    let value=value_span(lexer,keyword.end)?;
                    CommandKind::SetOption {keyword:keyword.text(text),value}
                }
            }
            6=>CommandKind::SetInfo,
            7=>CommandKind::SetLogic,
            8=>CommandKind::CheckSat,
            9=>CommandKind::CheckSatAssuming,
            10=>CommandKind::CheckSatUsing,
            12=>CommandKind::Get,
            13=>CommandKind::Echo,
            14=>CommandKind::Reset,
            15=>CommandKind::ResetAssertions,
            16=>CommandKind::Exit,
            _=>CommandKind::Other,
        };
        Ok(kind)
    }
}

sail_verified! {
    /// Actual native shallow classifier over the complete original buffer.
    /// The theorem is its exact lexical/mechanical correspondence, not SMT
    /// semantic validity or truthful production of a solver reply.
    ///
    /// # Errors
    /// Preserves exactly the existing lexical and shallow argument failures.
    pub fn classify(text:&[u8]) -> (out:Result<Command<'_>,LexFailure>)
        requires (true) ensures (command_view(out)==classification(text@))
    {
        let mut lexer=Lexer::new(text);
        let (open,name)=lexer.command_head()?;
        let kind=classify_kind(&mut lexer,name,open)?;
        Ok(Command {name,kind})
    }
}

sail_spec! {
    pub open spec fn response_expectation(command:Seq<u8>,print_success:Option<Seq<u8>>) -> Output {
        if match print_success {Some(value)=>crate::query_policy::named(value,"true"@),None=>false} {Output::One}
        else {
            match classification(command) {
                Err(_)=>Output::Unknown,
                Ok((_,kind))=>if kind.0==8 || kind.0==9 || kind.0==10 || kind.0==11 || kind.0==12 || kind.0==13 {
                    Output::One
                } else if kind.0==17 {Output::Unknown} else {Output::None},
            }
        }
    }
}

sail_verified! {
    /// Actual Session response decision over the exact command and its native
    /// print-success readback. A returned One is the existing mechanical
    /// prediction; it does not establish that Z3 accepted an earlier option.
    pub fn expects_output(command:&[u8],print_success:Option<&[u8]>) -> (out:Output)
        requires (true)
        ensures (out==response_expectation(command@,match print_success {Some(v)=>Some(v@),None=>None}))
    {
        let enabled=match print_success {Some(value)=>crate::query_policy::word_eq(value,"true"),None=>false};
        if enabled {return Output::One;}
        match classify(command) {Ok(c)=>c.kind.output(),Err(_)=>Output::Unknown}
    }
}

sail_spec! {
    proof fn push_trace(raw:Seq<u8>,name:Seq<u8>)
        requires raw==seq![b'(',b'p',b'u',b's',b'h',b')'],name==seq![b'p',b'u',b's',b'h'],
        ensures classification(raw)==Ok((name,(0u8,1u64,seq![],seq![]))),
    {
        reveal_strlit("push");
        assert(vstd::string::is_ascii("push"));
        vstd::string::is_ascii_spec_bytes("push");
        assert(name =~= vstd::utf8::encode_utf8("push"@));
        assert(crate::query_policy::named(name,"push"@)) by {reveal(crate::query_policy::named);}
        assert(crate::query_lex::lex_transition(raw,0,false)
            ==(1int,false,Some(Ok(Token {kind:TokenKind::Open,start:0,end:1}))));
        assert(crate::query_lex::word_stop(raw,1)==5) by {reveal_with_fuel(crate::query_lex::word_stop,6);}
        assert(crate::query_lex::lex_transition(raw,1,false)
            ==(5int,false,Some(Ok(Token {kind:TokenKind::Symbol,start:1,end:5}))));
        assert(raw.subrange(1,5) =~= name);
        assert(crate::query_lex::head_transition(raw)==(5int,false,Ok((0usize,name)))) by {
            reveal(crate::query_lex::head_transition);
        }
        assert(crate::query_lex::lex_transition(raw,5,false)
            ==(6int,false,Some(Ok(Token {kind:TokenKind::Close,start:5,end:6}))));
        assert(head_tag(name)==0);
        assert(kind_transition(raw,5,false,0,0).2==Ok((0u8,1u64,seq![],seq![])));
    }
}

sail_verified! {
    /// Nonempty real classifier path, including the original default count
    /// and silent-output decision. An always-refusing classifier cannot pass.
    #[allow(clippy::byte_char_slices, reason = "Concrete byte arrays expose the finite original-token trace to pinned Stock.")]
    pub fn classifier_constructive_control() -> (passed:bool)
        requires (true) ensures (passed)
    {
        let raw=[b'(',b'p',b'u',b's',b'h',b')'];
        let name=[b'p',b'u',b's',b'h'];
        sail_proof! {push_trace(raw@,name@);}
        let Ok(command)=classify(&raw) else {return false;};
        crate::inputs::byte_eq(command.name,&name) && matches!(command.kind,CommandKind::Push(1))
            && !command.kind.is_check() && matches!(command.kind.output(),Output::None)
    }
}

sail_verified! {
    /// A real nonempty compound argument is returned with its original outer
    /// delimiters. No argument-success flag or separately supplied span exists.
    #[allow(clippy::byte_char_slices, reason = "Concrete byte arrays keep the original nonempty argument walk visible to pinned Stock.")]
    pub fn value_constructive_control() -> (passed:bool)
        requires (true) ensures (passed)
    {
        let raw=[b'(',b'x',b')'];
        sail_proof! {
            assert(crate::query_lex::lex_transition(raw@,0,false)
                ==(1int,false,Some(Ok(Token {kind:TokenKind::Open,start:0,end:1}))));
            assert(crate::query_lex::word_stop(raw@,1)==2) by {reveal_with_fuel(crate::query_lex::word_stop,3);}
            assert(crate::query_lex::lex_transition(raw@,1,false)
                ==(2int,false,Some(Ok(Token {kind:TokenKind::Symbol,start:1,end:2}))));
            assert(crate::query_lex::lex_transition(raw@,2,false)
                ==(3int,false,Some(Ok(Token {kind:TokenKind::Close,start:2,end:3}))));
            assert(raw@.subrange(0,3) =~= raw@);
            assert(value_scan(raw@,2,false,1,Some(0int),0)==(3int,false,Ok(raw@))) by {reveal_with_fuel(value_scan,1);}
            assert(value_scan(raw@,1,false,1,Some(0int),0)==(3int,false,Ok(raw@))) by {reveal_with_fuel(value_scan,1);}
            assert(value_scan(raw@,0,false,0,None,0)==(3int,false,Ok(raw@))) by {reveal_with_fuel(value_scan,1);}
        }
        let mut lexer=Lexer::new(&raw);
        let Ok(value)=value_span(&mut lexer,0) else {return false;};
        crate::inputs::byte_eq(value,&raw) && lexer.position()==raw.len()
    }
}

sail_verified! {
    /// Concrete check-sat-using response/check categorization retained from the
    /// actual command type, plus exact true/false print-success readback.
    #[allow(clippy::byte_char_slices, reason = "Explicit original command/option bytes support the nonempty checked readback witness.")]
    pub fn expectation_constructive_control() -> (passed:bool)
        requires (true) ensures (passed)
    {
        let check=CommandKind::CheckSatUsing;
        if !check.is_check() || !matches!(check.output(),Output::One) {return false;}
        let raw=[b'(',b'p',b'u',b's',b'h',b')'];
        let yes=[b't',b'r',b'u',b'e'];
        sail_proof! {
            push_trace(raw@,seq![b'p',b'u',b's',b'h']);
            reveal_strlit("true");
            assert(vstd::string::is_ascii("true"));
            vstd::string::is_ascii_spec_bytes("true");
            assert(yes@ =~= vstd::utf8::encode_utf8("true"@));
            assert(crate::query_policy::named(yes@,"true"@)) by {reveal(crate::query_policy::named);}
        }
        matches!(expects_output(&raw,None),Output::None)
            && matches!(expects_output(&raw,Some(&yes)),Output::One)
    }
}
