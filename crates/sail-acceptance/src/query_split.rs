//! Actual owned incremental list bytes used by the SMT stream splitters.
//! This establishes structural framing, not solver interpretation or EOF.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    /// Exact existing input-delimiter/output-line policy.
    pub open spec fn atom_ends(byte: u8, input: bool) -> bool {
        if input { crate::query::word_delimiter(byte) } else { byte == b'\n' }
    }
    /// A trailing CR is removed only when an actual delimiter ends an atom.
    pub open spec fn terminated_atom(bytes: Seq<u8>) -> Seq<u8> {
        if bytes.len() > 0 && bytes.last() == b'\r' { bytes.drop_last() } else { bytes }
    }
}

sail_code! {
    /// Actual owned top-level atom bytes and original absolute start.
    #[derive(Debug)]
    pub struct AtomFrame { bytes: Vec<u8>, start: u64 }
    /// Consuming the actual next byte either appends or terminates the atom.
    #[derive(Debug)]
    pub enum AtomStep {
        /// Original atom remains open.
        Pending(AtomFrame),
        /// Exact terminated bytes, original start, and delimiter reprocessing.
        Complete(Vec<u8>,u64,bool),
    }
    /// Disjoint actions of the actual outer idle dispatch.
    #[derive(Debug)]
    pub enum OuterByte {
        /// One of the original four whitespace bytes.
        Whitespace,
        /// Input-side top-level comment opener.
        Comment,
        /// Root list opening parenthesis.
        List,
        /// Input-side unmatched closing parenthesis.
        Stray,
        /// Actual first byte of a top-level atom.
        Atom,
    }
    /// Actual absolute cursor, updated only by consumed byte counts.
    #[derive(Debug)]
    pub struct StreamPosition { offset: u64 }
    impl StreamPosition {
        sail_impl_spec! { pub closed spec fn view(&self) -> int { self.offset as int } }
        sail_method! {
            /// Begins before any stream byte is consumed.
            pub fn start() -> (out: Self)
                requires (true) ensures (out.view() == 0)
            { Self { offset:0 } }
        }
        sail_method! {
            /// Reads the cursor used by the actual next item start.
            pub fn offset(&self) -> (out: u64)
                requires (true) ensures (out as int == self.view())
            { self.offset }
        }
        sail_method! {
            /// Advances by the consumed prefix without wrapping.
            pub fn advance(&mut self, count: u64) -> (out: ())
                requires (old(self).view() + count <= u64::MAX)
                ensures (final(self).view() == old(self).view() + count)
            { self.offset += count; }
        }
        sail_method! {
            /// Preserves the native public splitter Clone operation.
            #[must_use]
            pub fn duplicate(&self) -> (out: Self)
                requires (true) ensures (out.view() == self.view())
            { Self { offset:self.offset } }
        }
    }
    impl AtomFrame {
        sail_impl_spec! {
            pub closed spec fn bytes_view(&self) -> Seq<u8> { self.bytes@ }
            pub closed spec fn start_view(&self) -> u64 { self.start }
        }
        sail_method! {
            /// Owns exactly the actual byte beginning the atom.
            #[allow(clippy::vec_init_then_push, reason = "Keep the original opening-byte append explicit for source-shared constructor controls.")]
            pub fn start(byte: u8, offset: u64) -> (out: Self)
                requires (true)
                ensures (out.bytes_view() == seq![byte],out.start_view() == offset)
            {
                let mut buffer=Vec::new(); buffer.push(byte);
                Self { bytes:buffer,start:offset }
            }
        }
        sail_method! {
            /// Copies original atom bytes and offset for native Clone.
            #[must_use]
            pub fn duplicate(&self) -> (out: Self)
                requires (true)
                ensures (out.bytes_view() == self.bytes_view(),out.start_view() == self.start_view())
            { Self { bytes:self.bytes.clone(),start:self.start } }
        }
        sail_method! {
            /// Applies the unchanged delimiter and CR-trim policy to the
            /// actual owned atom, preserving delimiter reprocessing.
            pub fn push(self, byte: u8, input: bool) -> (out: AtomStep)
                requires (self.bytes_view().len() < usize::MAX)
                ensures (match out {
                    AtomStep::Pending(next) => !atom_ends(byte,input)
                        && next.bytes_view() == self.bytes_view().push(byte)
                        && next.start_view() == self.start_view(),
                    AtomStep::Complete(bytes,start,reprocess) => atom_ends(byte,input)
                        && bytes@ == terminated_atom(self.bytes_view())
                        && start == self.start_view() && reprocess == (byte != b'\n'),
                })
            { append_atom(self.bytes,self.start,byte,input) }
        }
        sail_method! {
            /// Finish preserves a trailing CR, unlike delimiter termination.
            /// This operation does not establish physical EOF.
            pub fn finish(self) -> (out: (Vec<u8>,u64))
                requires (true)
                ensures (out.0@ == self.bytes_view(),out.1 == self.start_view())
            { (self.bytes,self.start) }
        }
    }
}

sail_verified! {
    /// Actual shared atom append/termination over the owned native bytes.
    fn append_atom(mut buffer: Vec<u8>, start: u64, byte: u8, input: bool) -> (out: AtomStep)
        requires (buffer@.len() < usize::MAX)
        ensures (match out {
            AtomStep::Pending(next) => !atom_ends(byte,input)
                && next.bytes_view() == buffer@.push(byte) && next.start_view() == start,
            AtomStep::Complete(done,offset,reprocess) => atom_ends(byte,input)
                && done@ == terminated_atom(buffer@) && offset == start
                && reprocess == (byte != b'\n'),
        })
    {
        let ends=if input { crate::query::delimiter(byte) } else { byte == b'\n' };
        if ends {
            if buffer.last() == Some(&b'\r') { buffer.pop(); }
            AtomStep::Complete(buffer,start,byte != b'\n')
        } else {
            buffer.push(byte);
            AtomStep::Pending(AtomFrame { bytes:buffer,start })
        }
    }
}

