//! Typed local publication over actual owned input/output observations.
//! This conjunction does not authenticate a solver, cache writer or disk origin.
use crate::key_prefix::HashHost;
use crate::query::{CorrelatedQueue, PublicationBatch, PublicationGate};
use crate::query_delivery::Delivered;
use crate::query_output::CompleteOutput;
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Owns the frozen input delivery, complete stdout and supplied closure
    /// observations until the original pending payloads are released.
    /// It is not a complete reply-origin or persistent-recovery certificate.
    pub struct PublicationObservation<Host> {
        input: Delivered<Host>,
        output: CompleteOutput,
        gate: PublicationGate,
    }

    impl<Host: HashHost> PublicationObservation<Host> {
        sail_impl_spec! {
            pub closed spec fn input_view(&self) -> Delivered<Host> { self.input }
            pub closed spec fn output_view(&self) -> CompleteOutput { self.output }
            pub closed spec fn gate_view(&self) -> PublicationGate { self.gate }
            pub open spec fn complete(&self) -> bool {
                self.output_view().complete() && self.gate_view().complete()
            }
        }
        sail_method! {
            /// Borrows the exact acknowledged-input token, never a saved key.
            pub fn input(&self) -> (out: &Delivered<Host>)
                requires (true) ensures (*out == self.input_view()) { &self.input }
        }
        sail_method! {
            /// Borrows the exact complete stdout owner retained at publication.
            pub fn output(&self) -> (out: &CompleteOutput)
                requires (true) ensures (*out == self.output_view()) { &self.output }
        }
        sail_method! {
            /// Borrows the original supplied closure observations. Native
            /// process/pipe attribution remains a separate provider obligation.
            pub fn gate(&self) -> (out: &PublicationGate)
                requires (true) ensures (*out == self.gate_view()) { &self.gate }
        }
    }
}

sail_verified! {
    /// Requires both owned stream witnesses and every existing closure check.
    /// `CompleteOutput`'s private constructor supplies its exact EOF/coverage
    /// invariant; native process identity is not assumed by this operation.
    #[allow(clippy::question_mark, reason = "Keep both explicit typed-observation refusal branches in the source shared with stock Verus.")]
    pub fn observe_publication<Host>(input: Option<Delivered<Host>>,
        output: Option<CompleteOutput>, gate: PublicationGate)
        -> (out: Option<PublicationObservation<Host>>)
        where (Host: HashHost)
        requires (output.is_some() ==> output.unwrap().complete())
        ensures (out.is_some() == (input.is_some() && output.is_some() && gate.complete()),
            out.is_some() ==> out.unwrap().complete(),
            out.is_some() ==> out.unwrap().input_view() == input.unwrap(),
            out.is_some() ==> out.unwrap().output_view() == output.unwrap(),
            out.is_some() ==> out.unwrap().gate_view() == gate)
    {
        let Some(input) = input else { return None; };
        let Some(output) = output else { return None; };
        if !gate.allows() { return None; }
        Some(PublicationObservation { input, output, gate })
    }
}

sail_verified! {
    /// Releases exactly the pending owned payloads only with the typed joint
    /// observation. This preserves data and order, not entry-to-stream origin.
    pub fn release_observed<T, Host>(pending: PublicationBatch<T>,
        observation: Option<PublicationObservation<Host>>) -> (out: Option<Vec<T>>)
        where (Host: HashHost)
        requires (observation.is_some() ==> observation.unwrap().complete())
        ensures (out.is_some() == observation.is_some(),
            out.is_some() ==> out.unwrap()@ == pending.contents())
    {
        pending.release(observation.is_some())
    }
}

sail_verified! {
    /// Constructive nonempty supplied-domain witness. The input/output tokens
    /// come from their original checked controllers; this is not an OS run.
    pub fn nonempty_publication_control<Host>(input: Delivered<Host>, output: CompleteOutput)
        -> (passed: bool)
        where (Host: HashHost)
        requires (input.length() > 0, output.complete(), output.view().len() > 0)
        ensures (passed)
    {
        let gate = PublicationGate { input_closed: true, pairing_active: true,
            queues_empty: true, normal_exit: true, quiet: true, forwarding_ok: true };
        let mut pairs = CorrelatedQueue::empty();
        pairs.expect(7u64);
        pairs.reply(8u64);
        let Some(pair) = pairs.take_pair() else { return false; };
        let mut pending = PublicationBatch::empty();
        if !pending.push(pair, 1, 1) { return false; }
        let Some(observed) = observe_publication(Some(input), Some(output), gate) else { return false; };
        let Some(released) = release_observed(pending, Some(observed)) else { return false; };
        released.len() == 1 && *released[0].expectation() == 7 && *released[0].reply() == 8
    }
}

sail_verified! {
    /// An otherwise clean supplied gate never replaces missing input delivery.
    pub fn missing_delivery_refused<Host>(output: CompleteOutput, gate: PublicationGate) -> (refused: bool)
        where (Host: HashHost)
        requires (output.complete())
        ensures (refused)
    { observe_publication::<Host>(None, Some(output), gate).is_none() }
}

sail_verified! {
    /// A frozen input and all closure flags cannot replace complete stdout.
    pub fn missing_output_refused<Host>(input: Delivered<Host>, gate: PublicationGate) -> (refused: bool)
        where (Host: HashHost)
        requires (true)
        ensures (refused)
    { observe_publication(Some(input), None, gate).is_none() }
}

sail_verified! {
    /// Any missing original closure observation still refuses publication.
    pub fn incomplete_closure_refused<Host>(input: Delivered<Host>, output: CompleteOutput,
        gate: PublicationGate) -> (refused: bool)
        where (Host: HashHost)
        requires (output.complete(), !gate.complete())
        ensures (refused)
    { observe_publication(Some(input), Some(output), gate).is_none() }
}

sail_verified! {
    /// Pending payloads alone are insufficient for the typed release path.
    pub fn missing_observation_withholds<T, Host>(pending: PublicationBatch<T>) -> (refused: bool)
        where (Host: HashHost)
        requires (true)
        ensures (refused)
    { release_observed::<T, Host>(pending, None).is_none() }
}
