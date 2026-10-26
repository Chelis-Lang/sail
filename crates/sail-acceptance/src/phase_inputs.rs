//! Exact accounting of the two recorded compiler-declaration phases.
//!
//! This does not certify complete physical compiler inputs. Native parsing,
//! protected producer correspondence and opaque compiler I/O remain separate.

#[cfg(sail_verus)]
use vstd::prelude::*;

use crate::authority::{
    CompilerUnit, ObservedAttempt, ProcessDescriptor, ReservedAttempt, RunningAttempt,
    UnsealedOutput,
};
#[cfg(sail_verus)]
use crate::authority::{
    observation_complete, observed_running, reserved_descriptor, reserved_run, reserved_slot,
    reserved_unit, running_exit, running_invalid, running_pid, running_reserved, running_stderr,
    running_stdout,
};
// Preserve the existing phase API while sharing one original ownership primitive.
pub use crate::inputs::OwnerIdentity;
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

    /// Original final command identity sealed after configuration and before spawn.
    pub struct PhaseRequest {
        run: Vec<u8>,
        unit: CompilerUnit,
        slot: usize,
        binding: Vec<u8>,
        descriptor: ProcessDescriptor,
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
        /// Child PID reported by the producer; checked against the observed attempt.
        pub pid: u32,
        /// Original private request binding echoed by the producer.
        pub binding: &'a [u8],
        /// Existing final-document field, not an inferred all-inputs predicate.
        pub complete: bool,
        /// Every parsed phase, including unknown tags that must be refused.
        pub phases: &'a [PhaseRecord<'a>],
    }

    /// Request-matching two-phase declarations; never a `CompilerDomain` or authenticated file origin.
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
            && same_optional(a.cargo_name, b.cargo_name)
    }
    /// Absence and empty values remain distinct for directories and overrides.
    pub open spec fn same_optional(a: Option<Vec<u8>>, b: Option<Vec<u8>>) -> bool {
        match (a, b) { (Some(a), Some(b)) => a@ == b@, (None, None) => true, _ => false }
    }
    /// Every ordered argument is preserved as actual bytes, not a digest.
    pub open spec fn same_arguments(a: Seq<Vec<u8>>, b: Seq<Vec<u8>>) -> bool {
        a.len() == b.len() && forall|i: int| 0 <= i < a.len() ==> #[trigger] a[i]@ == b[i]@
    }
    /// Every ordered environment operation keeps its exact name and value/removal.
    pub open spec fn same_overrides(a: Seq<(Vec<u8>, Option<Vec<u8>>)>, b: Seq<(Vec<u8>, Option<Vec<u8>>)>) -> bool {
        a.len() == b.len() && forall|i: int| 0 <= i < a.len() ==>
            #[trigger] a[i].0@ == b[i].0@ && same_optional(a[i].1, b[i].1)
    }
    /// All fields of the final configured command participate in the request.
    pub open spec fn same_descriptor(a: ProcessDescriptor, b: ProcessDescriptor) -> bool {
        a.program@ == b.program@ && same_arguments(a.arguments@, b.arguments@)
            && same_optional(a.directory, b.directory) && same_overrides(a.overrides@, b.overrides@)
    }
    /// Compose exact byte-field correspondence without treating copied Vec
    /// allocation identities as argument or environment identity.
    pub proof fn descriptor_correspondence_transitive(a: ProcessDescriptor, b: ProcessDescriptor, c: ProcessDescriptor)
        requires same_descriptor(a, b), same_descriptor(b, c),
        ensures same_descriptor(a, c),
    {
        assert(same_optional(a.directory, c.directory));
        assert forall |i: int| 0 <= i < a.arguments@.len() implies
            #[trigger] a.arguments@[i]@ == c.arguments@[i]@ by {
            assert(a.arguments@[i]@ == b.arguments@[i]@);
            assert(b.arguments@[i]@ == c.arguments@[i]@);
        }
        assert forall |i: int| 0 <= i < a.overrides@.len() implies
            #[trigger] a.overrides@[i].0@ == c.overrides@[i].0@
                && same_optional(a.overrides@[i].1, c.overrides@[i].1) by {
            assert(a.overrides@[i].0@ == b.overrides@[i].0@);
            assert(b.overrides@[i].0@ == c.overrides@[i].0@);
            assert(same_optional(a.overrides@[i].1, b.overrides@[i].1));
            assert(same_optional(b.overrides@[i].1, c.overrides@[i].1));
            assert(same_optional(a.overrides@[i].1, c.overrides@[i].1));
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
        /// The complete final command captured before the child was spawned.
        pub closed spec fn descriptor_view(&self) -> ProcessDescriptor { self.descriptor }
    }
    sail_method! {
        /// Seal the request after every command configuration and before spawn.
        /// Consuming the reservation prevents a history borrow from attaching
        /// a fresh original binding after completion.
        ///
        /// ```compile_fail,E0308
        /// use sail_acceptance::{authority::ObservedAttempt, phase_inputs::PhaseRequest};
        /// let _ = |history: &ObservedAttempt<'_>| {
        ///     PhaseRequest::for_reserved(history.reserved(), b"new-binding")
        /// };
        /// ```
        pub fn for_reserved<'a>(attempt: ReservedAttempt<'a>, binding: &[u8]) -> (out: (Self, ReservedAttempt<'a>))
            requires (true)
            ensures (out.1 == attempt, out.0.run_view() == reserved_run(&attempt)@,
                same_unit(out.0.unit_view(), *reserved_unit(&attempt)),
                out.0.slot_view() == reserved_slot(&attempt), out.0.binding_view() == binding@,
                same_descriptor(out.0.descriptor_view(), reserved_descriptor(&attempt)))
        {
            let request = Self { run: copy_bytes(attempt.run()), unit: copy_compiler_unit(attempt.unit()), slot: attempt.slot(), binding: copy_bytes(binding), descriptor: copy_descriptor(attempt.descriptor()) };
            (request, attempt)
        }
    }
}

sail_verified! {
    /// Copy an optional field without turning absence into an empty byte value.
    #[allow(clippy::manual_map, reason = "Explicit Option branches retain the copied-byte relation in both checked and native bodies.")]
    pub fn copy_optional_bytes(value: &Option<Vec<u8>>) -> (copied: Option<Vec<u8>>)
        requires (true) ensures (same_optional(copied, *value))
    {
        match value { Some(value) => Some(copy_bytes(value)), None => None }
    }
}

sail_verified! {
    /// Preserve exact optional bytes, including removal versus an empty value.
    pub fn optional_bytes_equal(a: &Option<Vec<u8>>, b: &Option<Vec<u8>>) -> (equal: bool)
        requires (true) ensures (equal == same_optional(*a, *b))
    {
        match (a, b) { (Some(a), Some(b)) => byte_eq(a, b), (None, None) => true, _ => false }
    }
}

