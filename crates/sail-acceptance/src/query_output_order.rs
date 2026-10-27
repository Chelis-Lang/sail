//! Actual item, frame, replay and queue-handover order over one owned stdout tape.
//! Physical reads/writes and the native EOF/drop/wait/tail block remain separate.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    struct Pending { fresh:bool,start:usize }
    enum FrameStage { Unframed,Refused,Admitted(usize) }
    struct Prepared { end:usize,previous:Option<Pending>,frame:FrameStage }
    /// Owns the original parser/output, replay comparison and pending boundary.
    pub struct OutputOrder<'a,Owner> {
        output:crate::query_output_split::ParsedOutput<Owner>,
        comparison:crate::query_replay::Replay<'a,Owner>,
        pending:Option<Pending>,
        forward_from:Option<usize>,
    }

    /// Owns the original final pending response after the native wait observation.
    pub struct OutputTail<'a,Owner> { order:OutputOrder<'a,Owner>,current:Option<Prepared> }
    /// Retains the actual sealed stdout and original replay through finish/join.
    /// Presence proves output coverage only, separately from native exit/origin.
    pub struct SealedOrder<'a,Owner> {
        output:Option<crate::query_output_owner::CompletedStream<Owner>>,
        comparison:crate::query_replay::Replay<'a,Owner>,
    }
    /// Owns the actual emitted vector and its original controller while one
    /// read chunk is consumed. Its cursor never removes/reorders payloads.
    pub struct OutputBatch<'a,Owner> {
        order:OutputOrder<'a,Owner>,
        items:Vec<crate::query_split::Item>,
        at:usize,
        current:Option<Prepared>,
    }
}

sail_spec! {
    spec fn pending_view(pending:Option<Pending>) -> Option<(bool,int)> {
        match pending {Some(p) => Some((p.fresh,p.start as int)),None => None}
    }
    spec fn stage_view(stage:FrameStage) -> Option<Option<int>> {
        match stage {FrameStage::Unframed=>None,FrameStage::Refused=>Some(None),FrameStage::Admitted(i)=>Some(Some(i as int))}
    }
}

