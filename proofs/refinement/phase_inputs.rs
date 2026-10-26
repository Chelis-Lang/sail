//! Same-attempt two-phase declaration accounting and constructive controls.
#![forbid(unsafe_code)]

use crate::authority::*;
use crate::inputs::*;
use crate::phase_inputs::*;
use vstd::prelude::*;

verus! {

/// A concrete finite observation trace; this is not a model of OS success.
pub fn complete_phase_attempt<'a>(attempt: ReservedAttempt<'a>, pid: u32, exit: Option<i32>) -> (out: Option<ObservedAttempt<'a>>)
    requires pid != 0,
    ensures out.is_some(),
        out.is_some() ==> observation_complete(&observed_running(&out.unwrap()))
            && running_reserved(&observed_running(&out.unwrap())) == attempt
            && running_pid(&observed_running(&out.unwrap())) == pid
            && running_exit(&observed_running(&out.unwrap())).unwrap() == exit,
{
    let running = attempt.started(pid);
    let running = match running { Some(running) => running, None => { assert(false); return None; } };
    running.stdout_eof(Vec::new()).stderr_eof(Vec::new()).exited(exit).finish()
}

/// Verification-only environment reads survive the union with compilation,
/// including an absent value. Completion accounting permits normal exit one.
pub fn phase_union_preserves_ghost_environment_and_failure() {
    let run = vec![1u8];
    let unit = CompilerUnit { manifest: vec![2u8], name: vec![3u8], crate_type: vec![4u8], cargo_name: None };
    let binding = vec![5u8];
    let mut ledger = AttemptLedger::new(&run, &unit);
    let allocated = ledger.allocate();
    assert(allocated.is_some());
    let allocated = match allocated { Some(a) => a, None => { assert(false); return; } };
    let reserved = allocated.configure(ProcessDescriptor { program: vec![6u8], arguments: Vec::new(),
        directory: None, overrides: Vec::new() });
    let (request, reserved) = PhaseRequest::for_reserved(reserved, &binding);
    let observed = complete_phase_attempt(reserved, 7, Some(1));
    let observed = match observed { Some(a) => a, None => { assert(false); return; } };

    let ghost_name = vec![10u8];
    let shared_name = vec![11u8];
    let value = vec![12u8];
    let verification = vec![
        InputFact { namespace: 3, name: &ghost_name, value: InputValue::Present(&value) },
        InputFact { namespace: 3, name: &shared_name, value: InputValue::Absent },
    ];
    let compilation = vec![InputFact { namespace: 3, name: &shared_name, value: InputValue::Absent }];
    let joined = vec![
        InputFact { namespace: 3, name: &ghost_name, value: InputValue::Present(&value) },
        InputFact { namespace: 3, name: &shared_name, value: InputValue::Absent },
    ];
    let two = vec![0usize, 1usize];
    let one = vec![0usize];
    let phases = vec![
        PhaseRecord { tag: 0, fields: Inventory { rows: &verification, order: &two, inverse: &two } },
        PhaseRecord { tag: 1, fields: Inventory { rows: &compilation, order: &one, inverse: &one } },
    ];
    let document = PhaseDocument { kind: b"sail-compiler-read-inputs", schema: 1, pid: 7, binding: &binding,
        complete: true, phases: &phases };
    let joined_fields = Inventory { rows: &joined, order: &two, inverse: &two };
    proof {
        assert(lex_at(ghost_name@, shared_name@, 0));
        assert(lex_less(ghost_name@, shared_name@));
        assert(matching_request(request, &observed));
        assert(matching_document(request, &observed, document));
        assert(exact_join(document, joined_fields));
    }
    let seal = seal_phase_inputs(request, &observed, &document,
        joined_fields);
    assert(seal.is_some());
    let seal = match seal { Some(seal) => seal, None => { assert(false); return; } };
    let facts = seal.facts();
    assert(facts.len() == 2);
    assert(contains_fact(facts@, verification[0]));
    let exit = observed.exit();
    assert(exit == Some(1));
}

/// A changed value cannot be hidden by overwriting one phase in a native map.
pub fn conflicting_phase_values_and_foreign_union_rows_refuse() {
    let name = vec![1u8]; let value = vec![2u8]; let other = vec![3u8]; let foreign_name = vec![4u8];
    let left = vec![InputFact { namespace: 3, name: &name, value: InputValue::Present(&value) }];
    let right = vec![InputFact { namespace: 3, name: &name, value: InputValue::Present(&other) }];
    let candidate = vec![InputFact { namespace: 3, name: &name, value: InputValue::Present(&other) }];
    let one = vec![0usize]; let two = vec![0usize, 1usize];
    let rejected = inventory_covers(&Inventory { rows: &candidate, order: &one, inverse: &one },
        &Inventory { rows: &left, order: &one, inverse: &one });
    assert(!rejected);
    let extra = vec![
        InputFact { namespace: 3, name: &name, value: InputValue::Present(&value) },
        InputFact { namespace: 3, name: &foreign_name, value: InputValue::Absent },
    ];
    let rejected = joined_rows_recorded(&Inventory { rows: &left, order: &one, inverse: &one },
        &Inventory { rows: &right, order: &one, inverse: &one },
        &Inventory { rows: &extra, order: &two, inverse: &two });
    assert(!rejected);
}

/// Reusing a PID cannot change the independently allocated run/unit/slot.
pub fn same_pid_different_attempts_do_not_rebind_phase_requests() {
    let run = vec![1u8]; let other_run = vec![2u8]; let binding = vec![3u8];
    let unit = CompilerUnit { manifest: vec![4u8], name: vec![5u8], crate_type: vec![6u8], cargo_name: None };
    let mut ledger = AttemptLedger::new(&run, &unit);
    let first = ledger.allocate();
    let first = match first { Some(a) => a, None => { assert(false); return; } };
    let first = first.configure(ProcessDescriptor { program: vec![7u8], arguments: Vec::new(), directory: None, overrides: Vec::new() });
    let (request, _first) = PhaseRequest::for_reserved(first, &binding);
    let second = ledger.allocate();
    let second = match second { Some(a) => a, None => { assert(false); return; } };
    let descriptor = ProcessDescriptor { program: vec![7u8], arguments: Vec::new(), directory: None, overrides: Vec::new() };
    let observed = complete_phase_attempt(second.configure(descriptor), 8, Some(0));
    let observed = match observed { Some(a) => a, None => { assert(false); return; } };
    let matches = request_matches(&request, &observed);
    assert(!matches);
    let mut other = AttemptLedger::new(&other_run, &unit);
    let other = other.allocate();
    let other = match other { Some(a) => a, None => { assert(false); return; } };
    let other = other.configure(copy_descriptor(observed.reserved().descriptor()));
    let (other_request, _other) = PhaseRequest::for_reserved(other, &binding);
    let matches = request_matches(&other_request, &observed);
    assert(!matches);

    let different = CompilerUnit { manifest: vec![4u8], name: vec![5u8], crate_type: vec![6u8], cargo_name: Some(Vec::new()) };
    let equal = compiler_units_equal(&unit, &different);
    assert(!equal);
}

/// Equal nominal run/unit/slot/PID does not identify different final commands.
pub fn final_descriptor_prevents_cross_argument_namespace_replay() {
    let run = vec![1u8]; let binding = vec![2u8];
    let unit = CompilerUnit { manifest: vec![3u8], name: vec![4u8], crate_type: vec![5u8], cargo_name: None };
    let mut first = AttemptLedger::new(&run, &unit);
    let reserved = first.reserve(ProcessDescriptor { program: vec![6u8], arguments: vec![vec![7u8]],
        directory: None, overrides: Vec::new() });
    let reserved = match reserved { Some(a) => a, None => { assert(false); return; } };
    let (request, reserved) = PhaseRequest::for_reserved(reserved, &binding);
    proof { assert(request.descriptor_view().arguments[0]@ == seq![7u8]); }
    let mut second = AttemptLedger::new(&run, &unit);
    let reserved = second.reserve(ProcessDescriptor { program: vec![6u8], arguments: vec![vec![8u8]],
        directory: None, overrides: Vec::new() });
    let reserved = match reserved { Some(a) => a, None => { assert(false); return; } };
    let observed = complete_phase_attempt(reserved, 9, Some(0));
    let observed = match observed { Some(a) => a, None => { assert(false); return; } };
    proof { assert(reserved_descriptor(&running_reserved(&observed_running(&observed))).arguments[0]@ == seq![8u8]); }
    let matches = request_matches(&request, &observed);
    assert(!matches);
}

/// Copies preserve argv boundaries, operation order, absence and empty values.
pub fn descriptor_copy_and_nonempty_boundary_controls() {
    let first = ProcessDescriptor { program: vec![1u8], arguments: vec![vec![2u8], vec![3u8, 4u8]],
        directory: Some(Vec::new()), overrides: vec![(vec![5u8], None), (vec![6u8], Some(Vec::new()))] };
    let copied = copy_descriptor(&first);
    assert(same_descriptor(copied, first));
    assert(same_overrides(copied.overrides@, first.overrides@));
    assert forall|i: int| 0 <= i < first.arguments.len() implies
        #[trigger] first.arguments[i]@ == copied.arguments[i]@ by {
        assert(copied.arguments[i]@ == first.arguments[i]@);
    }
    assert forall|i: int| 0 <= i < first.overrides.len() implies
        #[trigger] first.overrides[i].0@ == copied.overrides[i].0@
            && same_optional(first.overrides[i].1, copied.overrides[i].1) by {
        assert(copied.overrides[i].0@ == first.overrides[i].0@);
        assert(same_optional(copied.overrides[i].1, first.overrides[i].1));
    }
    let equal = descriptors_equal(&first, &copied);
    assert(equal);
    let other = ProcessDescriptor { program: vec![1u8], arguments: vec![vec![2u8, 3u8], vec![4u8]],
        directory: Some(Vec::new()), overrides: vec![(vec![5u8], None), (vec![6u8], Some(Vec::new()))] };
    let equal = descriptors_equal(&first, &other);
    assert(!equal);
    let equal = optional_bytes_equal(&None, &Some(Vec::new()));
    assert(!equal);
}