sail_verified! {
    /// Copy every ordered argument without joining or hashing argument bytes.
    pub fn copy_arguments(values: &[Vec<u8>]) -> (copied: Vec<Vec<u8>>)
        requires (true) ensures (same_arguments(copied@, values@))
    {
        let mut copied: Vec<Vec<u8>> = Vec::new(); let mut i = 0;
        sail_while! { (i < values.len())
            invariant (i <= values.len(), copied.len() == i,
                forall|k: int| 0 <= k < i ==> #[trigger] copied[k]@ == values[k]@)
            decreases (values.len() - i)
            { copied.push(copy_bytes(&values[i])); i += 1; }
        }
        copied
    }
}

sail_verified! {
    /// Compare every argument and its exact position.
    pub fn arguments_equal(a: &[Vec<u8>], b: &[Vec<u8>]) -> (equal: bool)
        requires (true) ensures (equal == same_arguments(a@, b@))
    {
        if a.len() != b.len() { return false; }
        let mut i = 0;
        sail_while! { (i < a.len())
            invariant (i <= a.len(), a.len() == b.len(),
                forall|k: int| 0 <= k < i ==> #[trigger] a[k]@ == b[k]@)
            decreases (a.len() - i)
            { if !byte_eq(&a[i], &b[i]) { return false; } i += 1; }
        }
        true
    }
}

sail_verified! {
    /// Copy ordered environment operations with their original name/value pairs.
    pub fn copy_overrides(values: &[(Vec<u8>, Option<Vec<u8>>)]) -> (copied: Vec<(Vec<u8>, Option<Vec<u8>>)>)
        requires (true) ensures (same_overrides(copied@, values@))
    {
        let mut copied: Vec<(Vec<u8>, Option<Vec<u8>>)> = Vec::new(); let mut i = 0;
        sail_while! { (i < values.len())
            invariant (i <= values.len(), copied.len() == i,
                forall|k: int| 0 <= k < i ==> #[trigger] copied[k].0@ == values[k].0@
                    && same_optional(copied[k].1, values[k].1))
            decreases (values.len() - i)
            {
                let name = copy_bytes(&values[i].0); let value = copy_optional_bytes(&values[i].1);
                copied.push((name, value)); i += 1;
            }
        }
        copied
    }
}

sail_verified! {
    /// Compare the complete ordered override/removal operation list.
    pub fn overrides_equal(a: &[(Vec<u8>, Option<Vec<u8>>)], b: &[(Vec<u8>, Option<Vec<u8>>)]) -> (equal: bool)
        requires (true) ensures (equal == same_overrides(a@, b@))
    {
        if a.len() != b.len() { return false; }
        let mut i = 0;
        sail_while! { (i < a.len())
            invariant (i <= a.len(), a.len() == b.len(),
                forall|k: int| 0 <= k < i ==> #[trigger] a[k].0@ == b[k].0@
                    && same_optional(a[k].1, b[k].1))
            decreases (a.len() - i)
            {
                if !byte_eq(&a[i].0, &b[i].0) || !optional_bytes_equal(&a[i].1, &b[i].1) { return false; }
                sail_proof! {
                    assert(a[i as int].0@ == b[i as int].0@);
                    assert(same_optional(a[i as int].1, b[i as int].1));
                    assert forall|k: int| 0 <= k < i + 1 implies
                        #[trigger] a[k].0@ == b[k].0@ && same_optional(a[k].1, b[k].1) by {
                        if k == i { assert(same_optional(a[k].1, b[k].1)); }
                    }
                }
                i += 1;
            }
        }
        true
    }
}

sail_verified! {
    /// Copy every final command field before its reservation is consumed by spawn.
    pub fn copy_descriptor(value: &ProcessDescriptor) -> (copied: ProcessDescriptor)
        requires (true) ensures (same_descriptor(copied, *value))
    {
        ProcessDescriptor { program: copy_bytes(&value.program), arguments: copy_arguments(&value.arguments),
            directory: copy_optional_bytes(&value.directory), overrides: copy_overrides(&value.overrides) }
    }
}

sail_verified! {
    /// Compare literal final program, ordered argv, directory and environment operations.
    pub fn descriptors_equal(a: &ProcessDescriptor, b: &ProcessDescriptor) -> (equal: bool)
        requires (true) ensures (equal == same_descriptor(*a, *b))
    {
        byte_eq(&a.program, &b.program) && arguments_equal(&a.arguments, &b.arguments)
            && optional_bytes_equal(&a.directory, &b.directory) && overrides_equal(&a.overrides, &b.overrides)
    }
}

sail_verified! {
    /// Copy every raw unit field while preserving absent versus empty names.
    pub fn copy_compiler_unit(unit: &CompilerUnit) -> (copied: CompilerUnit)
        requires (true) ensures (same_unit(copied, *unit))
    {
        let cargo_name = copy_optional_bytes(&unit.cargo_name);
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
            && request.slot_view() == reserved_slot(&running_reserved(&observed_running(observed)))
            && same_descriptor(request.descriptor_view(), reserved_descriptor(&running_reserved(&observed_running(observed))))))
    {
        let reserved = observed.reserved();
        byte_eq(&request.run, reserved.run()) && compiler_units_equal(&request.unit, reserved.unit())
            && request.slot == reserved.slot() && descriptors_equal(&request.descriptor, reserved.descriptor())
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
            && same_descriptor(request.descriptor_view(), reserved_descriptor(&running_reserved(&observed_running(observed))))
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
    /// Check a request-matching two-phase declaration union and reported metadata.
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

sail_code! {
    /// One decoded source declaration, before any role or byte encoding is assigned.
    pub struct SourceDeclaration<'a> {
        /// Exact producer pathname bytes.
        pub name: &'a [u8],
        /// Exact producer checksum text, without interpretation as observed contents.
        pub checksum: &'a [u8],
        /// Exact producer unnormalized source length.
        pub size: u64,
    }
    /// One decoded environment field; absence is distinct from an empty value.
    pub struct EnvironmentDeclaration<'a> {
        /// Exact name bytes; whitespace and invalid UTF-8 are not normalized.
        pub name: &'a [u8],
        /// Exact bytes or explicit absence.
        pub value: Option<&'a [u8]>,
    }
    /// Expanded declaration values, retaining the owned little-endian length.
    pub enum DeclarationValue<'a> {
        /// Original borrowed bytes.
        Bytes(&'a [u8]),
        /// Lossless eight-byte source length encoding.
        Length([u8; 8]),
        /// Explicit absent environment variable.
        Absent,
    }
    /// A raw expanded field; this supplies no observation or admission authority.
    pub struct DeclarationRow<'a> {
        /// Source checksum=0, source length=1, paths path=2, environment=3.
        pub namespace: u8,
        /// Original unmodified name bytes.
        pub name: &'a [u8],
        /// Exact expanded value.
        pub value: DeclarationValue<'a>,
    }
}

