//! Opaque stream-lease preservation around the actual output controller.
//! Native lease creation and endpoint attribution remain narrow OS contracts.
use crate::inputs::OwnerIdentity;
use crate::key_prefix::HashHost;
use crate::query::{PublicationBatch, PublicationGate, ReplayCursor, ReplayedPrefix};
use crate::query_delivery::Delivered;
use crate::query_output::{CompleteOutput, Frame, FramePayload, Transcript};
use crate::query_publication::{PublicationObservation, observe_publication, release_observed};
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// One actual stdout controller and its original opaque ownership lease.
    pub struct StreamOutput<Owner> { owner: Owner, output: Transcript, replay: ReplayCursor }
    /// A frame still borrowing its original controller and ownership lease.
    pub struct StreamFrame<'a, Owner> { owner: &'a Owner, frame: Frame<'a> }
    /// Original response bytes/position and a retained same-owner lease.
    pub struct OwnedFrame<Owner> { owner: Owner, frame: FramePayload }
    /// Original complete EOF/coverage observation with the same stream lease.
    pub struct CompletedStream<Owner> { owner: Owner, output: CompleteOutput, replayed: Option<ReplayedPrefix> }
    /// Typed joint publication retaining original input/output ownership.
    /// This does not authenticate a runtime, arbitrary writer or restart data.
    pub struct OwnedPublication<Host, Owner, Identity> {
        publication: PublicationObservation<Host>,
        input_owner: Owner,
        output_owner: Owner,
        identity: Identity,
        replayed: Option<ReplayedPrefix>,
    }
}