/// Both source fields, every tracked path and each exact environment field survive conversion.
pub fn native_row_expansion_preserves_nonempty_raw_declarations() {
    let sources = vec![SourceDeclaration { name: b" source\xff ", checksum: b"checksum\x00", size: 0x0807_0605_0403_0201u64 },
        SourceDeclaration { name: b"other", checksum: b"second", size: u64::MAX }];
    let paths: Vec<&[u8]> = vec![b" extra "];
    let environment = vec![EnvironmentDeclaration { name: b" FLAG ", value: Some(b"\xff\x00") },
        EnvironmentDeclaration { name: b"missing", value: None }, EnvironmentDeclaration { name: b"empty", value: Some(b"") }];
    let expanded = expand_declarations(&sources, &paths, &environment);
    assert(expanded.is_some());
    let expanded = match expanded { Some(rows) => rows, None => { assert(false); return; } };
    assert(expanded.len() == 8);
    let facts = declaration_facts(&expanded);
    assert(facts.len() == 8);
    assert(checksum_row(expanded[0], sources[0]));
    assert(declaration_fact(expanded[0], facts[0]));
    assert(sources[0].name@ == b" source\xff "@);
    assert(facts[0].namespace == 0 && facts[0].name@ == b" source\xff "@);
    assert(match facts[0].value { InputValue::Present(bytes) => bytes@ == b"checksum\x00"@, _ => false });
    assert(size_row(expanded[1], sources[0]));
    assert(declaration_fact(expanded[1], facts[1]));
    assert(encoded_size(0x0807_0605_0403_0201u64) == seq![1u8,2u8,3u8,4u8,5u8,6u8,7u8,8u8]) by(compute);
    assert(match facts[1].value { InputValue::Present(bytes) => bytes@ == seq![1u8,2u8,3u8,4u8,5u8,6u8,7u8,8u8], _ => false });
    assert(size_row(expanded[3], sources[1]));
    assert(declaration_fact(expanded[3], facts[3]));
    assert(encoded_size(0xffff_ffff_ffff_ffffu64) == seq![255u8,255u8,255u8,255u8,255u8,255u8,255u8,255u8]) by(compute);
    assert(match facts[3].value { InputValue::Present(bytes) => bytes@ == seq![255u8,255u8,255u8,255u8,255u8,255u8,255u8,255u8], _ => false });
    assert(facts[4].namespace == 2 && facts[4].name@ == b" extra "@);
    assert(match facts[4].value { InputValue::Present(bytes) => bytes.len() == 0, _ => false });
    assert(facts[5].namespace == 3 && facts[5].name@ == b" FLAG "@);
    assert(match facts[5].value { InputValue::Present(bytes) => bytes@ == b"\xff\x00"@, _ => false });
    assert(facts[6].value is Absent);
    assert(match facts[7].value { InputValue::Present(bytes) => bytes.len() == 0, _ => false });
}

