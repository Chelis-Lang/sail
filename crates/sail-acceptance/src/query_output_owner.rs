//! Opaque stream-lease preservation around the actual output controller.
//! Native lease creation and endpoint attribution remain narrow OS contracts.
use crate::inputs::OwnerIdentity;
use crate::key_prefix::HashHost;
use crate::query::{PublicationBatch, PublicationGate};
use crate::query_delivery::Delivered;
use crate::query_output::{CompleteOutput, Frame, FramePayload, Transcript};
use crate::query_publication::{PublicationObservation, observe_publication, release_observed};
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// One actual stdout controller and its original opaque ownership lease.
    pub struct StreamOutput<Owner> { owner: Owner, output: Transcript }
    /// A frame still borrowing its original controller and ownership lease.
    pub struct StreamFrame<'a, Owner> { owner: &'a Owner, frame: Frame<'a> }
    /// Original response bytes/position and a retained same-owner lease.
    pub struct OwnedFrame<Owner> { owner: Owner, frame: FramePayload }
    /// Original complete EOF/coverage observation with the same stream lease.
    pub struct CompletedStream<Owner> { owner: Owner, output: CompleteOutput }
    /// Typed joint publication retaining original input/output ownership.
    /// This does not authenticate a runtime, arbitrary writer or restart data.
    pub struct OwnedPublication<Host, Owner, Identity> {
        publication: PublicationObservation<Host>,
        input_owner: Owner,
        output_owner: Owner,
        identity: Identity,
    }
}

sail_code! {
    impl<Owner> StreamOutput<Owner> {
        sail_impl_spec! {
            pub closed spec fn owner_view(&self) -> Owner { self.owner }
            pub closed spec fn output_view(&self) -> Transcript { self.output }
        }
        sail_method! {
            /// Starts before reads, around the actual endpoint's supplied lease.
            pub fn new(owner: Owner) -> (out: Self)
                requires (true)
                ensures (out.owner_view() == owner, out.output_view().well_formed(),
                    out.output_view().healthy(), !out.output_view().eof(), out.output_view().view().len() == 0,
                    out.output_view().endpoints().len() == 0)
            { Self { owner, output: Transcript::new() } }
        }
        sail_method! {
            /// Appends exactly the original read chunk through the same controller.
            pub fn append(&mut self, bytes: &[u8]) -> (out: ())
                requires (old(self).output_view().well_formed())
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).output_view().well_formed(),
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
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).output_view().eof(),
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
                ensures (final(self).owner_view() == old(self).owner_view(), !final(self).output_view().healthy(),
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
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).output_view().well_formed(),
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
            pub fn frame<'a>(&'a mut self, start: usize, end: usize) -> (out: Option<StreamFrame<'a, Owner>>)
                requires (old(self).output_view().well_formed())
                ensures (final(self).owner_view() == old(self).owner_view(), final(self).output_view().well_formed(),
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
            pub fn seal(self) -> (out: Option<CompletedStream<Owner>>)
                requires (self.output_view().well_formed())
                ensures (out.is_some() == self.output_view().complete(),
                    out.is_some() ==> out.unwrap().complete() && out.unwrap().owner_view() == self.owner_view()
                        && out.unwrap().output_view().view() == self.output_view().view()
                        && out.unwrap().output_view().endpoints() == self.output_view().endpoints())
            {
                let Some(output) = self.output.seal() else { return None; };
                Some(CompletedStream { owner:self.owner, output })
            }
        }
    }
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
        sail_impl_spec! {
            pub closed spec fn owner_view(&self) -> Owner { self.owner }
            pub closed spec fn output_view(&self) -> CompleteOutput { self.output }
            pub closed spec fn complete(&self) -> bool { self.output.complete() }
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
            out.is_some() ==> out.unwrap().publication_view().output_view() == output.unwrap().output_view())
    {
        let Some(output) = output else { return None; };
        if !identity.same_owner(&input_owner,&output.owner) { return None; }
        let Some(publication) = observe_publication(input,Some(output.output),gate) else { return None; };
        Some(OwnedPublication { publication, input_owner, output_owner:output.owner, identity })
    }
}

sail_code! {
    impl<Host: HashHost, Owner, Identity: OwnerIdentity<Owner>> OwnedPublication<Host, Owner, Identity> {
        sail_impl_spec! {
            pub closed spec fn publication_view(&self) -> PublicationObservation<Host> { self.publication }
            pub closed spec fn input_owner_view(&self) -> Owner { self.input_owner }
            pub closed spec fn output_owner_view(&self) -> Owner { self.output_owner }
            pub closed spec fn complete(&self) -> bool { self.publication.complete()
                && Identity::identity(&self.input_owner) == Identity::identity(&self.output_owner) }
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
            reveal(Transcript::consumed);
            reveal(Transcript::complete);
            assert(crate::query::response_trim(bytes@,0,5));
            assert(crate::query::response_span(bytes@,0,5));
            assert(crate::query::response_shape(bytes@));
            assert(output.output_view().view() =~= bytes@);
        }
        let Some(frame) = output.frame(0,6) else { return false; };
        let Some(owned) = frame.into_owned(retained,identity) else { return false; };
        output.end();
        let Some(completed) = output.seal() else { return false; };
        completed.contains_frame(&owned,identity)
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
