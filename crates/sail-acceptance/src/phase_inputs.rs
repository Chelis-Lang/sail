//! Exact accounting of the two recorded compiler-declaration phases.
//!
//! This does not certify complete physical compiler inputs. Native parsing,
//! protected producer correspondence and opaque compiler I/O remain separate.

#[cfg(sail_verus)]
use vstd::prelude::*;

use crate::authority::{CompilerUnit, ObservedAttempt, ProcessDescriptor, ReservedAttempt};
#[cfg(sail_verus)]
use crate::authority::{
    observation_complete, observed_running, reserved_descriptor, reserved_run, reserved_slot,
    reserved_unit, running_exit, running_pid, running_reserved,
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