/// Complete emptiness remains distinct from a duplicate declaration list.
pub fn empty_native_expansion_and_duplicate_path_controls() {
    let empty = expand_declarations(&[], &[], &[]);
    assert(empty.is_some());
    let empty = match empty { Some(rows) => rows, None => { assert(false); return; } };
    let facts = declaration_facts(&empty);
    assert(facts.len() == 0);
    let paths: Vec<&[u8]> = vec![b"same", b"same"];
    let expanded = expand_declarations(&[], &paths, &[]);
    assert(expanded.is_some());
    let expanded = match expanded { Some(rows) => rows, None => { assert(false); return; } };
    let facts = declaration_facts(&expanded);
    assert(facts.len() == 2 && same_key(facts[0], facts[1]));
    let order = vec![0usize,1usize];
    let usable = input_order(&facts, &order, &order);
    assert(!usable);
}

/// The exact eight-byte representation admits no truncation of a source length.
pub proof fn source_size_encoding_is_lossless(a: u64, b: u64)
    requires encoded_size(a) == encoded_size(b),
    ensures a == b,
{
    assert((a & 255) as u8 == (b & 255) as u8);
    assert(((a >> 8) & 255) as u8 == ((b >> 8) & 255) as u8);
    assert(((a >> 16) & 255) as u8 == ((b >> 16) & 255) as u8);
    assert(((a >> 24) & 255) as u8 == ((b >> 24) & 255) as u8);
    assert(((a >> 32) & 255) as u8 == ((b >> 32) & 255) as u8);
    assert(((a >> 40) & 255) as u8 == ((b >> 40) & 255) as u8);
    assert(((a >> 48) & 255) as u8 == ((b >> 48) & 255) as u8);
    assert(((a >> 56) & 255) as u8 == ((b >> 56) & 255) as u8);
    assert(a == b) by(bit_vector)
        requires (a & 255) as u8 == (b & 255) as u8,
            ((a >> 8) & 255) as u8 == ((b >> 8) & 255) as u8, ((a >> 16) & 255) as u8 == ((b >> 16) & 255) as u8,
            ((a >> 24) & 255) as u8 == ((b >> 24) & 255) as u8, ((a >> 32) & 255) as u8 == ((b >> 32) & 255) as u8,
            ((a >> 40) & 255) as u8 == ((b >> 40) & 255) as u8, ((a >> 48) & 255) as u8 == ((b >> 48) & 255) as u8,
            ((a >> 56) & 255) as u8 == ((b >> 56) & 255) as u8;
}