sail_code! {
    impl<Owner> StreamOutput<Owner> {
        sail_impl_spec! {
            pub closed spec fn owner_view(&self) -> Owner { self.owner }
            pub closed spec fn output_view(&self) -> Transcript { self.output }
            pub closed spec fn replay_view(&self) -> ReplayCursor { self.replay }
        }
        sail_method! {
            /// Starts before reads, around the actual endpoint's supplied lease.
            pub fn new(owner: Owner) -> (out: Self)
                requires (true)
                ensures (out.owner_view() == owner, out.output_view().well_formed(),
                    out.output_view().healthy(), !out.output_view().eof(), out.output_view().view().len() == 0,
                    out.output_view().endpoints().len() == 0,
                    out.replay_view().original() == 0, out.replay_view().left() == 0)
            { Self::new_replayed(owner,0) }
        }
        sail_method! {
            /// Creates the actual output controller and suppression cursor
            /// together, before any response is observed.
            pub fn new_replayed(owner: Owner, served: u64) -> (out: Self)
                requires (true)
                ensures (out.owner_view() == owner, out.output_view().well_formed(),
                    out.output_view().healthy(), !out.output_view().eof(), out.output_view().view().len() == 0,
                    out.output_view().endpoints().len() == 0,
                    out.replay_view().original() == served, out.replay_view().left() == served)
            { Self { owner, output:Transcript::new(), replay:ReplayCursor::new(served) } }
        }
        sail_method! {
            /// Original forwarding decision from this stdout owner's cursor.
            pub fn forwarding(&self) -> (out: bool) requires (true)
                ensures (out == (self.replay_view().left() == 0))
            { self.replay.forwarding() }
        }
        sail_method! {
            /// Advances the same actual replay cursor at each original parser
            /// response boundary. Byte comparison remains separately required.
            pub fn advance_replay(&mut self) -> (out: bool) requires (true)
                ensures (final(self).owner_view() == old(self).owner_view(),
                    final(self).output_view() == old(self).output_view(),
                    final(self).replay_view().original() == old(self).replay_view().original(),
                    out == (old(self).replay_view().left() == 0),
                    final(self).replay_view().left() == if old(self).replay_view().left() == 0 {0nat}
                        else {(old(self).replay_view().left()-1) as nat})
            { self.replay.advance() }
        }
        sail_method! {
            /// Appends exactly the original read chunk through the same controller.
            pub fn append(&mut self, bytes: &[u8]) -> (out: ())
                requires (old(self).output_view().well_formed())
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), final(self).output_view().well_formed(),
                    final(self).output_view().view() == old(self).output_view().view() + bytes@,
                    final(self).output_view().endpoints() == old(self).output_view().endpoints(),
                    final(self).output_view().eof() == old(self).output_view().eof(),
                    final(self).output_view().healthy() == (old(self).output_view().healthy() && !old(self).output_view().eof()))
            { self.output.append(bytes); }
        }
        sail_method! {
            /// Marks the original endpoint's actual EOF without replacing it.
            pub fn end(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), final(self).output_view().eof(),
                    final(self).output_view().view() == old(self).output_view().view(),
                    final(self).output_view().endpoints() == old(self).output_view().endpoints(),
                    final(self).output_view().healthy() == (old(self).output_view().healthy() && !old(self).output_view().eof()),
                    final(self).output_view().well_formed() == old(self).output_view().well_formed())
            { self.output.end(); }
        }
        sail_method! {
            /// Permanent failure keeps original owner and byte observations.
            pub fn refuse(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), !final(self).output_view().healthy(),
                    final(self).output_view().view() == old(self).output_view().view(),
                    final(self).output_view().endpoints() == old(self).output_view().endpoints(),
                    final(self).output_view().eof() == old(self).output_view().eof(),
                    final(self).output_view().well_formed() == old(self).output_view().well_formed())
            { self.output.refuse(); }
        }
        sail_method! {
            /// Releases recording storage on the unchanged streaming fallback.
            pub fn discard(&mut self) -> (out: ())
                requires (true)
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), final(self).output_view().well_formed(),
                    !final(self).output_view().healthy(), final(self).output_view().view().len() == 0)
            { self.output.discard(); }
        }
        sail_method! {
            /// Borrows the retained original stdout bytes.
            pub fn bytes(&self) -> (out: &[u8]) requires (true)
                ensures (out@ == self.output_view().view()) { self.output.bytes() }
        }
        sail_method! {
            /// Original buffer length, with unchanged accounting.
            pub fn len(&self) -> (out: usize) requires (true)
                ensures (out == self.output_view().view().len()) { self.output.len() }
        }
        sail_method! {
            /// Original admitted frame count, with unchanged buffer accounting.
            pub fn frames(&self) -> (out: usize) requires (true)
                ensures (out == self.output_view().endpoints().len()) { self.output.frames() }
        }
        sail_method! {
            /// Preserves the original empty-buffer query.
            pub fn is_empty(&self) -> (out: bool) requires (true)
                ensures (out == (self.output_view().view().len() == 0)) { self.output.is_empty() }
        }
        sail_method! {
            /// Borrows the actual next admitted interval and its original lease.
            #[allow(clippy::question_mark, reason = "Explicit refusal exits retain the shared stock-Verus contracts and full-body mutation controls.")]
            pub fn frame<'a>(&'a mut self, start: usize, end: usize) -> (out: Option<StreamFrame<'a, Owner>>)
                requires (old(self).output_view().well_formed())
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).replay_view() == old(self).replay_view(), final(self).output_view().well_formed(),
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
            {
                let Some(frame) = self.output.frame(start,end) else { return None; };
                Some(StreamFrame { owner:&self.owner, frame })
            }
        }
        sail_method! {
            /// Consumes the exact original controller and retains its owner.
            #[allow(clippy::question_mark, reason = "Explicit refusal exits retain the shared stock-Verus contracts and full-body mutation controls.")]
            pub fn seal(self) -> (out: Option<CompletedStream<Owner>>)
                requires (self.output_view().well_formed())
                ensures (out.is_some() == self.output_view().complete(),
                    out.is_some() ==> out.unwrap().complete() && out.unwrap().owner_view() == self.owner_view()
                        && out.unwrap().output_view().view() == self.output_view().view()
                        && out.unwrap().output_view().endpoints() == self.output_view().endpoints()
                        && out.unwrap().replayed_view() == if self.replay_view().left() == 0
                            { Some(self.replay_view().original()) } else {None})
            {
                let replayed=self.replay.completed();
                let Some(output) = self.output.seal() else { return None; };
                Some(CompletedStream { owner:self.owner, output, replayed })
            }
        }
    }
    #[allow(clippy::elidable_lifetime_names, reason = "The named lifetime binds the proof-only owner selector to the original borrowed stream.")]
    impl<'a, Owner> StreamFrame<'a, Owner> {
        sail_impl_spec! {
            pub closed spec fn owner_view(&self) -> &'a Owner { self.owner }
            pub closed spec fn frame_view(&self) -> Frame<'a> { self.frame }
        }
        sail_method! {
            /// Copies only after the retained opaque lease matches this borrow.
            pub fn into_owned<Identity>(self, owner: Owner, identity: &Identity) -> (out: Option<OwnedFrame<Owner>>)
                where (Identity: OwnerIdentity<Owner>)
                requires (true)
                ensures (out.is_some() == (Identity::identity(&owner) == Identity::identity(self.owner_view())),
                    out.is_some() ==> out.unwrap().owner_view() == owner
                        && out.unwrap().frame_view().view() == self.frame_view().view()
                        && out.unwrap().frame_view().ordinal_view() == self.frame_view().ordinal_view()
                        && out.unwrap().frame_view().start_view() == self.frame_view().start_view()
                        && out.unwrap().frame_view().end_view() == self.frame_view().end_view())
            {
                if !identity.same_owner(&owner,self.owner) { return None; }
                Some(OwnedFrame { owner, frame:self.frame.into_owned() })
            }
        }
    }
    impl<Owner> OwnedFrame<Owner> {
        sail_impl_spec! {
            pub closed spec fn owner_view(&self) -> Owner { self.owner }
            pub closed spec fn frame_view(&self) -> FramePayload { self.frame }
        }
        sail_method! {
            /// Borrows the original payload after ownership/position transfer.
            pub fn bytes(&self) -> (out: &[u8]) requires (true)
                ensures (out@ == self.frame_view().view()) { self.frame.bytes() }
        }
        sail_method! {
            /// Borrows the original opaque stream lease.
            pub fn owner(&self) -> (out: &Owner) requires (true)
                ensures (*out == self.owner_view()) { &self.owner }
        }
        sail_method! {
            /// Borrows the exact original positional payload witness.
            pub fn payload(&self) -> (out: &FramePayload) requires (true)
                ensures (*out == self.frame_view()) { &self.frame }
        }
        sail_method! {
            /// Drops the witness only at the unchanged final wire conversion.
            pub fn into_bytes(self) -> (out: Vec<u8>) requires (true)
                ensures (out@ == self.frame_view().view()) { self.frame.into_bytes() }
        }
    }
    impl<Owner> CompletedStream<Owner> {
        sail_method! {
            /// Join completed comparisons to this actual stdout suppression
            /// cursor and recheck every actual retained suppressed frame.
            #[allow(clippy::question_mark, reason = "Explicit refusal exits preserve the shared stock-Verus contracts and full-body mutation controls.")]
            pub fn with_replay<Identity>(self, replay: &crate::query_replay::Replay<'_, Owner>, identity: &Identity) -> (out: Option<Self>)
                where (Identity: OwnerIdentity<Owner>)
                requires (true)
                ensures (out.is_some() == (replay.complete_view()
                    && self.replayed_view() == Some(replay.served_view().len())
                    && Identity::identity(&self.owner_view()) == Identity::identity(&replay.owner_view())
                    && self.replay_prefix_matches(replay)),
                    out.is_some() ==> out.unwrap() == self)
            {
                if !replay.complete() || !identity.same_owner(&self.owner,replay.owner()) { return None; }
                let Some(original) = &self.replayed else { return None; };
                if original.count() as u128 != replay.total() as u128 { return None; }
                let total = replay.total();
                if total > self.output.frames() { return None; }
                let mut ordinal = 0usize;
                sail_while! { (ordinal < total)
                    invariant (ordinal <= total, total == replay.served_view().len(),
                        total <= self.output_view().endpoints().len(),
                        forall|i:int| 0 <= i < ordinal ==> self.replay_frame_matches(replay,i))
                    decreases (total-ordinal)
                    {
                        let Some(bytes) = self.output.frame_bytes(ordinal) else {
                            sail_proof! { assert(!self.replay_frame_matches(replay,ordinal as int)); }
                            return None;
                        };
                        if !replay.reproduces_at(ordinal,bytes) {
                            sail_proof! { assert(!self.replay_frame_matches(replay,ordinal as int)); }
                            return None;
                        }
                        sail_proof! { assert(self.replay_frame_matches(replay,ordinal as int)); }
                        ordinal += 1;
                    }
                }
                Some(self)
            }
        }
        sail_impl_spec! {
            pub closed spec fn owner_view(&self) -> Owner { self.owner }
            pub closed spec fn output_view(&self) -> CompleteOutput { self.output }
            pub closed spec fn complete(&self) -> bool { self.output.complete() }
            pub closed spec fn replayed_view(&self) -> Option<nat> {
                match self.replayed {Some(replayed)=>Some(replayed.count_view()),None=>None}
            }
            pub open spec fn replay_frame_matches(&self, replay: &crate::query_replay::Replay<'_,Owner>, ordinal: int) -> bool {
                if !(0 <= ordinal < replay.served_view().len()) {false}
                else {
                    crate::query_output::frame_interval(self.output_view().view(),self.output_view().endpoints(),ordinal)
                    && replay.served_view()[ordinal].reproduces(self.output_view().view().subrange(
                        crate::query_output::frame_start(self.output_view().endpoints(),ordinal),
                        self.output_view().endpoints()[ordinal] as int))
                }
            }
            pub open spec fn replay_prefix_matches(&self, replay: &crate::query_replay::Replay<'_,Owner>) -> bool {
                replay.served_view().len() <= self.output_view().endpoints().len()
                    && forall|i:int| 0 <= i < replay.served_view().len() ==> self.replay_frame_matches(replay,i)
            }
        }
        sail_method! {
            /// Compares real ownership before the exact value/interval check.
            pub fn contains_frame<Identity>(&self, frame: &OwnedFrame<Owner>, identity: &Identity) -> (out: bool)
                where (Identity: OwnerIdentity<Owner>)
                requires (self.complete())
                ensures (out == (Identity::identity(&self.owner_view()) == Identity::identity(&frame.owner_view())
                    && self.output_view().contains(&frame.frame_view())))
            { identity.same_owner(&self.owner, &frame.owner) && self.output.contains_frame(&frame.frame) }
        }
    }
}