sail_verified! {
    /// Exact original idle dispatch, without SMT semantic classification.
    pub fn outer_byte(byte: u8, input: bool) -> (out: OuterByte)
        requires (true)
        ensures ((out is Whitespace) == (byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r'),
            (out is Comment) == (input && byte == b';'),
            (out is List) == (byte == b'('),
            (out is Stray) == (input && byte == b')'),
            (out is Atom) == (!(byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r')
                && !(input && (byte == b';' || byte == b')')) && byte != b'('))
    {
        match byte {
            b' ' | b'\t' | b'\n' | b'\r' => OuterByte::Whitespace,
            b';' if input => OuterByte::Comment,
            b'(' => OuterByte::List,
            b')' if input => OuterByte::Stray,
            _ => OuterByte::Atom,
        }
    }
}

sail_verified! {
    /// Exact CR-or-LF termination of a top-level input comment.
    pub fn comment_continues(byte: u8) -> (out: bool)
        requires (true) ensures (out == (byte != b'\n' && byte != b'\r'))
    { byte != b'\n' && byte != b'\r' }
}

sail_spec! {
    /// Every consumed byte is inside the same unfinished root list.
    pub open spec fn pending_list(bytes: Seq<u8>, depth: usize,
        lex: crate::query::FrameLex, comments: bool) -> bool {
        bytes.len() > 0 && bytes[0] == b'(' && 0 < depth <= bytes.len()
            && crate::query::frame_scan(bytes,comments) == (depth as int,lex)
            && forall|i: int| 1 <= i <= bytes.len() ==>
                crate::query::frame_scan(bytes.take(i),comments).0 > 0
    }

    /// Establish the future exact-byte scan before mutating the owned buffer.
    /// This is the existing framing grammar, not a second parser or an axiom.
    pub proof fn pending_append_facts(bytes: Seq<u8>, depth: usize,
        lex: crate::query::FrameLex, byte: u8, comments: bool)
        requires pending_list(bytes,depth,lex,comments),
        ensures crate::query::frame_scan(bytes.push(byte),comments)
            == (depth as int + if crate::query::structural(lex,byte) && byte == b'(' {1int}
                else if crate::query::structural(lex,byte) && byte == b')' {-1int} else {0int},
                crate::query::frame_lex(lex,byte,comments)),
            bytes.push(byte)[0] == b'(',
            forall|i: int| 1 <= i < bytes.push(byte).len() ==>
                crate::query::frame_scan(bytes.push(byte).take(i),comments).0 > 0,
    {
        assert(bytes.push(byte).drop_last() =~= bytes);
        assert(bytes.push(byte).last() == byte);
        reveal_with_fuel(crate::query::frame_scan,1);
        assert forall|i: int| 1 <= i < bytes.push(byte).len() implies
            crate::query::frame_scan(bytes.push(byte).take(i),comments).0 > 0 by {
            assert(i <= bytes.len());
            assert(bytes.push(byte).take(i) =~= bytes.take(i));
        }
    }
}

sail_code! {
    /// Owns the actual pending list; its constructors cannot import a scan flag.
    #[derive(Debug)]
    pub struct ListFrame {
        bytes: Vec<u8>,
        depth: usize,
        lex: crate::query::FrameLex,
        comments: bool,
    }

    /// A consumed actual byte either retains the same root or closes it once.
    #[derive(Debug)]
    pub enum ListStep {
        /// The owned original prefix is still incomplete.
        Pending(ListFrame),
        /// The exact original list bytes, including its first root close.
        Complete(Vec<u8>),
    }

    impl ListFrame {
        sail_impl_spec! {
            pub closed spec fn bytes_view(&self) -> Seq<u8> { self.bytes@ }
            pub closed spec fn comments_view(&self) -> bool { self.comments }
            pub closed spec fn well_formed(&self) -> bool {
                pending_list(self.bytes@,self.depth,self.lex,self.comments)
            }
        }

        sail_method! {
            /// Starts exactly at the actual opening parenthesis consumed by
            /// the native splitter. No bytes precede it in this owned frame.
            #[allow(clippy::vec_init_then_push, reason = "Keep the actual opening-byte push explicit for the checked constructor and its ordinary byte-substitution control.")]
            pub fn start(comments: bool) -> (out: Self)
                requires (true)
                ensures (out.well_formed(), out.bytes_view() == seq![b'('],
                    out.comments_view() == comments)
            {
                let mut bytes = Vec::new();
                bytes.push(b'(');
                sail_proof! { reveal_with_fuel(crate::query::frame_scan,2); }
                Self { bytes, depth:1, lex:crate::query::FrameLex::Plain, comments }
            }
        }

        sail_method! {
            /// Copies the same pending bytes/state for the native splitter's
            /// existing Clone API; it cannot mint a different framing history.
            #[must_use]
            pub fn duplicate(&self) -> (out: Self)
                requires (self.well_formed())
                ensures (out.well_formed(), out.bytes_view() == self.bytes_view(),
                    out.comments_view() == self.comments_view())
            {
                let bytes = self.bytes.clone();
                Self { bytes, depth:self.depth, lex:self.lex, comments:self.comments }
            }
        }

        sail_method! {
            /// Appends the actual next byte. The returned completion owns all
            /// original bytes through the first root close, without trimming.
            pub fn push(self, byte: u8) -> (out: ListStep)
                requires (self.well_formed(), self.bytes_view().len() < usize::MAX)
                ensures (match out {
                    ListStep::Pending(next) => next.well_formed()
                        && next.bytes_view() == self.bytes_view().push(byte)
                        && next.comments_view() == self.comments_view(),
                    ListStep::Complete(bytes) => bytes@ == self.bytes_view().push(byte)
                        && crate::query::single_list(bytes@,self.comments_view()),
                })
            {
                sail_proof! {
                    reveal(ListFrame::well_formed);
                    assert(pending_list(self.bytes@,self.depth,self.lex,self.comments));
                }
                append_list(self.bytes,self.depth,self.lex,byte,self.comments)
            }
        }

        sail_method! {
            /// Consumes exactly the next contiguous chunk prefix through a
            /// first root close, or all bytes when the root remains pending.
            /// The native push loop advances its cursor by this exact count.
            pub fn push_chunk(self, data: &[u8]) -> (out: (ListStep, usize))
                requires (self.well_formed(), self.bytes_view().len() + data@.len() <= usize::MAX)
                ensures (out.1 <= data.len(), data.len() > 0 ==> out.1 > 0,
                    match out.0 {
                        ListStep::Pending(next) => out.1 == data.len() && next.well_formed()
                            && next.bytes_view() == self.bytes_view() + data@.take(out.1 as int)
                            && next.comments_view() == self.comments_view(),
                        ListStep::Complete(bytes) => out.1 > 0
                            && bytes@ == self.bytes_view() + data@.take(out.1 as int)
                            && crate::query::single_list(bytes@,self.comments_view()),
                    })
            {
                let mut frame = self;
                let mut i = 0usize;
                sail_while! { (i < data.len())
                    invariant (i <= data.len(), frame.well_formed(),
                        frame.bytes_view() == self.bytes_view() + data@.take(i as int),
                        frame.comments_view() == self.comments_view(),
                        self.bytes_view().len() + data@.len() <= usize::MAX)
                    decreases (data.len() - i)
                    {
                        match frame.push(data[i]) {
                            ListStep::Complete(bytes) => {
                                sail_proof! {
                                    assert(data@.take(i as int).push(data@[i as int])
                                        =~= data@.take(i as int + 1));
                                }
                                return (ListStep::Complete(bytes),i+1);
                            }
                            ListStep::Pending(next) => { frame = next; }
                        }
                        sail_proof! {
                            assert(data@.take(i as int).push(data@[i as int])
                                =~= data@.take(i as int + 1));
                        }
                        i += 1;
                    }
                }
                (ListStep::Pending(frame),i)
            }
        }

        sail_method! {
            /// Ends this pending list without certifying completion. The
            /// native splitter returns these original bytes as a stray item.
            pub fn finish(self) -> (bytes: Vec<u8>)
                requires (self.well_formed())
                ensures (bytes@ == self.bytes_view(), !crate::query::single_list(bytes@,self.comments_view()))
            { self.bytes }
        }
    }
}

sail_verified! {
    /// The same actual byte append over its owned vector, separated from the
    /// containing record so its old byte-prefix facts remain explicit.
    fn append_list(mut bytes: Vec<u8>, depth: usize, lex: crate::query::FrameLex,
        byte: u8, comments: bool) -> (out: ListStep)
        requires (pending_list(bytes@,depth,lex,comments), bytes@.len() < usize::MAX)
        ensures (match out {
            ListStep::Pending(next) => next.well_formed()
                && next.bytes_view() == bytes@.push(byte) && next.comments_view() == comments,
            ListStep::Complete(complete) => complete@ == bytes@.push(byte)
                && crate::query::single_list(complete@,comments),
        })
    {
        sail_proof! {
            pending_append_facts(bytes@,depth,lex,byte,comments);
        }
        let next = crate::query::frame_step(depth,lex,byte,comments);
        bytes.push(byte);
        sail_proof! {
            reveal(ListFrame::well_formed);
            assert(crate::query::frame_scan(bytes@,comments) == (next.0 as int,next.1));
            assert(bytes@.last() == byte);
            assert forall|i: int| 1 <= i < bytes@.len() implies
                crate::query::frame_scan(bytes@.take(i),comments).0 > 0 by {
            }
        }
        if next.0 == 0 {
            ListStep::Complete(bytes)
        } else {
            sail_proof! {
                assert forall|i: int| 1 <= i <= bytes@.len() implies
                    crate::query::frame_scan(bytes@.take(i),comments).0 > 0 by {
                    if i == bytes@.len() { assert(bytes@.take(i) =~= bytes@); }
                }
            }
            ListStep::Pending(ListFrame { bytes, depth:next.0, lex:next.1, comments })
        }
    }
}

sail_verified! {
    /// A chunk cannot consume the second form after the original root closes.
    #[allow(clippy::byte_char_slices, reason = "Concrete byte arrays retain an explicit first-root endpoint witness for whole-query checking.")]
    pub fn chunk_endpoint_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let pending = ListFrame::start(true);
        let data = [b')', b'(', b')'];
        let (step,used) = pending.push_chunk(&data);
        sail_proof! {
            assert(crate::query::frame_scan(seq![b'(',b')'],true)
                == (0int,crate::query::FrameLex::Plain)) by {
                reveal_with_fuel(crate::query::frame_scan,3);
            }
            match &step {
                ListStep::Pending(frame) => {
                    reveal(ListFrame::well_formed);
                    assert(frame.bytes_view().take(2) =~= seq![b'(',b')']);
                    assert(crate::query::frame_scan(frame.bytes_view().take(2),true).0 > 0);
                }
                ListStep::Complete(bytes) => {
                    assert(bytes@.take(2) =~= seq![b'(',b')']);
                    if used > 1 {
                        assert(2 < bytes@.len());
                        assert(crate::query::frame_scan(bytes@.take(2),true).0 > 0);
                    }
                    assert(used == 1);
                }
            }
        }
        match step {
            ListStep::Complete(bytes) => used == 1 && bytes.len() == 2
                && bytes[0] == b'(' && bytes[1] == b')',
            ListStep::Pending(_) => false,
        }
    }
}

sail_verified! {
    /// A real nonempty command remains pending across the chunk boundary,
    /// survives an exact native Clone copy, then closes on its actual `)`.
    #[allow(clippy::byte_char_slices, reason = "Concrete byte arrays retain an explicit nonempty whole-root framing witness.")]
    pub fn command_chunks_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let pending = ListFrame::start(true);
        let prefix = [b'c',b'h',b'e',b'c',b'k',b'-',b's',b'a',b't'];
        let (step,used) = pending.push_chunk(&prefix);
        sail_proof! {
            assert forall|i: int| 0 <= i < prefix@.len() implies prefix@[i] != b')' by {}
            assert forall|i: int| 0 <= i < prefix@.len() implies crate::query_prefix::flat_byte(prefix@[i]) by {}
            if let ListStep::Complete(bytes) = &step {
                assert(used > 0);
                assert(bytes@.last() == prefix@[used as int - 1]);
                assert(bytes@.last() == b')');
            }
            crate::query_prefix::flat_list_frame(prefix@);
        }
        let ListStep::Pending(frame) = step else { return false; };
        let unfinished = frame.duplicate().finish();
        let terminal = frame.push(b')');
        sail_proof! {
            if let ListStep::Pending(next) = &terminal {
                reveal(ListFrame::well_formed);
                assert(next.bytes_view() =~= seq![b'(']+prefix@+seq![b')']);
                assert(crate::query::frame_scan(next.bytes_view(),true).0 > 0);
                assert(crate::query::frame_scan(next.bytes_view(),true).0 == 0);
            }
        }
        match terminal {
            ListStep::Complete(bytes) => used == 9 && unfinished.len() == 10
                && bytes.len() == 11 && bytes[0] == b'(' && bytes[1] == b'c'
                && bytes[9] == b't' && bytes[10] == b')',
            ListStep::Pending(_) => false,
        }
    }
}

