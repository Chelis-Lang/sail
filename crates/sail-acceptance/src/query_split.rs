//! Actual owned incremental list bytes used by the SMT stream splitters.
//! This establishes structural framing, not solver interpretation or EOF.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    /// Every consumed byte is inside the same unfinished root list.
    pub open spec fn pending_list(bytes: Seq<u8>, depth: usize,
        lex: crate::query::FrameLex, comments: bool) -> bool {
        bytes.len() > 0 && bytes[0] == b'(' && 0 < depth <= bytes.len()
            && crate::query::frame_scan(bytes,comments) == (depth as int,lex)
            && forall|i: int| 1 <= i <= bytes.len() ==>
                crate::query::frame_scan(bytes.take(i),comments).0 > 0
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
            pub fn push(mut self, byte: u8) -> (out: ListStep)
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
                    assert(crate::query::frame_scan(self.bytes@,self.comments)
                        == (self.depth as int,self.lex));
                }
                let next = crate::query::frame_step(self.depth,self.lex,byte,self.comments);
                self.bytes.push(byte);
                sail_proof! {
                    assert(crate::query::frame_scan(self.bytes@.drop_last(),self.comments)
                        == (self.depth as int,self.lex));
                    assert(self.bytes@.last() == byte);
                    reveal_with_fuel(crate::query::frame_scan,1);
                    assert forall|i: int| 1 <= i < self.bytes@.len() implies
                        crate::query::frame_scan(self.bytes@.take(i),self.comments).0 > 0 by {
                        assert(self.bytes@.take(i) =~= self.bytes@.drop_last().take(i));
                    }
                }
                self.depth = next.0;
                self.lex = next.1;
                if next.0 == 0 {
                    ListStep::Complete(self.bytes)
                } else {
                    ListStep::Pending(self)
                }
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
    /// A chunk cannot consume the second form after the original root closes.
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
