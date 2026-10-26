//! Exact owned solver-output bytes and checked contiguous response intervals.
//! Native reads and EOF are primitive observations; framed data is not solver truth.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    pub open spec fn frame_start(ends: Seq<usize>, i: int) -> int {
        if i == 0 { 0 } else { ends[i-1] as int }
    }
    pub open spec fn frames_wf(bytes: Seq<u8>, ends: Seq<usize>) -> bool {
        forall|i: int| 0 <= i < ends.len() ==> 0 <= frame_start(ends,i) < ends[i] <= bytes.len()
            && crate::query::response_shape(bytes.subrange(frame_start(ends,i),ends[i] as int))
    }
    /// Exposes the original closed partition relationships without exposing
    /// private fields or changing the admitted-output/EOF predicates.
    pub proof fn transcript_partition_facts(output: &Transcript)
        ensures output.consumed() == if output.endpoints().len()==0 {0nat}
                else {output.endpoints().last() as nat},
            output.complete() == (output.healthy() && output.eof()
                && output.consumed() == output.view().len())
    {}
}

sail_verified! {
    /// A nonempty original response can pass the complete output lifecycle.
    #[allow(clippy::byte_char_slices, reason = "Explicit array bytes give stock Verus a concrete nonempty response witness.")]
    pub fn nonempty_output_control() -> (accepted: bool)
        requires (true) ensures (accepted)
    {
        let original = [b'u', b'n', b's', b'a', b't', b'\n'];
        let mut output = Transcript::new();
        output.append(&original);
        sail_proof! {
            assert(crate::query::response_trim(original@, 0, 5));
            assert(crate::query::response_span(original@, 0, 5));
            assert(crate::query::response_shape(original@));
            assert(output.view() =~= original@);
            assert(output.healthy() && !output.eof());
            assert(output.endpoints().len() == 0 && output.consumed() == 0);
            assert(output.view().len() == 6);
            assert(crate::query::response_shape(output.view()));
            assert(output.view().subrange(0, 6) =~= output.view());
            assert(crate::query::response_shape(output.view().subrange(0, 6)));
        }
        if output.frame(0, 6).is_none() { return false; }
        output.end();
        output.seal().is_some()
    }
}

sail_verified! {
    /// Receiving bytes without actual EOF cannot produce a completed output.
    pub fn missing_eof_output_control() -> (refused: bool)
        requires (true) ensures (refused)
    {
        let mut output = Transcript::new();
        output.append(b"unsat\n");
        output.seal().is_none()
    }
}

sail_verified! {
    /// A failed output observation cannot recover authority from later EOF.
    pub fn failed_output_control() -> (refused: bool)
        requires (true) ensures (refused)
    {
        let mut output = Transcript::new();
        output.append(b"unsat\n");
        output.refuse();
        output.end();
        output.seal().is_none()
    }
}