sail_verified! {
    /// Nonempty real completion: the second actual byte closes the root.
    pub fn nonempty_list_control(comments: bool) -> (passed: bool)
        requires (true) ensures (passed)
    {
        let pending = ListFrame::start(comments);
        sail_proof! { reveal_with_fuel(crate::query::frame_scan,4); }
        match pending.push(b')') {
            ListStep::Complete(bytes) => bytes.len() == 2 && bytes[0] == b'(' && bytes[1] == b')',
            ListStep::Pending(_) => false,
        }
    }
}

sail_code! {
    /// What the original stream splitter emits, without interpretation authority.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ItemKind {
        /// First-root-complete parenthesised bytes.
        List,
        /// Original top-level atom, with delimiter-only trailing CR removal.
        Atom,
        /// Unmatched input closer or unfinished list at explicit finish.
        Stray,
    }
    /// Exact owned bytes and their original absolute stream start.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Item {
        /// Original splitter item kind.
        pub kind: ItemKind,
        /// Byte offset of the item's first byte in the whole stream.
        pub offset: u64,
        /// Exact original item bytes.
        pub bytes: Vec<u8>,
    }
    #[derive(Debug)]
    enum SplitState {
        Idle,
        Comment,
        Atom(AtomFrame),
        List(ListFrame,u64),
    }
    /// The actual outer stream machine, including pending owned bytes and the
    /// cursor used to mint every emitted item's absolute offset.
    #[derive(Debug)]
    pub struct StreamSplitter {
        input: bool,
        state: SplitState,
        position: StreamPosition,
    }
}

