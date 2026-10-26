//! Refinement of the actual executable declared-input core to exact model maps.
//!
//! Content values here are the identity bytes provided by the native adapter.
//! Physical-byte identity, observation order and compiler discovery remain
//! separate primitive/provider refinements; no hash or filesystem axiom is used.
#![forbid(unsafe_code)]

use crate::context::*;
use crate::inputs_core::*;
use vstd::prelude::*;

verus! {

pub open spec fn input_key(fact: InputFact) -> InputKey {
    InputKey { role: fact.namespace, lexical: fact.name@ }
}

/// Native namespace 1 is an alias/ancestor destination. All other namespaces
/// carry content identity bytes at their exact declared lexical name.
pub open spec fn input_outcome(fact: InputFact) -> ReadOutcome {
    match fact.value {
        InputValue::Present(bytes) => if fact.namespace == 1 {
            ReadOutcome::Present { resolved: bytes@, bytes: Seq::empty() }
        } else {
            ReadOutcome::Present { resolved: fact.name@, bytes: bytes@ }
        },
        InputValue::Absent => ReadOutcome::Absent,
        InputValue::Unreadable => ReadOutcome::Unreadable,
    }
}

pub open spec fn key_position(facts: Seq<InputFact>, key: InputKey) -> int {
    choose|i: int| 0 <= i < facts.len() && input_key(facts[i]) == key
}

/// Domain membership is generated from all actual raw rows. There is no
/// caller-selected domain, caller completeness flag or opaque identity tag.
#[verifier::opaque]
pub open spec fn input_map(facts: Seq<InputFact>) -> Map<InputKey, ReadOutcome> {
    Map::new(
        facts.map(|i: int, fact: InputFact| input_key(fact)).to_set(),
        |key: InputKey| input_outcome(facts[key_position(facts, key)]),
    )
}

pub proof fn map_domain(facts: Seq<InputFact>, key: InputKey)
    ensures input_map(facts).dom().contains(key)
        == (exists|i: int| 0 <= i < facts.len() && input_key(facts[i]) == key),
{
    reveal(input_map);
    let keys = facts.map(|i: int, fact: InputFact| input_key(fact));
    if input_map(facts).dom().contains(key) {
        assert(keys.contains(key));
        let i = choose|i: int| 0 <= i < keys.len() && keys[i] == key;
        assert(input_key(facts[i]) == key);
    } else if exists|i: int| 0 <= i < facts.len() && input_key(facts[i]) == key {
        let i = choose|i: int| 0 <= i < facts.len() && input_key(facts[i]) == key;
        assert(keys[i] == key);
        assert(keys.contains(key));
    }
}

pub proof fn unique_key_position(facts: Seq<InputFact>, i: int, j: int)
    requires unique_input_keys(facts), 0 <= i < facts.len(), 0 <= j < facts.len(),
        input_key(facts[i]) == input_key(facts[j]),
    ensures i == j,
{
    if i < j { assert(!same_key(facts[i], facts[j])); }
    if j < i { assert(!same_key(facts[j], facts[i])); }
}

pub proof fn map_value_at(facts: Seq<InputFact>, i: int)
    requires unique_input_keys(facts), 0 <= i < facts.len(),
    ensures input_map(facts).dom().contains(input_key(facts[i])),
        input_map(facts)[input_key(facts[i])] == input_outcome(facts[i]),
{
    reveal(input_map);
    let key = input_key(facts[i]);
    map_domain(facts, key);
    assert(exists|j: int| 0 <= j < facts.len() && input_key(facts[j]) == key);
    let j = key_position(facts, key);
    unique_key_position(facts, i, j);
}

pub proof fn exact_row_outcome(a: InputFact, b: InputFact)
    requires same_key(a, b), same_value(a.value, b.value),
    ensures input_key(a) == input_key(b), input_outcome(a) == input_outcome(b),
        readable(input_outcome(a)), readable(input_outcome(b)),
{
    match (a.value, b.value) {
        (InputValue::Present(_), InputValue::Present(_)) => {},
        (InputValue::Absent, InputValue::Absent) => {},
        _ => {},
    }
}

pub proof fn matching_key_refines_value(before: Seq<InputFact>, after: Seq<InputFact>, key: InputKey)
    requires unique_input_keys(before), unique_input_keys(after), same_inventory(before, after),
        input_map(before).dom().contains(key),
    ensures input_map(after).dom().contains(key), input_map(before)[key] == input_map(after)[key],
        readable(input_map(after)[key]),
{
    map_domain(before, key);
    let i = key_position(before, key);
    assert(contains_fact(after, before[i]));
    let j = choose|j: int| #![trigger after[j]] 0 <= j < after.len()
        && same_key(after[j], before[i]) && same_value(after[j].value, before[i].value);
    map_value_at(before, i);
    map_value_at(after, j);
    exact_row_outcome(after[j], before[i]);
}

/// Exact shared-core equality refines equality of the complete model maps.
pub proof fn matching_inventory_refines_map(before: Seq<InputFact>, after: Seq<InputFact>)
    requires unique_input_keys(before), unique_input_keys(after), same_inventory(before, after),
    ensures input_map(before) == input_map(after),
        forall|key: InputKey| input_map(after).dom().contains(key)
            ==> readable(#[trigger] input_map(after)[key]),
{
    let left = input_map(before);
    let right = input_map(after);
    assert forall|key: InputKey| #[trigger] left.dom().contains(key)
        implies right.dom().contains(key) && left[key] == right[key] by {
        matching_key_refines_value(before, after, key);
    }
    assert forall|key: InputKey| #[trigger] right.dom().contains(key)
        implies left.dom().contains(key) && readable(right[key]) by {
        matching_key_refines_value(after, before, key);
        matching_key_refines_value(before, after, key);
    }
    assert(left.dom() =~= right.dom());
    assert(left =~= right);
}

/// This adapter invokes the exact executable function used by read_check and
/// proves its result means complete modeled input identity, rather than a tag.
pub fn checked_readback_refines_model<'a>(expected: &'a [InputFact<'a>], observed: &'a [InputFact<'a>],
    before_order: &'a [usize], before_inverse: &[usize],
    after_order: &'a [usize], after_inverse: &[usize]) -> (equal: bool)
    ensures equal ==> input_map(expected@) == input_map(observed@),
        equal ==> forall|key: InputKey| input_map(observed@).dom().contains(key)
            ==> readable(#[trigger] input_map(observed@)[key]),
{
    let evidence = compare_readback(expected, observed, before_order, before_inverse, after_order, after_inverse);
    let equal = evidence.is_equal();
    proof {
        if equal { matching_inventory_refines_map(expected@, observed@); }
    }
    equal
}

/// Every fact of a projected snapshot remains an exact original model fact.
pub proof fn selected_inventory_refines_submap(source: Seq<InputFact>, selected: Seq<InputFact>)
    requires unique_input_keys(source), unique_input_keys(selected), covered_selection(source, selected),
    ensures input_map(selected).dom().subset_of(input_map(source).dom()),
        forall|key: InputKey| input_map(selected).dom().contains(key)
            ==> input_map(selected)[key] == input_map(source)[key],
{
    assert forall|key: InputKey| #[trigger] input_map(selected).dom().contains(key)
        implies input_map(source).dom().contains(key)
            && input_map(selected)[key] == input_map(source)[key] by {
        map_domain(selected, key);
        let i = key_position(selected, key);
        assert(contains_fact(source, selected[i]));
        let j = choose|j: int| 0 <= j < source.len()
            && same_key(source[j], selected[i]) && same_value(source[j].value, selected[i].value);
        map_value_at(selected, i);
        map_value_at(source, j);
        exact_row_outcome(source[j], selected[i]);
    }
    assert(input_map(selected).dom().subset_of(input_map(source).dom()));
}

/// Successful capture coverage supplies a present observation for every exact
/// required key. The compiler still has to declare its actual required names.
pub proof fn completed_reads_refine_coverage(required: Seq<InputFact>, observed: Seq<InputFact>)
    requires unique_input_keys(observed), complete_present_inventory(required, observed),
    ensures forall|i: int| 0 <= i < required.len() ==>
        input_map(observed).dom().contains(input_key(#[trigger] required[i]))
            && readable(input_map(observed)[input_key(required[i])]),
{
    assert forall|i: int| 0 <= i < required.len() implies
        input_map(observed).dom().contains(input_key(#[trigger] required[i]))
            && readable(input_map(observed)[input_key(required[i])]) by {
        let j = choose|j: int| 0 <= j < observed.len()
            && same_key(observed[j], required[i]) && value_present(observed[j].value);
        map_value_at(observed, j);
    }
}

/// The production projection validator establishes an actual exact model submap.
pub fn checked_projection_refines_model(source: &[InputFact<'_>], selected: &[InputFact<'_>],
    source_order: &[usize], source_inverse: &[usize],
    selected_order: &[usize], selected_inverse: &[usize]) -> (covered: bool)
    ensures covered ==> input_map(selected@).dom().subset_of(input_map(source@).dom()),
        covered ==> forall|key: InputKey| input_map(selected@).dom().contains(key)
            ==> input_map(selected@)[key] == #[trigger] input_map(source@)[key],
{
    let covered = selection_covered(source, selected, source_order, source_inverse, selected_order, selected_inverse);
    proof { if covered { selected_inventory_refines_submap(source@, selected@); } }
    covered
}

/// The production capture validator covers every name supplied by discovery.
pub fn checked_capture_refines_model(required: &[InputFact<'_>], observed: &[InputFact<'_>],
    required_order: &[usize], required_inverse: &[usize],
    observed_order: &[usize], observed_inverse: &[usize]) -> (complete: bool)
    ensures complete ==> forall|i: int| 0 <= i < required.len() ==>
        input_map(observed@).dom().contains(input_key(#[trigger] required[i]))
            && readable(input_map(observed@)[input_key(required[i])]),
{
    let complete = reads_complete(required, observed, required_order, required_inverse, observed_order, observed_inverse);
    proof { if complete { completed_reads_refine_coverage(required@, observed@); } }
    complete
}

/// Even a supplied stamp freshly equal to the current primitive stamp cannot
/// bind older guarded bytes unless it is the guard's actual original stamp.
pub fn fresh_stamp_does_not_rebind(guard: &OriginalGuard, revision: &[u8], fresh: &[u8]) -> (current: bool)
    ensures current ==> fresh@ == guard.stamp_view(),
        current ==> revision@ == guard.revision_view(),
{
    guard.retained(revision, Some(fresh), Some(fresh))
}

/// Source-shared matching preserves the actual request/scope/runtime/mode/solver
/// fields when an authority context replaces only its modeled input map.
pub proof fn matching_contexts_preserve_execution(metadata: ModelContext,
    before: Seq<InputFact>, after: Seq<InputFact>)
    requires unique_input_keys(before), unique_input_keys(after), same_inventory(before, after),
    ensures same_execution(ModelContext { inputs: input_map(before), ..metadata },
        ModelContext { inputs: input_map(after), ..metadata }),
{
    matching_inventory_refines_map(before, after);
}

/// A concrete nonempty read with a source, a linked ancestor and absent config
/// succeeds; the same actual core refuses incomplete and unreadable readbacks.
#[verifier::spinoff_prover]
pub fn nonempty_readback_and_partial_controls() {
    let name = vec![1u8];
    let alias = vec![2u8];
    let config = vec![3u8];
    let content = vec![4u8];
    let destination = vec![5u8];
    let before = vec![
        InputFact { namespace: 0, name: &name, value: InputValue::Present(&content) },
        InputFact { namespace: 1, name: &alias, value: InputValue::Present(&destination) },
        InputFact { namespace: 0, name: &config, value: InputValue::Absent },
    ];
    let after = vec![
        InputFact { namespace: 0, name: &config, value: InputValue::Absent },
        InputFact { namespace: 0, name: &name, value: InputValue::Present(&content) },
        InputFact { namespace: 1, name: &alias, value: InputValue::Present(&destination) },
    ];
    let before_order = vec![0usize, 2usize, 1usize];
    let before_inverse = vec![0usize, 2usize, 1usize];
    let after_order = vec![1usize, 0usize, 2usize];
    let after_inverse = vec![1usize, 0usize, 2usize];
    proof { assert(lex_at(name@, config@, 0)); assert(lex_less(name@, config@)); }
    let checked = compare_readback(&before, &after, &before_order, &before_inverse, &after_order, &after_inverse);
    let equal = checked.is_equal();
    assert(equal);
    proof { matching_inventory_refines_map(before@, after@); }
    let partial_order = vec![1usize, 0usize];
    let partial = compare_readback(&before, &after, &before_order, &before_inverse, &partial_order, &partial_order);
    let equal = partial.is_equal();
    assert(!equal);
    let failed = vec![InputFact { namespace: 0, name: &name, value: InputValue::Unreadable }];
    let one = vec![0usize];
    let failure = compare_readback(&failed, &failed, &one, &one, &one, &one);
    let equal = failure.is_equal();
    assert(!equal);
}

/// Actual guard constructors cannot restamp stale bytes. A fresh full capture
/// after an exact content revert may bind the old logical revision anew.
pub fn original_stamp_aba_and_exact_revert_controls() {
    let revision = vec![7u8];
    let original = vec![1u8];
    let mutated = vec![2u8];
    let reverted = vec![3u8];
    let capture = OriginalCapture::begin(Some(&original));
    let sealed = capture.finish(Some(&original), &revision);
    assert(sealed.is_some());
    let guard = sealed.unwrap();
    let current = guard.retained(&revision, Some(&original), Some(&original));
    assert(current);
    let restamped = guard.retained(&revision, Some(&mutated), Some(&mutated));
    assert(!restamped);
    let aba = guard.retained(&revision, Some(&original), Some(&reverted));
    assert(!aba);
    let fresh = OriginalCapture::begin(Some(&reverted));
    let resealed = fresh.finish(Some(&reverted), &revision);
    assert(resealed.is_some());
    let fresh_guard = resealed.unwrap();
    let fresh_current = fresh_guard.retained(&revision, Some(&reverted), Some(&reverted));
    assert(fresh_current);
    let interrupted = OriginalCapture::begin(Some(&original));
    let interrupted = interrupted.finish(Some(&mutated), &revision);
    assert(interrupted.is_none());
}


/// The provider's declared dep-info-required artifact positions. This set is
/// deliberately not the compiler's complete physical input domain.
#[verifier::opaque]
pub open spec fn declared_artifact_frames(events: Seq<ProviderEvent>) -> Set<usize> {
    events.map(|i: int, event: ProviderEvent| i as usize).to_set().filter(|frame: usize| 2 <= frame < events.len() - 1 && events[frame as int].kind == 2)
}

/// Exact bytes from tagged actual dep-info outcomes, with no digest assumption.
#[verifier::opaque]
pub open spec fn provider_read_map(reads: Seq<ArtifactRead>) -> Map<usize, Seq<u8>> {
    Map::new(reads.map(|j: int, read: ArtifactRead| read.frame).to_set(),
        |frame: usize| {
            let j = choose|j: int| 0 <= j < reads.len() && reads[j].frame == frame;
            reads[j].raw.unwrap()@
        })
}

pub proof fn declared_frame_domain(events: Seq<ProviderEvent>, frame: usize)
    ensures declared_artifact_frames(events).contains(frame)
        == (2 <= frame < events.len() - 1 && events[frame as int].kind == 2),
{
    reveal(declared_artifact_frames);
    let positions = events.map(|i: int, event: ProviderEvent| i as usize);
    if 2 <= frame < events.len() - 1 && events[frame as int].kind == 2 {
        assert(positions[frame as int] == frame);
        assert(positions.contains(frame));
    }
}

pub proof fn provider_map_domain(reads: Seq<ArtifactRead>, frame: usize)
    ensures provider_read_map(reads).dom().contains(frame)
        == (exists|j: int| 0 <= j < reads.len() && reads[j].frame == frame),
{
    reveal(provider_read_map);
    let positions = reads.map(|j: int, read: ArtifactRead| read.frame);
    if provider_read_map(reads).dom().contains(frame) {
        assert(positions.contains(frame));
        let j = choose|j: int| 0 <= j < positions.len() && positions[j] == frame;
        assert(reads[j].frame == frame);
    } else if exists|j: int| 0 <= j < reads.len() && reads[j].frame == frame {
        let j = choose|j: int| 0 <= j < reads.len() && reads[j].frame == frame;
        assert(positions[j] == frame);
        assert(positions.contains(frame));
    }
}

/// Concrete coverage establishes both domain directions. No assumed inventory
/// completeness bridges this declared provider domain to physical compiler IO.
pub proof fn provider_coverage_refines_exact_domain(events: Seq<ProviderEvent>, reads: Seq<ArtifactRead>)
    requires provider_trace(events), artifact_read_coverage(events, reads),
    ensures provider_read_map(reads).dom() == declared_artifact_frames(events),
        forall|j: int| 0 <= j < reads.len() ==> #[trigger] provider_read_map(reads)[reads[j].frame] == reads[j].raw.unwrap()@,
{
    assert forall|frame: usize| provider_read_map(reads).dom().contains(frame) implies declared_artifact_frames(events).contains(frame) by {
        provider_map_domain(reads, frame);
        declared_frame_domain(events, frame);
        let j = choose|j: int| 0 <= j < reads.len() && reads[j].frame == frame;
        assert(artifact_read_at(events, reads[j], reads[j].frame as int));
    }
    assert forall|frame: usize| declared_artifact_frames(events).contains(frame) implies provider_read_map(reads).dom().contains(frame) by {
        provider_map_domain(reads, frame);
        declared_frame_domain(events, frame);
        let j = choose|j: int| 0 <= j < reads.len() && reads[j].frame == frame;
        assert(artifact_read_at(events, reads[j], reads[j].frame as int));
    }
    assert(provider_read_map(reads).dom() =~= declared_artifact_frames(events));
    assert forall|j: int| 0 <= j < reads.len() implies #[trigger] provider_read_map(reads)[reads[j].frame] == reads[j].raw.unwrap()@ by {
        reveal(provider_read_map);
        provider_map_domain(reads, reads[j].frame);
        assert(artifact_read_at(events, reads[j], reads[j].frame as int));
        let k = choose|k: int| 0 <= k < reads.len() && reads[k].frame == reads[j].frame && reads[k].raw.is_some();
        if k < j { assert(reads[k].frame < reads[j].frame); }
        if j < k { assert(reads[j].frame < reads[k].frame); }
        assert(k == j);
    }
}

/// The real source-shared constructor refines the explicit provider map.
pub fn checked_compiler_provider_view(events: &[ProviderEvent], reads: &[ArtifactRead]) -> (seal: Option<ProviderSeal>)
    ensures seal.is_some() ==> provider_read_map(reads@).dom() == declared_artifact_frames(events@),
{
    let seal = seal_compiler_provider(events, reads);
    if seal.is_some() { proof { provider_coverage_refines_exact_domain(events@, reads@); } }
    seal
}

/// The actual production seal connects a nonempty completed provider to exact
/// keyed read coverage; a missing read cannot be repaired by a terminal stamp.
pub fn compiler_provider_nonempty_and_missing_read_controls() {
    let events = vec![
        ProviderEvent { kind: 0, producer: 7, child: 0, frames: 0, exited: false },
        ProviderEvent { kind: 1, producer: 7, child: 8, frames: 0, exited: false },
        ProviderEvent { kind: 2, producer: 7, child: 8, frames: 0, exited: false },
        ProviderEvent { kind: 3, producer: 7, child: 8, frames: 1, exited: true },
    ];
    let raw = vec![2u8];
    let reads = vec![ArtifactRead { producer: 7, child: 8, frame: 2, raw: Some(&raw) }];
    let sealed = seal_compiler_provider(&events, &reads);
    proof { assert(artifact_read_at(events@, reads[0], 2)); }
    assert(sealed.is_some());
    proof {
        provider_coverage_refines_exact_domain(events@, reads@);
        declared_frame_domain(events@, 2);
        assert(provider_read_map(reads@).dom().contains(2));
        assert(provider_read_map(reads@)[2] == raw@);
    }
    let trace = seal_provider(&events);
    assert(trace.is_some());
    let seal = trace.unwrap();
    let producer = seal.producer(); let child = seal.child(); let frames = seal.frames();
    assert(producer == 7 && child == 8 && frames == 1);
    let empty = vec![];
    let absent = seal_compiler_provider(&events, &empty);
    // Non-vacuous success and missing-read refusal also run as native tests;
    // this proof establishes the general concrete-constructor implication.
    if absent.is_some() {
        assert(artifact_read_coverage(events@, empty@));
        assert(exists|r: int| 0 <= r < empty.len() && empty[r].frame == 2);
        assert(false);
    }

}

/// A completed empty child is observable. No child, truncated execution,
/// replayed start, wrong child or failed wait cannot mint a seal.
pub fn compiler_provider_lifecycle_controls() {
    let empty: Vec<ProviderEvent> = vec![];
    let missing = seal_provider(&empty); assert(missing.is_none());
    let intent = vec![ProviderEvent { kind: 0, producer: 7, child: 0, frames: 0, exited: false }];
    let truncated = seal_provider(&intent); assert(truncated.is_none());
    let completed = vec![
        ProviderEvent { kind: 0, producer: 7, child: 0, frames: 0, exited: false },
        ProviderEvent { kind: 1, producer: 7, child: 8, frames: 0, exited: false },
        ProviderEvent { kind: 3, producer: 7, child: 8, frames: 0, exited: true },
    ];
    let sealed = seal_provider(&completed); assert(sealed.is_some());
    let wrong = vec![
        ProviderEvent { kind: 0, producer: 7, child: 0, frames: 0, exited: false },
        ProviderEvent { kind: 1, producer: 7, child: 8, frames: 0, exited: false },
        ProviderEvent { kind: 2, producer: 7, child: 9, frames: 0, exited: false },
        ProviderEvent { kind: 3, producer: 7, child: 8, frames: 1, exited: true },
    ];
    proof { assert(!provider_frame(wrong@, 2)); }
    let wrong = seal_provider(&wrong); assert(wrong.is_none());
    let failed = vec![
        ProviderEvent { kind: 0, producer: 7, child: 0, frames: 0, exited: false },
        ProviderEvent { kind: 1, producer: 7, child: 8, frames: 0, exited: false },
        ProviderEvent { kind: 3, producer: 7, child: 8, frames: 0, exited: false },
    ];
    let failed = seal_provider(&failed); assert(failed.is_none());
    let replayed = vec![
        ProviderEvent { kind: 0, producer: 7, child: 0, frames: 0, exited: false },
        ProviderEvent { kind: 1, producer: 7, child: 8, frames: 0, exited: false },
        ProviderEvent { kind: 1, producer: 7, child: 8, frames: 0, exited: false },
        ProviderEvent { kind: 3, producer: 7, child: 8, frames: 1, exited: true },
    ];
    proof { assert(!provider_frame(replayed@, 2)); }
    let replayed = seal_provider(&replayed); assert(replayed.is_none());
}

}
