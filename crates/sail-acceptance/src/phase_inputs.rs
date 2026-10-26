//! Exact accounting of the two recorded compiler-declaration phases.
//!
//! This does not certify complete physical compiler inputs. Native parsing,
//! protected producer correspondence and opaque compiler I/O remain separate.

#[cfg(sail_verus)]
use vstd::prelude::*;

use crate::authority::{CompilerUnit, ObservedAttempt, UnconfiguredAttempt};
#[cfg(sail_verus)]
use crate::authority::{
    observation_complete, observed_running, reserved_run, reserved_slot, reserved_unit,
    running_exit, running_pid, running_reserved, unconfigured_run, unconfigured_slot,
    unconfigured_unit,
};
use crate::inputs::{
    InputFact, byte_eq, copy_bytes, input_key_eq, input_order, ordered_key_less, selection_covered,
    value_difference,
};
#[cfg(sail_verus)]
use crate::inputs::{
    contains_fact, covered_selection, key_less, key_less_transitive, ordered_facts,
    ordered_lookup_stops_at_member, ordered_pair_less, permutation_at, same_key, same_value,
};

sail_code! {
    /// Raw rows with untrusted ordering proposals; no completeness is implied.
    pub struct Inventory<'a> {
        /// Literal declared keys and values, including explicit absence.
        pub rows: &'a [InputFact<'a>],
        /// Candidate sorted positions.
        pub order: &'a [usize],
        /// Candidate inverse positions.
        pub inverse: &'a [usize],
    }

    /// Original request identity prepared before configuring or spawning a child.
    pub struct PhaseRequest {
        run: Vec<u8>,
        unit: CompilerUnit,
        slot: usize,
        binding: Vec<u8>,
    }

    /// One parsed declaration phase; the tag is 0 for verification, 1 for compilation.
    pub struct PhaseRecord<'a> {
        /// Literal phase tag proposed by the strict document decoder.
        pub tag: u8,
        /// Exact raw declaration facts from that phase.
        pub fields: Inventory<'a>,
    }

    /// Parsed existing producer fields, without any compiler-closure claim.
    pub struct PhaseDocument<'a> {
        /// Exact producer document kind bytes.
        pub kind: &'a [u8],
        /// Actual document schema.
        pub schema: u32,
        /// Actual child PID carried by the producer.
        pub pid: u32,
        /// Original private request binding echoed by the producer.
        pub binding: &'a [u8],
        /// Existing final-document field, not an inferred all-inputs predicate.
        pub complete: bool,
        /// Every parsed phase, including unknown tags that must be refused.
        pub phases: &'a [PhaseRecord<'a>],
    }

    /// Same-attempt two-phase declaration accounting; never a `CompilerDomain`.
    pub struct PhaseInputs<'a, 'b> {
        request: PhaseRequest,
        observed: &'a ObservedAttempt<'b>,
        document: &'a PhaseDocument<'a>,
        joined: Inventory<'a>,
    }
}