sail_spec! {
    // Specification-only tuples: 0 idle, 1 comment, 2 atom, 3 list;
    // owned bytes and original start. No erased type binding shadows Cargo.
    spec fn model(state:SplitState) -> (u8,Seq<u8>,int) {
        match state {
            SplitState::Idle => (0u8,Seq::<u8>::empty(),0int),
            SplitState::Comment => (1u8,Seq::<u8>::empty(),0int),
            SplitState::Atom(atom) => (2u8,atom.bytes_view(),atom.start_view() as int),
            SplitState::List(frame,start) => (3u8,frame.bytes_view(),start as int),
        }
    }
    pub open spec fn item_model(item:Item) -> (ItemKind,int,Seq<u8>) {
        (item.kind,item.offset as int,item.bytes@)
    }
    pub open spec fn item_models(items:Seq<Item>) -> Seq<(ItemKind,int,Seq<u8>)> {
        items.map(|i:int,item:Item| item_model(item))
    }
    pub open spec fn option_models(item:Option<Item>) -> Seq<(ItemKind,int,Seq<u8>)> {
        match item { Some(item) => seq![item_model(item)],None => seq![] }
    }
    pub open spec fn idle_model(byte:u8,input:bool,position:int) -> ((u8,Seq<u8>,int),Seq<(ItemKind,int,Seq<u8>)>) {
        if byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r' {
            ((0u8,Seq::<u8>::empty(),0int),seq![])
        } else if input && byte == b';' { ((1u8,Seq::<u8>::empty(),0int),seq![]) }
        else if byte == b'(' { ((3u8,seq![byte],position),seq![]) }
        else if input && byte == b')' {
            ((0u8,Seq::<u8>::empty(),0int),seq![(ItemKind::Stray,position,seq![byte])])
        } else { ((2u8,seq![byte],position),seq![]) }
    }
    /// Exact scalar transition used only as a specification of the actual
    /// executable outer operations. List grammar remains frame_scan.
    pub open spec fn model_step(state:(u8,Seq<u8>,int),input:bool,position:int,byte:u8) -> ((u8,Seq<u8>,int),Seq<(ItemKind,int,Seq<u8>)>) {
        if state.0 == 0 { idle_model(byte,input,position) }
        else if state.0 == 1 {
            (if byte != b'\n' && byte != b'\r' { (1u8,Seq::<u8>::empty(),0int) }
                else { (0u8,Seq::<u8>::empty(),0int) },seq![])
        } else if state.0 == 2 {
            if atom_ends(byte,input) {
                let after=if byte != b'\n' { idle_model(byte,input,position) }
                    else { ((0u8,Seq::<u8>::empty(),0int),seq![]) };
                (after.0,seq![(ItemKind::Atom,state.2,terminated_atom(state.1))]+after.1)
            } else { ((2u8,state.1.push(byte),state.2),seq![]) }
        } else if crate::query::frame_scan(state.1.push(byte),input).0 == 0 {
            ((0u8,Seq::<u8>::empty(),0int),seq![(ItemKind::List,state.2,state.1.push(byte))])
        } else { ((3u8,state.1.push(byte),state.2),seq![]) }
    }
    /// Complete ordered emission inventory, independent of runtime chunking.
    pub open spec fn model_scan(state:(u8,Seq<u8>,int),input:bool,position:int,data:Seq<u8>) -> ((u8,Seq<u8>,int),Seq<(ItemKind,int,Seq<u8>)>)
        decreases data.len(),
    {
        if data.len() == 0 { (state,seq![]) }
        else {
            let first=model_step(state,input,position,data[0]);
            let tail=model_scan(first.0,input,position+1,data.drop_first());
            (tail.0,first.1+tail.1)
        }
    }
    proof fn model_scan_join(state:(u8,Seq<u8>,int),input:bool,position:int,left:Seq<u8>,right:Seq<u8>)
        ensures model_scan(state,input,position,left+right) == {
            let first=model_scan(state,input,position,left);
            let second=model_scan(first.0,input,position+left.len(),right);
            (second.0,first.1+second.1)
        },
        decreases left.len(),
    {
        if left.len() > 0 {
            let first=model_step(state,input,position,left[0]);
            assert((left+right).drop_first() =~= left.drop_first()+right);
            model_scan_join(first.0,input,position+1,left.drop_first(),right);
        } else { assert(left+right =~= right); }
    }
    spec fn open_list_bytes(bytes:Seq<u8>,input:bool) -> bool {
        bytes.len()>0 && bytes[0] == b'(' && forall|i:int| 1 <= i <= bytes.len() ==>
            crate::query::frame_scan(bytes.take(i),input).0 > 0
    }
    /// A pending first root emits nothing for any actual contiguous prefix.
    proof fn list_model_pending(bytes:Seq<u8>,start:int,input:bool,position:int,data:Seq<u8>)
        requires open_list_bytes(bytes,input),open_list_bytes(bytes+data,input),
        ensures model_scan((3u8,bytes,start),input,position,data)
            == ((3u8,bytes+data,start),seq![]),
        decreases data.len(),
    {
        if data.len()>0 {
            assert((bytes+data).take(bytes.len() as int+1) =~= bytes.push(data[0]));
            assert(bytes.push(data[0])+data.drop_first() =~= bytes+data);
            assert forall|i:int| 1 <= i <= bytes.push(data[0]).len() implies
                crate::query::frame_scan(bytes.push(data[0]).take(i),input).0 > 0 by {
                assert(bytes.push(data[0]).take(i) =~= (bytes+data).take(i));
            }
            list_model_pending(bytes.push(data[0]),start,input,position+1,data.drop_first());
        } else { assert(bytes+data =~= bytes); }
    }
    /// A first-close chunk emits exactly one whole original list at its end.
    proof fn list_model_complete(bytes:Seq<u8>,start:int,input:bool,position:int,data:Seq<u8>)
        requires open_list_bytes(bytes,input),data.len()>0,crate::query::single_list(bytes+data,input),
        ensures model_scan((3u8,bytes,start),input,position,data)
            == ((0u8,Seq::<u8>::empty(),0int),seq![(ItemKind::List,start,bytes+data)]),
        decreases data.len(),
    {
        reveal_with_fuel(model_scan,2);
        assert(bytes.push(data[0])+data.drop_first() =~= bytes+data);
        if data.len()>1 {
            assert((bytes+data).take(bytes.len() as int+1) =~= bytes.push(data[0]));
            assert forall|i:int| 1 <= i <= bytes.push(data[0]).len() implies
                crate::query::frame_scan(bytes.push(data[0]).take(i),input).0 > 0 by {
                assert(bytes.push(data[0]).take(i) =~= (bytes+data).take(i));
            }
            assert(bytes.push(data[0]).take(bytes.push(data[0]).len() as int) =~= bytes.push(data[0]));
            assert(crate::query::frame_scan(bytes.push(data[0]),input).0 > 0);
            list_model_complete(bytes.push(data[0]),start,input,position+1,data.drop_first());
        } else {
            assert(data.drop_first().len() == 0);
            assert(bytes.push(data[0]) =~= bytes+data);
            assert(crate::query::frame_scan(bytes+data,input).0 == 0);
        }
    }
    proof fn model_scan_one(state:(u8,Seq<u8>,int),input:bool,position:int,byte:u8)
        ensures model_scan(state,input,position,seq![byte]) == model_step(state,input,position,byte),
    {
        reveal_with_fuel(model_scan,2);
        assert(seq![byte].drop_first() =~= Seq::<u8>::empty());
    }
    proof fn item_models_facts(items:Seq<Item>)
        ensures item_models(items).len() == items.len(),
            forall|item:Item| item_models(items.push(item)) == item_models(items).push(item_model(item)),
    {
        assert forall|item:Item| item_models(items.push(item)) == item_models(items).push(item_model(item)) by {
            assert(item_models(items.push(item)) =~= item_models(items).push(item_model(item)));
        }
    }
    /// Original suffixes extend by exactly the actually appended bytes.
    proof fn suffix_append(history:Seq<u8>,start:int,tail:Seq<u8>)
        requires 0 <= start <= history.len(),
        ensures (history+tail).subrange(start,(history.len()+tail.len()) as int)
            == history.subrange(start,history.len() as int)+tail,
    {
        assert((history+tail).subrange(start,(history.len()+tail.len()) as int)
            =~= history.subrange(start,history.len() as int)+tail);
    }
    /// Already-consumed original intervals are stable under a later append.
    proof fn prefix_slice_append(history:Seq<u8>,start:int,end:int,tail:Seq<u8>)
        requires 0 <= start <= end <= history.len(),
        ensures (history+tail).subrange(start,end) == history.subrange(start,end),
    { assert((history+tail).subrange(start,end) =~= history.subrange(start,end)); }
    proof fn original_item_append(item:Item,history:Seq<u8>,tail:Seq<u8>,input:bool)
        requires original_item(item,history,input),
        ensures original_item(item,history+tail,input),
    {
        prefix_slice_append(history,item.offset as int,item.offset as int+item.bytes@.len(),tail);
    }
    proof fn original_items_append(items:Seq<Item>,floor:int,history:Seq<u8>,tail:Seq<u8>,input:bool)
        requires 0 <= floor <= items.len(),
            forall|i:int| floor <= i < items.len() ==> original_item(items[i],history,input),
        ensures forall|i:int| floor <= i < items.len() ==> original_item(items[i],history+tail,input),
    {
        assert forall|i:int| floor <= i < items.len() implies original_item(items[i],history+tail,input) by {
            original_item_append(items[i],history,tail,input);
        }
    }
    /// The original delimiter-only CR trim removes one final suffix byte.
    proof fn suffix_drop_last(history:Seq<u8>,start:int)
        requires 0 <= start < history.len(),
        ensures history.subrange(start,history.len() as int).drop_last()
            == history.subrange(start,history.len() as int-1),
    {
        assert(history.subrange(start,history.len() as int).drop_last()
            =~= history.subrange(start,history.len() as int-1));
    }
    /// Pending bytes are the actual suffix of this same original raw stream.
    spec fn state_matches(state: SplitState, input: bool, position: int,
        history: Seq<u8>) -> bool {
        history.len() == position && match state {
            SplitState::Idle => true,
            SplitState::Comment => input,
            SplitState::Atom(atom) => atom.bytes_view().len() > 0
                && atom.start_view() as int + atom.bytes_view().len() == position
                && atom.bytes_view() == history.subrange(atom.start_view() as int,position),
            SplitState::List(frame,start) => frame.well_formed() && frame.comments_view() == input
                && start as int + frame.bytes_view().len() == position
                && frame.bytes_view() == history.subrange(start as int,position),
        }
    }
    /// No emitted byte or offset can refer to a different original stream.
    pub open spec fn original_item(item: Item, history: Seq<u8>, input: bool) -> bool {
        item.offset as int + item.bytes@.len() <= history.len()
            && item.bytes@ == history.subrange(item.offset as int,item.offset as int + item.bytes@.len())
            && (item.kind is List ==> crate::query::single_list(item.bytes@,input))
    }
    pub open spec fn optional_original(item: Option<Item>, history: Seq<u8>, input: bool) -> bool {
        match item { Some(item) => original_item(item,history,input), None => true }
    }
    spec fn idle_shape(state: SplitState, item: Option<Item>, byte: u8, input: bool, position: u64) -> bool {
        if byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r' {
            state is Idle && item.is_none()
        } else if input && byte == b';' { state is Comment && item.is_none() }
        else if byte == b'(' {
            (match state { SplitState::List(frame,start) => start == position
                && frame.bytes_view() == seq![byte], _ => false }) && item.is_none()
        } else if input && byte == b')' {
            state is Idle && match item { Some(item) => item.kind is Stray
                && item.offset == position && item.bytes@ == seq![byte], None => false }
        } else {
            (match state { SplitState::Atom(atom) => atom.start_view() == position
                && atom.bytes_view() == seq![byte], _ => false }) && item.is_none()
        }
    }
    /// Exact operational transition, including emitted inventory. Together
    /// with original_item this excludes both substituted bytes and omissions.
    spec fn step_shape(before: SplitState, after: SplitState, first: Option<Item>, second: Option<Item>,
        byte: u8, input: bool, position: u64) -> bool {
        match before {
            SplitState::Idle => idle_shape(after,first,byte,input,position) && second.is_none(),
            SplitState::Comment => first.is_none() && second.is_none()
                && if byte != b'\n' && byte != b'\r' { after is Comment } else { after is Idle },
            SplitState::Atom(atom) => if atom_ends(byte,input) {
                (match first { Some(item) => item.kind is Atom && item.offset == atom.start_view()
                    && item.bytes@ == terminated_atom(atom.bytes_view()), None => false })
                && if byte != b'\n' { idle_shape(after,second,byte,input,position) }
                    else { after is Idle && second.is_none() }
            } else {
                first.is_none() && second.is_none() && match after {
                    SplitState::Atom(next) => next.start_view() == atom.start_view()
                        && next.bytes_view() == atom.bytes_view().push(byte), _ => false }
            },
            SplitState::List(frame,start) => second.is_none()
                && if crate::query::frame_scan(frame.bytes_view().push(byte),input).0 == 0 {
                    after is Idle && match first { Some(item) => item.kind is List
                        && item.offset == start && item.bytes@ == frame.bytes_view().push(byte), None => false }
                } else {
                    first.is_none() && match after { SplitState::List(next,next_start) => next_start == start
                        && next.bytes_view() == frame.bytes_view().push(byte), _ => false }
                },
        }
    }
}