sail_verified! {
    /// Requires actual typed stream owners and one common opaque child lease;
    /// equal bytes or a PID cannot replace the ownership comparison.
    #[allow(clippy::question_mark, reason = "Explicit refusal exits retain the shared stock-Verus contracts and full-body mutation controls.")]
    pub fn observe_owned_publication<Host, Owner, Identity>(input: Option<Delivered<Host>>, input_owner: Owner,
        output: Option<CompletedStream<Owner>>, gate: PublicationGate, identity: Identity)
        -> (out: Option<OwnedPublication<Host, Owner, Identity>>)
        where (Host: HashHost, Identity: OwnerIdentity<Owner>)
        requires (output.is_some() ==> output.unwrap().complete())
        ensures (out.is_some() == (input.is_some() && output.is_some() && gate.complete()
                && Identity::identity(&input_owner) == Identity::identity(&output.unwrap().owner_view())),
            out.is_some() ==> out.unwrap().complete(),
            out.is_some() ==> out.unwrap().input_owner_view() == input_owner,
            out.is_some() ==> out.unwrap().output_owner_view() == output.unwrap().owner_view(),
            out.is_some() ==> out.unwrap().publication_view().input_view() == input.unwrap(),
            out.is_some() ==> out.unwrap().publication_view().output_view() == output.unwrap().output_view(),
            out.is_some() ==> out.unwrap().replayed_view() == output.unwrap().replayed_view())
    {
        let Some(output) = output else { return None; };
        if !identity.same_owner(&input_owner,&output.owner) { return None; }
        let Some(publication) = observe_publication(input,Some(output.output),gate) else { return None; };
        Some(OwnedPublication { publication, input_owner, output_owner:output.owner, identity, replayed:output.replayed })
    }
}

