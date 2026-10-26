//! Bind solver-launch journals to independently retained compiler attempts.
//!
//! The request codec includes the exact final raw descriptor. No decoded PID,
//! report identity or caller-supplied expected request selects the association.
use super::authority::{CompilerUnit, ObservedAttempt, ProcessDescriptor, ReservedAttempt};
#[cfg(sail_verus)]
use super::authority::{
    observation_complete, observed_running, reserved_descriptor, reserved_run, reserved_slot,
    reserved_unit, running_exit, running_pid, running_reserved,
};
use super::solver_observations::{self as journal, AdmittedJournal, JournalEvent, WaitObservation};
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    pub open spec fn optional_bytes(value: Option<Vec<u8>>) -> Seq<u8> {
        match value { None => seq![0u8], Some(bytes) => seq![1u8] + journal::field_bytes(bytes@) }
    }
    pub open spec fn override_fields(values: Seq<(Vec<u8>, Option<Vec<u8>>)>) -> Seq<u8>
        decreases values.len(),
    {
        if values.len() == 0 { Seq::empty() }
        else { override_fields(values.drop_last()) + journal::field_bytes(values.last().0@) + optional_bytes(values.last().1) }
    }
    pub open spec fn unit_bytes(unit: CompilerUnit) -> Seq<u8> {
        journal::field_bytes(unit.manifest@) + journal::field_bytes(unit.name@)
            + journal::field_bytes(unit.crate_type@) + optional_bytes(unit.cargo_name)
    }
    pub open spec fn descriptor_bytes(descriptor: ProcessDescriptor) -> Seq<u8> {
        journal::field_bytes(descriptor.program@) + journal::field_bytes(journal::argument_bytes(descriptor.arguments@))
            + optional_bytes(descriptor.directory) + journal::word_bytes(descriptor.overrides.len() as u64)
            + override_fields(descriptor.overrides@)
    }
    pub open spec fn request_bytes(run: Seq<u8>, unit: CompilerUnit, slot: usize, descriptor: ProcessDescriptor) -> Seq<u8> {
        journal::field_bytes(b"sail-solver-invocation/v1"@)
            + journal::field_bytes(run) + journal::word_bytes(slot as u64) + unit_bytes(unit) + descriptor_bytes(descriptor)
    }
    pub open spec fn reserved_request(attempt: ReservedAttempt) -> Seq<u8> {
        request_bytes(reserved_run(&attempt)@, *reserved_unit(&attempt), reserved_slot(&attempt), reserved_descriptor(&attempt))
    }
}

sail_verified! {
    /// Encodes absence separately from an explicitly present empty byte string.
    pub fn append_optional(out: &mut Vec<u8>, value: &Option<Vec<u8>>) -> (done: ())
        requires(true)
        ensures(final(out)@ == old(out)@ + optional_bytes(*value))
    {
        match value {
            None => out.push(0),
            Some(bytes) => { out.push(1); journal::append_field(out, bytes); },
        }
    }
}

sail_verified! {
    /// Encodes every exact ordered environment name and override/removal value.
    pub fn append_overrides(out: &mut Vec<u8>, values: &[(Vec<u8>, Option<Vec<u8>>)]) -> (done: ())
        requires(true)
        ensures(final(out)@ == old(out)@ + journal::word_bytes(values.len() as u64) + override_fields(values@))
    {
        journal::append_word(out, values.len() as u64);
        let mut i = 0;
        sail_while! { (i < values.len()) invariant(
            i <= values.len(), out@ == old(out)@ + journal::word_bytes(values.len() as u64) + override_fields(values@.take(i as int))
        ) decreases(values.len() - i) {
            journal::append_field(out, &values[i].0);
            append_optional(out, &values[i].1);
            sail_proof! {
                assert(values@.take(i as int + 1).drop_last() =~= values@.take(i as int));
                assert(values@.take(i as int + 1).last() == values@[i as int]);
                reveal_with_fuel(override_fields, 1);
            }
            i += 1;
        }}
        sail_proof! { assert(values@.take(values.len() as int) =~= values@); }
    }
}

