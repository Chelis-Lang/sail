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
    let request = PhaseRequest::for_attempt(&allocated, &binding);
    let reserved = allocated.configure(ProcessDescriptor { program: vec![6u8], arguments: Vec::new(),
        directory: None, overrides: Vec::new() });
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
    let request = PhaseRequest::for_attempt(&first, &binding);
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
    let other_request = PhaseRequest::for_attempt(&other, &binding);
    let matches = request_matches(&other_request, &observed);
    assert(!matches);

    let different = CompilerUnit { manifest: vec![4u8], name: vec![5u8], crate_type: vec![6u8], cargo_name: Some(Vec::new()) };
    let equal = compiler_units_equal(&unit, &different);
    assert(!equal);
}

}