sail_code! {
    impl StreamSplitter {
        sail_impl_spec! {
            pub closed spec fn input_view(&self) -> bool { self.input }
            pub open spec fn has_pending_item_spec(&self) -> bool { self.transition_view().0 == 2 || self.transition_view().0 == 3 }
            pub closed spec fn fresh(&self) -> bool { self.state is Idle && self.position_view() == 0 }
            /// Logical projection of the actual owned state, with no
            /// runtime copy and no supplied parse-success flag.
            pub closed spec fn transition_view(&self) -> (u8,Seq<u8>,int) { model(self.state) }
            pub closed spec fn step_result(&self,after:Self,first:Option<Item>,second:Option<Item>,byte:u8) -> bool {
                step_shape(self.state,after.state,first,second,byte,self.input,self.position.offset)
            }
            pub closed spec fn finish_result(&self,out:Option<Item>) -> bool {
                match self.state {
                    SplitState::Atom(atom) => match out { Some(item) => item.kind is Atom
                        && item.offset == atom.start_view() && item.bytes@ == atom.bytes_view(),None => false },
                    SplitState::List(frame,start) => match out { Some(item) => item.kind is Stray
                        && item.offset == start && item.bytes@ == frame.bytes_view(),None => false },
                    _ => out.is_none(),
                }
            }
            pub closed spec fn position_view(&self) -> int { self.position.view() }
            pub closed spec fn matches(&self, history: Seq<u8>) -> bool {
                state_matches(self.state,self.input,self.position.view(),history)
            }
            pub closed spec fn well_formed(&self) -> bool {
                exists|history: Seq<u8>| self.matches(history)
            }
        }
        sail_method! {
            /// Starts the actual input/output splitter before any bytes.
            pub fn start(input: bool) -> (out: Self)
                requires (true)
                ensures (out.input_view() == input,out.matches(Seq::<u8>::empty()),out.well_formed(),out.fresh(),out.position_view() == 0,out.transition_view() == (0u8,Seq::<u8>::empty(),0int))
            {
                let out=Self { input,state:SplitState::Idle,position:StreamPosition::start() };
                sail_proof! {
                    assert(out.matches(Seq::<u8>::empty()));
                    assert(exists|history:Seq<u8>| out.matches(history));
                    reveal(StreamSplitter::well_formed);
                }
                out
            }
        }
        sail_method! {
            /// Copies the same actual pending state and original cursor.
            #[must_use]
            pub fn duplicate(&self) -> (out: Self)
                requires (self.well_formed())
                ensures (out.input_view() == self.input_view(),out.position_view() == self.position_view(),out.well_formed(),
                    out.transition_view() == self.transition_view(),
                    forall|history:Seq<u8>| self.matches(history) ==> out.matches(history))
            {
                let state=match &self.state {
                    SplitState::Idle => SplitState::Idle,
                    SplitState::Comment => SplitState::Comment,
                    SplitState::Atom(atom) => SplitState::Atom(atom.duplicate()),
                    SplitState::List(frame,start) => SplitState::List(frame.duplicate(),*start),
                };
                let out=Self { input:self.input,state,position:self.position.duplicate() };
                sail_proof! {
                    reveal(StreamSplitter::well_formed);
                    let ghost history=choose|history:Seq<u8>| self.matches(history);
                    assert(out.matches(history));
                    assert(exists|history:Seq<u8>| out.matches(history));
                    reveal(StreamSplitter::well_formed);
                }
                out
            }
        }
        sail_method! {
            /// Consumes one actual byte. Delimiter reprocessing does not advance
            /// the cursor twice, even when it also emits an unmatched closer.
            pub fn step(self, byte: u8) -> (out: (Self,Option<Item>,Option<Item>))
                requires (self.well_formed(),self.position_view() < u64::MAX,
                    self.position_view() < usize::MAX)
                ensures (out.0.input_view() == self.input_view(),out.0.position_view() == self.position_view()+1,
                    out.0.well_formed(),
                    self.step_result(out.0,out.1,out.2,byte),
                    out.0.transition_view() == model_step(self.transition_view(),self.input_view(),self.position_view(),byte).0,
                    option_models(out.1)+option_models(out.2)
                        == model_step(self.transition_view(),self.input_view(),self.position_view(),byte).1,
                    forall|history:Seq<u8>| self.matches(history) ==>
                        out.0.matches(history.push(byte))
                        && optional_original(out.1,history.push(byte),self.input_view())
                        && optional_original(out.2,history.push(byte),self.input_view()))
            {
                let position=self.position.offset();
                let (state,first,second)=split_step(self.state,self.input,position,byte);
                let mut next_position=self.position; next_position.advance(1);
                let out=(Self { input:self.input,state,position:next_position },first,second);
                sail_proof! {
                    reveal(StreamSplitter::well_formed);
                    let ghost history=choose|history:Seq<u8>| self.matches(history);
                    assert(out.0.matches(history.push(byte)));
                    assert(exists|history:Seq<u8>| out.0.matches(history));
                    reveal(StreamSplitter::well_formed);
                }
                out
            }
        }
        sail_method! {
            /// Executes the original chunk loop over the actual owned state.
            /// In-list chunks retain the first-close fast path; outer bytes
            /// advance once even when their delimiter is reconsidered.
            #[allow(clippy::too_many_lines, reason = "Keep the actual owned state, consumed cursor, ordered emissions and their checked loop facts in one coherent production transition.")]
            pub fn push_items(self, data: &[u8], items: Vec<Item>) -> (out: (Self,Vec<Item>))
                requires (self.well_formed(),self.position_view()+data@.len() <= usize::MAX,
                    items@.len()+data@.len()+(if self.position_view()>0 {1int} else {0int}) <= usize::MAX,
                    self.position_view()+data@.len() <= u64::MAX)
                ensures (out.0.input_view() == self.input_view(),out.0.well_formed(),
                    out.0.position_view() == self.position_view()+data@.len(),
                    out.1@.take(items@.len() as int) == items@,out.1@.len() >= items@.len(),
                    out.0.transition_view() == model_scan(self.transition_view(),self.input_view(),self.position_view(),data@).0,
                    item_models(out.1@) == item_models(items@)
                        + model_scan(self.transition_view(),self.input_view(),self.position_view(),data@).1,
                    forall|history:Seq<u8>| self.matches(history) ==>
                        out.0.matches(history+data@)
                        && forall|i:int| items@.len() <= i < out.1@.len() ==>
                            original_item(out.1@[i],history+data@,self.input_view()))
            {
                let mut appended=items;
                let mut current=self;
                let mut cursor=0usize;
                sail_while! { (cursor < data.len())
                    invariant (cursor <= data.len(),self.well_formed(),current.well_formed(),current.input_view() == self.input_view(),
                        current.position_view() == self.position_view()+cursor,
                        appended@.len() <= items@.len()+cursor+(if self.position_view()>0 {1int} else {0int}),
                        appended@.take(items@.len() as int) == items@,
                        appended@.len() >= items@.len(),
                        model(current.state) == model_scan(model(self.state),self.input,self.position_view(),data@.take(cursor as int)).0,
                        item_models(appended@) == item_models(items@)
                            + model_scan(model(self.state),self.input,self.position_view(),data@.take(cursor as int)).1,
                        items@.len()+data@.len()+(if self.position_view()>0 {1int} else {0int}) <= usize::MAX,
                        self.position_view()+data@.len() <= usize::MAX,
                        self.position_view()+data@.len() <= u64::MAX,
                        forall|history:Seq<u8>| self.matches(history) ==>
                            current.matches(history+data@.take(cursor as int))
                            && forall|i:int| items@.len() <= i < appended@.len() ==>
                                original_item(appended@[i],history+data@.take(cursor as int),self.input_view()))
                    decreases (data.len()-cursor)
                    {
                        let _chunk_start=cursor;
                        match current.state {
                            SplitState::List(frame,start) => {
                                let (step,consumed)=frame.push_chunk(&data[_chunk_start..]);
                                cursor += consumed;
                                sail_proof! {
                                    assert(data@.subrange(_chunk_start as int,data@.len() as int).take(consumed as int)
                                        =~= data@.subrange(_chunk_start as int,cursor as int));
                                    assert forall|history:Seq<u8>| self.matches(history) implies
                                        frame.bytes_view() == (history+data@.take(_chunk_start as int)).subrange(start as int,
                                            (history.len()+_chunk_start) as int) by {}
                                    assert forall|history:Seq<u8>| self.matches(history) implies
                                        (history+data@.take(cursor as int)).subrange(start as int,(history.len()+cursor) as int)
                                            == frame.bytes_view()+data@.subrange(_chunk_start as int,cursor as int) by {
                                        suffix_append(history+data@.take(_chunk_start as int),start as int,
                                            data@.subrange(_chunk_start as int,cursor as int));
                                        assert((history+data@.take(_chunk_start as int))+data@.subrange(_chunk_start as int,cursor as int)
                                            =~= history+data@.take(cursor as int));
                                    }
                                }
                                sail_proof! {
                                    assert forall|history:Seq<u8>| self.matches(history) implies
                                        forall|i:int| items@.len() <= i < appended@.len() ==>
                                            original_item(appended@[i],history+data@.take(cursor as int),self.input) by {
                                        assert((history+data@.take(_chunk_start as int))
                                            +data@.subrange(_chunk_start as int,cursor as int)
                                            =~= history+data@.take(cursor as int));
                                        original_items_append(appended@,items@.len() as int,history+data@.take(_chunk_start as int),
                                            data@.subrange(_chunk_start as int,cursor as int),self.input);
                                    }
                                }
                                let mut position=current.position; position.advance(consumed as u64);
                                let state=match step {
                                    ListStep::Pending(next) => {
                                        sail_proof! {
                                            assert(frame.comments_view() == self.input);
                                            reveal(ListFrame::well_formed);
                                            assert(next.bytes_view() == frame.bytes_view()+data@.subrange(_chunk_start as int,cursor as int));
                                            list_model_pending(frame.bytes_view(),start as int,self.input,
                                                current.position.view(),data@.subrange(_chunk_start as int,cursor as int));
                                        }
                                        SplitState::List(next,start)
                                    }
                                    ListStep::Complete(bytes) => {
                                        sail_proof! {
                                            assert(frame.comments_view() == self.input);
                                            assert(bytes@ == frame.bytes_view()+data@.subrange(_chunk_start as int,cursor as int));
                                            list_model_complete(frame.bytes_view(),start as int,self.input,
                                                current.position.view(),data@.subrange(_chunk_start as int,cursor as int));
                                        }
                                        sail_proof! { item_models_facts(appended@); }
                                        appended.push(Item { kind:ItemKind::List,offset:start,bytes });
                                        SplitState::Idle
                                    }
                                };
                                current=Self { input:current.input,state,position };
                                sail_proof! {
                                    assert(data@.take(cursor as int) =~= data@.take(_chunk_start as int)
                                        +data@.subrange(_chunk_start as int,cursor as int));
                                    model_scan_join(model(self.state),self.input,self.position_view(),
                                        data@.take(_chunk_start as int),data@.subrange(_chunk_start as int,cursor as int));
                                }
                            }
                            state => {
                                let (next,first,second)=Self { input:current.input,state,position:current.position }.step(data[cursor]);
                                sail_proof! {
                                    assert forall|history:Seq<u8>| self.matches(history) implies
                                        forall|i:int| items@.len() <= i < appended@.len() ==>
                                            original_item(appended@[i],history+data@.take(_chunk_start as int+1),self.input) by {
                                        assert((history+data@.take(_chunk_start as int))+seq![data@[_chunk_start as int]]
                                            =~= history+data@.take(_chunk_start as int+1));
                                        original_items_append(appended@,items@.len() as int,history+data@.take(_chunk_start as int),
                                            seq![data@[_chunk_start as int]],self.input);
                                    }
                                    item_models_facts(appended@);
                                }
                                if let Some(item)=first { appended.push(item); }
                                sail_proof! { item_models_facts(appended@); }
                                if let Some(item)=second { appended.push(item); }
                                current=next;
                                cursor += 1;
                                sail_proof! {
                                    assert(data@.take(cursor as int) =~= data@.take(_chunk_start as int)+seq![data@[_chunk_start as int]]);
                                    model_scan_one(model(state),self.input,self.position_view()+_chunk_start,data@[_chunk_start as int]);
                                    model_scan_join(model(self.state),self.input,self.position_view(),
                                        data@.take(_chunk_start as int),seq![data@[_chunk_start as int]]);
                                }
                            }
                        }
                        sail_proof! {
                            reveal(StreamSplitter::well_formed);
                            let ghost history=choose|history:Seq<u8>| self.matches(history);
                            assert(current.matches(history+data@.take(cursor as int)));
                            assert(exists|history:Seq<u8>| current.matches(history));
                        }
                    }
                }
                sail_proof! { assert(data@.take(cursor as int) =~= data@); }
                (current,appended)
            }
        }
        sail_method! {
            /// Read-only counterpart of the original finish().is_some() test,
            /// retaining the owned parser for subsequent transcript operations.
            pub fn has_pending_item(&self) -> (out: bool)
                requires (true)
                ensures (out == self.has_pending_item_spec())
            { matches!(&self.state,SplitState::Atom(_) | SplitState::List(_,_)) }
        }
        sail_method! {
            /// Ends the actual pending state. This preserves the original
            /// unfinished atom/list policy and never asserts physical EOF.
            pub fn finish(self) -> (out: Option<Item>)
                requires (self.well_formed())
                ensures (out.is_some() == self.has_pending_item_spec(),
                    self.finish_result(out),
                    forall|history:Seq<u8>| self.matches(history) ==>
                    optional_original(out,history,self.input_view()))
            {
                match self.state {
                    SplitState::Atom(atom) => {
                        let (bytes,offset)=atom.finish();
                        Some(Item { kind:ItemKind::Atom,offset,bytes })
                    }
                    SplitState::List(frame,offset) => Some(Item { kind:ItemKind::Stray,offset,bytes:frame.finish() }),
                    _ => None,
                }
            }
        }
    }
}