sail_code! {
    impl<Host: HashHost, Owner, Identity: OwnerIdentity<Owner>> OwnedPublication<Host, Owner, Identity> {
        sail_impl_spec! {
            pub closed spec fn publication_view(&self) -> PublicationObservation<Host> { self.publication }
            pub closed spec fn input_owner_view(&self) -> Owner { self.input_owner }
            pub closed spec fn output_owner_view(&self) -> Owner { self.output_owner }
            pub closed spec fn replayed_view(&self) -> Option<nat> {
                match self.replayed {Some(replayed)=>Some(replayed.count_view()),None=>None}
            }
            pub closed spec fn complete(&self) -> bool { self.publication.complete()
                && Identity::identity(&self.input_owner) == Identity::identity(&self.output_owner) }
            pub open spec fn binding_rank(&self, frames: Seq<&OwnedFrame<Owner>>, order: Seq<usize>, rank: int) -> bool {
                if !self.replayed_view().is_some() || rank < 0 || rank >= order.len() {false}
                else if order[rank] >= frames.len() {false}
                else {
                    let frame=frames[order[rank] as int];
                    Identity::identity(&self.output_owner_view()) == Identity::identity(&frame.owner_view())
                        && self.publication_view().output_view().contains(&frame.frame_view())
                        && frame.frame_view().ordinal_view() == self.replayed_view().unwrap()+rank
                }
            }
            pub open spec fn bindings_match(&self, commands: Seq<&crate::query_prefix::FrozenCommand<Host>>,
                frames: Seq<&OwnedFrame<Owner>>, order: Seq<usize>, inverse: Seq<usize>) -> bool {
                self.replayed_view().is_some() && frames.len() == commands.len()
                    && self.replayed_view().unwrap() <= self.publication_view().output_view().endpoints().len()
                    && frames.len() == self.publication_view().output_view().endpoints().len()-self.replayed_view().unwrap()
                    && crate::query_checkpoints::checkpoints_match(&self.publication_view().input_view(),commands,order,inverse)
                    && forall|rank:int| 0 <= rank < order.len() ==> self.binding_rank(frames,order,rank)
            }
        }
        sail_method! {
            /// Original frozen complete-input key; no runtime identity is inferred.
            pub fn input_key(&self) -> (out: [u8;32]) requires (true)
                ensures (out == self.publication_view().input_view().key_view()) { self.publication.input().key() }
        }
        sail_method! {
            /// Original acknowledged complete-input length, without reconstructing
            /// it from the stored terminal key or output fields.
            pub fn input_length(&self) -> (out: u64) requires (true)
                ensures (out == self.publication_view().input_view().length()) { self.publication.input().length_value() }
        }
        sail_method! {
            /// Checks original keys against one actual acknowledged tape in a
            /// complete certified visit order; this never reorders published data.
            pub fn validate_checkpoints(&self, commands: &[&crate::query_prefix::FrozenCommand<Host>],
                order: &[usize], inverse: &[usize]) -> (accepted: bool)
                requires (self.publication_view().input_view().capture_consistent())
                ensures (accepted == crate::query_checkpoints::checkpoints_match(&self.publication_view().input_view(),commands@,order@,inverse@))
            { crate::query_checkpoints::validate_checkpoints(self.publication.input(),commands,order,inverse) }
        }
        sail_method! {
            /// Checks the complete fresh suffix against original command order
            /// and original same-owner frame ordinals, including held releases.
            /// Evidence sorting leaves every actual pending payload untouched.
            #[allow(clippy::cast_possible_truncation, reason = "The original replay count is checked against the actual usize frame count before conversion.")]
            pub fn validate_bindings(&self, commands: &[&crate::query_prefix::FrozenCommand<Host>],
                frames: &[&OwnedFrame<Owner>], order: &[usize], inverse: &[usize]) -> (accepted: bool)
                requires (self.complete(), self.publication_view().input_view().capture_consistent())
                ensures (accepted == self.bindings_match(commands@,frames@,order@,inverse@))
            {
                let Some(replayed)=&self.replayed else {return false;};
                let skipped=replayed.count(); let total=self.publication.output().frames();
                if skipped > total as u64 || frames.len() != commands.len() {return false;}
                let skipped=skipped as usize;
                if frames.len() != total-skipped {return false;}
                if !self.validate_checkpoints(commands,order,inverse) {return false;}
                let mut rank=0usize;
                sail_while! { (rank < order.len())
                    invariant (rank <= order.len(), self.complete(), self.publication_view().input_view().capture_consistent(),
                        frames@.len() == commands@.len(),
                        self.replayed_view() == Some(skipped as nat),
                        skipped <= total, frames.len() == total-skipped,
                        total == self.publication_view().output_view().endpoints().len(),
                        crate::query_checkpoints::checkpoints_match(&self.publication_view().input_view(),commands@,order@,inverse@),
                        forall|i:int| 0 <= i < rank ==> self.binding_rank(frames@,order@,i))
                    decreases (order.len()-rank)
                    {
                        sail_proof! {assert(crate::inputs::permutation_at(order@,inverse@,commands@.len(),rank as int));}
                        let frame=frames[order[rank]];
                        sail_proof! {
                            if !(Identity::identity(&self.output_owner_view()) == Identity::identity(&frame.owner_view())
                                && self.publication_view().output_view().contains(&frame.frame_view()))
                                || frame.frame_view().ordinal_view() != skipped+rank {
                                assert(!self.binding_rank(frames@,order@,rank as int));
                                assert(!self.bindings_match(commands@,frames@,order@,inverse@));
                            }
                        }
                        if !self.contains_frame(frame) || frame.payload().ordinal() != skipped+rank {return false;}
                        sail_proof! {assert(self.binding_rank(frames@,order@,rank as int));}
                        rank+=1;
                    }
                }
                true
            }
        }
        sail_method! {
            /// Requires original stream ownership as well as exact frame values.
            pub fn contains_frame(&self, frame: &OwnedFrame<Owner>) -> (out: bool)
                requires (self.complete())
                ensures (out == (Identity::identity(&self.output_owner_view()) == Identity::identity(&frame.owner_view())
                    && self.publication_view().output_view().contains(&frame.frame_view())))
            { self.identity.same_owner(&self.output_owner,&frame.owner) && self.publication.output().contains_frame(&frame.frame) }
        }
        sail_method! {
            /// Consumes the joint ownership observation without changing a payload.
            pub fn release<T>(self, pending: PublicationBatch<T>) -> (out: Option<Vec<T>>)
                requires (self.complete())
                ensures (out.is_some(), out.unwrap()@ == pending.contents())
            {
                if !self.identity.same_owner(&self.input_owner,&self.output_owner) { return None; }
                release_observed(pending,Some(self.publication))
            }
        }
    }
}