/// Raw filename encoding preserves all ordinary bytes and escapes only spaces.
pub fn compiler_filename_encoding_controls() {
    reveal_with_fuel(encoded_dependency_name, 8);
    assert(encoded_dependency_name(seq![97u8, 194u8, 160u8, 98u8, 99u8]) == seq![97u8, 194u8, 160u8, 98u8, 99u8]) by(compute);
    assert(encoded_dependency_name(seq![97u8, 9u8, 98u8]) == seq![97u8, 9u8, 98u8]) by(compute);
    assert(encoded_dependency_name(seq![97u8, 92u8, 98u8]) == seq![97u8, 92u8, 98u8]) by(compute);
    assert(encoded_dependency_name(seq![97u8, 32u8, 98u8]) == seq![97u8, 92u8, 32u8, 98u8]) by(compute);
    let unicode: [u8; 5] = [97, 194, 160, 98, 99];
    let tab: [u8; 3] = [97, 9, 98];
    let slash: [u8; 3] = [97, 92, 98];
    let encoded: [u8; 4] = [97, 92, 32, 98];
    let spaced: [u8; 3] = [97, 32, 98];
    let wrong: [u8; 2] = [97, 98];
    let newline: [u8; 3] = [97, 10, 98];
    let literal = crate::inputs::dependency_filename_matches(&unicode, &unicode); assert(literal);
    let literal = crate::inputs::dependency_filename_matches(&tab, &tab); assert(literal);
    let literal = crate::inputs::dependency_filename_matches(&slash, &slash); assert(literal);
    let escaped = crate::inputs::dependency_filename_matches(&encoded, &spaced); assert(escaped);
    let trimmed = crate::inputs::dependency_filename_matches(&encoded, &wrong); assert(!trimmed);
    let escaped_wrong = crate::inputs::dependency_filename_matches(&slash, &wrong); assert(!escaped_wrong);
    let unsupported = crate::inputs::dependency_filename_matches(&newline, &newline); assert(!unsupported);
}