sail_spec! {
    /// Every selected row has its exact readable counterpart in the source.
    pub open spec fn subset(source: Seq<InputFact>, selected: Seq<InputFact>) -> bool {
        forall|i: int| 0 <= i < selected.len() ==> contains_fact(source, #[trigger] selected[i])
    }
    /// Candidate orders cover all raw rows with strict unique keys.
    pub open spec fn ordered(inventory: Inventory) -> bool {
        ordered_facts(inventory.rows@, inventory.order@, inventory.inverse@)
    }
    /// Each joined row was recorded by at least one of the two actual phases.
    pub open spec fn union_only(left: Inventory, right: Inventory, joined: Inventory) -> bool {
        forall|i: int| 0 <= i < joined.rows.len() ==>
            contains_fact(left.rows@, #[trigger] joined.rows[i])
                || contains_fact(right.rows@, joined.rows[i])
    }
    /// Every raw compiler-unit field participates in correspondence.
    pub open spec fn same_unit(a: CompilerUnit, b: CompilerUnit) -> bool {
        a.manifest@ == b.manifest@ && a.name@ == b.name@ && a.crate_type@ == b.crate_type@
            && match (a.cargo_name, b.cargo_name) {
                (Some(a), Some(b)) => a@ == b@, (None, None) => true, _ => false,
            }
    }
}

impl PhaseRequest {
    sail_impl_spec! {
        /// Immutable original run bytes.
        pub closed spec fn run_view(&self) -> Seq<u8> { self.run@ }
        /// Immutable original compiler-unit fields.
        pub closed spec fn unit_view(&self) -> CompilerUnit { self.unit }
        /// Independent pre-spawn attempt slot.
        pub closed spec fn slot_view(&self) -> usize { self.slot }
        /// Exact original private binding bytes.
        pub closed spec fn binding_view(&self) -> Seq<u8> { self.binding@ }
    }
    sail_method! {
        /// Copy the request before the unconfigured attempt becomes a child.
        pub fn for_attempt(attempt: &UnconfiguredAttempt<'_>, binding: &[u8]) -> (request: Self)
            requires (true)
            ensures (request.run_view() == unconfigured_run(attempt)@,
                same_unit(request.unit_view(), *unconfigured_unit(attempt)),
                request.slot_view() == unconfigured_slot(attempt), request.binding_view() == binding@)
        {
            Self { run: copy_bytes(attempt.run()), unit: copy_compiler_unit(attempt.unit()), slot: attempt.slot(), binding: copy_bytes(binding) }
        }
    }
}

sail_verified! {
    /// Copy every raw unit field while preserving absent versus empty names.
    #[allow(clippy::manual_map, reason = "Explicit Option branches retain the copied-byte relation in both checked and native bodies.")]
    pub fn copy_compiler_unit(unit: &CompilerUnit) -> (copied: CompilerUnit)
        requires (true) ensures (same_unit(copied, *unit))
    {
        let cargo_name = match &unit.cargo_name {
            Some(name) => Some(copy_bytes(name)), None => None,
        };
        CompilerUnit { manifest: copy_bytes(&unit.manifest), name: copy_bytes(&unit.name),
            crate_type: copy_bytes(&unit.crate_type), cargo_name }
    }
}

sail_verified! {
    /// Compare all raw compiler-unit fields, including absent versus empty Cargo names.
    pub fn compiler_units_equal(a: &CompilerUnit, b: &CompilerUnit) -> (equal: bool)
        requires (true) ensures (equal == same_unit(*a, *b))
    {
        byte_eq(&a.manifest, &b.manifest) && byte_eq(&a.name, &b.name)
            && byte_eq(&a.crate_type, &b.crate_type) && match (&a.cargo_name, &b.cargo_name) {
                (Some(a), Some(b)) => byte_eq(a, b), (None, None) => true, _ => false,
            }
    }
}

sail_verified! {
    /// Join the original request to the actual sealed child attempt.
    pub fn request_matches(request: &PhaseRequest, observed: &ObservedAttempt<'_>) -> (matches: bool)
        requires (true)
        ensures (matches == (request.run_view() == reserved_run(&running_reserved(&observed_running(observed)))@
            && same_unit(request.unit_view(), *reserved_unit(&running_reserved(&observed_running(observed))))
            && request.slot_view() == reserved_slot(&running_reserved(&observed_running(observed)))))
    {
        let reserved = observed.reserved();
        byte_eq(&request.run, reserved.run()) && compiler_units_equal(&request.unit, reserved.unit())
            && request.slot == reserved.slot()
    }
}

sail_verified! {
    /// Check a full subset while permitting a legitimately empty phase.
    pub fn inventory_covers(source: &Inventory<'_>, selected: &Inventory<'_>) -> (covers: bool)
        requires (true)
        ensures (covers == (ordered(*source) && ordered(*selected) && subset(source.rows@, selected.rows@)))
    {
        if selected.rows.is_empty() {
            input_order(source.rows, source.order, source.inverse)
                && input_order(selected.rows, selected.order, selected.inverse)
        } else {
            selection_covered(source.rows, selected.rows, source.order, source.inverse,
                selected.order, selected.inverse)
        }
    }
}

sail_verified! {
    /// Find one exact row without rewinding a certified sorted cursor.
    pub fn seek_member(inventory: &Inventory<'_>, start: usize, wanted: &InputFact<'_>) -> (out: (usize, bool))
        requires (ordered(*inventory), start <= inventory.rows.len(),
            forall|k: int| 0 <= k < start ==> key_less(inventory.rows[#[trigger] inventory.order[k] as int], *wanted))
        ensures (start <= out.0 <= inventory.rows.len(),
            out.1 == contains_fact(inventory.rows@, *wanted),
            forall|k: int| 0 <= k < out.0 ==> key_less(inventory.rows[#[trigger] inventory.order[k] as int], *wanted))
    {
        let mut cursor = start;
        sail_while! { (cursor < inventory.rows.len()
            && ordered_key_less(inventory.rows, inventory.order, inventory.inverse, cursor, wanted))
            invariant (start <= cursor <= inventory.rows.len(), ordered(*inventory),
                forall|k: int| 0 <= k < cursor ==> key_less(inventory.rows[#[trigger] inventory.order[k] as int], *wanted))
            decreases (inventory.rows.len() - cursor)
            { cursor += 1; }
        }
        sail_proof! {
            if contains_fact(inventory.rows@, *wanted) {
                ordered_lookup_stops_at_member(inventory.rows@, inventory.order@,
                    inventory.inverse@, *wanted, cursor as int);
            }
        }
        if cursor == inventory.rows.len() { return (cursor, false); }
        sail_proof! { assert(permutation_at(inventory.order@, inventory.inverse@, inventory.rows.len() as nat, cursor as int)); }
        let found = &inventory.rows[inventory.order[cursor]];
        let matched = input_key_eq(found, wanted) && value_difference(&found.value, &wanted.value) == 0;
        sail_proof! {
            if matched { assert(contains_fact(inventory.rows@, *wanted)); }
        }
        (cursor, matched)
    }
}

sail_verified! {
    /// Reject extra joined rows with a linear pass over certified phase orders.
    pub fn joined_rows_recorded(left: &Inventory<'_>, right: &Inventory<'_>, joined: &Inventory<'_>) -> (valid: bool)
        requires (true)
        ensures (valid == (ordered(*left) && ordered(*right) && ordered(*joined) && union_only(*left, *right, *joined)))
    {
        if !input_order(left.rows, left.order, left.inverse)
            || !input_order(right.rows, right.order, right.inverse)
            || !input_order(joined.rows, joined.order, joined.inverse) { return false; }
        let mut i = 0;
        let mut l = 0;
        let mut r = 0;
        sail_while! { (i < joined.rows.len())
            invariant (i <= joined.rows.len(), l <= left.rows.len(), r <= right.rows.len(),
                ordered(*left), ordered(*right), ordered(*joined),
                forall|k: int| 0 <= k < i ==> contains_fact(left.rows@, joined.rows[#[trigger] joined.order[k] as int])
                    || contains_fact(right.rows@, joined.rows[joined.order[k] as int]),
                i < joined.rows.len() ==> forall|k: int| 0 <= k < l ==>
                    key_less(left.rows[#[trigger] left.order[k] as int], joined.rows[joined.order[i as int] as int]),
                i < joined.rows.len() ==> forall|k: int| 0 <= k < r ==>
                    key_less(right.rows[#[trigger] right.order[k] as int], joined.rows[joined.order[i as int] as int]))
            decreases (joined.rows.len() - i)
            {
                sail_proof! { assert(permutation_at(joined.order@, joined.inverse@, joined.rows.len() as nat, i as int)); }
                let wanted = &joined.rows[joined.order[i]];
                let (next_l, in_left) = seek_member(left, l, wanted);
                let (next_r, in_right) = seek_member(right, r, wanted);
                l = next_l; r = next_r;
                if !in_left && !in_right { return false; }
                sail_proof! {
                    if i + 1 < joined.rows.len() {
                        ordered_pair_less(joined.rows@, joined.order@, joined.inverse@, i as int, i as int + 1);
                        assert forall|k: int| 0 <= k < l implies
                            key_less(left.rows[#[trigger] left.order[k] as int], joined.rows[joined.order[i as int + 1] as int]) by {
                            key_less_transitive(left.rows[left.order[k] as int], *wanted, joined.rows[joined.order[i as int + 1] as int]);
                        }
                        assert forall|k: int| 0 <= k < r implies
                            key_less(right.rows[#[trigger] right.order[k] as int], joined.rows[joined.order[i as int + 1] as int]) by {
                            key_less_transitive(right.rows[right.order[k] as int], *wanted, joined.rows[joined.order[i as int + 1] as int]);
                        }
                    }
                }
                i += 1;
            }
        }
        sail_proof! {
            assert forall|p: int| 0 <= p < joined.rows.len() implies
                contains_fact(left.rows@, #[trigger] joined.rows[p]) || contains_fact(right.rows@, joined.rows[p]) by {
                assert(permutation_at(joined.order@, joined.inverse@, joined.rows.len() as nat, p));
                let rank = joined.inverse[p];
                assert(joined.order[rank as int] == p);
            }
        }
        true
    }
}

sail_spec! {
    /// Exact original request and actual completed child correspondence.
    pub open spec fn matching_request(request: PhaseRequest, observed: &ObservedAttempt) -> bool {
        request.run_view() == reserved_run(&running_reserved(&observed_running(observed)))@
            && same_unit(request.unit_view(), *reserved_unit(&running_reserved(&observed_running(observed))))
            && request.slot_view() == reserved_slot(&running_reserved(&observed_running(observed)))
    }
    /// Both known phases and the literal existing final-document fields.
    pub open spec fn matching_document(request: PhaseRequest, observed: &ObservedAttempt,
        document: PhaseDocument) -> bool {
        document.kind@ == b"sail-compiler-read-inputs"@ && document.schema == 1
            && document.complete && document.pid != 0
            && document.pid == running_pid(&observed_running(observed))
            && document.binding@ == request.binding_view() && request.binding_view().len() > 0
            && document.phases.len() == 2
            && (document.phases[0].tag == 0 && document.phases[1].tag == 1
                || document.phases[0].tag == 1 && document.phases[1].tag == 0)
    }
    /// Exactly the recorded declaration union, not complete physical inputs.
    pub open spec fn exact_join(document: PhaseDocument, joined: Inventory) -> bool {
        document.phases.len() == 2 && ordered(document.phases[0].fields)
            && ordered(document.phases[1].fields) && ordered(joined)
            && subset(joined.rows@, document.phases[0].fields.rows@)
            && subset(joined.rows@, document.phases[1].fields.rows@)
            && union_only(document.phases[0].fields, document.phases[1].fields, joined)
    }
}

impl<'a, 'b> PhaseInputs<'a, 'b> {
    sail_impl_spec! {
        /// Original pre-spawn request retained without rewriting.
        pub closed spec fn request_view(&self) -> PhaseRequest { self.request }
        /// Actual process observation retained by this declaration admission.
        pub closed spec fn observed_view(&self) -> &'a ObservedAttempt<'b> { self.observed }
        /// Exact parsed provider document retained by this admission.
        pub closed spec fn document_view(&self) -> &'a PhaseDocument<'a> { self.document }
        /// Exact joined declaration rows, with their validated ordering proposals.
        pub closed spec fn joined_view(&self) -> Inventory<'a> { self.joined }
    }
    sail_method! {
        /// Borrow every joined declared fact; no field can be removed here.
        pub fn facts(&self) -> (facts: &'a [InputFact<'a>])
            requires (true) ensures (facts@ == self.joined_view().rows@)
        { self.joined.rows }
    }
    sail_method! {
        /// Borrow the same actual child observation used for admission.
        pub fn observed(&self) -> (observed: &'a ObservedAttempt<'b>)
            requires (true) ensures (observed == self.observed_view())
        { self.observed }
    }
    sail_method! {
        /// Borrow the immutable original request binding.
        pub fn binding(&self) -> (binding: &[u8])
            requires (true) ensures (binding@ == self.request_view().binding_view())
        { &self.request.binding }
    }
    sail_method! {
        /// Return the actual retained phase count, never an inferred completion bit.
        pub fn phase_count(&self) -> (count: usize)
            requires (true) ensures (count == self.document_view().phases.len())
        { self.document.phases.len() }
    }
}

sail_verified! {
    /// Admit one exact same-child two-phase declaration union.
    /// A normal nonzero exit may finish accounting; this never establishes a proof.
    pub fn seal_phase_inputs<'a, 'b>(request: PhaseRequest, observed: &'a ObservedAttempt<'b>,
        document: &'a PhaseDocument<'a>, joined: Inventory<'a>) -> (seal: Option<PhaseInputs<'a, 'b>>)
        requires (observation_complete(&observed_running(observed)))
        ensures (seal.is_some() == (matching_request(request, observed)
                && matching_document(request, observed, *document)
                && running_exit(&observed_running(observed)).unwrap().is_some()
                && exact_join(*document, joined)),
            seal.is_some() ==> seal.unwrap().request_view() == request
                && seal.unwrap().observed_view() == observed && seal.unwrap().document_view() == document
                && seal.unwrap().joined_view() == joined)
    {
        if !request_matches(&request, observed) || request.binding.is_empty()
            || !byte_eq(document.kind, b"sail-compiler-read-inputs") || document.schema != 1
            || !document.complete || document.pid == 0 || document.pid != observed.pid()
            || !byte_eq(document.binding, &request.binding) || observed.exit().is_none()
            || document.phases.len() != 2 { return None; }
        let first = &document.phases[0];
        let second = &document.phases[1];
        if !(first.tag == 0 && second.tag == 1 || first.tag == 1 && second.tag == 0) { return None; }
        if !inventory_covers(&joined, &first.fields) || !inventory_covers(&joined, &second.fields)
            || !joined_rows_recorded(&first.fields, &second.fields, &joined) { return None; }
        Some(PhaseInputs { request, observed, document, joined })
    }
}
