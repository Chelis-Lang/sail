//! Actual output framing and the retained stdout controller share one owner.
//! Physical reads/EOF, replay comparison and solver-origin authority stay separate.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// One retained output controller and the splitter consuming those same bytes.
    /// Private fields prevent replacing either half or supplying another history.
    pub struct ParsedOutput<Owner> {
        controller:crate::query_output_owner::StreamOutput<Owner>,
        splitter:Option<crate::query_split::StreamSplitter>,
    }
}

sail_code! {
    impl<Owner> ParsedOutput<Owner> {
        sail_impl_spec! {
            pub closed spec fn owner_view(&self) -> Owner { self.controller.owner_view() }
            pub closed spec fn output_view(&self) -> crate::query_output::Transcript { self.controller.output_view() }
            pub closed spec fn replay_view(&self) -> crate::query::ReplayCursor { self.controller.replay_view() }
            pub closed spec fn parser_view(&self) -> (u8,Seq<u8>,int) {
                match self.splitter { Some(splitter) => splitter.transition_view(),None => (0u8,Seq::<u8>::empty(),0int) }
            }
            pub closed spec fn coherent(&self) -> bool {
                self.output_view().well_formed() && match self.splitter {
                    Some(splitter) => !splitter.input_view() && splitter.well_formed()
                        && splitter.matches(self.output_view().view())
                        && splitter.position_view() == self.output_view().view().len(),
                    None => false,
                }
            }
        }
        sail_method! {
            /// Creates the actual output controller and suppression cursor
            /// together, before any response is observed.
            pub fn new_replayed(owner: Owner, served: u64) -> (out: Self)
                requires (true)
                ensures (out.coherent(), out.parser_view() == (0u8,Seq::<u8>::empty(),0int), out.owner_view() == owner, out.output_view().well_formed(),
                    out.output_view().healthy(), !out.output_view().eof(), out.output_view().view().len() == 0,
                    out.output_view().endpoints().len() == 0,
                    out.replay_view().original() == served, out.replay_view().left() == served)
            { Self { controller:crate::query_output_owner::StreamOutput::new_replayed(owner,served), splitter:Some(crate::query_split::StreamSplitter::start(false)) } }
        }
        sail_method! {
            /// Original forwarding decision from this stdout owner's cursor.
            pub fn forwarding(&self) -> (out: bool) requires (self.coherent())
                ensures (out == (self.replay_view().left() == 0))
            { self.controller.forwarding() }
        }
        sail_method! {
            /// Advances the same actual replay cursor at each original parser
            /// response boundary. Byte comparison remains separately required.
            pub fn advance_replay(&mut self) -> (out: bool) requires (old(self).coherent())
                ensures (final(self).coherent(), final(self).parser_view() == old(self).parser_view(), final(self).owner_view() == old(self).owner_view(),
                    final(self).output_view() == old(self).output_view(),
                    final(self).replay_view().original() == old(self).replay_view().original(),
                    out == (old(self).replay_view().left() == 0),
                    final(self).replay_view().left() == if old(self).replay_view().left() == 0 {0nat}
                        else {(old(self).replay_view().left()-1) as nat})
            { self.controller.advance_replay() }
        }
        sail_method! {
            /// Marks the original endpoint's actual EOF without replacing it.
            pub fn end(&mut self) -> (out: ())
                requires (old(self).coherent())
                ensures (final(self).coherent(), final(self).parser_view() == old(self).parser_view(), final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), final(self).output_view().eof(),
                    final(self).output_view().view() == old(self).output_view().view(),
                    final(self).output_view().endpoints() == old(self).output_view().endpoints(),
                    final(self).output_view().healthy() == (old(self).output_view().healthy() && !old(self).output_view().eof()),
                    final(self).output_view().well_formed() == old(self).output_view().well_formed())
            { self.controller.end(); }
        }
        sail_method! {
            /// Permanent failure keeps original owner and byte observations.
            pub fn refuse(&mut self) -> (out: ())
                requires (old(self).coherent())
                ensures (final(self).coherent(), final(self).parser_view() == old(self).parser_view(), final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), !final(self).output_view().healthy(),
                    final(self).output_view().view() == old(self).output_view().view(),
                    final(self).output_view().endpoints() == old(self).output_view().endpoints(),
                    final(self).output_view().eof() == old(self).output_view().eof(),
                    final(self).output_view().well_formed() == old(self).output_view().well_formed())
            { self.controller.refuse(); }
        }
        sail_method! {
            /// Releases recording storage on the unchanged streaming fallback.
            pub fn discard(&mut self) -> (out: ())
                requires (old(self).coherent())
                ensures (final(self).coherent(), final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), final(self).output_view().well_formed(),
                    !final(self).output_view().healthy(), final(self).output_view().view().len() == 0)
            { self.controller.discard(); self.splitter=Some(crate::query_split::StreamSplitter::start(false)); }
        }
        sail_method! {
            /// Borrows the retained original stdout bytes.
            pub fn bytes(&self) -> (out: &[u8]) requires (self.coherent())
                ensures (out@ == self.output_view().view()) { self.controller.bytes() }
        }
        sail_method! {
            /// Original buffer length, with unchanged accounting.
            pub fn len(&self) -> (out: usize) requires (self.coherent())
                ensures (out == self.output_view().view().len()) { self.controller.len() }
        }
        sail_method! {
            /// Original empty-buffer observation, without interpreting a reply.
            pub fn is_empty(&self) -> (out:bool)
                requires (self.coherent())
                ensures (out == (self.output_view().view().len() == 0))
            { self.controller.is_empty() }
        }
        sail_method! {
            /// Original admitted frame count, with unchanged buffer accounting.
            pub fn frames(&self) -> (out: usize) requires (self.coherent())
                ensures (out == self.output_view().endpoints().len()) { self.controller.frames() }
        }
        sail_method! {
            /// Borrows the actual next admitted interval and its original lease.
            #[allow(clippy::question_mark, reason = "Explicit refusal exits retain the shared stock-Verus contracts and full-body mutation controls.")]
            pub fn frame<'a>(&'a mut self, start: usize, end: usize) -> (out: Option<crate::query_output_owner::StreamFrame<'a, Owner>>)
                requires (old(self).coherent(), old(self).output_view().well_formed())
                ensures (final(self).coherent(), final(self).parser_view() == old(self).parser_view(), final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), final(self).output_view().well_formed(),
                    final(self).output_view().view() == old(self).output_view().view(),
                    final(self).output_view().eof() == old(self).output_view().eof(),
                    out.is_some() == (old(self).output_view().healthy() && start == old(self).output_view().consumed()
                        && start < end <= old(self).output_view().view().len()
                        && crate::query::response_shape(old(self).output_view().view().subrange(start as int,end as int))),
                    out.is_some() ==> *out.unwrap().owner_view() == old(self).owner_view()
                        && out.unwrap().frame_view().view() == old(self).output_view().view().subrange(start as int,end as int)
                        && out.unwrap().frame_view().start_view() == start && out.unwrap().frame_view().end_view() == end
                        && out.unwrap().frame_view().ordinal_view() == old(self).output_view().endpoints().len()
                        && final(self).output_view().healthy()
                        && final(self).output_view().endpoints() == old(self).output_view().endpoints().push(end),
                    out.is_none() ==> !final(self).output_view().healthy()
                        && final(self).output_view().endpoints() == old(self).output_view().endpoints())
            { self.controller.frame(start,end) }
        }
        sail_method! {
            /// Consumes the exact original controller and retains its owner.
            #[allow(clippy::question_mark, reason = "Explicit refusal exits retain the shared stock-Verus contracts and full-body mutation controls.")]
            pub fn seal(self) -> (out: Option<crate::query_output_owner::CompletedStream<Owner>>)
                requires (self.coherent(), self.output_view().well_formed())
                ensures (out.is_some() == self.output_view().complete(),
                    out.is_some() ==> out.unwrap().complete() && out.unwrap().owner_view() == self.owner_view()
                        && out.unwrap().output_view().view() == self.output_view().view()
                        && out.unwrap().output_view().endpoints() == self.output_view().endpoints()
                        && out.unwrap().replayed_view() == if self.replay_view().left() == 0
                            { Some(self.replay_view().original()) } else {None})
            { self.controller.seal() }
        }
        sail_method! {
            /// Appends each actual stdout read once, then emits exactly its
            /// original ordered splitter items against this retained tape.
            /// Forwarding and independent frame refusal still use the raw bytes.
            #[allow(clippy::single_match_else, reason = "Explicit owned Option transfer preserves the checked original parser and unreachable defensive branch.")]
            pub fn append_items(&mut self,bytes:&[u8]) -> (out:Vec<crate::query_split::Item>)
                requires (old(self).coherent())
                ensures (final(self).coherent(),final(self).owner_view() == old(self).owner_view(),
                    final(self).replay_view() == old(self).replay_view(),
                    final(self).output_view().view() == old(self).output_view().view()+bytes@,
                    final(self).output_view().endpoints() == old(self).output_view().endpoints(),
                    final(self).output_view().eof() == old(self).output_view().eof(),
                    final(self).output_view().healthy() == (old(self).output_view().healthy() && !old(self).output_view().eof()),
                    final(self).parser_view() == crate::query_split::model_scan(old(self).parser_view(),false,old(self).output_view().view().len() as int,bytes@).0,
                    crate::query_split::item_models(out@) == crate::query_split::model_scan(old(self).parser_view(),false,old(self).output_view().view().len() as int,bytes@).1,
                    forall|i:int| 0 <= i < out@.len() ==>
                        crate::query_split::original_item(out@[i],final(self).output_view().view(),false))
            {
                self.controller.append(bytes);
                let _retained_len=self.controller.bytes().len();
                match self.splitter.take() {
                    Some(splitter) => {
                        let (next,items)=splitter.push_items(bytes,Vec::new());
                        self.splitter=Some(next);
                        items
                    }
                    None => {
                        // Private coherent factories preserve Some. This
                        // unreachable branch retains all raw forwarding data.
                        self.controller.refuse();
                        Vec::new()
                    }
                }
            }
        }
    }
}