/// A constructive inherited environment preserves every name and final removal.
pub fn effective_environment_nonempty_inherited_and_clear_controls() {
    let a = vec![65u8]; let b = vec![66u8]; let c = vec![67u8];
    let parent = vec![InputFact { namespace: 3, name: &a, value: InputValue::Present(b"7") },
        InputFact { namespace: 3, name: &b, value: InputValue::Present(b"") }];
    let operations = vec![InputFact { namespace: 3, name: &a, value: InputValue::Absent },
        InputFact { namespace: 3, name: &c, value: InputValue::Present(b"") }];
    let result = vec![InputFact { namespace: 3, name: &a, value: InputValue::Absent },
        InputFact { namespace: 3, name: &b, value: InputValue::Present(b"") },
        InputFact { namespace: 3, name: &c, value: InputValue::Present(b"") }];
    let two = vec![0usize, 1usize]; let three = vec![0usize, 1usize, 2usize];
    assert(lex_at(a@, b@, 0)); assert(lex_at(b@, c@, 0));
    assert(lex_at(a@, c@, 0));
    let parent_inventory = Inventory { rows: &parent, order: &two, inverse: &two };
    let operations_inventory = Inventory { rows: &operations, order: &two, inverse: &two };
    let result_inventory = Inventory { rows: &result, order: &three, inverse: &three };
    let valid = environment_rows_valid(&parent, true); assert(valid);
    let valid = environment_rows_valid(&operations, false); assert(valid);
    let valid = environment_rows_valid(&result, false); assert(valid);
    let valid = environment_parent_keys_covered(&parent_inventory, &result_inventory); assert(valid);
    let valid = inventory_covers(&result_inventory, &operations_inventory); assert(valid);
    let valid = effective_environment_origins(&parent_inventory, &operations_inventory, &result_inventory); assert(valid);
    let admitted = seal_environment_projection(EnvironmentInheritance::Inherit,
        parent_inventory, operations_inventory, result_inventory);
    assert(admitted.is_some());
    let admitted = match admitted { Some(admitted) => admitted, None => { assert(false); return; } };
    let rows = admitted.facts(); assert(rows.len() == 3 && rows[0].value is Absent);
    let original = admitted.parent(); assert(original.len() == 2);
    let clear = seal_environment_projection(EnvironmentInheritance::Clear,
        Inventory { rows: &parent, order: &two, inverse: &two },
        Inventory { rows: &operations, order: &two, inverse: &two },
        Inventory { rows: &operations, order: &two, inverse: &two });
    assert(clear.is_some());
}

/// Neither a dropped inherited name nor a retained old value survives override validation.
pub fn effective_environment_wrong_value_and_omission_controls() {
    let parent = vec![InputFact { namespace: 3, name: b"A", value: InputValue::Present(b"7") }];
    let operation = vec![InputFact { namespace: 3, name: b"A", value: InputValue::Absent }];
    let one = vec![0usize];
    let stale = seal_environment_projection(EnvironmentInheritance::Inherit,
        Inventory { rows: &parent, order: &one, inverse: &one },
        Inventory { rows: &operation, order: &one, inverse: &one },
        Inventory { rows: &parent, order: &one, inverse: &one });
    assert(stale.is_none());
    let missing = seal_environment_projection(EnvironmentInheritance::Inherit,
        Inventory { rows: &parent, order: &one, inverse: &one },
        Inventory { rows: &[], order: &[], inverse: &[] },
        Inventory { rows: &[], order: &[], inverse: &[] });
    assert(missing.is_none());
    let clears = seal_environment_projection(EnvironmentInheritance::Clear,
        Inventory { rows: &parent, order: &one, inverse: &one },
        Inventory { rows: &[], order: &[], inverse: &[] },
        Inventory { rows: &parent, order: &one, inverse: &one });
    assert(clears.is_none());
}

