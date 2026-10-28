//! Actual held-check payload storage, separate from mutable effort metadata.
//! This owns production values; it is not a parallel publication-history ledger.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Owns at most one deferred original payload. Metadata may be detached for
    /// annotation, but annotation never obtains mutable access to the payload.
    pub struct Holding<Payload, Metadata> {
        payload: Option<Payload>,
        metadata: Option<Metadata>,
    }
    impl<Payload, Metadata> Holding<Payload, Metadata> {
        sail_impl_spec! {
            pub closed spec fn payload_view(&self) -> Option<Payload> { self.payload }
            pub closed spec fn metadata_view(&self) -> Option<Metadata> { self.metadata }
            pub open spec fn well_formed(&self) -> bool {
                self.metadata_view().is_some() ==> self.payload_view().is_some()
            }
        }
        sail_method! {
            /// Starts with no held payload or annotation.
            #[allow(clippy::new_without_default, reason = "The explicit empty constructor is checked directly with stock Verus.")]
            pub fn new() -> (out: Self)
                requires (true)
                ensures (out.well_formed(), out.payload_view().is_none(), out.metadata_view().is_none())
            { Self { payload:None, metadata:None } }
        }
        sail_method! {
            /// Replaces the held value while returning the exact original
            /// displaced value and annotation; nothing is silently overwritten.
            pub fn replace(&mut self, payload: Payload, metadata: Metadata)
                -> (previous: Option<(Payload, Option<Metadata>)>)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(),
                    final(self).payload_view() == Some(payload), final(self).metadata_view() == Some(metadata),
                    previous.is_some() == old(self).payload_view().is_some(),
                    previous.is_some() ==> previous.unwrap().0 == old(self).payload_view().unwrap()
                        && previous.unwrap().1 == old(self).metadata_view())
            {
                let previous = self.take();
                self.payload=Some(payload); self.metadata=Some(metadata);
                previous
            }
        }
        sail_method! {
            /// Releases exactly the original held payload once, clearing both
            /// storage fields. A second release cannot duplicate it.
            #[allow(clippy::manual_map, reason = "Explicit Option cases preserve the original payload and metadata transition checked directly with stock Verus.")]
            pub fn take(&mut self) -> (out: Option<(Payload, Option<Metadata>)>)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(), final(self).payload_view().is_none(), final(self).metadata_view().is_none(),
                    out.is_some() == old(self).payload_view().is_some(),
                    out.is_some() ==> out.unwrap().0 == old(self).payload_view().unwrap()
                        && out.unwrap().1 == old(self).metadata_view())
            {
                let payload=self.payload.take(); let metadata=self.metadata.take();
                match payload { Some(payload)=>Some((payload,metadata)), None=>None }
            }
        }
        sail_method! {
            /// Emits the exact held value before this current value. This is
            /// the actual statistics-release permutation, with no re-pairing.
            pub fn release_before(&mut self, current: Payload) -> (out: Vec<(Payload, Option<Metadata>)>)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(), final(self).payload_view().is_none(), final(self).metadata_view().is_none(),
                    out@ == match old(self).payload_view() {
                        Some(previous) => seq![(previous,old(self).metadata_view()),(current,None)],
                        None => seq![(current,None)],
                    })
            {
                let held=self.take(); let mut out=Vec::new();
                if let Some(previous)=held { out.push(previous); }
                out.push((current,None)); out
            }
        }
        sail_method! {
            /// Detaches only metadata for native annotation. The original
            /// payload remains private and unchanged in this controller.
            pub fn take_metadata(&mut self) -> (out: Option<Metadata>)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(), final(self).payload_view() == old(self).payload_view(),
                    final(self).metadata_view().is_none(), out == old(self).metadata_view(),
                    out.is_some() ==> final(self).payload_view().is_some())
            { self.metadata.take() }
        }
        sail_method! {
            /// Restores annotation only while its original payload is present.
            /// Metadata cannot create a new held payload.
            pub fn put_metadata(&mut self, metadata: Metadata) -> (accepted: bool)
                requires (old(self).well_formed())
                ensures (final(self).well_formed(), final(self).payload_view() == old(self).payload_view(),
                    accepted == old(self).payload_view().is_some(),
                    accepted ==> final(self).metadata_view() == Some(metadata),
                    !accepted ==> final(self).metadata_view() == old(self).metadata_view())
            {
                if self.payload.is_none() { return false; }
                self.metadata=Some(metadata); true
            }
        }
    }
}

sail_verified! {
    /// Actual correlated payloads survive detach/update, displacement and
    /// final release with their original expectation and reply unchanged.
    pub fn held_pair_preservation_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let mut pairs=crate::query::CorrelatedQueue::empty();
        pairs.expect(2u64); pairs.reply(22u64);
        let Some(first)=pairs.take_pair() else { return false; };
        pairs.expect(4u64); pairs.reply(44u64);
        let Some(second)=pairs.take_pair() else { return false; };
        let mut held=Holding::new();
        if held.replace(first,100u64).is_some() { return false; }
        let Some(mut metadata)=held.take_metadata() else { return false; };
        metadata+=60;
        if !held.put_metadata(metadata) { return false; }
        let Some((first, metadata))=held.replace(second,200) else { return false; };
        if *first.expectation()!=2 || *first.reply()!=22 || metadata!=Some(160) { return false; }
        let Some((second,metadata))=held.take() else { return false; };
        *second.expectation()==4 && *second.reply()==44 && metadata==Some(200) && held.take().is_none()
    }
}

sail_verified! {
    /// The real deferred-storage operations preserve the existing statistics /
    /// reason-unknown publication permutation rather than forcing source order.
    pub fn held_release_order_control() -> (passed: bool)
        requires (true) ensures (passed)
    {
        let mut held=Holding::new();
        let mut out=Vec::new();
        out.push(1u64);
        if held.replace(2u64,100u64).is_some() { return false; }
        out.push(3u64);
        let Some(metadata)=held.take_metadata() else { return false; };
        if !held.put_metadata(metadata+60) { return false; }
        let released=held.release_before(4u64);
        if released.len()!=2 { return false; }
        out.push(released[0].0); out.push(released[1].0);
        out.len()==4 && out[0]==1 && out[1]==3 && out[2]==2 && out[3]==4
            && released[0].1==Some(160) && released[1].1.is_none()
    }
}

sail_verified! {
    /// An orphan annotation cannot manufacture a deferred response.
    pub fn orphan_metadata_refused<Payload, Metadata>(metadata: Metadata) -> (refused: bool)
        requires (true) ensures (refused)
    {
        let mut held: Holding<Payload,Metadata>=Holding::new();
        !held.put_metadata(metadata) && held.take().is_none()
    }
}