sail_code! {
    /// One response interval borrowing the actual immutable output owner.
    pub struct Frame<'a> {
        bytes: &'a [u8],
        ordinal: usize,
        start: usize,
        end: usize,
    }
    /// Value-preserving copy of one admitted interval, retaining its exact
    /// ordinal and byte endpoints through queues, annotation and publication.
    pub struct FramePayload {
        bytes: Vec<u8>,
        ordinal: usize,
        start: usize,
        end: usize,
    }
    /// The solver stdout bytes already received and all admitted endpoints.
    pub struct Transcript {
        bytes: Vec<u8>,
        ends: Vec<usize>,
        healthy: bool,
        eof: bool,
    }
    /// Complete EOF and an exact partition of every retained output byte.
    /// This is output coverage, not a normal-exit or truthful-origin assertion.
    pub struct CompleteOutput {
        bytes: Vec<u8>,
        ends: Vec<usize>,
    }
    impl Transcript {
        sail_impl_spec! {
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            pub closed spec fn endpoints(&self) -> Seq<usize> { self.ends@ }
            pub closed spec fn healthy(&self) -> bool { self.healthy }
            pub closed spec fn eof(&self) -> bool { self.eof }
            pub closed spec fn consumed(&self) -> nat {
                if self.ends@.len() == 0 { 0 } else { self.ends@.last() as nat }
            }
            pub closed spec fn well_formed(&self) -> bool { frames_wf(self.bytes@,self.ends@) }
            pub closed spec fn complete(&self) -> bool {
                self.healthy && self.eof && self.consumed() == self.bytes@.len()
            }
        }
        sail_method! {
            /// Starts before the first actual stdout read.
            #[allow(clippy::new_without_default, reason = "An explicit constructor records the pre-read boundary.")]
            pub fn new() -> (out: Self)
                requires (true)
                ensures (out.well_formed(), out.healthy(), !out.eof(),out.view().len() == 0,out.endpoints().len() == 0)
            { Self { bytes:Vec::new(),ends:Vec::new(),healthy:true,eof:false } }
        }
        sail_method! {
            /// Appends the exact returned read chunk; data after EOF refuses authority.
            pub fn append(&mut self, bytes: &[u8]) -> (out: ())
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),final(self).view() == old(self).view() + bytes@,
                    final(self).endpoints() == old(self).endpoints(),final(self).eof() == old(self).eof(),
                    final(self).healthy() == (old(self).healthy() && !old(self).eof()))
            {
                if self.eof { self.healthy=false; }
                self.bytes.extend_from_slice(bytes);
                sail_proof! {
                    assert forall|i: int| 0 <= i < self.ends@.len() implies
                        0 <= frame_start(self.ends@,i) < self.ends@[i] <= self.bytes@.len()
                        && crate::query::response_shape(self.bytes@.subrange(frame_start(self.ends@,i),self.ends@[i] as int)) by {
                        assert(self.bytes@.subrange(frame_start(self.ends@,i),self.ends@[i] as int)
                            =~= old(self).view().subrange(frame_start(self.ends@,i),self.ends@[i] as int));
                    }
                }
            }
        }
        sail_method! {
            /// Borrows every original retained byte for unchanged forwarding.
            pub fn bytes(&self) -> (out: &[u8])
                requires (true) ensures (out@ == self.view())
            { &self.bytes }
        }
        sail_method! {
            /// Reports retained byte length without trusting parser offsets.
            pub fn len(&self) -> (out: usize)
                requires (true) ensures (out == self.view().len())
            { self.bytes.len() }
        }
        sail_method! {
            /// Number of retained response endpoints for native buffer accounting.
            pub fn frames(&self) -> (out: usize)
                requires (true) ensures (out == self.endpoints().len())
            { self.ends.len() }
        }
        sail_method! {
            /// Whether no output byte is retained.
            pub fn is_empty(&self) -> (out: bool)
                requires (true) ensures (out == (self.view().len()==0))
            { self.bytes.is_empty() }
        }
        sail_method! {
            /// A read, forwarding, framing or coverage failure is permanent.
            pub fn refuse(&mut self) -> (out: ())
                requires (true)
                ensures (!final(self).healthy(),final(self).view()==old(self).view(),
                    final(self).endpoints()==old(self).endpoints(),final(self).eof()==old(self).eof(),
                    final(self).well_formed()==old(self).well_formed())
            { self.healthy=false; }
        }
        sail_method! {
            /// Marks only the actual zero-byte read as EOF. Repeated EOF refuses.
            pub fn end(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).eof(),final(self).healthy()==(old(self).healthy() && !old(self).eof()),
                    final(self).view()==old(self).view(),final(self).endpoints()==old(self).endpoints(),
                    final(self).well_formed()==old(self).well_formed())
            { if self.eof { self.healthy=false; } self.eof=true; }
        }
        sail_method! {
            /// Checks an exact next response interval before it can enter pairing.
            pub fn frame<'a>(&'a mut self, start: usize, end: usize) -> (out: Option<Frame<'a>>)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),final(self).view()==old(self).view(),final(self).eof()==old(self).eof(),
                    out.is_some() == (old(self).healthy() && start == old(self).consumed()
                        && start < end <= old(self).view().len()
                        && crate::query::response_shape(old(self).view().subrange(start as int,end as int))),
                    out.is_some() ==> final(self).healthy()
                        && final(self).endpoints() == old(self).endpoints().push(end)
                        && out.unwrap().view() == old(self).view().subrange(start as int,end as int)
                        && out.unwrap().ordinal_view() == old(self).endpoints().len()
                        && out.unwrap().start_view() == start && out.unwrap().end_view() == end,
                    out.is_none() ==> !final(self).healthy() && final(self).endpoints()==old(self).endpoints())
            {
                let previous = match self.ends.last() { None=>0,Some(value)=>*value };
                if !self.healthy || start != previous || start>=end || end>self.bytes.len() {
                    self.healthy=false; return None;
                }
                if !crate::query::response_frame(&self.bytes[start..end]) { self.healthy=false; return None; }
                let ordinal=self.ends.len();
                self.ends.push(end);
                sail_proof! {
                    assert forall|i: int| 0 <= i < self.ends@.len() implies
                        0 <= frame_start(self.ends@,i) < self.ends@[i] <= self.bytes@.len()
                        && crate::query::response_shape(self.bytes@.subrange(frame_start(self.ends@,i),self.ends@[i] as int)) by {
                        if i<ordinal {} else { assert(i==ordinal); }
                    }
                }
                Some(Frame { bytes:&self.bytes[start..end],ordinal,start,end })
            }
        }
        sail_method! {
            /// Drops regenerable retained output when entering permanent streaming.
            pub fn discard(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).well_formed(),!final(self).healthy(),final(self).view().len()==0,
                    final(self).endpoints().len()==0,final(self).eof()==old(self).eof())
            { self.bytes=Vec::new();self.ends=Vec::new();self.healthy=false; }
        }
        sail_method! {
            /// Consumes the actual output owner only after EOF and exact coverage.
            pub fn seal(self) -> (out: Option<CompleteOutput>)
                requires (self.well_formed())
                ensures (out.is_some()==self.complete(),out.is_some() ==> out.unwrap().view()==self.view()
                    && out.unwrap().endpoints()==self.endpoints() && out.unwrap().complete())
            {
                let consumed=match self.ends.last() { None=>0,Some(value)=>*value };
                if !self.healthy || !self.eof || consumed!=self.bytes.len() { return None; }
                Some(CompleteOutput { bytes:self.bytes,ends:self.ends })
            }
        }
    }
    impl Frame<'_> {
        sail_impl_spec! {
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            pub closed spec fn ordinal_view(&self) -> nat { self.ordinal as nat }
            pub closed spec fn start_view(&self) -> nat { self.start as nat }
            pub closed spec fn end_view(&self) -> nat { self.end as nat }
        }
        sail_method! {
            /// Exact bytes borrowed from the actual admitted response interval.
            pub fn bytes(&self) -> (out: &[u8]) requires (true) ensures (out@==self.view()) { self.bytes }
        }
        sail_method! {
            /// Original ordinal in the complete physical stdout frame sequence.
            pub fn ordinal(&self) -> (out: usize) requires (true) ensures (out==self.ordinal_view()) { self.ordinal }
        }
        sail_method! {
            /// Original byte range beginning in the owning stdout buffer.
            pub fn start(&self) -> (out: usize) requires (true) ensures (out==self.start_view()) { self.start }
        }
        sail_method! {
            /// Original byte range ending in the owning stdout buffer.
            pub fn end(&self) -> (out: usize) requires (true) ensures (out==self.end_view()) { self.end }
        }
        sail_method! {
            /// Transfers this exact interval into owned storage without
            /// dropping its source position or reinterpreting its bytes.
            pub fn into_owned(self) -> (out: FramePayload)
                requires (true)
                ensures (out.view() == self.view(), out.ordinal_view() == self.ordinal_view(),
                    out.start_view() == self.start_view(), out.end_view() == self.end_view())
            {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(self.bytes);
                FramePayload { bytes, ordinal:self.ordinal, start:self.start, end:self.end }
            }
        }
    }
    impl FramePayload {
        sail_impl_spec! {
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            pub closed spec fn ordinal_view(&self) -> nat { self.ordinal as nat }
            pub closed spec fn start_view(&self) -> nat { self.start as nat }
            pub closed spec fn end_view(&self) -> nat { self.end as nat }
        }
        sail_method! {
            /// Borrows the original response bytes after ownership transfer.
            pub fn bytes(&self) -> (out: &[u8]) requires (true) ensures (out@ == self.view()) { &self.bytes }
        }
        sail_method! {
            /// Original frame ordinal, independent of held-check release order.
            pub fn ordinal(&self) -> (out: usize) requires (true) ensures (out == self.ordinal_view()) { self.ordinal }
        }
        sail_method! {
            /// Original start offset in the completed stdout owner.
            pub fn start(&self) -> (out: usize) requires (true) ensures (out == self.start_view()) { self.start }
        }
        sail_method! {
            /// Original end offset in the completed stdout owner.
            pub fn end(&self) -> (out: usize) requires (true) ensures (out == self.end_view()) { self.end }
        }
        sail_method! {
            /// Consumes the positional witness only at unchanged wire conversion.
            pub fn into_bytes(self) -> (out: Vec<u8>) requires (true) ensures (out@ == self.view()) { self.bytes }
        }
    }
    impl CompleteOutput {
        sail_impl_spec! {
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            pub closed spec fn endpoints(&self) -> Seq<usize> { self.ends@ }
            pub closed spec fn complete(&self) -> bool { frames_wf(self.view(),self.endpoints())
                && (if self.endpoints().len()==0 { self.view().len()==0 } else { self.endpoints().last()==self.view().len() }) }
            pub open spec fn contains(&self, frame: &FramePayload) -> bool {
                frame.ordinal_view() < self.endpoints().len()
                    && frame.start_view() == frame_start(self.endpoints(), frame.ordinal_view() as int)
                    && frame.end_view() == self.endpoints()[frame.ordinal_view() as int]
                    && frame.view() == self.view().subrange(frame.start_view() as int,frame.end_view() as int)
            }
        }
        sail_method! {
            /// Number of exact frames in the complete actual output partition.
            pub fn frames(&self) -> (out: usize) requires (true) ensures (out==self.endpoints().len()) { self.ends.len() }
        }
        sail_method! {
            /// Original complete stdout, retained without copying by the seal.
            pub fn bytes(&self) -> (out: &[u8]) requires (true) ensures (out@==self.view()) { &self.bytes }
        }
        sail_method! {
            /// Checks exact ordinal, endpoints and bytes against this complete
            /// owner; shape or an equal payload at another position is insufficient.
            pub fn contains_frame(&self, frame: &FramePayload) -> (out: bool)
                requires (self.complete()) ensures (out == self.contains(frame))
            {
                if frame.ordinal >= self.ends.len() { return false; }
                let start = if frame.ordinal == 0 { 0 } else { self.ends[frame.ordinal - 1] };
                let end = self.ends[frame.ordinal];
                if frame.start != start || frame.end != end { return false; }
                crate::inputs::byte_eq(&frame.bytes, &self.bytes[start..end])
            }
        }
    }
}