sail_code! {
    impl<'a,Owner> OutputOrder<'a,Owner> {
        sail_impl_spec! {
            pub closed spec fn view(&self) -> Seq<u8> {self.output.output_view().view()}
            pub closed spec fn output_view(&self) -> crate::query_output_split::ParsedOutput<Owner> {self.output}
            pub closed spec fn replay_view(&self) -> crate::query_replay::Replay<'a,Owner> {self.comparison}
            pub closed spec fn pending_view(&self) -> Option<(bool,int)> {pending_view(self.pending)}
            pub closed spec fn forwarding_view(&self) -> Option<usize> {self.forward_from}
            pub closed spec fn base(&self) -> bool {
                self.output.coherent() && self.output.output_view().well_formed() && self.comparison.well_formed()
                    && self.output.replay_view().left() <= self.output.replay_view().original()
                    && self.output.replay_view().original() == self.comparison.served_view().len()
            }
            pub closed spec fn matches_pending(&self,pending:Option<(bool,int)>) -> bool {
                self.comparison.diverged_view() || match pending {
                    None => self.comparison.checked_view()+self.output.replay_view().left()
                        == self.output.replay_view().original(),
                    Some((fresh,_)) => if fresh {
                        self.output.replay_view().left() == 0 && self.comparison.checked_view() == self.comparison.served_view().len()
                    } else {
                        self.comparison.checked_view()+self.output.replay_view().left()+1
                            == self.output.replay_view().original()
                    },
                }
            }
            pub closed spec fn coherent(&self) -> bool {
                self.base() && self.matches_pending(self.pending_view())
                    && match self.pending_view() {Some((_,start))=>0 <= start <= self.view().len(),None=>true}
            }
        }
        sail_method! {
            /// Initializes the actual suppression count from the original served
            /// slice. Both opaque leases are retained; final identity is still
            /// checked by the existing completed-stream replay join.
            #[allow(clippy::cast_possible_truncation, reason = "Sail's supported native targets have usize no wider than u64, matching the original bounded served-count conversion.")]
            pub fn new(served:&'a [crate::query_replay::ServedReply],owner:Owner,replay_owner:Owner) -> (out:Self)
                requires (true)
                ensures (out.coherent(),out.view().len() == 0,out.pending_view().is_none(),
                    out.output_view().owner_view() == owner,out.replay_view().owner_view() == replay_owner,
                    out.replay_view().served_view() == served@,out.replay_view().checked_view() == 0,
                    !out.replay_view().diverged_view(),out.output_view().output_view().healthy(),!out.output_view().output_view().eof(),
                    out.output_view().output_view().endpoints().len()==0,
                    out.output_view().parser_view() == (0u8,Seq::<u8>::empty(),0int),
                    out.forwarding_view() == if served.len() == 0 {Some(0usize)} else {None})
            {
                let output=crate::query_output_split::ParsedOutput::new_replayed(owner,served.len() as u64);
                let comparison=crate::query_replay::Replay::new(served,replay_owner);
                Self {output,comparison,pending:None,forward_from:if served.is_empty(){Some(0)}else{None}}
            }
        }
        sail_method! {
            /// Borrows the exact retained raw bytes for unchanged forwarding.
            pub fn bytes(&self) -> (out:&[u8]) requires (self.base()) ensures (out@ == self.view())
            {self.output.bytes()}
        }
        sail_method! {
            /// Retained raw length used by the original buffering checks.
            pub fn len(&self) -> (out:usize) requires (self.base()) ensures (out == self.view().len())
            {self.output.len()}
        }
        sail_method! {
            /// Completes the public retained-buffer observation pair.
            pub fn is_empty(&self) -> (out:bool) requires (self.base()) ensures (out == (self.view().len()==0))
            {self.output.is_empty()}
        }
        sail_method! {
            /// Original independently admitted frame count.
            pub fn frames(&self) -> (out:usize) requires (self.base())
                ensures (out == self.output_view().output_view().endpoints().len())
            {self.output.frames()}
        }
        sail_method! {
            /// Actual first fresh boundary, never a caller-selected offset.
            pub fn forward_from(&self) -> (out:Option<usize>) requires (true) ensures (out == self.forwarding_view())
            {self.forward_from}
        }
        sail_method! {
            /// Actual sticky comparison refusal, used by the native break/kill path.
            pub fn diverged(&self) -> (out:bool) requires (true) ensures (out == self.replay_view().diverged_view())
            {self.comparison.diverged()}
        }
        sail_method! {
            /// Actual complete original served-prefix comparison state.
            pub fn replay_complete(&self) -> (out:bool) requires (true) ensures (out == self.replay_view().complete_view())
            {self.comparison.complete()}
        }
        sail_method! {
            /// Records the unchanged native read-end event, without authenticating it.
            pub fn end(&mut self) -> (out:()) requires (old(self).coherent())
                ensures (final(self).coherent(),final(self).view() == old(self).view(),
                    final(self).pending_view() == old(self).pending_view(),final(self).replay_view() == old(self).replay_view(),
                    final(self).forwarding_view() == old(self).forwarding_view(),final(self).output_view().output_view().eof(),
                    final(self).output_view().owner_view() == old(self).output_view().owner_view(),
                    final(self).output_view().replay_view() == old(self).output_view().replay_view(),
                    final(self).output_view().parser_view() == old(self).output_view().parser_view(),
                    final(self).output_view().output_view().endpoints() == old(self).output_view().output_view().endpoints(),
                    final(self).output_view().output_view().healthy() == (old(self).output_view().output_view().healthy()
                        && !old(self).output_view().output_view().eof()))
            {self.output.end();}
        }
        sail_method! {
            /// Refuses recording after the original native read error.
            pub fn refuse(&mut self) -> (out:()) requires (old(self).coherent())
                ensures (final(self).coherent(),final(self).view() == old(self).view(),
                    final(self).pending_view() == old(self).pending_view(),final(self).replay_view() == old(self).replay_view(),
                    final(self).forwarding_view() == old(self).forwarding_view(),!final(self).output_view().output_view().healthy())
            {self.output.refuse();}
        }
        sail_method! {
            /// Releases original storage and the original pending boundary only
            /// after the native loop has entered streaming fallback.
            pub fn discard(&mut self) -> (out:()) requires (old(self).coherent(),old(self).replay_view().complete_view())
                ensures (final(self).coherent(),final(self).view().len()==0,final(self).pending_view().is_none(),
                    final(self).replay_view() == old(self).replay_view(),final(self).forwarding_view() == old(self).forwarding_view(),
                    !final(self).output_view().output_view().healthy())
            {self.output.discard();self.pending=None;}
        }
        sail_method! {
            /// Consumes exactly one actual read chunk and retains its complete
            /// emitted vector with this same original output owner.
            pub fn append_batch(mut self,bytes:&[u8]) -> (out:OutputBatch<'a,Owner>)
                requires (self.coherent())
                ensures (out.well_formed(),out.at_view()==0,out.current_view().is_none(),
                    out.order_view().view() == self.view()+bytes@,
                    out.order_view().output_view().owner_view() == self.output_view().owner_view(),
                    out.order_view().output_view().output_view().endpoints() == self.output_view().output_view().endpoints(),
                    out.order_view().output_view().output_view().eof() == self.output_view().output_view().eof(),
                    out.order_view().output_view().output_view().healthy() == (self.output_view().output_view().healthy() && !self.output_view().output_view().eof()),
                    out.order_view().output_view().replay_view() == self.output_view().replay_view(),
                    out.order_view().output_view().parser_view() == crate::query_split::model_scan(self.output_view().parser_view(),false,self.view().len() as int,bytes@).0,
                    out.order_view().replay_view() == self.replay_view(),
                    out.order_view().pending_view() == self.pending_view(),
                    out.order_view().forwarding_view() == self.forwarding_view(),
                    crate::query_split::item_models(out.items_view())
                        == crate::query_split::model_scan(self.output_view().parser_view(),false,self.view().len() as int,bytes@).1)
            {
                let items=self.output.append_items(bytes);
                OutputBatch {order:self,items,at:0,current:None}
            }
        }
        sail_method! {
            /// Transfers the same original fields back to the unchanged native
            /// EOF/drop/wait/tail/seal/finish/replay-join block. No status is minted.
            #[allow(clippy::type_complexity, reason = "The returned tuple exposes the four original owned native fields together, rather than introducing another opaque constructor or losing their exact transfer contract.")]
            #[allow(clippy::manual_map, reason = "The pinned Stock whole-root Option::map closure attempt leaves the exact original pending-field postcondition unproved; explicit owned Option branches expose both field identities without an extra trusted closure contract.")]
            pub fn into_parts(self) -> (out:(crate::query_output_split::ParsedOutput<Owner>,crate::query_replay::Replay<'a,Owner>,Option<(bool,usize)>,Option<usize>))
                requires (self.coherent())
                ensures (out.0 == self.output_view(),out.1 == self.replay_view(),out.3 == self.forwarding_view(),
                    match out.2 {Some((fresh,start))=>self.pending_view()==Some((fresh,start as int)),None=>self.pending_view().is_none()})
            {
                let pending=match self.pending {Some(p)=>Some((p.fresh,p.start)),None=>None};
                (self.output,self.comparison,pending,self.forward_from)
            }
        }
    }
}

sail_code! {
    impl<'a,Owner> OutputBatch<'a,Owner> {
        sail_impl_spec! {
            pub closed spec fn order_view(&self) -> OutputOrder<'a,Owner> {self.order}
            pub closed spec fn items_view(&self) -> Seq<crate::query_split::Item> {self.items@}
            pub closed spec fn at_view(&self) -> nat {self.at as nat}
            pub closed spec fn current_view(&self) -> Option<(int,Option<(bool,int)>,Option<Option<int>>)> {
                match self.current {Some(c)=>Some((c.end as int,pending_view(c.previous),stage_view(c.frame))),None=>None}
            }
            pub closed spec fn interval_view(&self) -> Seq<u8> {
                match self.current_view() {
                    Some((end,Some((_,start)),_)) => if 0 <= start <= end <= self.order.view().len() {
                        self.order.view().subrange(start,end)
                    } else {Seq::<u8>::empty()},
                    _ => Seq::<u8>::empty(),
                }
            }
            pub closed spec fn admitted_shape(&self) -> bool {
                match self.current_view() {
                    Some((end,Some((_,start)),Some(Some(ordinal)))) =>
                        0 <= ordinal < self.order.output_view().output_view().endpoints().len()
                        && 0 <= start < end <= self.order.view().len()
                        && crate::query_output::frame_start(self.order.output_view().output_view().endpoints(),ordinal) == start
                        && self.order.output_view().output_view().endpoints()[ordinal] == end
                        && crate::query::response_shape(self.interval_view()),
                    _ => true,
                }
            }
            pub closed spec fn well_formed(&self) -> bool {
                self.order.base() && self.at_view() <= self.items_view().len()
                    && (forall|i:int| 0 <= i < self.items_view().len() ==>
                        crate::query_split::original_item(self.items_view()[i],self.order.view(),false))
                    && self.admitted_shape()
                    && match self.current_view() {
                        None => self.order.coherent(),
                        Some((end,previous,_)) => self.at_view() < self.items_view().len()
                            && end == self.items_view()[self.at_view() as int].offset as int
                            && 0 <= end <= self.order.view().len()
                            && self.order.pending_view().is_none()
                            && self.order.matches_pending(previous)
                            && match previous {Some((_,start))=>0 <= start <= self.order.view().len(),None=>true},
                    }
            }
            pub closed spec fn ready_to_settle(&self) -> bool {
                self.current_view().is_some() && (self.current_view().unwrap().1.is_none()
                    || self.current_view().unwrap().2.is_some())
            }
            pub closed spec fn comparison_accepts(&self) -> bool {
                let replay=self.order.replay_view();
                self.ready_to_settle() && !replay.diverged_view() && match self.current_view() {
                    Some((_,Some((false,_)),_)) => replay.checked_view() < replay.served_view().len()
                        && (!replay.served_view()[replay.checked_view() as int].statistics_view()
                            || crate::query_split::split_single_response(self.interval_view()))
                        && replay.served_view()[replay.checked_view() as int].reproduces(self.interval_view()),
                    _ => true,
                }
            }
            pub closed spec fn frame_matches<Identity>(&self,frame:crate::query_output_owner::OwnedFrame<Owner>) -> bool
                where Identity:crate::inputs::OwnerIdentity<Owner>
            {
                match self.current_view() {
                    Some((end,Some((_,start)),Some(Some(ordinal)))) =>
                        Identity::identity(&self.order.output_view().owner_view()) == Identity::identity(&frame.owner_view())
                        && frame.frame_view().start_view() == start && frame.frame_view().end_view() == end
                        && frame.frame_view().ordinal_view() == ordinal && frame.frame_view().view() == self.interval_view(),
                    _ => false,
                }
            }
        }
        sail_method! {
            /// Prepares exactly the next original vector item, without advancing
            /// suppression or forgetting the preceding pending interval.
            #[allow(clippy::cast_possible_truncation, reason = "The original-item/raw-vector bound proves every actual emitted offset fits usize on the supported targets.")]
            pub fn begin_next(&mut self) -> (found:bool)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),found == (old(self).current_view().is_none() && old(self).at_view() < old(self).items_view().len()),
                    final(self).at_view() == old(self).at_view(),final(self).items_view() == old(self).items_view(),
                    final(self).order_view().view() == old(self).order_view().view(),
                    final(self).order_view().output_view() == old(self).order_view().output_view(),
                    final(self).order_view().replay_view().owner_view() == old(self).order_view().replay_view().owner_view(),
                    final(self).order_view().replay_view().served_view() == old(self).order_view().replay_view().served_view(),
                    final(self).order_view().replay_view().checked_view() == old(self).order_view().replay_view().checked_view(),
                    final(self).order_view().replay_view().diverged_view() == (old(self).order_view().replay_view().diverged_view() || old(self).current_view().is_some()),
                    final(self).order_view().forwarding_view() == old(self).order_view().forwarding_view(),
                    found ==> final(self).current_view() == Some((old(self).items_view()[old(self).at_view() as int].offset as int,
                        old(self).order_view().pending_view(),None)) && final(self).order_view().pending_view().is_none(),
                    !found ==> final(self).current_view() == old(self).current_view()
                        && final(self).order_view().pending_view() == old(self).order_view().pending_view())
            {
                if self.current.is_some() {self.order.comparison.refuse();return false;}
                if self.at == self.items.len() {return false;}
                let _retained_len=self.order.output.len();
                sail_proof! {
                    assert(crate::query_split::original_item(self.items@[self.at as int],self.order.view(),false));
                    assert(self.items@[self.at as int].offset as int <= _retained_len);
                }
                let end=self.items[self.at].offset as usize;
                let previous=self.order.pending.take();
                self.current=Some(Prepared {end,previous,frame:FrameStage::Unframed});
                true
            }
        }
        sail_method! {
            /// Actual prior classification and saturating interval charge, for
            /// the unchanged native budget/copy branch before settlement.
            pub fn previous(&self) -> (out:Option<(bool,usize)>)
                requires (self.well_formed())
                ensures (match out {
                    Some((fresh,cost)) => match self.current_view() {
                        Some((end,Some((expected,start)),_)) => fresh == expected
                            && cost as int == if start <= end {end-start}else{0int},_=>false},
                    None => self.current_view().is_none() || self.current_view().unwrap().1.is_none(),
                })
            {
                let Some(current)=&self.current else {return None;};
                let Some(previous)=&current.previous else {return None;};
                Some((previous.fresh,current.end.saturating_sub(previous.start)))
            }
        }
        sail_method! {
            /// Calls the unchanged independent frame check at the actual prior
            /// start and current original item boundary, recording its result.
            pub fn frame_previous<'b>(&'b mut self) -> (out:Option<crate::query_output_owner::StreamFrame<'b,Owner>>)
                requires (old(self).well_formed(),old(self).current_view().is_some(),
                    old(self).current_view().unwrap().1.is_some(),old(self).current_view().unwrap().2.is_none())
                ensures (final(self).well_formed(),final(self).at_view() == old(self).at_view(),
                    final(self).items_view() == old(self).items_view(),
                    final(self).order_view().view() == old(self).order_view().view(),
                    final(self).order_view().pending_view() == old(self).order_view().pending_view(),
                    final(self).order_view().replay_view() == old(self).order_view().replay_view(),
                    final(self).order_view().forwarding_view() == old(self).order_view().forwarding_view(),
                    final(self).order_view().output_view().owner_view() == old(self).order_view().output_view().owner_view(),
                    final(self).order_view().output_view().replay_view() == old(self).order_view().output_view().replay_view(),
                    final(self).order_view().output_view().parser_view() == old(self).order_view().output_view().parser_view(),
                    final(self).order_view().output_view().output_view().eof() == old(self).order_view().output_view().output_view().eof(),
                    final(self).current_view().is_some() && final(self).current_view().unwrap().0 == old(self).current_view().unwrap().0
                        && final(self).current_view().unwrap().1 == old(self).current_view().unwrap().1,
                    out.is_some() == (old(self).order_view().output_view().output_view().healthy()
                        && old(self).current_view().unwrap().1.unwrap().1 == old(self).order_view().output_view().output_view().consumed()
                        && 0 <= old(self).current_view().unwrap().1.unwrap().1 < old(self).current_view().unwrap().0 <= old(self).order_view().view().len()
                        && crate::query::response_shape(old(self).interval_view())),
                    out.is_some() ==> final(self).current_view().unwrap().2
                        == Some(Some(old(self).order_view().output_view().output_view().endpoints().len() as int))
                        && out.unwrap().frame_view().view() == old(self).interval_view()
                        && *out.unwrap().owner_view() == old(self).order_view().output_view().owner_view()
                        && out.unwrap().frame_view().start_view() == old(self).current_view().unwrap().1.unwrap().1
                        && out.unwrap().frame_view().end_view() == old(self).current_view().unwrap().0
                        && out.unwrap().frame_view().ordinal_view() == old(self).order_view().output_view().output_view().endpoints().len()
                        && final(self).order_view().output_view().output_view().endpoints()
                            == old(self).order_view().output_view().output_view().endpoints().push(old(self).current_view().unwrap().0 as usize)
                        && final(self).order_view().output_view().output_view().healthy(),
                    out.is_none() ==> final(self).order_view().output_view().output_view().endpoints() == old(self).order_view().output_view().output_view().endpoints()
                        && !final(self).order_view().output_view().output_view().healthy(),
                    out.is_none() ==> final(self).current_view().unwrap().2 == Some(None))
            {
                sail_proof! {
                    crate::query_output::transcript_partition_facts(&self.order.output_view().output_view());
                }
                let Some(current)=&mut self.current else {return None;};
                let Some(previous)=&current.previous else {return None;};
                let ordinal=self.order.output.frames();
                current.frame=FrameStage::Refused;
                let frame=self.order.output.frame(previous.start,current.end);
                if frame.is_some() {current.frame=FrameStage::Admitted(ordinal);}
                frame
            }
        }
        sail_method! {
            /// Rechecks a copied payload against this exact admitted original
            /// owner/ordinal/range and raw bytes before actual queue insertion.
            /// The additional linear byte comparison has unmeasured runtime cost.
            pub fn payload_matches<Identity>(&self,frame:&crate::query_output_owner::OwnedFrame<Owner>,identity:&Identity) -> (matched:bool)
                where (Identity:crate::inputs::OwnerIdentity<Owner>)
                requires (self.well_formed())
                ensures (matched == self.frame_matches::<Identity>(*frame))
            {
                let Some(current)=&self.current else {return false;};
                let Some(previous)=&current.previous else {return false;};
                let FrameStage::Admitted(ordinal)=&current.frame else {return false;};
                let payload=frame.payload();
                if payload.start() != previous.start || payload.end() != current.end || payload.ordinal() != *ordinal
                    || !identity.same_owner(self.order.output.owner(),frame.owner()) {return false;}
                let bytes=self.order.output.bytes();
                if previous.start > current.end || current.end > bytes.len() {return false;}
                crate::inputs::byte_eq(frame.bytes(),&bytes[previous.start..current.end])
            }
        }
        sail_impl_spec! {
            pub closed spec fn handed_payload<Identity>(&self,payload:Option<crate::query_output_owner::OwnedFrame<Owner>>)
                -> Option<crate::query_output_owner::OwnedFrame<Owner>>
                where Identity:crate::inputs::OwnerIdentity<Owner>
            {
                match (self.current_view(),payload) {
                    (Some((_,Some((true,_)),_)),Some(frame)) => if self.frame_matches::<Identity>(frame) {Some(frame)}else{None},
                    _ => None,
                }
            }
            pub closed spec fn must_stop<Identity>(&self,payload:Option<crate::query_output_owner::OwnedFrame<Owner>>) -> bool
                where Identity:crate::inputs::OwnerIdentity<Owner>
            {
                self.ready_to_settle() && match self.current_view() {
                    Some((_,Some((fresh,_)),stage)) => stage == Some(None)
                        || fresh && self.handed_payload::<Identity>(payload).is_none(),
                    _ => false,
                }
            }
            pub closed spec fn suppressed_previous(&self) -> bool {
                match self.current_view() {Some((_,Some((false,_)),_))=>true,_=>false}
            }
        }
        sail_method! {
            /// Settles the actual prepared interval, then starts exactly this
            /// original item. It cannot compare caller-selected bytes/classification.
            /// Fresh handover uses the real correlated queue operations; invalid
            /// payload bindings stop pairing without changing forwarded data.
            pub fn settle_current<E,Identity>(&mut self,pending:&mut crate::query::CorrelatedQueue<E,crate::query_output_owner::OwnedFrame<Owner>>,
                payload:Option<crate::query_output_owner::OwnedFrame<Owner>>,identity:&Identity) -> (continued:bool)
                where (Identity:crate::inputs::OwnerIdentity<Owner>)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),continued == old(self).comparison_accepts(),
                    final(self).order_view().view() == old(self).order_view().view(),
                    final(self).items_view() == old(self).items_view(),
                    final(self).order_view().output_view().owner_view() == old(self).order_view().output_view().owner_view(),
                    final(self).order_view().output_view().parser_view() == old(self).order_view().output_view().parser_view(),
                    final(self).order_view().output_view().replay_view().original() == old(self).order_view().output_view().replay_view().original(),
                    final(self).order_view().output_view().replay_view().left() == if continued && old(self).order_view().output_view().replay_view().left()>0 {
                        (old(self).order_view().output_view().replay_view().left()-1) as nat
                    } else {old(self).order_view().output_view().replay_view().left()},
                    final(self).order_view().output_view().output_view() == old(self).order_view().output_view().output_view(),
                    final(self).order_view().replay_view().owner_view() == old(self).order_view().replay_view().owner_view(),
                    final(self).order_view().replay_view().served_view() == old(self).order_view().replay_view().served_view(),
                    final(self).order_view().replay_view().diverged_view() == !continued,
                    final(self).order_view().replay_view().checked_view() == old(self).order_view().replay_view().checked_view()
                        + if old(self).suppressed_previous() && continued {1nat}else{0nat},
                    final(pending).expectations() == old(pending).expectations(),
                    final(pending).halted() == (old(pending).halted() || old(self).must_stop::<Identity>(payload)),
                    final(pending).responses() == if !old(pending).halted() && old(self).handed_payload::<Identity>(payload).is_some() {
                        old(pending).responses().push(old(self).handed_payload::<Identity>(payload).unwrap())
                    } else {old(pending).responses()},
                    continued ==> final(self).at_view() == old(self).at_view()+1 && final(self).current_view().is_none()
                        && final(self).order_view().coherent()
                        && final(self).order_view().pending_view() == Some((old(self).order_view().output_view().replay_view().left()==0,
                            old(self).current_view().unwrap().0))
                        && final(self).order_view().forwarding_view() == if old(self).order_view().output_view().replay_view().left()==0
                            && old(self).order_view().forwarding_view().is_none() {
                                Some(old(self).current_view().unwrap().0 as usize)
                            } else {old(self).order_view().forwarding_view()},
                    !continued ==> final(self).at_view() == old(self).at_view()
                        && final(self).current_view() == old(self).current_view()
                        && final(self).order_view().pending_view() == (if old(self).current_view().is_some() {None}else{old(self).order_view().pending_view()})
                        && final(self).order_view().forwarding_view() == old(self).order_view().forwarding_view())
            {
                let _items_len=self.items.len();
                sail_proof! {assert(self.current_view().is_some() ==> self.at < _items_len);}
                let Some(current)=&self.current else {self.order.comparison.refuse();return false;};
                let end=current.end;
                if let Some(previous)=&current.previous {
                    if matches!(&current.frame,FrameStage::Unframed) {self.order.comparison.refuse();return false;}
                    if matches!(&current.frame,FrameStage::Refused) {pending.stop();}
                    if previous.fresh {
                        match payload {
                            Some(frame) => {
                                if !pending.is_stopped() {
                                    if self.payload_matches(&frame,identity) {pending.reply(frame);}
                                    else {pending.stop();}
                                }
                            }
                            None => pending.stop(),
                        }
                    } else {
                        let start=previous.start;
                        let bytes=self.order.output.bytes();
                        let actual=if start <= end && end <= bytes.len() {&bytes[start..end]}else{&[]};
                        sail_proof! {assert(actual@ == self.interval_view());}
                        if self.order.comparison.next_statistics() && !crate::query_split::single_response(actual) {
                            self.order.comparison.refuse();
                        } else {
                            self.order.comparison.compare(actual);
                        }
                    }
                }
                if self.order.comparison.diverged() {return false;}
                sail_proof! {assert(self.at < _items_len);}
                let fresh=self.order.output.advance_replay();
                if fresh && self.order.forward_from.is_none() {self.order.forward_from=Some(end);}
                self.order.pending=Some(Pending {fresh,start:end});
                self.current=None;
                self.at+=1;
                true
            }
        }
        sail_method! {
            /// Returns this same owner after the actual vector loop. Dropping a
            /// not-yet-consumed prefix without an actual replay refusal latches
            /// refusal; normal native iteration and divergent breaks are unchanged.
            pub fn finish(mut self) -> (out:OutputOrder<'a,Owner>)
                requires (self.well_formed())
                ensures (out.coherent(),out.view() == self.order_view().view(),
                    out.output_view() == self.order_view().output_view(),
                    out.pending_view() == self.order_view().pending_view(),out.forwarding_view() == self.order_view().forwarding_view(),
                    out.replay_view().owner_view() == self.order_view().replay_view().owner_view(),
                    out.replay_view().served_view() == self.order_view().replay_view().served_view(),
                    out.replay_view().checked_view() == self.order_view().replay_view().checked_view(),
                    out.replay_view().diverged_view() == (self.order_view().replay_view().diverged_view()
                        || self.at_view()!=self.items_view().len() || self.current_view().is_some()))
            {
                if self.at != self.items.len() || self.current.is_some() {self.order.comparison.refuse();}
                self.order
            }
        }
    }
}

sail_spec! {
    proof fn control_lines()
        ensures
            crate::query_split::model_scan((0u8,Seq::<u8>::empty(),0int),false,0,b"unsat\n"@)
                == ((0u8,Seq::<u8>::empty(),0int),seq![(crate::query_split::ItemKind::Atom,0int,b"unsat"@)]),
            crate::query_split::model_scan((0u8,Seq::<u8>::empty(),0int),false,6,b"sat\nsat\n"@)
                == ((0u8,Seq::<u8>::empty(),0int),seq![(crate::query_split::ItemKind::Atom,6int,b"sat"@),
                    (crate::query_split::ItemKind::Atom,10int,b"sat"@)]),
            b"unsat\nsat\nsat\n"@ =~= b"unsat\n"@+b"sat\nsat\n"@,
            crate::query::response_shape(b"unsat\n"@),crate::query::response_shape(b"sat\n"@),
    {
        reveal_byteslit(b"unsat\n");reveal_byteslit(b"unsat");reveal_byteslit(b"sat\n");reveal_byteslit(b"sat");
        reveal_byteslit(b"sat\nsat\n");reveal_byteslit(b"unsat\nsat\nsat\n");
        assert(b"unsat\nsat\nsat\n"@ =~= b"unsat\n"@+b"sat\nsat\n"@);
        assert(crate::query::response_trim(b"unsat\n"@,0,5));
        assert(crate::query::response_span(b"unsat\n"@,0,5));
        assert(crate::query::response_trim(b"sat\n"@,0,3));
        assert(crate::query::response_span(b"sat\n"@,0,3));
        assert(crate::query_split::model_scan((0u8,Seq::<u8>::empty(),0int),false,0,b"unsat\n"@)
            == ((0u8,Seq::<u8>::empty(),0int),seq![(crate::query_split::ItemKind::Atom,0int,b"unsat"@)])) by {
            reveal(crate::query_split::model_step);reveal(crate::query_split::idle_model);
            reveal(crate::query_split::atom_ends);reveal(crate::query_split::terminated_atom);
            assert(seq![b'u'].push(b'n').push(b's').push(b'a').push(b't') =~= b"unsat"@);
            reveal_with_fuel(crate::query_split::model_scan,7);
        }
        assert(crate::query_split::model_scan((0u8,Seq::<u8>::empty(),0int),false,6,b"sat\nsat\n"@)
            == ((0u8,Seq::<u8>::empty(),0int),seq![(crate::query_split::ItemKind::Atom,6int,b"sat"@),
                (crate::query_split::ItemKind::Atom,10int,b"sat"@)])) by {
            reveal(crate::query_split::model_step);reveal(crate::query_split::idle_model);
            reveal(crate::query_split::atom_ends);reveal(crate::query_split::terminated_atom);
            assert(seq![b's'].push(b'a').push(b't') =~= b"sat"@);
            reveal_with_fuel(crate::query_split::model_scan,9);
        }
    }
}

sail_verified! {
    #[allow(clippy::type_complexity, reason = "The constructive fixture returns its actual owned controller and real queue together so the subsequent fresh handover exercises both without an abstract replacement.")]
    fn prepared_fresh_control<'a,Identity>(served:&'a [crate::query_replay::ServedReply],identity:&Identity)
        -> (out:Option<(OutputBatch<'a,u64>,crate::query::CorrelatedQueue<u8,crate::query_output_owner::OwnedFrame<u64>>)>)
        where (Identity:crate::inputs::OwnerIdentity<u64>)
        requires (served@.len()==1,!served@[0].statistics_view(),served@[0].bytes_view()==b"unsat\n"@)
        ensures (out.is_some(),out.unwrap().0.well_formed(),out.unwrap().0.at_view()==1,out.unwrap().0.items_view().len()==2,
            out.unwrap().0.current_view()==Some((10int,Some((true,6int)),None)),
            out.unwrap().0.order_view().view()==b"unsat\nsat\nsat\n"@,
            out.unwrap().0.order_view().output_view().owner_view()==7u64,
            out.unwrap().0.order_view().forwarding_view()==Some(6usize),
            out.unwrap().0.order_view().replay_view().complete_view(),
            out.unwrap().0.order_view().output_view().output_view().endpoints()==seq![6usize],
            out.unwrap().0.order_view().output_view().output_view().healthy(),
            !out.unwrap().0.order_view().output_view().output_view().eof(),out.unwrap().0.interval_view()==b"sat\n"@,
            !out.unwrap().1.halted(),out.unwrap().1.expectations()==seq![5u8],out.unwrap().1.responses().len()==0)
    {
        sail_proof! {control_lines();}
        let mut queue=crate::query::CorrelatedQueue::empty();queue.expect(5u8);
        let order=OutputOrder::new(served,7u64,7u64);
        let mut first=order.append_batch(b"unsat\n");
        sail_proof! {
            reveal_byteslit(b"unsat\n");reveal_byteslit(b"sat\nsat\n");
            assert(crate::query_split::item_models(first.items_view()).len()==first.items_view().len());
            assert(first.items_view().len()==1);
            assert(crate::query_split::item_models(first.items_view()) == seq![(crate::query_split::ItemKind::Atom,0int,b"unsat"@)]);
            assert(crate::query_split::item_models(first.items_view())[0] == crate::query_split::item_model(first.items_view()[0]));
            assert(first.items_view()[0].offset==0);
        }
        if !first.begin_next() || first.previous().is_some() {return None;}
        if !first.settle_current(&mut queue,None,identity) || first.begin_next() {return None;}
        let order=first.finish();
        let mut batch=order.append_batch(b"sat\nsat\n");
        sail_proof! {
            assert(crate::query_split::item_models(batch.items_view()).len()==batch.items_view().len());
            reveal_byteslit(b"unsat\nsat\nsat\n");
            assert(batch.order_view().view() =~= b"unsat\nsat\nsat\n"@);
            assert(batch.items_view().len()==2);
            assert(crate::query_split::item_models(batch.items_view())[0] == crate::query_split::item_model(batch.items_view()[0]));
            assert(crate::query_split::item_models(batch.items_view())[1] == crate::query_split::item_model(batch.items_view()[1]));
            assert(batch.items_view()[0].offset==6);
            assert(batch.items_view()[1].offset==10);
        }
        if !batch.begin_next() || batch.previous()!=Some((false,6)) {return None;}
        sail_proof! {
            crate::query_output::transcript_partition_facts(&batch.order_view().output_view().output_view());
            assert(batch.interval_view() =~= b"unsat\n"@);
        }
        batch.frame_previous()?;
        if !batch.settle_current(&mut queue,None,identity) {return None;}
        if !batch.begin_next() || batch.previous()!=Some((true,4)) {return None;}
        sail_proof! {
            crate::query_output::transcript_partition_facts(&batch.order_view().output_view().output_view());
            reveal_byteslit(b"sat\n");reveal_byteslit(b"unsat\nsat\nsat\n");
            assert(batch.interval_view() =~= b"sat\n"@);
        }
        Some((batch,queue))
    }
}

sail_verified! {
    /// Nonempty cross-chunk and multi-item replay-to-fresh transition preserves
    /// actual original intervals, real queue insertion and exact field transfer.
    pub fn output_order_constructive_control<Identity>(identity:&Identity) -> (passed:bool)
        where (Identity:crate::inputs::OwnerIdentity<u64>)
        requires (true) ensures (passed)
    {
        let served=[crate::query_replay::ServedReply::new(b"(check-sat)",crate::inputs::copy_bytes(b"unsat\n"),None)];
        let Some((mut batch,mut queue))=prepared_fresh_control(&served,identity) else {return false;};
        sail_proof! {control_lines();crate::query_output::transcript_partition_facts(&batch.order_view().output_view().output_view());}
        let Some(frame)=batch.frame_previous() else {return false;};
        let Some(payload)=frame.into_owned(7u64,identity) else {return false;};
        if !batch.settle_current(&mut queue,Some(payload),identity) || batch.begin_next() {return false;}
        let order=batch.finish();
        if order.forward_from()!=Some(6) || !order.replay_complete() || order.diverged() {return false;}
        let Some(pair)=queue.take_pair() else {return false;};
        if *pair.expectation()!=5 || !crate::inputs::byte_eq(pair.reply().bytes(),b"sat\n") {return false;}
        let (output,replay,previous,forward)=order.into_parts();
        crate::inputs::byte_eq(output.bytes(),b"unsat\nsat\nsat\n") && replay.complete()
            && previous==Some((true,10)) && forward==Some(6)
    }
}

sail_verified! {
    /// A same-owner/same-ordinal/same-range foreign payload differs only in
    /// original bytes. The actual handover guard stops pairing and forwards the
    /// real tape unchanged, rather than admitting those unrelated bytes.
    pub fn output_order_foreign_payload_refused<Identity>(identity:&Identity) -> (refused:bool)
        where (Identity:crate::inputs::OwnerIdentity<u64>)
        requires (true) ensures (refused)
    {
        let served=[crate::query_replay::ServedReply::new(b"(check-sat)",crate::inputs::copy_bytes(b"unsat\n"),None)];
        let Some((mut batch,mut queue))=prepared_fresh_control(&served,identity) else {return false;};
        sail_proof! {control_lines();crate::query_output::transcript_partition_facts(&batch.order_view().output_view().output_view());}
        if batch.frame_previous().is_none() {return false;}
        let mut other=crate::query_output_owner::StreamOutput::new(7u64);
        other.append(b"unsat\nbad\n");
        sail_proof! {
            reveal_byteslit(b"unsat\nbad\n");reveal_byteslit(b"unsat\n");reveal_byteslit(b"bad\n");reveal_byteslit(b"sat\n");
            control_lines();crate::query_output::transcript_partition_facts(&other.output_view());
            assert(b"unsat\nbad\n"@.subrange(0,6) =~= b"unsat\n"@);
        }
        if other.frame(0,6).is_none() {return false;}
        sail_proof! {
            crate::query_output::transcript_partition_facts(&other.output_view());
            assert(b"unsat\nbad\n"@.subrange(6,10) =~= b"bad\n"@);
            assert(crate::query::response_trim(b"bad\n"@,0,3));
            assert(crate::query::response_span(b"bad\n"@,0,3));
            assert(crate::query::response_shape(b"bad\n"@));
        }
        let Some(frame)=other.frame(6,10) else {return false;};
        let Some(foreign)=frame.into_owned(7u64,identity) else {return false;};
        if !batch.settle_current(&mut queue,Some(foreign),identity) || batch.begin_next() {return false;}
        let order=batch.finish();
        queue.is_stopped() && queue.take_pair().is_none() && !order.diverged()
            && order.forward_from()==Some(6) && crate::inputs::byte_eq(order.bytes(),b"unsat\nsat\nsat\n")
    }
}

sail_verified! {
    /// Real unconsumed emissions cannot be silently dropped into a clean
    /// controller, and preparing a second item cannot skip an unsettled first.
    pub fn output_order_unconsumed_refused() -> (refused:bool)
        requires (true) ensures (refused)
    {
        let served: [crate::query_replay::ServedReply;0]=[];
        let order=OutputOrder::new(&served,7u64,7u64);
        let batch=order.append_batch(b"sat\n");
        sail_proof! {
            reveal_byteslit(b"sat\n");reveal_byteslit(b"sat");
            assert(crate::query_split::model_scan((0u8,Seq::<u8>::empty(),0int),false,0,b"sat\n"@).1.len()==1) by {
                reveal(crate::query_split::model_step);reveal(crate::query_split::idle_model);
                reveal(crate::query_split::atom_ends);reveal(crate::query_split::terminated_atom);
                reveal_with_fuel(crate::query_split::model_scan,5);
            }
            assert(crate::query_split::item_models(batch.items_view()).len()==batch.items_view().len());
        }
        let order=batch.finish();
        if !order.diverged() {return false;}
        let clean=OutputOrder::new(&served,7u64,7u64);
        let mut second=clean.append_batch(b"sat\n");
        sail_proof! {assert(crate::query_split::item_models(second.items_view()).len()==second.items_view().len());}
        if !second.begin_next() || second.begin_next() {return false;}
        second.finish().diverged()
    }
}

sail_code! {
    impl<'a,Owner> OutputOrder<'a,Owner> {
        sail_method! {
            /// Selects only this owner's original pending tail on the existing
            /// nondivergent, zero-native-status branch. Physical wait stays native.
            #[allow(clippy::manual_map, reason = "Explicit pending-ownership branches expose each original interval field; the preceding whole-root Option-map trial did not establish the exact transferred-field relation without another closure contract.")]
            pub fn prepare_tail(mut self,code:u8) -> (out:OutputTail<'a,Owner>)
                requires (self.coherent())
                ensures (out.well_formed(),out.order_view().output_view() == self.output_view(),
                    out.order_view().replay_view() == self.replay_view(),
                    out.order_view().forwarding_view() == self.forwarding_view(),
                    out.tail_view().is_some() == (!self.replay_view().diverged_view() && code==0 && self.pending_view().is_some()),
                    out.tail_view().is_some() ==> out.tail_view() == Some((self.view().len() as int,self.pending_view(),None))
                        && out.order_view().pending_view().is_none(),
                    out.tail_view().is_none() ==> out.order_view().pending_view() == self.pending_view())
            {
                let current=if !self.comparison.diverged() && code==0 {
                    let end=self.output.len();
                    match self.pending.take() {
                        Some(previous)=>Some(Prepared {end,previous:Some(previous),frame:FrameStage::Unframed}),
                        None=>None,
                    }
                } else {None};
                OutputTail {order:self,current}
            }
        }
        sail_method! {
            /// Consumes this exact output owner through the existing coverage
            /// seal; it does not manufacture native EOF, exit or origin evidence.
            pub fn seal(self) -> (out:SealedOrder<'a,Owner>)
                requires (self.coherent())
                ensures (out.replay_view() == self.replay_view(),
                    out.output_view().is_some() == self.output_view().output_view().complete(),
                    out.output_view().is_some() ==> out.output_view().unwrap().complete()
                        && out.output_view().unwrap().owner_view() == self.output_view().owner_view()
                        && out.output_view().unwrap().output_view().view() == self.view()
                        && out.output_view().unwrap().output_view().endpoints() == self.output_view().output_view().endpoints()
                        && out.output_view().unwrap().replayed_view() == if self.output_view().replay_view().left()==0 {
                            Some(self.output_view().replay_view().original())
                        } else {None})
            {
                SealedOrder {output:self.output.seal(),comparison:self.comparison}
            }
        }
    }
    impl<'a,Owner> OutputTail<'a,Owner> {
        sail_impl_spec! {
            pub closed spec fn order_view(&self) -> OutputOrder<'a,Owner> {self.order}
            pub closed spec fn tail_view(&self) -> Option<(int,Option<(bool,int)>,Option<Option<int>>)> {
                match self.current {Some(c)=>Some((c.end as int,pending_view(c.previous),stage_view(c.frame))),None=>None}
            }
            pub closed spec fn tail_bytes_view(&self) -> Seq<u8> {
                match self.tail_view() {
                    Some((end,Some((_,start)),_)) => if 0 <= start <= end <= self.order.view().len() {
                        self.order.view().subrange(start,end)
                    } else {Seq::<u8>::empty()},
                    _ => Seq::<u8>::empty(),
                }
            }
            pub closed spec fn tail_shape(&self) -> bool {
                match self.tail_view() {
                    Some((end,Some((_,start)),Some(Some(ordinal)))) =>
                        0 <= ordinal < self.order.output_view().output_view().endpoints().len()
                        && 0 <= start < end <= self.order.view().len()
                        && crate::query_output::frame_start(self.order.output_view().output_view().endpoints(),ordinal) == start
                        && self.order.output_view().output_view().endpoints()[ordinal] == end
                        && crate::query::response_shape(self.tail_bytes_view()),
                    _ => true,
                }
            }

            pub closed spec fn well_formed(&self) -> bool {
                self.order.base() && self.tail_shape() && match self.tail_view() {
                    None => self.order.coherent(),
                    Some((end,previous,_)) => end == self.order.view().len()
                        && previous.is_some() && self.order.pending_view().is_none()
                        && self.order.matches_pending(previous)
                        && 0 <= previous.unwrap().1 <= self.order.view().len(),
                }
            }
            pub closed spec fn tail_ready(&self) -> bool {
                self.tail_view().is_some() && (self.tail_view().unwrap().1.is_none()
                    || self.tail_view().unwrap().2.is_some())
            }
            pub closed spec fn tail_accepts(&self) -> bool {
                let replay=self.order.replay_view();
                self.tail_ready() && !replay.diverged_view() && match self.tail_view() {
                    Some((_,Some((false,_)),_)) => replay.checked_view() < replay.served_view().len()
                        && (!replay.served_view()[replay.checked_view() as int].statistics_view()
                            || crate::query_split::split_single_response(self.tail_bytes_view()))
                        && replay.served_view()[replay.checked_view() as int].reproduces(self.tail_bytes_view()),
                    _ => true,
                }
            }
            pub closed spec fn tail_matches<Identity>(&self,frame:crate::query_output_owner::OwnedFrame<Owner>) -> bool
                where Identity:crate::inputs::OwnerIdentity<Owner>
            {
                match self.tail_view() {
                    Some((end,Some((_,start)),Some(Some(ordinal)))) =>
                        Identity::identity(&self.order.output_view().owner_view()) == Identity::identity(&frame.owner_view())
                        && frame.frame_view().start_view() == start && frame.frame_view().end_view() == end
                        && frame.frame_view().ordinal_view() == ordinal && frame.frame_view().view() == self.tail_bytes_view(),
                    _ => false,
                }
            }
            pub closed spec fn tail_payload<Identity>(&self,payload:Option<crate::query_output_owner::OwnedFrame<Owner>>)
                -> Option<crate::query_output_owner::OwnedFrame<Owner>>
                where Identity:crate::inputs::OwnerIdentity<Owner>
            {
                match (self.tail_view(),payload) {
                    (Some((_,Some((true,_)),_)),Some(frame)) => if self.tail_matches::<Identity>(frame) {Some(frame)}else{None},
                    _ => None,
                }
            }
            pub closed spec fn tail_stops<Identity>(&self,payload:Option<crate::query_output_owner::OwnedFrame<Owner>>) -> bool
                where Identity:crate::inputs::OwnerIdentity<Owner>
            {
                self.tail_ready() && match self.tail_view() {
                    Some((_,Some((fresh,_)),stage)) => stage == Some(None)
                        || fresh && self.tail_payload::<Identity>(payload).is_none(),
                    _ => false,
                }
            }
            pub closed spec fn suppressed_tail(&self) -> bool {
                match self.tail_view() {Some((_,Some((false,_)),_))=>true,_=>false}
            }
        }
        sail_method! {
            /// Actual prior classification and saturating interval charge, for
            /// the unchanged native budget/copy branch before settlement.
            pub fn pending_tail(&self) -> (out:Option<(bool,usize)>)
                requires (self.well_formed())
                ensures (match out {
                    Some((fresh,cost)) => match self.tail_view() {
                        Some((end,Some((expected,start)),_)) => fresh == expected
                            && cost as int == if start <= end {end-start}else{0int},_=>false},
                    None => self.tail_view().is_none() || self.tail_view().unwrap().1.is_none(),
                })
            {
                let Some(current)=&self.current else {return None;};
                let Some(pending_tail)=&current.previous else {return None;};
                Some((pending_tail.fresh,current.end.saturating_sub(pending_tail.start)))
            }
        }
        sail_method! {
            /// Calls the unchanged independent frame check at the actual prior
            /// start and current original item boundary, recording its result.
            pub fn frame_tail<'b>(&'b mut self) -> (out:Option<crate::query_output_owner::StreamFrame<'b,Owner>>)
                requires (old(self).well_formed(),old(self).tail_view().is_some(),
                    old(self).tail_view().unwrap().1.is_some(),old(self).tail_view().unwrap().2.is_none())
                ensures (final(self).well_formed(),
                    final(self).order_view().view() == old(self).order_view().view(),
                    final(self).order_view().pending_view() == old(self).order_view().pending_view(),
                    final(self).order_view().replay_view() == old(self).order_view().replay_view(),
                    final(self).order_view().forwarding_view() == old(self).order_view().forwarding_view(),
                    final(self).order_view().output_view().owner_view() == old(self).order_view().output_view().owner_view(),
                    final(self).order_view().output_view().replay_view() == old(self).order_view().output_view().replay_view(),
                    final(self).order_view().output_view().parser_view() == old(self).order_view().output_view().parser_view(),
                    final(self).order_view().output_view().output_view().eof() == old(self).order_view().output_view().output_view().eof(),
                    final(self).tail_view().is_some() && final(self).tail_view().unwrap().0 == old(self).tail_view().unwrap().0
                        && final(self).tail_view().unwrap().1 == old(self).tail_view().unwrap().1,
                    out.is_some() == (old(self).order_view().output_view().output_view().healthy()
                        && old(self).tail_view().unwrap().1.unwrap().1 == old(self).order_view().output_view().output_view().consumed()
                        && 0 <= old(self).tail_view().unwrap().1.unwrap().1 < old(self).tail_view().unwrap().0 <= old(self).order_view().view().len()
                        && crate::query::response_shape(old(self).tail_bytes_view())),
                    out.is_some() ==> final(self).tail_view().unwrap().2
                        == Some(Some(old(self).order_view().output_view().output_view().endpoints().len() as int))
                        && out.unwrap().frame_view().view() == old(self).tail_bytes_view()
                        && *out.unwrap().owner_view() == old(self).order_view().output_view().owner_view()
                        && out.unwrap().frame_view().start_view() == old(self).tail_view().unwrap().1.unwrap().1
                        && out.unwrap().frame_view().end_view() == old(self).tail_view().unwrap().0
                        && out.unwrap().frame_view().ordinal_view() == old(self).order_view().output_view().output_view().endpoints().len()
                        && final(self).order_view().output_view().output_view().endpoints()
                            == old(self).order_view().output_view().output_view().endpoints().push(old(self).tail_view().unwrap().0 as usize)
                        && final(self).order_view().output_view().output_view().healthy(),
                    out.is_none() ==> final(self).order_view().output_view().output_view().endpoints() == old(self).order_view().output_view().output_view().endpoints()
                        && !final(self).order_view().output_view().output_view().healthy(),
                    out.is_none() ==> final(self).tail_view().unwrap().2 == Some(None))
            {
                sail_proof! {
                    crate::query_output::transcript_partition_facts(&self.order.output_view().output_view());
                }
                let Some(current)=&mut self.current else {return None;};
                let Some(pending_tail)=&current.previous else {return None;};
                let ordinal=self.order.output.frames();
                current.frame=FrameStage::Refused;
                let frame=self.order.output.frame(pending_tail.start,current.end);
                if frame.is_some() {current.frame=FrameStage::Admitted(ordinal);}
                frame
            }
        }
        sail_method! {
            /// Rechecks a copied payload against this exact admitted original
            /// owner/ordinal/range and raw bytes before actual queue insertion.
            /// The additional linear byte comparison has unmeasured runtime cost.
            pub fn validate_tail<Identity>(&self,frame:&crate::query_output_owner::OwnedFrame<Owner>,identity:&Identity) -> (matched:bool)
                where (Identity:crate::inputs::OwnerIdentity<Owner>)
                requires (self.well_formed())
                ensures (matched == self.tail_matches::<Identity>(*frame))
            {
                let Some(current)=&self.current else {return false;};
                let Some(pending_tail)=&current.previous else {return false;};
                let FrameStage::Admitted(ordinal)=&current.frame else {return false;};
                let payload=frame.payload();
                if payload.start() != pending_tail.start || payload.end() != current.end || payload.ordinal() != *ordinal
                    || !identity.same_owner(self.order.output.owner(),frame.owner()) {return false;}
                let bytes=self.order.output.bytes();
                if pending_tail.start > current.end || current.end > bytes.len() {return false;}
                crate::inputs::byte_eq(frame.bytes(),&bytes[pending_tail.start..current.end])
            }
        }
        sail_method! {
            /// Settles the actual prepared interval, then starts exactly this
            /// original item. It cannot compare caller-selected bytes/classification.
            /// Fresh handover uses the real correlated queue operations; invalid
            /// payload bindings stop pairing without changing forwarded data.
            pub fn settle_tail<E,Identity>(&mut self,pending:&mut crate::query::CorrelatedQueue<E,crate::query_output_owner::OwnedFrame<Owner>>,
                payload:Option<crate::query_output_owner::OwnedFrame<Owner>>,identity:&Identity) -> (continued:bool)
                where (Identity:crate::inputs::OwnerIdentity<Owner>)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),continued == old(self).tail_accepts(),
                    final(self).order_view().view() == old(self).order_view().view(),
                    final(self).order_view().output_view().owner_view() == old(self).order_view().output_view().owner_view(),
                    final(self).order_view().output_view().parser_view() == old(self).order_view().output_view().parser_view(),
                    final(self).order_view().output_view().replay_view().original() == old(self).order_view().output_view().replay_view().original(),
                    final(self).order_view().output_view().replay_view() == old(self).order_view().output_view().replay_view(),
                    final(self).order_view().output_view().output_view() == old(self).order_view().output_view().output_view(),
                    final(self).order_view().replay_view().owner_view() == old(self).order_view().replay_view().owner_view(),
                    final(self).order_view().replay_view().served_view() == old(self).order_view().replay_view().served_view(),
                    final(self).order_view().replay_view().diverged_view() == !continued,
                    final(self).order_view().replay_view().checked_view() == old(self).order_view().replay_view().checked_view()
                        + if old(self).suppressed_tail() && continued {1nat}else{0nat},
                    final(pending).expectations() == old(pending).expectations(),
                    final(pending).halted() == (old(pending).halted() || old(self).tail_stops::<Identity>(payload)),
                    final(pending).responses() == if !old(pending).halted() && old(self).tail_payload::<Identity>(payload).is_some() {
                        old(pending).responses().push(old(self).tail_payload::<Identity>(payload).unwrap())
                    } else {old(pending).responses()},
                    continued ==> final(self).tail_view().is_none() && final(self).order_view().coherent()
                        && final(self).order_view().pending_view().is_none(),
                    !continued ==> final(self).tail_view() == old(self).tail_view(),
                    final(self).order_view().pending_view() == old(self).order_view().pending_view(),
                    final(self).order_view().forwarding_view() == old(self).order_view().forwarding_view())
            {
                let Some(current)=&self.current else {self.order.comparison.refuse();return false;};
                let end=current.end;
                if let Some(pending_tail)=&current.previous {
                    if matches!(&current.frame,FrameStage::Unframed) {self.order.comparison.refuse();return false;}
                    if matches!(&current.frame,FrameStage::Refused) {pending.stop();}
                    if pending_tail.fresh {
                        match payload {
                            Some(frame) => {
                                if !pending.is_stopped() {
                                    if self.validate_tail(&frame,identity) {pending.reply(frame);}
                                    else {pending.stop();}
                                }
                            }
                            None => pending.stop(),
                        }
                    } else {
                        let start=pending_tail.start;
                        let bytes=self.order.output.bytes();
                        let actual=if start <= end && end <= bytes.len() {&bytes[start..end]}else{&[]};
                        sail_proof! {assert(actual@ == self.tail_bytes_view());}
                        if self.order.comparison.next_statistics() && !crate::query_split::single_response(actual) {
                            self.order.comparison.refuse();
                        } else {
                            self.order.comparison.compare(actual);
                        }
                    }
                }
                if self.order.comparison.diverged() {return false;}
                self.current=None;
                true
            }
        }
        sail_method! {
            /// Returns the same controller after settlement. Skipping an active
            /// tail permanently refuses comparison, without altering raw bytes.
            pub fn finish(mut self) -> (out:OutputOrder<'a,Owner>)
                requires (self.well_formed())
                ensures (out.coherent(),out.output_view() == self.order_view().output_view(),
                    out.pending_view() == self.order_view().pending_view(),
                    out.forwarding_view() == self.order_view().forwarding_view(),
                    out.replay_view().owner_view() == self.order_view().replay_view().owner_view(),
                    out.replay_view().served_view() == self.order_view().replay_view().served_view(),
                    out.replay_view().checked_view() == self.order_view().replay_view().checked_view(),
                    out.replay_view().diverged_view() == (self.order_view().replay_view().diverged_view() || self.tail_view().is_some()))
            {
                if self.current.is_some() {self.order.comparison.refuse();}
                self.order
            }
        }
    }
    #[allow(clippy::elidable_lifetime_names, reason = "The named lifetime retains the original served-slice type in the checked replay_view specification; Cargo erases that specification but Stock checks its exact borrowed-data relation.")]
    impl<'a,Owner> SealedOrder<'a,Owner> {
        sail_impl_spec! {
            pub closed spec fn output_view(&self) -> Option<crate::query_output_owner::CompletedStream<Owner>> {self.output}
            pub closed spec fn replay_view(&self) -> crate::query_replay::Replay<'a,Owner> {self.comparison}
            pub closed spec fn can_join<Identity>(&self) -> bool where Identity:crate::inputs::OwnerIdentity<Owner> {
                self.output_view().is_some() && self.replay_view().complete_view()
                    && self.output_view().unwrap().replayed_view() == Some(self.replay_view().served_view().len())
                    && Identity::identity(&self.output_view().unwrap().owner_view()) == Identity::identity(&self.replay_view().owner_view())
                    && self.output_view().unwrap().replay_prefix_matches(&self.replay_view())
            }
        }
        sail_method! {
            /// Coverage presence for the original native stop-pairing branch.
            pub fn has_output(&self) -> (out:bool) requires (true) ensures (out == self.output_view().is_some())
            {self.output.is_some()}
        }
        sail_method! {
            /// Actual comparison index retained for the unchanged diagnostic.
            pub fn checked(&self) -> (out:usize) requires (true) ensures (out == self.replay_view().checked_view())
            {self.comparison.checked()}
        }
        sail_method! {
            /// Applies the existing mechanical finish rule to the actual replay.
            /// `clean` remains the native forwarding/status observation, not origin.
            pub fn finish_replay(&mut self,clean:bool) -> (diverged:bool)
                requires (old(self).replay_view().well_formed())
                ensures (final(self).output_view() == old(self).output_view(),
                    final(self).replay_view().well_formed(),
                    final(self).replay_view().owner_view() == old(self).replay_view().owner_view(),
                    final(self).replay_view().served_view() == old(self).replay_view().served_view(),
                    final(self).replay_view().checked_view() == old(self).replay_view().checked_view(),
                    diverged == (old(self).replay_view().diverged_view()
                        || clean && old(self).replay_view().checked_view() < old(self).replay_view().served_view().len()),
                    final(self).replay_view().diverged_view() == diverged)
            {self.comparison.finish(clean)}
        }
        sail_method! {
            /// Rechecks the original suppressed raw prefix using the existing
            /// complete-stream join, after the original finish/diagnostic branch.
            pub fn join_replay<Identity>(self,identity:&Identity) -> (out:Option<crate::query_output_owner::CompletedStream<Owner>>)
                where (Identity:crate::inputs::OwnerIdentity<Owner>)
                requires (true)
                ensures (out.is_some() == self.can_join::<Identity>(),out.is_some() ==> out == self.output_view())
            {
                match self.output {
                    Some(output)=>output.with_replay(&self.comparison,identity),
                    None=>None,
                }
            }
        }
    }
}

sail_verified! {
    fn terminal_fixture<'a,Identity>(served:&'a [crate::query_replay::ServedReply],identity:&Identity)
        -> (out:Option<OutputOrder<'a,u64>>)
        where (Identity:crate::inputs::OwnerIdentity<u64>)
        requires (served@.len()<=1,served@.len()==1 ==> !served@[0].statistics_view() && served@[0].bytes_view()==b"unsat\n"@)
        ensures (out.is_some(),out.unwrap().coherent(),out.unwrap().view()==b"unsat\n"@,
            out.unwrap().output_view().owner_view()==7u64,out.unwrap().output_view().output_view().healthy(),
            !out.unwrap().output_view().output_view().eof(),out.unwrap().output_view().output_view().endpoints().len()==0,
            out.unwrap().output_view().replay_view().original()==served@.len(),out.unwrap().output_view().replay_view().left()==0,
            out.unwrap().replay_view().owner_view()==7u64,out.unwrap().replay_view().served_view()==served@,
            out.unwrap().replay_view().checked_view()==0,!out.unwrap().replay_view().diverged_view(),
            out.unwrap().pending_view()==Some((served@.len()==0,0int)))
    {
        sail_proof! {control_lines();reveal_byteslit(b"unsat\n");}
        let order=OutputOrder::new(served,7u64,7u64);
        let mut batch=order.append_batch(b"unsat\n");
        let mut queue:crate::query::CorrelatedQueue<u8,crate::query_output_owner::OwnedFrame<u64>>=crate::query::CorrelatedQueue::empty();
        sail_proof! {
            assert(crate::query_split::item_models(batch.items_view()).len()==batch.items_view().len());
            assert(batch.items_view().len()==1);
            assert(crate::query_split::item_models(batch.items_view())[0] == crate::query_split::item_model(batch.items_view()[0]));
            assert(batch.items_view()[0].offset==0);
        }
        if !batch.begin_next() || batch.previous().is_some() {return None;}
        if !batch.settle_current(&mut queue,None,identity) || batch.begin_next() {return None;}
        Some(batch.finish())
    }
}

sail_verified! {
    /// A nonempty original served reply is settled from the actual final raw
    /// interval and reaches the existing complete-prefix recheck after EOF.
    pub fn output_tail_replay_constructive<Identity>(identity:&Identity) -> (passed:bool)
        where (Identity:crate::inputs::OwnerIdentity<u64>)
        requires (true) ensures (passed)
    {
        let served=[crate::query_replay::ServedReply::new(b"(check-sat)",crate::inputs::copy_bytes(b"unsat\n"),None)];
        let Some(mut order)=terminal_fixture(&served,identity) else {return false;};
        order.end();
        let mut tail=order.prepare_tail(0);
        sail_proof! {
            control_lines();reveal_byteslit(b"unsat\n");
            crate::query_output::transcript_partition_facts(&tail.order_view().output_view().output_view());
            assert(tail.tail_bytes_view() =~= b"unsat\n"@);
        }
        if tail.pending_tail()!=Some((false,6)) || tail.frame_tail().is_none() {return false;}
        let mut queue:crate::query::CorrelatedQueue<u8,crate::query_output_owner::OwnedFrame<u64>>=crate::query::CorrelatedQueue::empty();
        if !tail.settle_tail(&mut queue,None,identity) {return false;}
        sail_proof! {crate::query_output::transcript_partition_facts(&tail.order_view().output_view().output_view());}
        let mut sealed=tail.finish().seal();
        if !sealed.has_output() || sealed.finish_replay(true) || sealed.checked()!=1 {return false;}
        sail_proof! {
            let output=sealed.output_view().unwrap();
            assert(output.output_view().view() =~= b"unsat\n"@);
            assert(output.output_view().view().subrange(0,6) =~= b"unsat\n"@);
            assert forall|i:int| 0<=i<sealed.replay_view().served_view().len()
                implies output.replay_frame_matches(&sealed.replay_view(),i) by {assert(i==0);}
        }
        sealed.join_replay(identity).is_some() && queue.is_empty() && !queue.is_stopped()
    }
}

sail_verified! {
    /// A nonempty fresh tail enters the real correlated queue with its exact
    /// original owner/range/bytes before the same coverage and replay join.
    pub fn output_tail_fresh_constructive<Identity>(identity:&Identity) -> (passed:bool)
        where (Identity:crate::inputs::OwnerIdentity<u64>)
        requires (true) ensures (passed)
    {
        let served:[crate::query_replay::ServedReply;0]=[];
        let Some(mut order)=terminal_fixture(&served,identity) else {return false;};
        order.end();
        let mut tail=order.prepare_tail(0);
        sail_proof! {
            control_lines();reveal_byteslit(b"unsat\n");
            crate::query_output::transcript_partition_facts(&tail.order_view().output_view().output_view());
            assert(tail.tail_bytes_view() =~= b"unsat\n"@);
        }
        if tail.pending_tail()!=Some((true,6)) {return false;}
        let Some(frame)=tail.frame_tail() else {return false;};
        let Some(payload)=frame.into_owned(7u64,identity) else {return false;};
        let mut queue=crate::query::CorrelatedQueue::empty();queue.expect(9u8);
        if !tail.settle_tail(&mut queue,Some(payload),identity) {return false;}
        let Some(pair)=queue.take_pair() else {return false;};
        if *pair.expectation()!=9 || !crate::inputs::byte_eq(pair.reply().bytes(),b"unsat\n") {return false;}
        sail_proof! {crate::query_output::transcript_partition_facts(&tail.order_view().output_view().output_view());}
        let mut sealed=tail.finish().seal();
        if !sealed.has_output() || sealed.finish_replay(true) {return false;}
        sealed.join_replay(identity).is_some()
    }
}

sail_verified! {
    /// Missing actual EOF, nonzero native status and an unsettled original
    /// tail cannot complete the terminal join. Failed status alone does not
    /// invent a replay mismatch or change the existing clean-finish policy.
    pub fn output_tail_refusal_control<Identity>(identity:&Identity) -> (refused:bool)
        where (Identity:crate::inputs::OwnerIdentity<u64>)
        requires (true) ensures (refused)
    {
        let served=[crate::query_replay::ServedReply::new(b"(check-sat)",crate::inputs::copy_bytes(b"unsat\n"),None)];
        let Some(order)=terminal_fixture(&served,identity) else {return false;};
        let mut tail=order.prepare_tail(0);
        sail_proof! {
            control_lines();reveal_byteslit(b"unsat\n");
            crate::query_output::transcript_partition_facts(&tail.order_view().output_view().output_view());
            assert(tail.tail_bytes_view() =~= b"unsat\n"@);
        }
        if tail.frame_tail().is_none() {return false;}
        let mut queue:crate::query::CorrelatedQueue<u8,crate::query_output_owner::OwnedFrame<u64>>=crate::query::CorrelatedQueue::empty();
        if !tail.settle_tail(&mut queue,None,identity) {return false;}
        sail_proof! {crate::query_output::transcript_partition_facts(&tail.order_view().output_view().output_view());}
        let mut sealed=tail.finish().seal();
        if sealed.has_output() || sealed.finish_replay(true) || sealed.join_replay(identity).is_some() {return false;}
        let Some(mut order)=terminal_fixture(&served,identity) else {return false;};
        order.end();
        let tail=order.prepare_tail(1);
        if tail.pending_tail().is_some() {return false;}
        sail_proof! {crate::query_output::transcript_partition_facts(&tail.order_view().output_view().output_view());}
        let mut sealed=tail.finish().seal();
        if sealed.has_output() || sealed.finish_replay(false) || sealed.join_replay(identity).is_some() {return false;}
        let Some(mut order)=terminal_fixture(&served,identity) else {return false;};
        order.end();
        let tail=order.prepare_tail(0);
        sail_proof! {crate::query_output::transcript_partition_facts(&tail.order_view().output_view().output_view());}
        let mut sealed=tail.finish().seal();
        !sealed.has_output() && sealed.finish_replay(false) && sealed.join_replay(identity).is_none()
    }
}