/// Nonempty native builder operations admit their exact actual readback.
pub fn command_environment_nonempty_and_substitution_controls() {
    let a = vec![65u8]; let seven = vec![55u8]; let nine = vec![57u8];
    let parent = vec![InputFact { namespace: 3, name: &a, value: InputValue::Present(&seven) }];
    let operations = vec![InputFact { namespace: 3, name: &a, value: InputValue::Present(&nine) }];
    let one = vec![0usize];
    let valid = seal_command_environment(EnvironmentInheritance::Inherit,
        Inventory { rows: &parent, order: &one, inverse: &one },
        Inventory { rows: &operations, order: &one, inverse: &one },
        Inventory { rows: &operations, order: &one, inverse: &one },
        Inventory { rows: &operations, order: &one, inverse: &one });
    assert(valid.is_some());
}

/// A coherent readback/result pair still cannot replace an independent builder value.
pub fn command_environment_different_readback_value_control() {
    let a = vec![65u8]; let seven = vec![55u8]; let nine = vec![57u8];
    let parent = vec![InputFact { namespace: 3, name: &a, value: InputValue::Present(&seven) }];
    let operations = vec![InputFact { namespace: 3, name: &a, value: InputValue::Present(&nine) }];
    let one = vec![0usize];
    assert(!same_value(operations[0].value, parent[0].value));
    assert(!contains_fact(parent@, operations[0]));
    let matches = inventory_covers(
        &Inventory { rows: &parent, order: &one, inverse: &one },
        &Inventory { rows: &operations, order: &one, inverse: &one });
    assert(!matches);
    let wrong = seal_command_environment(EnvironmentInheritance::Inherit,
        Inventory { rows: &parent, order: &one, inverse: &one },
        Inventory { rows: &operations, order: &one, inverse: &one },
        Inventory { rows: &parent, order: &one, inverse: &one },
        Inventory { rows: &parent, order: &one, inverse: &one });
    assert(wrong.is_none());
}

/// Missing actual readback cannot erase a configured original operation.
pub fn command_environment_missing_readback_control() {
    let a = vec![65u8]; let seven = vec![55u8]; let nine = vec![57u8];
    let parent = vec![InputFact { namespace: 3, name: &a, value: InputValue::Present(&seven) }];
    let operations = vec![InputFact { namespace: 3, name: &a, value: InputValue::Present(&nine) }];
    let one = vec![0usize];
    let omitted = seal_command_environment(EnvironmentInheritance::Inherit,
        Inventory { rows: &parent, order: &one, inverse: &one },
        Inventory { rows: &operations, order: &one, inverse: &one },
        Inventory { rows: &[], order: &[], inverse: &[] },
        Inventory { rows: &parent, order: &one, inverse: &one });
    assert(omitted.is_none());
}

/// Concrete primitive identity comparison over retained scalar owner witnesses.
/// This fixture is not an OS owner or a compiler input-domain provider.
pub struct ScalarOwnerIdentity;

impl OwnerIdentity<u64> for ScalarOwnerIdentity {
    open spec fn identity(owner: &u64) -> int { *owner as int }
    fn same_owner(&self, original: &u64, actual: &u64) -> bool { *original == *actual }
}