sail_verified! {
    /// Actual nonempty parser/delivery/frame constructors reach the complete
    /// declared pair relation. Scripted I/O and supplied equal opaque leases
    /// are explicit; this is not a physical solver-origin witness.
    #[allow(clippy::byte_char_slices, reason = "Literal bytes expose the constructive stock-Verus framing witness.")]
    pub fn constructed_binding_control<Host, Owner, Identity>(host: Host, input_owner: Owner,
        output_owner: Owner, retained: Owner, identity: Identity) -> (accepted: bool)
        where (Host: HashHost, Identity: OwnerIdentity<Owner>)
        requires (host.bytes().len() == 0,
            Identity::identity(&input_owner) == Identity::identity(&output_owner),
            Identity::identity(&output_owner) == Identity::identity(&retained))
        ensures (accepted)
    {
        let bytes=[b'(',b'x',b')'];
        let seed=crate::key_prefix::Prefix::new(host,b"z3",b"pinned",b"binary",b"platform",None);
        let mut parser=crate::query_prefix::QueryPrefix::new(seed.duplicate());
        let mut delivery=crate::query_delivery::Delivery::new_captured(
            crate::query_delivery::ScriptPipe::new(false),seed,3);
        parser.append(&bytes);
        sail_proof! {
            crate::query_prefix::flat_list_frame(seq![b'x']);
            assert(bytes@ =~= seq![b'(']+seq![b'x']+seq![b')']);
            assert(crate::query::symbol_first(bytes@[1]));
            assert(crate::query::trivia_after(Seq::<u8>::empty(),false) == Some(false));
            assert(parser.active() && parser.fed()==0 && parser.view() == bytes@);
            assert(crate::query::single_list(bytes@,true));
            assert(parser.view().subrange(0,0) =~= Seq::<u8>::empty());
            assert(parser.view().subrange(0,3) =~= bytes@);
        }
        let Some(borrowed)=parser.command(0,&bytes) else {return false;};
        let command=borrowed.freeze();
        sail_proof! {
            assert(parser.matches((b"z3"@,b"pinned"@,b"binary"@,b"platform"@,None)));
            assert(command.material_view() == parser.material());
            assert(command.material_view() =~= crate::key_prefix::header(b"z3"@,b"pinned"@,b"binary"@,b"platform"@,None)+bytes@);
            assert(command.key_view() == Host::digest(crate::key_prefix::end(command.material_view(),3)));
        }
        if !parser.hash_tail() || !delivery.write(&bytes) {return false;}
        sail_proof! {
            assert(parser.matches((b"z3"@,b"pinned"@,b"binary"@,b"platform"@,None)));
            assert(parser.material() =~= delivery.material());
            assert(parser.fed() == delivery.length());
        }
        let Some(input)=delivery.seal(parser.silence_key(),true) else {return false;};
        let response=[b'u',b'n',b's',b'a',b't',b'\n'];
        let mut output=StreamOutput::new(output_owner);
        output.append(&response);
        sail_proof! {
            crate::query_output::transcript_partition_facts(&output.output_view());
            assert(crate::query::response_trim(response@,0,5));
            assert(crate::query::response_span(response@,0,5));
            assert(crate::query::response_shape(response@));
            assert(output.output_view().view() =~= response@);
            assert(output.output_view().consumed()==0 && output.output_view().view().len()==6);
            assert(output.output_view().view().subrange(0,6) =~= response@);
            assert(crate::query::response_shape(output.output_view().view().subrange(0,6)));
        }
        let Some(frame)=output.frame(0,6) else {return false;};
        let Some(frame)=frame.into_owned(retained,&identity) else {return false;};
        output.end();
        sail_proof! {crate::query_output::transcript_partition_facts(&output.output_view());}
        let Some(output)=output.seal() else {return false;};
        let gate=PublicationGate {input_closed:true,pairing_active:true,queues_empty:true,
            normal_exit:true,quiet:true,forwarding_ok:true};
        let Some(publication)=observe_owned_publication(Some(input),input_owner,Some(output),gate,identity) else {return false;};
        let commands=[&command]; let frames=[&frame]; let order=[0usize]; let inverse=[0usize];
        sail_proof! {
            assert(crate::inputs::valid_permutation(order@,inverse@,1));
            assert(command.material_view() =~= publication.publication_view().input_view().initial_material()+bytes@);
            assert(bytes@.subrange(0,3) =~= bytes@);
            assert(bytes@.take(3) =~= bytes@);
            assert(publication.publication_view().input_view().capture_view() == Some(bytes@));
            assert(crate::query_checkpoints::checkpoint_rank(&publication.publication_view().input_view(),commands@,order@,0));
            assert(crate::query_checkpoints::checkpoints_match(&publication.publication_view().input_view(),commands@,order@,inverse@));
            assert(publication.binding_rank(frames@,order@,0));
            assert(publication.bindings_match(commands@,frames@,order@,inverse@));
        }
        publication.validate_bindings(&commands,&frames,&order,&inverse)
    }
}