sail_verified! {
    /// Nonempty atom termination, same-byte unmatched-closer reprocessing and
    /// absolute offsets arise from the actual owned transition, not flags.
    pub fn outer_atom_offsets_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let initial=StreamSplitter::start(true);
        let (first,none,also_none)=initial.step(b'a');
        if none.is_some() || also_none.is_some() { return false; }
        let (last,atom,stray)=first.step(b')');
        let Some(atom)=atom else { return false; };
        let Some(stray)=stray else { return false; };
        matches!(atom.kind,ItemKind::Atom) && atom.offset == 0 && atom.bytes.len() == 1 && atom.bytes[0] == b'a'
            && matches!(stray.kind,ItemKind::Stray) && stray.offset == 1 && stray.bytes.len() == 1 && stray.bytes[0] == b')'
            && !last.has_pending_item() && last.finish().is_none()
    }
}

sail_verified! {
    /// Delimiter CR trim and explicit unfinished-atom finish remain distinct.
    pub fn outer_crlf_finish_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let (first,_,_)=StreamSplitter::start(false).step(b'x');
        let (second,_,_)=first.step(b'\r');
        let copy=second.duplicate();
        let Some(trailing)=copy.finish() else { return false; };
        let (last,line,extra)=second.step(b'\n');
        let Some(line)=line else { return false; };
        trailing.bytes.len() == 2 && trailing.bytes[0] == b'x' && trailing.bytes[1] == b'\r'
            && line.bytes.len() == 1 && line.bytes[0] == b'x'
            && trailing.offset == 0 && line.offset == 0
            && extra.is_none() && !last.has_pending_item()
    }
}