/// Nonempty typed environment and exact final descriptor admit a configured intent.
pub fn configured_intent_witness(inherit: bool) -> (out: Option<ConfiguredIntent>)
    ensures out.is_some(), out.is_some() ==> out.unwrap().descriptor_view().program@ == seq![99u8]
        && out.unwrap().descriptor_view().arguments@ == Seq::<Vec<u8>>::empty()
        && out.unwrap().descriptor_view().directory.is_none()
        && out.unwrap().descriptor_view().overrides.len() == 1
        && out.unwrap().descriptor_view().overrides[0].0@ == seq![65u8]
        && out.unwrap().descriptor_view().overrides[0].1.is_some()
        && out.unwrap().descriptor_view().overrides[0].1.unwrap()@ == seq![55u8]
        && out.unwrap().inheritance_view() == if inherit { EnvironmentInheritance::Inherit } else { EnvironmentInheritance::Clear },
{
    let rows = vec![InputFact { namespace: 3, name: b"A", value: InputValue::Present(b"7") }];
    let one = vec![0usize];
    let policy = if inherit { EnvironmentInheritance::Inherit } else { EnvironmentInheritance::Clear };
    let environment = seal_command_environment(policy,
        Inventory { rows: &[], order: &[], inverse: &[] },
        Inventory { rows: &rows, order: &one, inverse: &one },
        Inventory { rows: &rows, order: &one, inverse: &one },
        Inventory { rows: &rows, order: &one, inverse: &one });
    assert(environment.is_some());
    let environment = match environment { Some(value) => value, None => { assert(false); return None; } };
    let descriptor = ProcessDescriptor { program: vec![99u8], arguments: Vec::new(), directory: None,
        overrides: vec![(vec![65u8], Some(vec![55u8]))] };
    let admitted = seal_configured_intent(&descriptor, &descriptor, &environment);
    assert(admitted.is_some());
    admitted
}

/// A typed environment token for different native rows cannot authorize a descriptor.
pub fn configured_intent_wrong_environment_and_command_controls() {
    let rows = vec![InputFact { namespace: 3, name: b"A", value: InputValue::Present(b"7") }];
    let one = vec![0usize];
    let environment = seal_command_environment(EnvironmentInheritance::Clear,
        Inventory { rows: &[], order: &[], inverse: &[] },
        Inventory { rows: &rows, order: &one, inverse: &one },
        Inventory { rows: &rows, order: &one, inverse: &one },
        Inventory { rows: &rows, order: &one, inverse: &one });
    let environment = match environment { Some(value) => value, None => { assert(false); return; } };
    let missing = ProcessDescriptor { program: vec![99u8], arguments: Vec::new(), directory: None, overrides: Vec::new() };
    let missing = seal_configured_intent(&missing, &missing, &environment);
    assert(missing.is_none());
    let wrong = ProcessDescriptor { program: vec![99u8], arguments: Vec::new(), directory: None,
        overrides: vec![(vec![65u8], Some(vec![57u8]))] };
    let wrong = seal_configured_intent(&wrong, &wrong, &environment);
    assert(wrong.is_none());
    let original = ProcessDescriptor { program: vec![99u8], arguments: Vec::new(), directory: None,
        overrides: vec![(vec![65u8], Some(vec![55u8]))] };
    let different = ProcessDescriptor { program: vec![99u8], arguments: vec![vec![9u8]], directory: None,
        overrides: vec![(vec![65u8], Some(vec![55u8]))] };
    let different = seal_configured_intent(&original, &different, &environment);
    assert(different.is_none());
}

/// A concrete started attempt with no supplied stream/wait success token.
pub fn owned_compiler_start_witness<'a>(run: &'a [u8], unit: &'a CompilerUnit, configuration: &ConfiguredIntent) -> (out: Option<OwnedCompilerAttempt<'a, u64>>)
    ensures out.is_some(), out.is_some() ==> out.unwrap().ready()
        && out.unwrap().owner_view() == 17 && out.unwrap().pid_view() == 7
        && same_descriptor(out.unwrap().descriptor_view(), configuration.descriptor_view())
        && out.unwrap().inheritance_view() == configuration.inheritance_view(),
{
    let mut ledger = AttemptLedger::new(run, unit);
    let reserved = ledger.reserve(copy_descriptor(configuration.descriptor()));
    let reserved = match reserved { Some(value) => value, None => { assert(false); return None; } };
    let admitted = begin_owned_compiler(reserved, OwnedSpawnFields { owner: 17u64, pid: 7, configuration: Some(configuration) });
    assert(admitted.is_some());
    admitted
}