sail_verified! {
    /// A missing fresh payload cannot pass the complete original pair domain.
    pub fn missing_binding_refused<Host, Owner, Identity>(publication: &OwnedPublication<Host, Owner, Identity>,
        commands: &[&crate::query_prefix::FrozenCommand<Host>], frames: &[&OwnedFrame<Owner>], order: &[usize], inverse: &[usize])
        -> (refused: bool)
        where (Host: HashHost, Identity: OwnerIdentity<Owner>)
        requires (publication.complete(), publication.publication_view().input_view().capture_consistent(),
            commands@.len() != frames@.len())
        ensures (refused)
    { !publication.validate_bindings(commands,frames,order,inverse) }
}

sail_verified! {
    /// Wrong original owner, missing ordinal or swapped original ordinal refuses
    /// even if the retained bytes at two positions happen to be identical.
    pub fn wrong_original_binding_refused<Host, Owner, Identity>(publication: &OwnedPublication<Host, Owner, Identity>,
        commands: &[&crate::query_prefix::FrozenCommand<Host>], frames: &[&OwnedFrame<Owner>], order: &[usize], inverse: &[usize])
        -> (refused: bool)
        where (Host: HashHost, Identity: OwnerIdentity<Owner>)
        requires (publication.complete(), publication.publication_view().input_view().capture_consistent(),
            exists|rank:int| 0 <= rank < order@.len() && !publication.binding_rank(frames@,order@,rank))
        ensures (refused)
    { !publication.validate_bindings(commands,frames,order,inverse) }
}