sail_verified! {
    /// A nonempty fragmented stdout tape produces both original ordered reply
    /// items and retains exactly the raw CRLF bytes used by independent framing.
    #[allow(clippy::byte_char_slices, reason = "Explicit original chunks keep the complete constructive output trace visible to Stock.")]
    pub fn parsed_output_constructive_control() -> (passed:bool)
        requires (true) ensures (passed)
    {
        let first=[b'u',b'n'];
        let second=[b's',b'a',b't',b'\r',b'\n',b's',b'a',b't',b'\n'];
        let raw=[b'u',b'n',b's',b'a',b't',b'\r',b'\n',b's',b'a',b't',b'\n'];
        sail_proof! {
            assert(crate::query_split::model_scan((0u8,Seq::<u8>::empty(),0int),false,0,first@)
                == ((2u8,seq![b'u',b'n'],0int),seq![])) by {
                reveal(crate::query_split::model_step); reveal(crate::query_split::idle_model);
                reveal(crate::query_split::atom_ends); reveal(crate::query_split::terminated_atom);
                reveal_with_fuel(crate::query_split::model_scan,3);
            }
            assert(crate::query_split::model_scan((2u8,seq![b'u',b'n'],0int),false,2,second@)
                == ((0u8,Seq::<u8>::empty(),0int),seq![
                    (crate::query_split::ItemKind::Atom,0int,seq![b'u',b'n',b's',b'a',b't']),
                    (crate::query_split::ItemKind::Atom,7int,seq![b's',b'a',b't'])])) by {
                reveal(crate::query_split::model_step); reveal(crate::query_split::idle_model);
                reveal(crate::query_split::atom_ends); reveal(crate::query_split::terminated_atom);
                reveal_with_fuel(crate::query_split::model_scan,10);
            }
        }
        let mut output=ParsedOutput::new_replayed(7u64,2);
        if !output.append_items(&first).is_empty() { return false; }
        let items=output.append_items(&second);
        sail_proof! { crate::query_split::item_models_facts(items@); }
        if items.len() != 2 || items[0].offset != 0 || items[1].offset != 7
            || items[0].bytes.len() != 5 || items[1].bytes.len() != 3
            || !crate::inputs::byte_eq(output.bytes(),&raw) { return false; }
        if output.forwarding() || output.advance_replay() || output.forwarding()
            || output.advance_replay() || !output.forwarding() { return false; }
        sail_proof! {
            assert(raw@ =~= first@+second@);
            crate::query_output::transcript_partition_facts(&output.output_view());
            assert(crate::query::response_span(raw@.subrange(0,7),0,5));
            assert(crate::query::response_shape(output.output_view().view().subrange(0,7)));
        }
        if output.frame(0,7).is_none() { return false; }
        sail_proof! {
            crate::query_output::transcript_partition_facts(&output.output_view());
            assert(crate::query::response_span(raw@.subrange(7,11),0,3));
            assert(crate::query::response_shape(output.output_view().view().subrange(7,11)));
        }
        if output.frame(7,11).is_none() { return false; }
        output.end();
        sail_proof! { crate::query_output::transcript_partition_facts(&output.output_view()); }
        output.seal().is_some()
    }
}

sail_verified! {
    /// An actual nonempty unfinished list remains raw data; neither a supplied
    /// interval nor the explicit end event bypasses independent frame refusal.
    pub fn parsed_output_partial_refused() -> (refused:bool)
        requires (true) ensures (refused)
    {
        let bytes=[b'('];
        let mut output=ParsedOutput::new_replayed(7u64,0);
        let _items=output.append_items(&bytes);
        output.end();
        sail_proof! {
            assert(!crate::query::response_shape(bytes@)) by {
                if crate::query::response_shape(bytes@) {
                    let (start,end)=choose|start:int,end:int| crate::query::response_span(bytes@,start,end);
                    assert(start == 0 && end == 1);
                }
            }
            assert(output.output_view().view() =~= bytes@);
            assert(output.output_view().view().subrange(0,1) =~= bytes@);
        }
        if output.frame(0,1).is_some() { return false; }
        output.seal().is_none()
    }
}