/// Successful, failing and signaled waits preserve output; missing/mismatched fields refuse.
pub fn owned_compiler_completion_constructive_and_refusal_controls() {
    let run = vec![1u8];
    let unit = CompilerUnit { manifest: vec![2u8], name: vec![3u8], crate_type: vec![4u8], cargo_name: None };
    let identity = ScalarOwnerIdentity;
    let mut case = 0u8;
    while case < 11
        invariant case <= 11,
        decreases 11 - case,
    {
        let configuration = configured_intent_witness(false);
        let configuration = match configuration { Some(value) => value, None => { assert(false); return; } };
        let attempt = owned_compiler_start_witness(&run, &unit, &configuration);
        let attempt = if case == 9 { None } else { attempt };
        let configuration = if case == 10 {
            let different = configured_intent_witness(true);
            match different { Some(value) => value, None => { assert(false); return; } }
        } else { configuration };
        let wait = if case == 6 { None } else if case == 8 { Some(None) }
            else if case == 7 { Some(Some(1i32)) } else { Some(Some(0i32)) };
        let completion = OwnedCompletionFields {
            owner: if case == 3 { None } else if case == 1 { Some(18u64) } else { Some(17u64) },
            pid: if case == 4 { None } else if case == 2 { Some(8u32) } else { Some(7u32) },
            configuration: if case == 5 { None } else { Some(configuration) }, wait,
            stdout: vec![65u8, 66u8], stderr: vec![67u8],
        };
        let observed = complete_owned_compiler(&identity, attempt, completion);
        assert(observed.is_ok() == (case == 0 || case == 7 || case == 8));
        match observed {
            Ok(observed) => {
                let stdout = observed.stdout(); let stderr = observed.stderr();
                assert(stdout@ == seq![65u8, 66u8]); assert(stderr@ == seq![67u8]);
                let exit = observed.exit();
                assert(exit == if case == 8 { None } else if case == 7 { Some(1i32) } else { Some(0i32) });
            },
            Err(raw) => { assert(raw.stdout@ == seq![65u8, 66u8]); assert(raw.stderr@ == seq![67u8]); },
        }
        case += 1;
    }
}

/// Spawn identity alone cannot substitute a missing configuration or a zero PID.
pub fn owned_compiler_start_missing_and_different_command_controls() {
    let run = vec![1u8];
    let unit = CompilerUnit { manifest: vec![2u8], name: vec![3u8], crate_type: vec![4u8], cargo_name: None };
    let configuration = configured_intent_witness(false);
    let configuration = match configuration { Some(value) => value, None => { assert(false); return; } };
    let mut case = 0u8;
    while case < 3
        invariant case <= 3,
        decreases 3 - case,
    {
        let mut ledger = AttemptLedger::new(&run, &unit);
        let descriptor = if case == 2 {
            ProcessDescriptor { program: vec![99u8], arguments: vec![vec![9u8]], directory: None,
                overrides: vec![(vec![65u8], Some(vec![55u8]))] }
        } else { copy_descriptor(configuration.descriptor()) };
        let reserved = ledger.reserve(descriptor);
        let reserved = match reserved { Some(value) => value, None => { assert(false); return; } };
        let admitted = begin_owned_compiler(reserved, OwnedSpawnFields {
            owner: 17u64, pid: if case == 0 { 0 } else { 7 },
            configuration: if case == 1 { None } else { Some(&configuration) },
        });
        assert(admitted.is_none());
        case += 1;
    }
}

/// Equal owner and PID cannot replace different argv boundaries at completion.
pub fn owned_identity_wrong_descriptor_control() {
    let identity = ScalarOwnerIdentity;
    let original = ProcessDescriptor { program: vec![99u8], arguments: vec![vec![65u8], vec![66u8]],
        directory: None, overrides: Vec::new() };
    let different = ProcessDescriptor { program: vec![99u8], arguments: vec![vec![65u8, 66u8]],
        directory: None, overrides: Vec::new() };
    let valid = owned_identity_matches(&identity, &17u64, 7, &original, &17u64, 7, &original);
    assert(valid);
    let different = owned_identity_matches(&identity, &17u64, 7, &original, &17u64, 7, &different);
    assert(!different);
}

}