sail_verified! {
    /// The complete original read/frame/copy/EOF path is constructive for
    /// nonempty bytes and a retained equal lease, not an always-refuse wrapper.
    #[allow(clippy::byte_char_slices, reason = "Explicit nonempty response bytes retain the directly checked stock-Verus framing witness.")]
    pub fn nonempty_owned_stream_control<Owner, Identity>(owner: Owner, retained: Owner, identity: &Identity)
        -> (accepted: bool)
        where (Identity: OwnerIdentity<Owner>)
        requires (Identity::identity(&owner) == Identity::identity(&retained))
        ensures (accepted)
    {
        let bytes = [b'u',b'n',b's',b'a',b't',b'\n'];
        let mut output = StreamOutput::new(owner);
        output.append(&bytes);
        sail_proof! {
            crate::query_output::transcript_partition_facts(&output.output_view());
            assert(crate::query::response_trim(bytes@,0,5));
            assert(crate::query::response_span(bytes@,0,5));
            assert(crate::query::response_shape(bytes@));
            assert(output.output_view().view() =~= bytes@);
            assert(output.output_view().consumed()==0 && output.output_view().view().len()==6);
            assert(output.output_view().view().subrange(0,6) =~= bytes@);
            assert(crate::query::response_shape(output.output_view().view().subrange(0,6)));
        }
        let Some(frame) = output.frame(0,6) else { return false; };
        let Some(payload) = frame.into_owned(retained,identity) else { return false; };
        output.end();
        sail_proof! {crate::query_output::transcript_partition_facts(&output.output_view());}
        let Some(completed) = output.seal() else { return false; };
        completed.contains_frame(&payload,identity)
    }
}

sail_verified! {
    /// A nonempty original borrowed frame can transfer only its own live lease.
    pub fn retained_frame_owner_control<Owner, Identity>(frame: StreamFrame<'_, Owner>, owner: Owner,
        identity: &Identity) -> (accepted: bool)
        where (Identity: OwnerIdentity<Owner>)
        requires (frame.frame_view().view().len() > 0,
            Identity::identity(frame.owner_view()) == Identity::identity(&owner))
        ensures (accepted)
    { frame.into_owned(owner,identity).is_some() }
}

sail_verified! {
    /// Equal response values cannot substitute for the original borrowed owner.
    pub fn wrong_retained_owner_refused<Owner, Identity>(frame: StreamFrame<'_, Owner>, owner: Owner,
        identity: &Identity) -> (refused: bool)
        where (Identity: OwnerIdentity<Owner>)
        requires (Identity::identity(frame.owner_view()) != Identity::identity(&owner))
        ensures (refused)
    { frame.into_owned(owner,identity).is_none() }
}

sail_verified! {
    /// Even equal frame bytes/endpoints from a different live stream are refused.
    pub fn swapped_equal_stream_refused<Owner, Identity>(frame: &OwnedFrame<Owner>, output: &CompletedStream<Owner>,
        identity: &Identity) -> (refused: bool)
        where (Identity: OwnerIdentity<Owner>)
        requires (output.complete(), output.output_view().contains(&frame.frame_view()),
            frame.frame_view().view().len() > 0,
            Identity::identity(&output.owner_view()) != Identity::identity(&frame.owner_view()))
        ensures (refused)
    { !output.contains_frame(frame,identity) }
}

sail_verified! {
    /// Nonempty exact original owners admit the common-child publication slice.
    pub fn common_owner_publication_control<Host, Owner, Identity>(input: Delivered<Host>, input_owner: Owner,
        output: CompletedStream<Owner>, identity: Identity) -> (accepted: bool)
        where (Host: HashHost, Identity: OwnerIdentity<Owner>)
        requires (input.length() > 0, output.complete(), output.output_view().view().len() > 0,
            Identity::identity(&input_owner) == Identity::identity(&output.owner_view()))
        ensures (accepted)
    {
        let gate = PublicationGate { input_closed:true, pairing_active:true, queues_empty:true,
            normal_exit:true, quiet:true, forwarding_ok:true };
        observe_owned_publication(Some(input),input_owner,Some(output),gate,identity).is_some()
    }
}