sail_verified! {
    /// Actual top-level comment, CRLF, chunk boundary and list fast path.
    #[allow(clippy::byte_char_slices, reason = "Keep explicit nonempty chunk bytes and lexical boundaries available to Stock without opaque byte-string literals.")]
    pub fn outer_chunks_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let leading=[b';',b'c',b'\r'];
        let opening=[b'\n',b'('];
        let following=[b')',b'a',b')'];
        sail_proof! {
            assert(model_scan((0u8,Seq::<u8>::empty(),0int),true,0,leading@)
                == ((0u8,Seq::<u8>::empty(),0int),seq![])) by { reveal(model_step); reveal(idle_model); reveal(atom_ends); reveal(terminated_atom);
                reveal(crate::query::word_delimiter); reveal_with_fuel(model_scan,4); }
            assert(model_scan((0u8,Seq::<u8>::empty(),0int),true,3,opening@)
                == ((3u8,seq![b'('],4int),seq![])) by { reveal(model_step); reveal(idle_model); reveal(atom_ends); reveal(terminated_atom);
                reveal(crate::query::word_delimiter); reveal_with_fuel(model_scan,3); }
            assert(crate::query::frame_scan(seq![b'(',b')'],true)
                == (0int,crate::query::FrameLex::Plain)) by {
                reveal_with_fuel(crate::query::frame_scan,3);
            }
            assert(seq![b'('].push(b')') =~= seq![b'(',b')']);
            assert(model_step((3u8,seq![b'('],4int),true,5,b')')
                == ((0u8,Seq::<u8>::empty(),0int),seq![(ItemKind::List,4int,seq![b'(',b')'])])) by { reveal(model_step); }
            assert(model_step((0u8,Seq::<u8>::empty(),0int),true,6,b'a')
                == ((2u8,seq![b'a'],6int),seq![])) by { reveal(model_step); reveal(idle_model); }
            assert(model_step((2u8,seq![b'a'],6int),true,7,b')')
                == ((0u8,Seq::<u8>::empty(),0int),seq![(ItemKind::Atom,6int,seq![b'a']),
                    (ItemKind::Stray,7int,seq![b')'])])) by { reveal(model_step); reveal(idle_model); reveal(atom_ends); reveal(terminated_atom); reveal(crate::query::word_delimiter); }
            assert(model_scan((3u8,seq![b'('],4int),true,5,following@)
                == ((0u8,Seq::<u8>::empty(),0int),seq![(ItemKind::List,4int,seq![b'(',b')']),
                    (ItemKind::Atom,6int,seq![b'a']),(ItemKind::Stray,7int,seq![b')'])])) by { reveal(model_step); reveal(idle_model); reveal(atom_ends); reveal(terminated_atom);
                reveal(crate::query::word_delimiter); reveal_with_fuel(model_scan,4); }
        }
        let (first,items)=StreamSplitter::start(true).push_items(&leading,Vec::new());
        sail_proof! { item_models_facts(items@); }
        if !items.is_empty() { return false; }
        let (second,items)=first.push_items(&opening,items);
        sail_proof! { item_models_facts(items@); }
        if !items.is_empty() { return false; }
        let (last,items)=second.push_items(&following,items);
        sail_proof! { item_models_facts(items@); }
        if items.len() != 3 { return false; }
        matches!(items[0].kind,ItemKind::List) && items[0].offset == 4 && items[0].bytes.len() == 2 && items[0].bytes[0] == b'(' && items[0].bytes[1] == b')'
            && matches!(items[1].kind,ItemKind::Atom) && items[1].offset == 6 && items[1].bytes.len() == 1 && items[1].bytes[0] == b'a'
            && matches!(items[2].kind,ItemKind::Stray) && items[2].offset == 7 && items[2].bytes.len() == 1 && items[2].bytes[0] == b')'
            && last.finish().is_none()
    }
}