sail_verified! {
    /// Serializes the proposed final descriptor before configuration/spawn.
    /// Operational paths are retained exactly; no query/argument normalization occurs.
    pub fn encode_request(run: &[u8], unit: &CompilerUnit, slot: usize, descriptor: &ProcessDescriptor) -> (bytes: Vec<u8>)
        requires(true)
        ensures(bytes@ == request_bytes(run@, *unit, slot, *descriptor))
    {
        let mut bytes = Vec::new();
        journal::append_field(&mut bytes, b"sail-solver-invocation/v1");
        journal::append_field(&mut bytes, run);
        journal::append_word(&mut bytes, slot as u64);
        journal::append_field(&mut bytes, &unit.manifest);
        journal::append_field(&mut bytes, &unit.name);
        journal::append_field(&mut bytes, &unit.crate_type);
        append_optional(&mut bytes, &unit.cargo_name);
        sail_proof! { assert(bytes@ == journal::field_bytes(b"sail-solver-invocation/v1"@) + journal::field_bytes(run@) + journal::word_bytes(slot as u64) + unit_bytes(*unit)); }
        journal::append_field(&mut bytes, &descriptor.program);
        let arguments = journal::encode_arguments(&descriptor.arguments);
        journal::append_field(&mut bytes, &arguments);
        append_optional(&mut bytes, &descriptor.directory);
        append_overrides(&mut bytes, &descriptor.overrides);
        sail_proof! { assert(bytes@ =~= request_bytes(run@, *unit, slot, *descriptor)); }
        bytes
    }
}

sail_verified! {
    /// Recomputes expected request bytes from the immutable retained reservation.
    pub fn encode_reserved(attempt: &ReservedAttempt) -> (bytes: Vec<u8>)
        requires(true) ensures(bytes@ == reserved_request(*attempt))
    { encode_request(attempt.run(), attempt.unit(), attempt.slot(), attempt.descriptor()) }
}

sail_code! {
    /// Lifecycle evidence tied to the exact independently retained compiler child.
    /// Protected journal ownership and actual wrapper/child lineage remain required.
    pub struct BoundLaunchJournal<'a, 'p, 'r> {
        observed: &'p ObservedAttempt<'r>,
        journal: AdmittedJournal<'a>,
    }
    impl<'a, 'p, 'r> BoundLaunchJournal<'a, 'p, 'r> {
        sail_impl_spec! {
            pub closed spec fn observed_view(&self) -> &'p ObservedAttempt<'r> { self.observed }
            pub closed spec fn journal_view(&self) -> AdmittedJournal<'a> { self.journal }
        }
        sail_method! {
            /// Returns the original complete compiler-child observation.
            pub fn observed(&self) -> (out: &'p ObservedAttempt<'r>)
                requires(true) ensures(out == self.observed_view())
            { self.observed }
        }
        sail_method! {
            /// Returns the exact-byte admitted launch inventory for that child.
            pub fn journal(&self) -> (out: &AdmittedJournal<'a>)
                requires(true) ensures(*out == self.journal_view())
            { &self.journal }
        }
    }
}

sail_verified! {
    /// Binds from actual retained observations; decoded records choose no expected identity.
    /// Signal exits and wrapper/actual-compiler PID mismatches refuse this direct bridge.
    pub fn bind_launch_journal<'a, 'p, 'r>(observed: &'p ObservedAttempt<'r>, raw: &'a [u8], events: &'a [JournalEvent])
        -> (bound: Option<BoundLaunchJournal<'a, 'p, 'r>>)
        requires(observation_complete(&observed_running(observed)))
        ensures(bound.is_some() ==> bound.unwrap().observed_view() == observed
            && bound.unwrap().journal_view().raw_view() == raw@
            && bound.unwrap().journal_view().events_view() == events@
            && running_exit(&observed_running(observed)).unwrap().is_some()
            && journal::journal_records_valid(events@, reserved_request(running_reserved(&observed_running(observed))),
                running_pid(&observed_running(observed)), running_exit(&observed_running(observed)).unwrap().unwrap()))
    {
        let code = observed.exit()?;
        let request = encode_reserved(observed.reserved());
        let status = WaitObservation { normal: true, code };
        let journal = journal::admit_journal(raw, events, &request, observed.pid(), status)?;
        Some(BoundLaunchJournal { observed, journal })
    }
}