sail_verified! {
    /// A frozen input and complete equal-valued output still need one child lease.
    pub fn different_child_publication_refused<Host, Owner, Identity>(input: Delivered<Host>, input_owner: Owner,
        output: CompletedStream<Owner>, gate: PublicationGate, identity: Identity) -> (refused: bool)
        where (Host: HashHost, Identity: OwnerIdentity<Owner>)
        requires (output.complete(), Identity::identity(&input_owner) != Identity::identity(&output.owner_view()))
        ensures (refused)
    { observe_owned_publication(Some(input),input_owner,Some(output),gate,identity).is_none() }
}
sail_verified! {
    /// One original served payload is compared against the actual completed
    /// stdout frame, then joined to that same owner's drained replay cursor.
    #[allow(clippy::byte_char_slices, reason = "Literal bytes provide the nonempty whole-operation replay join witness.")]
    pub fn compared_stdout_control<Identity>(owner: u64, identity: &Identity) -> (accepted: bool)
        where (Identity: OwnerIdentity<u64>)
        requires (true) ensures (accepted)
    {
        let bytes = [b'(',b'x',b')'];
        let served = [crate::query_replay::ServedReply::new(&bytes,crate::inputs::copy_bytes(&bytes),None)];
        let mut replay = crate::query_replay::Replay::new(&served,owner);
        let mut output = StreamOutput::new_replayed(owner,1);
        output.append(&bytes);
        sail_proof! {
            crate::query_prefix::flat_list_frame(seq![b'x']);
            assert(bytes@ =~= seq![b'(']+seq![b'x']+seq![b')']);
            reveal_with_fuel(crate::query::frame_scan,4);
            assert(crate::query::single_list(bytes@,false));
            assert(crate::query::response_span(bytes@,0,3));
            assert(crate::query::response_shape(bytes@));
            crate::query_output::transcript_partition_facts(&output.output_view());
            assert(output.output_view().view() =~= bytes@);
            assert(output.output_view().view().subrange(0,3) =~= bytes@);
        }
        if output.frame(0,3).is_none() || output.advance_replay() { return false; }
        output.end();
        sail_proof! { crate::query_output::transcript_partition_facts(&output.output_view()); }
        let Some(output) = output.seal() else { return false; };
        if !replay.compare(&bytes) { return false; }
        output.with_replay(&replay,identity).is_some()
    }
}

sail_verified! {
    /// Same owner/count and completed comparisons cannot substitute unrelated
    /// bytes for the actual completed stdout prefix (#457).
    #[allow(clippy::byte_char_slices, reason = "Literal unsat/sat frames expose the concrete different-output refusal witness.")]
    pub fn different_compared_stdout_refused<Identity>(owner: u64, identity: &Identity) -> (refused: bool)
        where (Identity: OwnerIdentity<u64>)
        requires (true) ensures (refused)
    {
        let cached = [b'u',b'n',b's',b'a',b't',b'\n'];
        let actual = [b's',b'a',b't',b'\n'];
        let served = [crate::query_replay::ServedReply::new(b"(check-sat)",crate::inputs::copy_bytes(&cached),None)];
        let mut replay = crate::query_replay::Replay::new(&served,owner);
        if !replay.compare(&cached) { return false; }
        let mut output = StreamOutput::new_replayed(owner,1);
        output.append(&actual);
        sail_proof! {
            assert(crate::query::response_trim(actual@,0,3));
            assert(crate::query::response_span(actual@,0,3));
            assert(crate::query::response_shape(actual@));
            assert(output.output_view().view().subrange(0,4) =~= actual@);
            crate::query_output::transcript_partition_facts(&output.output_view());
        }
        if output.frame(0,4).is_none() || output.advance_replay() { return false; }
        output.end();
        sail_proof! { crate::query_output::transcript_partition_facts(&output.output_view()); }
        let Some(output) = output.seal() else { return false; };
        sail_proof! {
            assert(output.output_view().view() =~= actual@);
            assert(output.output_view().endpoints() =~= seq![4usize]);
            assert(replay.served_view().len() == 1);
            assert(!replay.served_view()[0].statistics_view());
            assert(replay.served_view()[0].bytes_view() == cached@);
            assert(!replay.served_view()[0].reproduces(actual@));
            assert(!output.replay_frame_matches(&replay,0));
            assert(!output.replay_prefix_matches(&replay));
        }
        output.with_replay(&replay,identity).is_none()
    }
}
