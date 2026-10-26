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
                    final(self).endpoints()==old(self).endpoints(),final(self).eof()==old(self).eof())
            { self.healthy=false; }
        }
        sail_method! {
            /// Marks only the actual zero-byte read as EOF. Repeated EOF refuses.
            pub fn end(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).eof(),final(self).healthy()==(old(self).healthy() && !old(self).eof()),
                    final(self).view()==old(self).view(),final(self).endpoints()==old(self).endpoints())
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
    }
    impl CompleteOutput {
        sail_impl_spec! {
            pub closed spec fn view(&self) -> Seq<u8> { self.bytes@ }
            pub closed spec fn endpoints(&self) -> Seq<usize> { self.ends@ }
            pub closed spec fn complete(&self) -> bool { frames_wf(self.view(),self.endpoints())
                && (if self.endpoints().len()==0 { self.view().len()==0 } else { self.endpoints().last()==self.view().len() }) }
        }
        sail_method! {
            /// Number of exact frames in the complete actual output partition.
            pub fn frames(&self) -> (out: usize) requires (true) ensures (out==self.endpoints().len()) { self.ends.len() }
        }
        sail_method! {
            /// Original complete stdout, retained without copying by the seal.
            pub fn bytes(&self) -> (out: &[u8]) requires (true) ensures (out@==self.view()) { &self.bytes }
        }
    }
}