sail_verified! {
    /// Actual nonrecursive idle transition at the original unadvanced offset.
    fn begin_outer(byte: u8, input: bool, position: u64) -> (out: (SplitState,Option<Item>))
        requires (position < u64::MAX,position < usize::MAX)
        ensures (idle_shape(out.0,out.1,byte,input,position),
            forall|history:Seq<u8>| history.len() == position ==>
            state_matches(out.0,input,position as int+1,history.push(byte))
            && optional_original(out.1,history.push(byte),input))
    {
        let out=match outer_byte(byte,input) {
            OuterByte::Whitespace => (SplitState::Idle,None),
            OuterByte::Comment => (SplitState::Comment,None),
            OuterByte::List => (SplitState::List(ListFrame::start(input),position),None),
            OuterByte::Stray => {
                let mut buffer=Vec::new(); buffer.push(byte);
                (SplitState::Idle,Some(Item { kind:ItemKind::Stray,offset:position,bytes:buffer }))
            }
            OuterByte::Atom => (SplitState::Atom(AtomFrame::start(byte,position)),None),
        };
        sail_proof! {
            assert forall|history:Seq<u8>| history.len() == position implies
                state_matches(out.0,input,position as int+1,history.push(byte))
                && optional_original(out.1,history.push(byte),input) by {
                assert(history.push(byte) =~= history+seq![byte]);
                suffix_append(history,position as int,seq![byte]);
                assert(history.subrange(position as int,position as int) =~= Seq::<u8>::empty());
            }
        }
        out
    }
}

sail_verified! {
    /// Actual outer transition over the owned state. A delimiter may emit the
    /// pending atom plus one unmatched closer, both in original order.
    fn split_step(state: SplitState, input: bool, position: u64, byte: u8)
        -> (out: (SplitState,Option<Item>,Option<Item>))
        requires (position < u64::MAX,position < usize::MAX,
            exists|history:Seq<u8>| state_matches(state,input,position as int,history))
        ensures (step_shape(state,out.0,out.1,out.2,byte,input,position),
            forall|history:Seq<u8>| state_matches(state,input,position as int,history) ==>
            state_matches(out.0,input,position as int+1,history.push(byte))
            && optional_original(out.1,history.push(byte),input)
            && optional_original(out.2,history.push(byte),input))
    {
        let out=match state {
            SplitState::Idle => {
                let (next,item)=begin_outer(byte,input,position);
                (next,item,None)
            }
            SplitState::Comment => {
                (if comment_continues(byte) { SplitState::Comment } else { SplitState::Idle },None,None)
            }
            SplitState::Atom(atom) => match atom.push(byte,input) {
                AtomStep::Pending(next) => (SplitState::Atom(next),None,None),
                AtomStep::Complete(bytes,offset,reprocess) => {
                    let first=Some(Item { kind:ItemKind::Atom,offset,bytes });
                    if reprocess {
                        let (next,item)=begin_outer(byte,input,position);
                        (next,first,item)
                    } else { (SplitState::Idle,first,None) }
                }
            },
            SplitState::List(frame,offset) => match frame.push(byte) {
                ListStep::Pending(next) => (SplitState::List(next,offset),None,None),
                ListStep::Complete(bytes) => (SplitState::Idle,Some(Item { kind:ItemKind::List,offset,bytes }),None),
            },
        };
        sail_proof! {
            assert(step_shape(state,out.0,out.1,out.2,byte,input,position));
            assert forall|history:Seq<u8>| state_matches(state,input,position as int,history) implies
                state_matches(out.0,input,position as int+1,history.push(byte))
                && optional_original(out.1,history.push(byte),input)
                && optional_original(out.2,history.push(byte),input) by {
                assert(history.push(byte) =~= history+seq![byte]);
                if model(state).0 == 0 {
                    assert(state_matches(out.0,input,position as int+1,history.push(byte)));
                    assert(optional_original(out.1,history.push(byte),input));
                } else if model(state).0 == 1 {
                    assert(state_matches(out.0,input,position as int+1,history.push(byte)));
                } else if model(state).0 == 2 || model(state).0 == 3 {
                    let start=model(state).2;
                    assert(0 <= start < history.len());
                    suffix_append(history,start,seq![byte]);
                    assert(model(state).1.push(byte) =~= model(state).1+seq![byte]);
                    prefix_slice_append(history,start,history.len() as int,seq![byte]);
                    if model(state).0 == 2 && model(state).1.last() == b'\r' {
                        suffix_drop_last(history,start);
                        prefix_slice_append(history,start,history.len() as int-1,seq![byte]);
                    }
                    if model(state).0 == 2 {
                        if atom_ends(byte,input) {
                            assert(optional_original(out.1,history.push(byte),input));
                            assert(optional_original(out.2,history.push(byte),input));
                            assert(state_matches(out.0,input,position as int+1,history.push(byte)));
                        } else {
                            assert(state_matches(out.0,input,position as int+1,history.push(byte)));
                        }
                    } else {
                        assert(state_matches(out.0,input,position as int+1,history.push(byte)));
                        assert(optional_original(out.1,history.push(byte),input));
                    }
                }
            }
        }
        out
    }
}

sail_verified! {
    /// A real copied input splitter retains Idle rather than becoming a
    /// comment parser: the original following atom and closer both emit.
    pub fn outer_clone_initial_control() -> (passed:bool)
        requires (true) ensures (passed)
    {
        let copied=StreamSplitter::start(true).duplicate();
        let (first,none,also_none)=copied.step(b'a');
        if none.is_some() || also_none.is_some() { return false; }
        let (last,atom,stray)=first.step(b')');
        let Some(atom)=atom else { return false; };
        let Some(stray)=stray else { return false; };
        matches!(atom.kind,ItemKind::Atom) && atom.offset == 0
            && atom.bytes.len() == 1 && atom.bytes[0] == b'a'
            && matches!(stray.kind,ItemKind::Stray) && stray.offset == 1
            && stray.bytes.len() == 1 && stray.bytes[0] == b')'
            && last.finish().is_none()
    }
}