sail_spec! {
    /// Exact little-endian representation; all 64 bits participate.
    pub open spec fn encoded_size(value: u64) -> Seq<u8> {
        seq![(value & 255) as u8, ((value >> 8) & 255) as u8, ((value >> 16) & 255) as u8, ((value >> 24) & 255) as u8,
            ((value >> 32) & 255) as u8, ((value >> 40) & 255) as u8, ((value >> 48) & 255) as u8, ((value >> 56) & 255) as u8]
    }
    /// Exact raw declaration-to-inventory conversion without UTF-8 conversion.
    pub open spec fn declaration_fact(row: DeclarationRow, fact: InputFact) -> bool {
        fact.namespace == row.namespace && fact.name@ == row.name@ && match row.value {
            DeclarationValue::Bytes(bytes) => match fact.value { crate::inputs::InputValue::Present(value) => value@ == bytes@, _ => false },
            DeclarationValue::Length(bytes) => match fact.value { crate::inputs::InputValue::Present(value) => value@ == bytes@, _ => false },
            DeclarationValue::Absent => fact.value is Absent,
        }
    }
    /// Exact source checksum field; no hash truth is assumed.
    pub open spec fn checksum_row(row: DeclarationRow, source: SourceDeclaration) -> bool {
        row.namespace == 0 && row.name@ == source.name@ && match row.value {
            DeclarationValue::Bytes(value) => value@ == source.checksum@, _ => false,
        }
    }
    /// Exact source length field, paired with the same original source name.
    pub open spec fn size_row(row: DeclarationRow, source: SourceDeclaration) -> bool {
        row.namespace == 1 && row.name@ == source.name@ && match row.value {
            DeclarationValue::Length(value) => value@ == encoded_size(source.size), _ => false,
        }
    }
    /// A tracked path is a request, with no invented attestation of its contents.
    pub open spec fn tracked_row(row: DeclarationRow, name: &[u8]) -> bool {
        row.namespace == 2 && row.name@ == name@ && match row.value {
            DeclarationValue::Bytes(value) => value@ == Seq::<u8>::empty(), _ => false,
        }
    }
    /// Absence and an empty present value stay distinct, without name normalization.
    pub open spec fn environment_row(row: DeclarationRow, field: EnvironmentDeclaration) -> bool {
        row.namespace == 3 && row.name@ == field.name@ && match (row.value, field.value) {
            (DeclarationValue::Bytes(value), Some(original)) => value@ == original@,
            (DeclarationValue::Absent, None) => true, _ => false,
        }
    }
    /// Exact expansion length and both directions of field correspondence.
    pub open spec fn expanded_declarations(rows: Seq<DeclarationRow>, sources: Seq<SourceDeclaration>,
        paths: Seq<&[u8]>, environment: Seq<EnvironmentDeclaration>) -> bool {
        rows.len() == 2 * sources.len() + paths.len() + environment.len()
            && (forall|i: int| 0 <= i < sources.len() ==> checksum_row(rows[2 * i], #[trigger] sources[i])
                && size_row(rows[2 * i + 1], sources[i]))
            && (forall|i: int| 0 <= i < paths.len() ==> tracked_row(rows[2 * sources.len() + i], #[trigger] paths[i]))
            && (forall|i: int| 0 <= i < environment.len() ==> environment_row(rows[2 * sources.len() + paths.len() + i], #[trigger] environment[i]))
    }
}

sail_verified! {
    /// Encode every bit of a source length with stable existing revision bytes.
    #[allow(clippy::cast_possible_truncation, reason = "Each byte intentionally keeps one disjoint eight-bit portion; the checked encoding retains all 64 bits.")]
    pub fn encode_source_size(value: u64) -> (out: [u8; 8])
        requires (true) ensures (out@ == encoded_size(value))
    {
        [(value & 255) as u8, ((value >> 8) & 255) as u8, ((value >> 16) & 255) as u8, ((value >> 24) & 255) as u8,
            ((value >> 32) & 255) as u8, ((value >> 40) & 255) as u8, ((value >> 48) & 255) as u8, ((value >> 56) & 255) as u8]
    }
}

impl DeclarationRow<'_> {
    sail_method! {
        /// Borrow the exact expanded bytes without converting names or values.
        pub fn fact(&self) -> (out: InputFact<'_>)
            requires (true) ensures (declaration_fact(*self, out))
        {
            InputFact { namespace: self.namespace, name: self.name, value: match &self.value {
                DeclarationValue::Bytes(value) => crate::inputs::InputValue::Present(value),
                DeclarationValue::Length(value) => crate::inputs::InputValue::Present(value),
                DeclarationValue::Absent => crate::inputs::InputValue::Absent,
            } }
        }
    }
}

sail_verified! {
    /// Convert every expanded declaration exactly once, preserving its original position.
    pub fn declaration_facts<'a>(rows: &'a [DeclarationRow<'a>]) -> (facts: Vec<InputFact<'a>>)
        requires (true) ensures (facts.len() == rows.len(),
            forall|i: int| 0 <= i < rows.len() ==> declaration_fact(#[trigger] rows[i], facts[i]))
    {
        let mut facts = Vec::new(); let mut i = 0;
        sail_while! { (i < rows.len())
            invariant (i <= rows.len(), facts.len() == i,
                forall|k: int| 0 <= k < i ==> declaration_fact(#[trigger] rows[k], facts[k]))
            decreases (rows.len() - i)
            { facts.push(rows[i].fact()); i += 1; }
        }
        facts
    }
}

sail_verified! {
    /// Append one field while retaining every earlier declaration exactly.
    pub fn append_declaration<'a>(rows: &mut Vec<DeclarationRow<'a>>, row: DeclarationRow<'a>) -> (out: ())
        requires (old(rows).len() < usize::MAX)
        ensures (final(rows).len() == old(rows).len() + 1, final(rows)[old(rows).len() as int] == row,
            forall|i: int| 0 <= i < old(rows).len() ==> #[trigger] final(rows)[i] == old(rows)[i])
    { rows.push(row); }
}

sail_spec! {
    /// The one permitted declaration at each position of the complete expansion.
    pub open spec fn declaration_at(row: DeclarationRow, sources: Seq<SourceDeclaration>,
        paths: Seq<&[u8]>, environment: Seq<EnvironmentDeclaration>, index: int) -> bool {
        if index < 2 * sources.len() {
            if index % 2 == 0 { checksum_row(row, sources[index / 2]) }
            else { size_row(row, sources[index / 2]) }
        } else if index < 2 * sources.len() + paths.len() {
            tracked_row(row, paths[index - 2 * sources.len()])
        } else { environment_row(row, environment[index - 2 * sources.len() - paths.len()]) }
    }
}

sail_verified! {
    /// Expand every decoded declaration with checked roles, keys, values and order.
    /// Overflow refuses; a complete empty decoded phase stays empty.
    pub fn expand_declarations<'a>(sources: &[SourceDeclaration<'a>], paths: &[&'a [u8]],
        environment: &[EnvironmentDeclaration<'a>]) -> (out: Option<Vec<DeclarationRow<'a>>>)
        requires (true) ensures (out.is_some() == (2 * sources.len() + paths.len() + environment.len() <= usize::MAX),
            out.is_some() ==> expanded_declarations(out.unwrap()@, sources@, paths@, environment@))
    {
        if sources.len() > usize::MAX / 2 { return None; }
        let source_end = sources.len() * 2;
        if paths.len() > usize::MAX - source_end { return None; }
        let paths_end = source_end + paths.len();
        if environment.len() > usize::MAX - paths_end { return None; }
        let total = paths_end + environment.len();
        let mut rows = Vec::new(); let mut i = 0;
        sail_while! { (i < total)
            invariant (i <= total, rows.len() == i, source_end == 2 * sources.len(),
                paths_end == source_end + paths.len(), total == paths_end + environment.len(),
                forall|k: int| 0 <= k < i ==> declaration_at(#[trigger] rows[k], sources@, paths@, environment@, k))
            decreases (total - i)
            {
                let row = if i < source_end {
                    let source = &sources[i / 2];
                    if i % 2 == 0 {
                        DeclarationRow { namespace: 0, name: source.name, value: DeclarationValue::Bytes(source.checksum) }
                    } else {
                        DeclarationRow { namespace: 1, name: source.name, value: DeclarationValue::Length(encode_source_size(source.size)) }
                    }
                } else if i < paths_end {
                    DeclarationRow { namespace: 2, name: paths[i - source_end], value: DeclarationValue::Bytes(b"") }
                } else {
                    let field = &environment[i - paths_end];
                    let value = match field.value { Some(value) => DeclarationValue::Bytes(value), None => DeclarationValue::Absent };
                    DeclarationRow { namespace: 3, name: field.name, value }
                };
                sail_proof! {
                    if i < source_end {
                        if i % 2 == 0 { assert(checksum_row(row, sources[i as int / 2])); }
                        else { assert(size_row(row, sources[i as int / 2])); }
                    } else if i < paths_end {
                        assert(row.namespace == 2);
                        assert(row.name@ == paths[i - source_end]@);
                        assert(match row.value { DeclarationValue::Bytes(value) => value@ == Seq::<u8>::empty(), _ => false });
                        assert(tracked_row(row, paths[i - source_end]));
                    }
                    else { assert(environment_row(row, environment[i - paths_end])); }
                    assert(declaration_at(row, sources@, paths@, environment@, i as int));
                }
                append_declaration(&mut rows, row);
                i += 1;
            }
        }
        sail_proof! {
            assert forall|k: int| 0 <= k < sources.len() implies checksum_row(#[trigger] rows[2 * k], sources[k])
                && size_row(rows[2 * k + 1], sources[k]) by {
                assert(declaration_at(rows[2 * k], sources@, paths@, environment@, 2 * k));
                assert(declaration_at(rows[2 * k + 1], sources@, paths@, environment@, 2 * k + 1));
            }
            assert forall|k: int| 0 <= k < paths.len() implies tracked_row(#[trigger] rows[2 * sources.len() + k], paths[k]) by {
                assert(declaration_at(rows[2 * sources.len() + k], sources@, paths@, environment@, 2 * sources.len() + k));
            }
            assert forall|k: int| 0 <= k < environment.len() implies environment_row(#[trigger] rows[2 * sources.len() + paths.len() + k], environment[k]) by {
                assert(declaration_at(rows[2 * sources.len() + paths.len() + k], sources@, paths@, environment@, 2 * sources.len() + paths.len() + k));
            }
        }
        Some(rows)
    }
}

sail_spec! {
    /// Key membership independent of a proposed replacement value.
    pub open spec fn environment_has_key(rows: Seq<InputFact>, wanted: InputFact) -> bool {
        exists|i: int| 0 <= i < rows.len() && same_key(#[trigger] rows[i], wanted)
    }
    /// Each result row comes from the final override, or an unoverridden parent row.
    pub open spec fn effective_environment_row(parent: Seq<InputFact>, overrides: Seq<InputFact>, wanted: InputFact) -> bool {
        contains_fact(overrides, wanted)
            || !environment_has_key(overrides, wanted) && contains_fact(parent, wanted)
    }
}

sail_verified! {
    /// Find a literal environment key without hashing, trimming or value comparison.
    pub fn environment_key_index(rows: &[InputFact<'_>], wanted: &InputFact<'_>) -> (out: Option<usize>)
        requires (true)
        ensures (out.is_some() == environment_has_key(rows@, *wanted),
            out.is_some() ==> out.unwrap() < rows.len() && same_key(rows[out.unwrap() as int], *wanted))
    {
        let mut i = 0;
        sail_while! { (i < rows.len())
            invariant (i <= rows.len(), forall|j: int| 0 <= j < i ==> !same_key(#[trigger] rows[j], *wanted))
            decreases (rows.len() - i)
            {
                if input_key_eq(&rows[i], wanted) {
                    sail_proof! { assert(environment_has_key(rows@, *wanted)); }
                    return Some(i);
                }
                i += 1;
            }
        }
        None
    }
}

sail_spec! {
    /// Unique keys make a found key the only possible value origin in that inventory.
    pub proof fn environment_unique_member(rows: Seq<InputFact>, wanted: InputFact, found: int)
        requires crate::inputs::unique_input_keys(rows), 0 <= found < rows.len(), same_key(rows[found], wanted),
        ensures contains_fact(rows, wanted) == same_value(rows[found].value, wanted.value),
    {
        if contains_fact(rows, wanted) {
            let i = choose|i: int| 0 <= i < rows.len() && same_key(rows[i], wanted) && same_value(rows[i].value, wanted.value);
            if i != found { assert(!same_key(rows[i], rows[found])); }
        } else if same_value(rows[found].value, wanted.value) {
            assert(contains_fact(rows, wanted));
        }
    }
}

sail_verified! {
    /// The final Command override/removal wins even when an inherited value exists.
    pub fn effective_environment_row_matches(parent: &[InputFact<'_>], overrides: &[InputFact<'_>], wanted: &InputFact<'_>) -> (valid: bool)
        requires (crate::inputs::unique_input_keys(parent@), crate::inputs::unique_input_keys(overrides@))
        ensures (valid == effective_environment_row(parent@, overrides@, *wanted))
    {
        if let Some(index) = environment_key_index(overrides, wanted) {
            sail_proof! { environment_unique_member(overrides@, *wanted, index as int); }
            value_difference(&overrides[index].value, &wanted.value) == 0
        } else if let Some(index) = environment_key_index(parent, wanted) {
            sail_proof! { environment_unique_member(parent@, *wanted, index as int); }
            value_difference(&parent[index].value, &wanted.value) == 0
        } else {
            sail_proof! { assert(!contains_fact(parent@, *wanted)); }
            false
        }
    }
}

sail_verified! {
    /// Refuse extra keys, stale inherited values and replacements that ignore removal.
    pub fn effective_environment_origins(parent: &Inventory<'_>, overrides: &Inventory<'_>, result: &Inventory<'_>) -> (valid: bool)
        requires (true)
        ensures (valid == (ordered(*parent) && ordered(*overrides) && ordered(*result)
            && forall|i: int| 0 <= i < result.rows.len() ==> effective_environment_row(parent.rows@, overrides.rows@, #[trigger] result.rows[i])))
    {
        if !input_order(parent.rows, parent.order, parent.inverse)
            || !input_order(overrides.rows, overrides.order, overrides.inverse)
            || !input_order(result.rows, result.order, result.inverse) { return false; }
        let mut i = 0;
        sail_while! { (i < result.rows.len())
            invariant (i <= result.rows.len(), ordered(*parent), ordered(*overrides), ordered(*result),
                crate::inputs::unique_input_keys(parent.rows@), crate::inputs::unique_input_keys(overrides.rows@),
                forall|j: int| 0 <= j < i ==> effective_environment_row(parent.rows@, overrides.rows@, #[trigger] result.rows[j]))
            decreases (result.rows.len() - i)
            {
                if !effective_environment_row_matches(parent.rows, overrides.rows, &result.rows[i]) { return false; }
                i += 1;
            }
        }
        true
    }
}

sail_verified! {
    /// Every original parent key remains represented, including explicit removals.
    pub fn environment_parent_keys_covered(parent: &Inventory<'_>, result: &Inventory<'_>) -> (valid: bool)
        requires (true)
        ensures (valid == (ordered(*parent) && ordered(*result)
            && forall|i: int| 0 <= i < parent.rows.len() ==> environment_has_key(result.rows@, #[trigger] parent.rows[i])))
    {
        if !input_order(parent.rows, parent.order, parent.inverse)
            || !input_order(result.rows, result.order, result.inverse) { return false; }
        let mut i = 0;
        sail_while! { (i < parent.rows.len())
            invariant (i <= parent.rows.len(), ordered(*parent), ordered(*result),
                forall|j: int| 0 <= j < i ==> environment_has_key(result.rows@, #[trigger] parent.rows[j]))
            decreases (parent.rows.len() - i)
            {
                if environment_key_index(result.rows, &parent.rows[i]).is_none() { return false; }
                i += 1;
            }
        }
        true
    }
}

sail_code! {
    /// Explicit construction policy; never inferred from `Command::get_envs`.
    pub enum EnvironmentInheritance {
        /// The actual command constructor retains the caller's environment.
        Inherit,
        /// The actual command constructor clears inheritance before final overrides.
        Clear,
    }

    /// Exact projection of supplied parent bytes and final override/removal rows.
    /// This token does not authenticate a process or certify an OS iterator's completeness.
    pub struct EnvironmentProjection<'a> {
        inheritance: EnvironmentInheritance,
        parent: Inventory<'a>,
        overrides: Inventory<'a>,
        result: Inventory<'a>,
    }
}

sail_spec! {
    /// Environment names retain arbitrary bytes except the OS's NUL and '=' separators.
    pub open spec fn environment_name_valid(name: Seq<u8>) -> bool {
        name.len() > 0 && forall|i: int| 0 <= i < name.len() ==> name[i] != 0 && name[i] != 61
    }
    /// One literal typed row; inherited values cannot represent removal.
    pub open spec fn environment_kind(row: InputFact, parent: bool) -> bool {
        row.namespace == 3 && environment_name_valid(row.name@)
            && crate::inputs::value_readable(row.value) && (!parent || row.value is Present)
    }
    /// Parent observations are present values; final operations may explicitly remove a key.
    pub open spec fn environment_kinds(rows: Seq<InputFact>, parent: bool) -> bool {
        forall|i: int| 0 <= i < rows.len() ==> environment_kind(#[trigger] rows[i], parent)
    }
    /// Exact supplied-key union with final override priority and explicit removal.
    pub open spec fn environment_projection(parent: Inventory, overrides: Inventory, result: Inventory) -> bool {
        ordered(parent) && ordered(overrides) && ordered(result)
            && environment_kinds(parent.rows@, true) && environment_kinds(overrides.rows@, false)
            && environment_kinds(result.rows@, false)
            && (forall|i: int| 0 <= i < parent.rows.len() ==> environment_has_key(result.rows@, #[trigger] parent.rows[i]))
            && subset(result.rows@, overrides.rows@)
            && (forall|i: int| 0 <= i < result.rows.len() ==> effective_environment_row(parent.rows@, overrides.rows@, #[trigger] result.rows[i]))
    }
}

sail_verified! {
    /// Validate raw names without Unicode conversion or exclusions by prefix.
    pub fn environment_raw_name_valid(name: &[u8]) -> (valid: bool)
        requires (true) ensures (valid == environment_name_valid(name@))
    {
        if name.is_empty() { return false; }
        let mut i = 0;
        sail_while! { (i < name.len())
            invariant (i <= name.len(), name.len() > 0,
                forall|j: int| 0 <= j < i ==> name[j] != 0 && name[j] != 61)
            decreases (name.len() - i)
            { if name[i] == 0 || name[i] == 61 { return false; } i += 1; }
        }
        true
    }
}

sail_verified! {
    /// Refuse unreadable observations and distinguish an absent value from an empty one.
    pub fn environment_rows_valid(rows: &[InputFact<'_>], parent: bool) -> (valid: bool)
        requires (true) ensures (valid == environment_kinds(rows@, parent))
    {
        let mut i = 0;
        sail_while! { (i < rows.len())
            invariant (i <= rows.len(), forall|j: int| 0 <= j < i ==> environment_kind(#[trigger] rows[j], parent))
            decreases (rows.len() - i)
            {
                let row = &rows[i];
                if row.namespace != 3 || !environment_raw_name_valid(row.name) { return false; }
                match &row.value {
                    crate::inputs::InputValue::Unreadable => return false,
                    crate::inputs::InputValue::Absent => if parent { return false; },
                    crate::inputs::InputValue::Present(_) => (),
                }
                sail_proof! {
                    assert(row.namespace == 3 && environment_name_valid(row.name@));
                    assert(environment_kind(*row, parent));
                    assert forall|j: int| 0 <= j < i + 1 implies environment_kind(#[trigger] rows[j], parent) by {
                        if j < i { assert(environment_kind(rows[j], parent)); }
                        else { assert(j == i); }
                    }
                }
                i += 1;
            }
        }
        true
    }
}

impl<'a> EnvironmentProjection<'a> {
    sail_impl_spec! {
        /// Exact original supplied parent rows, including all their raw values.
        pub closed spec fn parent_view(&self) -> Inventory<'a> { self.parent }
        /// Exact final configured environment operations.
        pub closed spec fn overrides_view(&self) -> Inventory<'a> { self.overrides }
        /// Accepted effective environment rows.
        pub closed spec fn result_view(&self) -> Inventory<'a> { self.result }
        /// Explicit construction policy retained through projection.
        pub closed spec fn inheritance_view(&self) -> EnvironmentInheritance { self.inheritance }
    }
    sail_method! {
        /// Original supplied parent bytes, preserved independently of projection policy.
        pub fn parent(&self) -> (rows: &'a [InputFact<'a>])
            requires (true) ensures (rows@ == self.parent_view().rows@)
        { self.parent.rows }
    }
    sail_method! {
        /// Literal final overrides, including explicit removal operations.
        pub fn overrides(&self) -> (rows: &'a [InputFact<'a>])
            requires (true) ensures (rows@ == self.overrides_view().rows@)
        { self.overrides.rows }
    }
    sail_method! {
        /// Actual construction policy supplied at this boundary.
        pub fn inheritance(&self) -> (policy: &EnvironmentInheritance)
            requires (true) ensures (*policy == self.inheritance_view())
        { &self.inheritance }
    }
    sail_method! {
        /// Accepted literal rows; removal remains an explicit `Absent` entry.
        pub fn facts(&self) -> (rows: &'a [InputFact<'a>])
            requires (true) ensures (rows@ == self.result_view().rows@)
        { self.result.rows }
    }
    sail_method! {
        /// Complete validated permutation for deterministic serialization.
        pub fn order(&self) -> (order: &'a [usize])
            requires (true) ensures (order@ == self.result_view().order@)
        { self.result.order }
    }
}

sail_verified! {
    /// Check every parent key, operation and effective result before exposing projected bytes.
    /// Inheritance policy comes from a separate actual command-construction boundary.
    pub fn seal_environment_projection<'a>(inheritance: EnvironmentInheritance, parent: Inventory<'a>, overrides: Inventory<'a>, result: Inventory<'a>) -> (out: Option<EnvironmentProjection<'a>>)
        requires (true)
        ensures (out.is_some() ==> out.unwrap().parent_view() == parent
            && out.unwrap().overrides_view() == overrides && out.unwrap().result_view() == result
            && out.unwrap().inheritance_view() == inheritance,
            out.is_some() == (ordered(parent) && environment_kinds(parent.rows@, true)
                && match inheritance {
                    EnvironmentInheritance::Inherit => environment_projection(parent, overrides, result),
                    EnvironmentInheritance::Clear => environment_projection(Inventory { rows: &[], order: &[], inverse: &[] }, overrides, result),
                }))
    {
        if !input_order(parent.rows, parent.order, parent.inverse) || !environment_rows_valid(parent.rows, true) { return None; }
        let empty = Inventory { rows: &[], order: &[], inverse: &[] };
        let selected = match &inheritance { EnvironmentInheritance::Inherit => &parent, EnvironmentInheritance::Clear => &empty };
        if !environment_rows_valid(overrides.rows, false) || !environment_rows_valid(result.rows, false)
            || !environment_parent_keys_covered(selected, &result)
            || !inventory_covers(&result, &overrides)
            || !effective_environment_origins(selected, &overrides, &result) { return None; }
        Some(EnvironmentProjection { inheritance, parent, overrides, result })
    }
}

sail_code! {
    /// Actual final environment readback agrees with independently recorded builder operations.
    /// OS/Command provenance remains the native construction boundary.
    pub struct CommandEnvironment<'a> {
        projection: EnvironmentProjection<'a>,
        recorded: Inventory<'a>,
    }
}

impl<'a> CommandEnvironment<'a> {
    sail_impl_spec! {
        /// Exact admitted effective projection.
        pub closed spec fn projection_view(&self) -> EnvironmentProjection<'a> { self.projection }
        /// Original wrapper-recorded operations, not reconstructed from readback.
        pub closed spec fn recorded_view(&self) -> Inventory<'a> { self.recorded }
    }
    sail_method! {
        /// Borrow the checked projection without reminting its construction policy.
        pub fn projection(&self) -> (projection: &EnvironmentProjection<'a>)
            requires (true) ensures (*projection == self.projection_view())
        { &self.projection }
    }
    sail_method! {
        /// Retain independent original builder data through readback admission.
        pub fn recorded(&self) -> (rows: &'a [InputFact<'a>])
            requires (true) ensures (rows@ == self.recorded_view().rows@)
        { self.recorded.rows }
    }
}

sail_verified! {
    /// Seal actual `get_envs` readback only after exact bidirectional operation matching.
    #[allow(clippy::manual_map, reason = "The explicit match preserves operation/readback binding through the same native and checked bodies.")]
    pub fn seal_command_environment<'a>(inheritance: EnvironmentInheritance, parent: Inventory<'a>, recorded: Inventory<'a>, actual: Inventory<'a>, result: Inventory<'a>) -> (out: Option<CommandEnvironment<'a>>)
        requires (true)
        ensures (out.is_some() ==> out.unwrap().recorded_view() == recorded
            && out.unwrap().projection_view().parent_view() == parent
            && out.unwrap().projection_view().overrides_view() == actual
            && out.unwrap().projection_view().result_view() == result
            && out.unwrap().projection_view().inheritance_view() == inheritance,
            out.is_some() == (ordered(recorded) && ordered(actual)
                && subset(recorded.rows@, actual.rows@) && subset(actual.rows@, recorded.rows@)
                && ordered(parent) && environment_kinds(parent.rows@, true)
                && match inheritance {
                    EnvironmentInheritance::Inherit => environment_projection(parent, actual, result),
                    EnvironmentInheritance::Clear => environment_projection(Inventory { rows: &[], order: &[], inverse: &[] }, actual, result),
                }))
    {
        if !inventory_covers(&recorded, &actual) || !inventory_covers(&actual, &recorded) { return None; }
        match seal_environment_projection(inheritance, parent, actual, result) {
            Some(projection) => Some(CommandEnvironment { projection, recorded }),
            None => None,
        }
    }
}

sail_code! {
    /// Final raw command sealed while its checked environment projection is live.
    /// This establishes construction correspondence, not a compiler input domain.
    pub struct ConfiguredIntent {
        descriptor: ProcessDescriptor,
        inheritance: EnvironmentInheritance,
    }

    /// Actual spawn readouts proposed by the private native owned-child adapter.
    /// The fields alone carry no process or compiler authority.
    pub struct OwnedSpawnFields<'a, Owner> {
        /// Native ownership lease copied from the actual owned child.
        pub owner: Owner,
        /// Actual owned child process ID.
        pub pid: u32,
        /// Configuration sealed before that owned child was spawned.
        pub configuration: Option<&'a ConfiguredIntent>,
    }

    /// Source-checked original reservation and native owned spawn.
    pub struct OwnedCompilerAttempt<'a, Owner> {
        running: RunningAttempt<'a>,
        owner: Owner,
        pid: u32,
        descriptor: ProcessDescriptor,
        inheritance: EnvironmentInheritance,
    }

    /// Actual completion readouts and original buffers from the private native adapter.
    /// Missing wait and a waited signal are distinct; no success Boolean is supplied.
    pub struct OwnedCompletionFields<Owner> {
        /// Same native ownership lease retained by the completed child.
        pub owner: Option<Owner>,
        /// Actual completed child PID, absent for unobserved planning output.
        pub pid: Option<u32>,
        /// Original pre-spawn checked construction, when available.
        pub configuration: Option<ConfiguredIntent>,
        /// Outer absence is no wait; inner absence is an actual signal.
        #[allow(clippy::option_option, reason = "No wait and an observed signaled exit are different primitive outcomes.")]
        pub wait: Option<Option<i32>>,
        /// Original complete stdout buffer, preserved on refusal.
        pub stdout: Vec<u8>,
        /// Original complete stderr buffer, preserved on refusal.
        pub stderr: Vec<u8>,
    }
}

sail_spec! {
    /// One final raw override/removal has the exact typed environment row.
    pub open spec fn descriptor_environment_row(operation: (Vec<u8>, Option<Vec<u8>>), row: InputFact) -> bool {
        row.namespace == 3 && row.name@ == operation.0@
            && match (operation.1, row.value) {
                (Some(value), crate::inputs::InputValue::Present(bytes)) => value@ == bytes@,
                (None, crate::inputs::InputValue::Absent) => true,
                _ => false,
            }
    }
    /// Every final native environment operation retains its original row position.
    pub open spec fn descriptor_environment(descriptor: ProcessDescriptor, rows: Seq<InputFact>) -> bool {
        descriptor.overrides.len() == rows.len()
            && forall|i: int| 0 <= i < rows.len() ==> descriptor_environment_row(descriptor.overrides[i], #[trigger] rows[i])
    }
}

sail_verified! {
    /// Bind a native descriptor to the actual rows admitted by its environment factory.
    pub fn descriptor_environment_matches(descriptor: &ProcessDescriptor, rows: &[InputFact<'_>]) -> (matches: bool)
        requires (true) ensures (matches == descriptor_environment(*descriptor, rows@))
    {
        if descriptor.overrides.len() != rows.len() { return false; }
        let mut i = 0;
        sail_while! { (i < rows.len())
            invariant (i <= rows.len(), descriptor.overrides.len() == rows.len(),
                forall|j: int| 0 <= j < i ==> descriptor_environment_row(descriptor.overrides[j], #[trigger] rows[j]))
            decreases (rows.len() - i)
            {
                let operation = &descriptor.overrides[i];
                let value = match &operation.1 {
                    Some(bytes) => crate::inputs::InputValue::Present(bytes),
                    None => crate::inputs::InputValue::Absent,
                };
                let expected = InputFact { namespace: 3, name: &operation.0, value };
                if !input_key_eq(&expected, &rows[i]) || value_difference(&expected.value, &rows[i].value) != 0 { return false; }
                sail_proof! { assert(descriptor_environment_row(descriptor.overrides[i as int], rows[i as int])); }
                i += 1;
            }
        }
        true
    }
}

impl ConfiguredIntent {
    sail_impl_spec! {
        /// Exact retained raw final command.
        pub closed spec fn descriptor_view(&self) -> ProcessDescriptor { self.descriptor }
        /// Original explicit inheritance policy.
        pub closed spec fn inheritance_view(&self) -> EnvironmentInheritance { self.inheritance }
    }
    sail_method! {
        /// Borrow the retained descriptor without changing its construction origin.
        pub fn descriptor(&self) -> (out: &ProcessDescriptor)
            requires (true) ensures (*out == self.descriptor_view())
        { &self.descriptor }
    }
    sail_method! {
        /// Borrow the original explicit inheritance policy retained by configuration.
        pub fn inheritance(&self) -> (out: &EnvironmentInheritance)
            requires (true) ensures (*out == self.inheritance_view())
        { &self.inheritance }
    }
}

sail_verified! {
    /// Check the final command against its original records and checked environment.
    pub fn seal_configured_intent(recorded: &ProcessDescriptor, actual: &ProcessDescriptor, environment: &CommandEnvironment<'_>) -> (out: Option<ConfiguredIntent>)
        requires (true)
        ensures (out.is_some() == (same_descriptor(*recorded, *actual)
            && descriptor_environment(*actual, environment.projection_view().overrides_view().rows@)),
            out.is_some() ==> same_descriptor(out.unwrap().descriptor_view(), *actual)
                && out.unwrap().inheritance_view() == environment.projection_view().inheritance_view())
    {
        if !descriptors_equal(recorded, actual) || !descriptor_environment_matches(actual, environment.projection().overrides()) { return None; }
        let inheritance = match environment.projection().inheritance() {
            EnvironmentInheritance::Inherit => EnvironmentInheritance::Inherit,
            EnvironmentInheritance::Clear => EnvironmentInheritance::Clear,
        };
        Some(ConfiguredIntent { descriptor: copy_descriptor(actual), inheritance })
    }
}

#[allow(
    clippy::elidable_lifetime_names,
    reason = "The named lifetime appears in the stock-Verus specification erased from Cargo."
)]
impl<'a, Owner> OwnedCompilerAttempt<'a, Owner> {
    sail_impl_spec! {
        /// Original reserved descriptor and initially empty observed process state.
        pub closed spec fn running_view(&self) -> RunningAttempt<'a> { self.running }
        /// Original owned-child lease, not an integer process label.
        pub closed spec fn owner_view(&self) -> Owner { self.owner }
        /// Original observed PID.
        pub closed spec fn pid_view(&self) -> u32 { self.pid }
        /// Independently reserved raw command.
        pub closed spec fn descriptor_view(&self) -> ProcessDescriptor { self.descriptor }
        /// Original configured inheritance policy retained at the owned spawn.
        pub closed spec fn inheritance_view(&self) -> EnvironmentInheritance { self.inheritance }
        /// The start factory creates no stream or wait observations.
        pub closed spec fn ready(&self) -> bool {
            self.pid != 0 && running_pid(&self.running) == self.pid
                && same_descriptor(self.descriptor, reserved_descriptor(&running_reserved(&self.running)))
                && running_stdout(&self.running).is_none() && running_stderr(&self.running).is_none()
                && running_exit(&self.running).is_none() && !running_invalid(&self.running)
        }
    }
}

sail_verified! {
    /// Bind an original reservation to owned spawn fields, never a supplied match flag.
    #[allow(clippy::manual_let_else, clippy::question_mark, reason = "Preserve the explicit refusal branch checked verbatim by stock Verus.")]
    pub fn begin_owned_compiler<'a, Owner>(reserved: ReservedAttempt<'a>, fields: OwnedSpawnFields<'_, Owner>) -> (out: Option<OwnedCompilerAttempt<'a, Owner>>)
        requires (true)
        ensures (out.is_some() == (fields.pid != 0 && fields.configuration.is_some()
            && same_descriptor(reserved_descriptor(&reserved), fields.configuration.unwrap().descriptor_view())),
            out.is_some() ==> out.unwrap().ready()
                && running_reserved(&out.unwrap().running_view()) == reserved
                && out.unwrap().owner_view() == fields.owner && out.unwrap().pid_view() == fields.pid
                && out.unwrap().inheritance_view() == fields.configuration.unwrap().inheritance_view()
                && same_descriptor(out.unwrap().descriptor_view(), fields.configuration.unwrap().descriptor_view()))
    {
        let configuration = match fields.configuration { Some(value) => value, None => return None };
        if !descriptors_equal(reserved.descriptor(), configuration.descriptor()) { return None; }
        let descriptor = copy_descriptor(reserved.descriptor());
        sail_proof! {
            descriptor_correspondence_transitive(descriptor, reserved_descriptor(&reserved), configuration.descriptor_view());
        }
        let inheritance = match configuration.inheritance() {
            EnvironmentInheritance::Inherit => EnvironmentInheritance::Inherit,
            EnvironmentInheritance::Clear => EnvironmentInheritance::Clear,
        };
        match reserved.started(fields.pid) {
            Some(running) => Some(OwnedCompilerAttempt { running, owner: fields.owner, pid: fields.pid, descriptor, inheritance }),
            None => None,
        }
    }
}

sail_spec! {
    /// Exact completion matching uses concrete fields, not a host-wide success premise.
    pub open spec fn owned_completion_matches<Owner, Identity: OwnerIdentity<Owner>>(attempt: OwnedCompilerAttempt<Owner>, completion: OwnedCompletionFields<Owner>) -> bool {
        completion.owner.is_some() && completion.pid == Some(attempt.pid_view())
            && completion.configuration.is_some() && completion.wait.is_some()
            && Identity::identity(&attempt.owner_view()) == Identity::identity(&completion.owner.unwrap())
            && same_descriptor(attempt.descriptor_view(), completion.configuration.unwrap().descriptor_view())
            && attempt.inheritance_view() == completion.configuration.unwrap().inheritance_view()
    }
    /// Success and refusal retain the original completed buffers/status exactly.
    pub open spec fn owned_completion_output<Owner>(result: Result<ObservedAttempt, UnsealedOutput>, completion: OwnedCompletionFields<Owner>) -> bool {
        match result {
            Ok(observed) => running_stdout(&observed_running(&observed)) == Some(completion.stdout)
                && running_stderr(&observed_running(&observed)) == Some(completion.stderr)
                && running_exit(&observed_running(&observed)) == completion.wait,
            Err(raw) => raw.stdout == completion.stdout && raw.stderr == completion.stderr
                && raw.exit == match completion.wait { Some(exit) => exit, None => None },
        }
    }
}

sail_verified! {
    /// Explicit construction policy remains distinct even when final override rows match.
    pub fn environment_inheritance_equal(original: &EnvironmentInheritance, actual: &EnvironmentInheritance) -> (matches: bool)
        requires (true) ensures (matches == (*original == *actual))
    {
        matches!((original, actual),
            (EnvironmentInheritance::Inherit, EnvironmentInheritance::Inherit)
                | (EnvironmentInheritance::Clear, EnvironmentInheritance::Clear))
    }
}

sail_verified! {
    /// Compare original and actual owner/PID/command fields with only identity as a primitive.
    pub fn owned_identity_matches<Owner, Identity>(identity: &Identity, original_owner: &Owner, original_pid: u32, original_descriptor: &ProcessDescriptor,
        actual_owner: &Owner, actual_pid: u32, actual_descriptor: &ProcessDescriptor) -> (matches: bool)
        where (Identity: OwnerIdentity<Owner>)
        requires (true)
        ensures (matches == (Identity::identity(original_owner) == Identity::identity(actual_owner)
            && original_pid == actual_pid && same_descriptor(*original_descriptor, *actual_descriptor)))
    {
        identity.same_owner(original_owner, actual_owner) && original_pid == actual_pid
            && descriptors_equal(original_descriptor, actual_descriptor)
    }
}

sail_verified! {
    /// Shared admission and original-buffer transfer for one owned compiler completion.
    /// Failing exits and signals remain observations; missing wait refuses authority.
    ///
    /// # Errors
    /// Returns the original buffers and observed exit projection when original
    /// start, owner, PID, configuration, policy or wait correspondence is missing.
    #[allow(clippy::manual_let_else, clippy::manual_unwrap_or_default, reason = "Preserve explicit wait projection and original-output refusal branches in the source checked by stock Verus.")]
    pub fn complete_owned_compiler<'a, Owner, Identity>(identity: &Identity, attempt: Option<OwnedCompilerAttempt<'a, Owner>>, completion: OwnedCompletionFields<Owner>) -> (out: Result<ObservedAttempt<'a>, UnsealedOutput>)
        where (Identity: OwnerIdentity<Owner>)
        requires (attempt.is_some() ==> attempt.unwrap().ready())
        ensures (out.is_ok() == (attempt.is_some() && owned_completion_matches::<Owner, Identity>(attempt.unwrap(), completion)),
            owned_completion_output(out, completion),
            out.is_ok() ==> observation_complete(&observed_running(&out.unwrap())),
            out.is_ok() ==> running_reserved(&observed_running(&out.unwrap())) == running_reserved(&attempt.unwrap().running_view())
                && running_pid(&observed_running(&out.unwrap())) == attempt.unwrap().pid_view())
    {
        let exit = match completion.wait { Some(exit) => exit, None => None };
        let attempt = match attempt {
            Some(attempt) => attempt,
            None => return Err(UnsealedOutput { stdout: completion.stdout, stderr: completion.stderr, exit }),
        };
        let owner = match &completion.owner {
            Some(owner) => owner,
            None => return Err(UnsealedOutput { stdout: completion.stdout, stderr: completion.stderr, exit }),
        };
        let configuration = match &completion.configuration {
            Some(configuration) => configuration,
            None => return Err(UnsealedOutput { stdout: completion.stdout, stderr: completion.stderr, exit }),
        };
        let pid = match completion.pid {
            Some(pid) => pid,
            None => return Err(UnsealedOutput { stdout: completion.stdout, stderr: completion.stderr, exit }),
        };
        if completion.wait.is_none()
            || !environment_inheritance_equal(&attempt.inheritance, configuration.inheritance())
            || !owned_identity_matches(identity, &attempt.owner, attempt.pid, &attempt.descriptor,
                owner, pid, configuration.descriptor()) {
            return Err(UnsealedOutput { stdout: completion.stdout, stderr: completion.stderr, exit });
        }
        attempt.running.stdout_eof(completion.stdout).stderr_eof(completion.stderr).exited(exit).finish_or_output()
    }
}
