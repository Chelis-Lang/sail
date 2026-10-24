//! Exact observed-input comparison and observation-point proof component.
//!
//! This root verifies a pure component, not Sail's filesystem adapter. Paths,
//! environment values and executable identities are represented by exact bytes.
//! Inventory completeness and concrete stamp semantics remain refinement work.

#![forbid(unsafe_code)]

use crate::context::*;
use vstd::prelude::*;

verus! {

/// One observed input fact. `kind` separates source, configuration, environment,
/// runtime, generated-input and ancestor-alias namespaces. Absence has
/// `present == false`. The adapter must distinguish read errors from absence.
pub struct Atom {
    pub kind: u8,
    pub lexical: Vec<u8>,
    pub resolved: Vec<u8>,
    pub present: bool,
    pub bytes: Vec<u8>,
}

pub type AtomView = (u8, Seq<u8>, Seq<u8>, bool, Seq<u8>);

pub open spec fn atom_view(atom: Atom) -> AtomView {
    (atom.kind, atom.lexical@, atom.resolved@, atom.present, atom.bytes@)
}

/// Ordered observation inventories use positional identity. Reordering safely
/// misses; no canonicalization or hash injectivity is needed by this relation.
pub open spec fn same_binding(left: Seq<Atom>, right: Seq<Atom>) -> bool {
    left.len() == right.len()
        && forall|i: int| 0 <= i < left.len() ==> atom_view(left[i]) == atom_view(right[i])
}

/// Every selected position is present on both sides and denotes the same fact.
pub open spec fn same_at(left: Seq<Atom>, right: Seq<Atom>, i: usize) -> bool {
    i < left.len() && i < right.len() && atom_view(left[i as int]) == atom_view(right[i as int])
}

pub open spec fn same_scope(
    left: Seq<Atom>, right: Seq<Atom>, selected: Seq<usize>,
) -> bool {
    forall|k: int| 0 <= k < selected.len()
        ==> #[trigger] same_at(left, right, selected[k])
}

/// Executable byte comparator; equality is proved rather than assumed for hashes.
pub fn same_bytes(left: &Vec<u8>, right: &Vec<u8>) -> (equal: bool)
    ensures equal == (left@ == right@),
{
    if left.len() != right.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < left.len()
        invariant
            i <= left.len(),
            left.len() == right.len(),
            forall|j: int| 0 <= j < i ==> left[j] == right[j],
        decreases left.len() - i,
    {
        if left[i] != right[i] {
            return false;
        }
        i = i + 1;
    }
    assert(left@ =~= right@);
    true
}

/// Every input field participates in executable acceptance of a matching fact.
pub fn matches_atom(left: &Atom, right: &Atom) -> (equal: bool)
    ensures equal == (atom_view(*left) == atom_view(*right)),
{
    left.kind == right.kind
        && left.present == right.present
        && same_bytes(&left.lexical, &right.lexical)
        // Negative control: dropping this call loses source-alias destinations.
        && same_bytes(&left.resolved, &right.resolved)
        && same_bytes(&left.bytes, &right.bytes)
}

/// Exact comparison of a retained inventory with a newly observed inventory.
/// This returns a matching fact, not authority that an inventory is complete.
pub fn matches_binding(left: &Vec<Atom>, right: &Vec<Atom>) -> (equal: bool)
    ensures equal == same_binding(left@, right@),
{
    if left.len() != right.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < left.len()
        invariant
            i <= left.len(),
            left.len() == right.len(),
            forall|j: int| 0 <= j < i ==> atom_view(left[j]) == atom_view(right[j]),
        decreases left.len() - i,
    {
        if !matches_atom(&left[i], &right[i]) {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Scope projection cannot silently accept missing indices or changed selected
/// aliases. Selecting the correct compiler-input closure remains the adapter's
/// obligation; an empty selection proves nothing about whole-target coverage.
pub fn matches_scope(
    left: &Vec<Atom>, right: &Vec<Atom>, selected: &Vec<usize>,
) -> (equal: bool)
    ensures equal == same_scope(left@, right@, selected@),
{
    let mut k: usize = 0;
    while k < selected.len()
        invariant
            k <= selected.len(),
            forall|j: int| 0 <= j < k
                ==> #[trigger] same_at(left@, right@, selected[j]),
        decreases selected.len() - k,
    {
        let i = selected[k];
        if i >= left.len() || i >= right.len() {
            proof {
                assert(!same_at(left@, right@, i));
                if same_scope(left@, right@, selected@) {
                    assert(same_at(left@, right@, selected[k as int]));
                }
            }
            return false;
        }
        if !matches_atom(&left[i], &right[i]) {
            proof {
                assert(!same_at(left@, right@, i));
                if same_scope(left@, right@, selected@) {
                    assert(same_at(left@, right@, selected[k as int]));
                }
            }
            return false;
        }
        k = k + 1;
    }
    true
}

pub proof fn binding_reflexive(binding: Seq<Atom>)
    ensures same_binding(binding, binding),
{
}

pub proof fn binding_transitive(a: Seq<Atom>, b: Seq<Atom>, c: Seq<Atom>)
    requires same_binding(a, b), same_binding(b, c),
    ensures same_binding(a, c),
{
    assert forall|i: int| 0 <= i < a.len()
        implies atom_view(a[i]) == atom_view(c[i]) by {
        assert(atom_view(a[i]) == atom_view(b[i]));
        assert(atom_view(b[i]) == atom_view(c[i]));
    }
}

/// Even when both canonical files' bytes remain in an inventory, changing an
/// alias or ancestor destination makes the selected observed binding unequal.
pub proof fn retarget_rejected(before: Seq<Atom>, after: Seq<Atom>, i: int)
    requires
        0 <= i < before.len(),
        0 <= i < after.len(),
        before[i].resolved@ != after[i].resolved@,
    ensures !same_binding(before, after),
{
    assert(atom_view(before[i]) != atom_view(after[i]));
}

/// Every namespace, lexical binding, absence and byte value is independently
/// binding. Equal contents alone are deliberately insufficient.
pub proof fn changed_fact_rejected(before: Seq<Atom>, after: Seq<Atom>, i: int)
    requires
        0 <= i < before.len(),
        0 <= i < after.len(),
        before[i].kind != after[i].kind
            || before[i].lexical@ != after[i].lexical@
            || before[i].present != after[i].present
            || before[i].bytes@ != after[i].bytes@,
    ensures !same_binding(before, after),
{
    assert(atom_view(before[i]) != atom_view(after[i]));
}

pub proof fn retarget_in_scope_rejected(
    before: Seq<Atom>, after: Seq<Atom>, selected: Seq<usize>, k: int,
)
    requires
        0 <= k < selected.len(),
        selected[k] < before.len(),
        selected[k] < after.len(),
        before[selected[k] as int].resolved@ != after[selected[k] as int].resolved@,
    ensures !same_scope(before, after, selected),
{
    let i = selected[k] as int;
    assert(atom_view(before[i]) != atom_view(after[i]));
    assert(!same_at(before, after, selected[k]));
    if same_scope(before, after, selected) {
        assert(same_at(before, after, selected[k]));
    }
}

pub proof fn complete_match_projects(
    before: Seq<Atom>, after: Seq<Atom>, selected: Seq<usize>,
)
    requires
        same_binding(before, after),
        forall|k: int| 0 <= k < selected.len() ==> selected[k] < before.len(),
    ensures same_scope(before, after, selected),
{
    assert forall|k: int| 0 <= k < selected.len()
        implies #[trigger] same_at(before, after, selected[k]) by {
        let i = selected[k] as int;
        assert(atom_view(before[i]) == atom_view(after[i]));
    }
}

/// A logical observation records the point whose exact facts it contains.
/// This is a model object; deserialized production candidates do not gain this
/// relation merely by supplying a point or a `complete` flag.
pub struct Observation {
    pub binding: Seq<Atom>,
    pub point: int,
}

pub open spec fn observed_at(history: Seq<Seq<Atom>>, observed: Observation) -> bool {
    0 <= observed.point < history.len()
        && same_binding(observed.binding, history[observed.point])
}

pub proof fn reuse_matches_observation_point(
    history: Seq<Seq<Atom>>, observed: Observation, retained: Seq<Atom>,
)
    requires observed_at(history, observed), same_binding(retained, observed.binding),
    ensures same_binding(retained, history[observed.point]),
{
    binding_transitive(retained, observed.binding, history[observed.point]);
}

/// A later stamp cannot bind bytes observed earlier to a changed world.
pub proof fn restamping_old_bytes_is_not_refresh(original: Seq<Atom>, changed: Seq<Atom>)
    requires !same_binding(original, changed),
    ensures !observed_at(seq![original, changed], Observation { binding: original, point: 1 }),
{
}

/// A logical mutation counter must be monotone and change on every changed
/// binding. Relating actual platform stamps to this contract is NOT proved here.
pub open spec fn counter_covers_changes(
    history: Seq<Seq<Atom>>, counters: Seq<nat>,
) -> bool {
    history.len() == counters.len()
        && (forall|a: int, b: int| 0 <= a <= b < counters.len()
            ==> counters[a] <= counters[b])
        && (forall|i: int| 1 <= i < history.len() && counters[i - 1] == counters[i]
            ==> #[trigger] same_binding(history[i - 1], history[i]))
}

/// An unchanged covering counter preserves the original snapshot's bytes.
/// The concrete FS adapter must justify counter coverage before applying this.
pub proof fn guarded_observation_unchanged(
    history: Seq<Seq<Atom>>, counters: Seq<nat>, start: int, end: int,
)
    requires
        counter_covers_changes(history, counters),
        0 <= start <= end < history.len(),
        counters[start] == counters[end],
    ensures same_binding(history[start], history[end]),
    decreases end - start,
{
    if start == end {
        binding_reflexive(history[start]);
    } else {
        assert(counters[start] <= counters[end - 1]);
        assert(counters[end - 1] <= counters[end]);
        assert(counters[start] == counters[end - 1]);
        guarded_observation_unchanged(history, counters, start, end - 1);
        assert(same_binding(history[end - 1], history[end]));
        binding_transitive(history[start], history[end - 1], history[end]);
    }
}

/// Every covering counter, not merely a chosen example, rejects the original
/// guard after a changed binding is restored to its original bytes.
pub proof fn aba_invalidates_original_guard(
    original: Seq<Atom>, changed: Seq<Atom>, counters: Seq<nat>,
)
    requires
        !same_binding(original, changed),
        counter_covers_changes(seq![original, changed, original], counters),
    ensures counters[0] != counters[2],
{
    let history = seq![original, changed, original];
    if counters[0] == counters[2] {
        assert(counters[0] <= counters[1]);
        assert(counters[1] <= counters[2]);
        assert(counters[0] == counters[1]);
        guarded_observation_unchanged(history, counters, 0, 1);
        assert(same_binding(original, changed));
    }
}

/// Exact content/binding reversion permits a freshly observed match. It does
/// not authorize reusing the original mutation guard through an ABA change.
pub proof fn exact_revert_requires_fresh_guard(original: Seq<Atom>, changed: Seq<Atom>)
    requires !same_binding(original, changed),
    ensures
        observed_at(seq![original, changed, original], Observation { binding: original, point: 2 }),
        counter_covers_changes(seq![original, changed, original], seq![0nat, 1nat, 2nat]),
        seq![0nat, 1nat, 2nat][0] != seq![0nat, 1nat, 2nat][2],
{
    binding_reflexive(original);
}

pub proof fn observation_does_not_promise_future(original: Seq<Atom>, changed: Seq<Atom>)
    requires !same_binding(original, changed),
    ensures
        observed_at(seq![original, changed], Observation { binding: original, point: 0 }),
        !observed_at(seq![original, changed], Observation { binding: original, point: 1 }),
{
    binding_reflexive(original);
}

fn example_atom(destination: u8, present: bool, value: u8) -> (atom: Atom)
    ensures
        atom.kind == 0,
        atom.lexical@ == seq![1u8],
        atom.resolved@ == seq![destination],
        atom.present == present,
        atom.bytes@ == seq![value],
{
    Atom {
        kind: 0,
        lexical: vec![1u8],
        resolved: vec![destination],
        present,
        bytes: vec![value],
    }
}

/// Closed examples show that the matching and rejection relations have useful
/// instances. These assertions use the executable checker, not an assumed result.
pub fn positive_controls() {
    let original = vec![example_atom(2, true, 3)];
    let unchanged = vec![example_atom(2, true, 3)];
    let retargeted = vec![example_atom(4, true, 3)];
    let changed_bytes = vec![example_atom(2, true, 5)];
    let absent = vec![example_atom(2, false, 3)];
    let reverted = vec![example_atom(2, true, 3)];
    let selected = vec![0usize];
    let missing = vec![1usize];
    proof {
        assert(original@[0].resolved@[0] == 2u8);
        assert(retargeted@[0].resolved@[0] == 4u8);
        assert(original@[0].bytes@[0] == 3u8);
        assert(changed_bytes@[0].bytes@[0] == 5u8);
        retarget_rejected(original@, retargeted@, 0);
        changed_fact_rejected(original@, changed_bytes@, 0);
        changed_fact_rejected(original@, absent@, 0);
        retarget_in_scope_rejected(original@, retargeted@, selected@, 0);
    }
    let same = matches_binding(&original, &unchanged);
    assert(same);
    let alias_changed = matches_binding(&original, &retargeted);
    assert(!alias_changed);
    let bytes_changed = matches_binding(&original, &changed_bytes);
    assert(!bytes_changed);
    let presence_changed = matches_binding(&original, &absent);
    assert(!presence_changed);
    let restored = matches_binding(&original, &reverted);
    assert(restored);
    let scope_matches = matches_scope(&original, &unchanged, &selected);
    assert(scope_matches);
    let scope_retargeted = matches_scope(&original, &retargeted, &selected);
    assert(!scope_retargeted);
    let scope_missing = matches_scope(&original, &unchanged, &missing);
    proof {
        if scope_missing {
            assert(same_at(original@, unchanged@, missing@[0]));
        }
    }
    assert(!scope_missing);
}

/// Roles are disjoint even when lexical names or value bytes happen to agree.
pub const ROLE_SOURCE: u8 = 0;
pub const ROLE_ANCESTOR: u8 = 1;
pub const ROLE_GENERATED: u8 = 2;
pub const ROLE_CONFIGURATION: u8 = 3;
pub const ROLE_ENVIRONMENT: u8 = 4;
pub const ROLE_RUNTIME: u8 = 5;

/// The modeled environment at one real adapter observation point. The map is
/// explicit data, not a predicate assumed to mean "all compiler inputs".
pub struct InputWorld {
    pub context: ModelContext,
    pub epoch: nat,
    pub point: nat,
}

/// A read event records both the original lexical key and its typed outcome.
pub struct InputRead {
    pub key: InputKey,
    pub outcome: ReadOutcome,
    pub epoch: nat,
    pub point: nat,
}

/// A raw transaction is not reusable input evidence. Its stamp cannot be
/// rewritten by a caller, and its observed rows cannot be deserialized into a
/// sealed token. Modeled reads are the only way to append observations.
pub struct RawCapture {
    base: ModelContext,
    required: Seq<InputKey>,
    original_epoch: nat,
    begin_point: nat,
    reads: Seq<InputRead>,
    invalidated: bool,
}

pub closed spec fn capture_stamp(capture: RawCapture) -> nat { capture.original_epoch }

pub closed spec fn capture_read_count(capture: RawCapture) -> nat { capture.reads.len() }

pub closed spec fn capture_begin_point(capture: RawCapture) -> nat { capture.begin_point }

pub closed spec fn capture_requirements(capture: RawCapture) -> Seq<InputKey> { capture.required }

pub closed spec fn capture_context(capture: RawCapture) -> ModelContext { capture.base }

pub closed spec fn capture_observations(capture: RawCapture) -> Seq<InputRead> { capture.reads }

pub open spec fn read_at(world: InputWorld, key: InputKey) -> ReadOutcome {
    if world.context.inputs.dom().contains(key) { world.context.inputs[key] }
    else { ReadOutcome::Unreadable }
}

pub closed spec fn capture_valid(capture: RawCapture, world: InputWorld) -> bool {
    capture.original_epoch <= world.epoch && capture.begin_point <= world.point
        && (capture.original_epoch == world.epoch ==> capture.base == world.context)
        && forall|i: int| #![trigger capture.reads[i]] 0 <= i < capture.reads.len() ==> {
            let read = capture.reads[i];
            capture.original_epoch <= read.epoch <= world.epoch
                && capture.begin_point <= read.point <= world.point
                && (read.epoch == world.epoch ==> read.outcome == read_at(world, read.key))
        }
}

/// Coverage checks the exact declared domain, not just the number of rows or
/// whether every row that happened to arrive looks plausible.
pub closed spec fn can_seal(capture: RawCapture, world: InputWorld) -> bool {
    !capture.invalidated
        && capture.original_epoch == world.epoch
        && capture.base == world.context
        && context_wf(capture.base)
        && capture.required.len() > 0
        && capture.required.no_duplicates()
        && capture.required.to_set() == capture.base.inputs.dom()
        && capture.reads.len() == capture.required.len()
        && forall|i: int| #![trigger capture.reads[i]] 0 <= i < capture.required.len() ==> {
            capture.required[i].role <= ROLE_RUNTIME
                && capture.reads[i].key == capture.required[i]
                && readable(capture.reads[i].outcome)
        }
}

pub closed spec fn begin_capture(world: InputWorld, required: Seq<InputKey>) -> RawCapture {
    RawCapture { base: world.context, required, original_epoch: world.epoch,
        begin_point: world.point, reads: Seq::empty(), invalidated: false }
}

pub proof fn begin_capture_preserves(world: InputWorld, required: Seq<InputKey>)
    ensures capture_valid(begin_capture(world, required), world),
{}

pub closed spec fn observe_input(capture: RawCapture, world: InputWorld, key: InputKey) -> RawCapture {
    let expected = capture.reads.len() < capture.required.len()
        && capture.required[capture.reads.len() as int] == key;
    let duplicate = exists|i: int| 0 <= i < capture.reads.len()
        && capture.reads[i].key == key;
    RawCapture {
        reads: capture.reads.push(InputRead {
            key, outcome: read_at(world, key), epoch: world.epoch, point: world.point }),
        invalidated: capture.invalidated || !expected || duplicate || !readable(read_at(world, key)),
        ..capture
    }
}

pub proof fn observation_preserves(capture: RawCapture, world: InputWorld, key: InputKey)
    requires capture_valid(capture, world),
    ensures
        capture_valid(observe_input(capture, world, key), world),
        capture_stamp(observe_input(capture, world, key)) == capture_stamp(capture),
{
    let next = observe_input(capture, world, key);
    assert forall|i: int| #![trigger next.reads[i]] 0 <= i < next.reads.len() implies {
        let read = next.reads[i];
        capture.original_epoch <= read.epoch <= world.epoch
            && capture.begin_point <= read.point <= world.point
            && (read.epoch == world.epoch ==> read.outcome == read_at(world, read.key))
    } by {
        if i < capture.reads.len() { assert(next.reads[i] == capture.reads[i]); }
        else { assert(i == capture.reads.len()); }
    }
}

/// Any modeled input or execution-context change advances the original guard,
/// even when a later change restores precisely the old bytes (ABA).
pub open spec fn change_world(world: InputWorld, context: ModelContext) -> InputWorld {
    InputWorld { context, epoch: world.epoch + 1, point: world.point + 1 }
}

pub proof fn world_change_preserves(capture: RawCapture, world: InputWorld, context: ModelContext)
    requires capture_valid(capture, world),
    ensures capture_valid(capture, change_world(world, context)),
{
    let next = change_world(world, context);
    assert forall|i: int| #![trigger capture.reads[i]] 0 <= i < capture.reads.len() implies {
        let read = capture.reads[i];
        capture.original_epoch <= read.epoch <= next.epoch
            && capture.begin_point <= read.point <= next.point
            && (read.epoch == next.epoch ==> read.outcome == read_at(next, read.key))
    } by { assert(capture.reads[i].epoch < next.epoch); }
}

/// Time and observation-only/authentication plumbing may advance without
/// changing the modeled compiler inputs or execution context. Classifying a
/// real variable as plumbing remains a concrete environment-adapter obligation.
pub open spec fn advance_point(world: InputWorld) -> InputWorld {
    InputWorld { point: world.point + 1, ..world }
}

pub proof fn point_advance_preserves(capture: RawCapture, world: InputWorld)
    requires capture_valid(capture, world),
    ensures capture_valid(capture, advance_point(world)),
{}

pub closed spec fn invalidate_capture(capture: RawCapture) -> RawCapture {
    RawCapture { invalidated: true, ..capture }
}

pub proof fn invalidation_preserves(capture: RawCapture, world: InputWorld)
    requires capture_valid(capture, world),
    ensures capture_valid(invalidate_capture(capture), world),
        !can_seal(invalidate_capture(capture), world),
{}

/// Reobservation begins an empty transaction. It does not attach a newly taken
/// stamp to observations read under the old transaction.
pub closed spec fn reobserve(capture: RawCapture, world: InputWorld) -> RawCapture {
    begin_capture(world, capture.required)
}

pub proof fn reobservation_resets_reads(capture: RawCapture, world: InputWorld)
    ensures capture_valid(reobserve(capture, world), world),
        capture_read_count(reobserve(capture, world)) == 0,
        capture_requirements(capture).len() > 0 ==> !can_seal(reobserve(capture, world), world),
{
    begin_capture_preserves(world, capture.required);
}

/// A newly discovered generated-file/environment requirement cannot silently
/// inherit completion of the old inventory. The conservative next step is a
/// fresh observation under the expanded request.
pub closed spec fn learn_requirement(capture: RawCapture, key: InputKey) -> RawCapture {
    RawCapture { required: capture.required.push(key), invalidated: true, ..capture }
}

pub proof fn learned_requirement_requires_reobservation(
    capture: RawCapture, world: InputWorld, key: InputKey,
)
    requires capture_valid(capture, world),
    ensures capture_valid(learn_requirement(capture, key), world),
        !can_seal(learn_requirement(capture, key), world),
{}

/// The sole usable-input token has private construction and records its actual
/// validation point, original stamp, complete key inventory and read witnesses.
pub struct InputEvidence {
    context: ModelContext,
    point: nat,
    epoch: nat,
    required: Seq<InputKey>,
    reads: Seq<InputRead>,
}

impl InputEvidence {
    pub closed spec fn view(&self) -> ModelContext { self.context }
    pub closed spec fn point(&self) -> nat { self.point }
    pub closed spec fn observations(&self) -> Seq<InputRead> { self.reads }
    pub closed spec fn observed_after(&self, point: nat) -> bool {
        forall|i: int| #![trigger self.reads[i]] 0 <= i < self.reads.len()
            ==> self.reads[i].point > point
    }

    pub closed spec fn valid_at(&self, world: InputWorld) -> bool {
        self.context == world.context && self.point == world.point && self.epoch == world.epoch
            && context_wf(self.context)
            && self.required.len() > 0 && self.required.no_duplicates()
            && self.required.to_set() == self.context.inputs.dom()
            && self.reads.len() == self.required.len()
            && forall|i: int| #![trigger self.reads[i]] 0 <= i < self.required.len() ==> {
                self.required[i].role <= ROLE_RUNTIME
                    && self.reads[i].key == self.required[i]
                    && self.reads[i].epoch == world.epoch
                    && self.reads[i].point <= world.point
                    && readable(self.reads[i].outcome)
                    && self.reads[i].outcome == self.context.inputs[self.required[i]]
            }
    }
}

/// This factory validates the complete raw transaction; no `complete: true`
/// field, supplied digest, timestamp or deserialized provenance can mint a token.
pub proof fn seal_capture(capture: RawCapture, world: InputWorld) -> (out: Option<InputEvidence>)
    requires capture_valid(capture, world),
    ensures out.is_some() == can_seal(capture, world),
        out.is_some() ==> out.unwrap().valid_at(world),
        out.is_some() ==> out.unwrap().view() == world.context,
        out.is_some() ==> out.unwrap().point() == world.point,
        out.is_some() ==> out.unwrap().observations() == capture_observations(capture),
{
    if !can_seal(capture, world) { return None; }
    assert forall|i: int| #![trigger capture.reads[i]] 0 <= i < capture.required.len() implies {
        capture.required[i].role <= ROLE_RUNTIME
            && capture.reads[i].key == capture.required[i]
            && capture.reads[i].epoch == world.epoch
            && capture.reads[i].point <= world.point
            && readable(capture.reads[i].outcome)
            && capture.reads[i].outcome == capture.base.inputs[capture.required[i]]
    } by {
        assert(capture.reads[i].epoch == world.epoch);
        assert(capture.reads[i].outcome == read_at(world, capture.reads[i].key));
        assert(capture.base.inputs.dom().contains(capture.required[i]));
    }
    Some(InputEvidence { context: capture.base, point: world.point,
        epoch: capture.original_epoch, required: capture.required, reads: capture.reads })
}

/// Export exact facts for composition without exposing token construction.
pub proof fn input_evidence_facts(evidence: &InputEvidence, world: InputWorld)
    requires evidence.valid_at(world),
    ensures evidence.view() == world.context, evidence.point() == world.point,
        context_wf(evidence.view()),
        forall|key: InputKey| evidence.view().inputs.dom().contains(key)
            ==> readable(evidence.view().inputs[key]),
{}

/// A projection is an exact restriction of the current modeled map. Keeping
/// the same scope, or claiming whole scope, may not drop any required inputs.
pub open spec fn context_projection(full: ModelContext, projected: ModelContext) -> bool {
    full.request == projected.request && full.runtime == projected.runtime
        && full.mode == projected.mode && full.solver == projected.solver
        && scope_wf(projected.scope)
        && projected.inputs.dom().subset_of(full.inputs.dom())
        && (forall|key: InputKey| projected.inputs.dom().contains(key)
            ==> projected.inputs[key] == full.inputs[key])
        && (projected.scope == full.scope ==> projected.inputs == full.inputs)
        && (projected.scope.whole ==> full.scope.whole && projected.inputs == full.inputs)
}

pub proof fn solver_change_cannot_project(full: ModelContext, projected: ModelContext)
    requires full.solver != projected.solver,
    ensures !context_projection(full, projected),
{}

pub closed spec fn projectable(
    capture: RawCapture, world: InputWorld, projected: ModelContext,
    required: Seq<InputKey>, positions: Seq<int>,
) -> bool {
    context_projection(world.context, projected)
        && required.len() > 0 && required.no_duplicates()
        && required.to_set() == projected.inputs.dom()
        && positions.len() == required.len() && positions.no_duplicates()
        && forall|i: int| #![trigger positions[i]] 0 <= i < positions.len()
            ==> 0 <= positions[i] < capture.reads.len()
                && capture.reads[positions[i]].key == required[i]
}

pub closed spec fn projection_pair(
    capture: RawCapture, world: InputWorld, projected: ModelContext,
    required: Seq<InputKey>, positions: Seq<int>,
) -> (RawCapture, InputWorld) {
    let reads = Seq::new(positions.len(), |i: int| capture.reads[positions[i]]);
    (RawCapture { base: projected, required, reads, ..capture },
        InputWorld { context: projected, ..world })
}

/// A shared capture supplies only previously observed rows. Projection preserves
/// the original epoch; final validation is still required after the entire batch.
pub proof fn project_capture(
    capture: RawCapture, world: InputWorld, projected: ModelContext,
    required: Seq<InputKey>, positions: Seq<int>,
) -> (out: Option<(RawCapture, InputWorld)>)
    requires capture_valid(capture, world),
    ensures out.is_some() == projectable(capture, world, projected, required, positions),
        out.is_some() ==> capture_valid(out.unwrap().0, out.unwrap().1),
        out.is_some() ==> capture_stamp(out.unwrap().0) == capture_stamp(capture),
        out.is_some() ==> out.unwrap().1.context == projected,
        out.is_some() ==> out.unwrap().1.point == world.point,
        out.is_some() ==> out.unwrap() == projection_pair(capture, world, projected, required, positions),
{
    if !projectable(capture, world, projected, required, positions) { return None; }
    let reads = Seq::new(positions.len(), |i: int| capture.reads[positions[i]]);
    let next_world = InputWorld { context: projected, ..world };
    let next_capture = RawCapture { base: projected, required, reads, ..capture };
    assert forall|i: int| #![trigger reads[i]] 0 <= i < reads.len() implies {
        let read = reads[i];
        capture.original_epoch <= read.epoch <= world.epoch
            && capture.begin_point <= read.point <= world.point
            && (read.epoch == world.epoch ==> read.outcome == read_at(next_world, read.key))
    } by {
        let source = positions[i];
        assert(reads[i] == capture.reads[source]);
        assert(reads[i].key == required[i]);
        assert(required.contains(required[i]));
        assert(projected.inputs.dom().contains(required[i]));
        assert(world.context.inputs.dom().contains(required[i]));
        if reads[i].epoch == world.epoch {
            assert(reads[i].outcome == read_at(world, reads[i].key));
        }
    }
    Some((next_capture, next_world))
}

pub closed spec fn project_state(
    state: InputState, projected: ModelContext, required: Seq<InputKey>, positions: Seq<int>,
) -> InputState {
    if projectable(state.capture, state.world, projected, required, positions) {
        let pair = projection_pair(state.capture, state.world, projected, required, positions);
        InputState { capture: pair.0, world: pair.1 }
    } else { InputState { capture: invalidate_capture(state.capture), ..state } }
}

pub proof fn projection_step_preserves(
    state: InputState, projected: ModelContext, required: Seq<InputKey>, positions: Seq<int>,
)
    requires capture_valid(state.capture, state.world),
    ensures capture_valid(project_state(state, projected, required, positions).capture,
        project_state(state, projected, required, positions).world),
{
    let result = project_capture(state.capture, state.world, projected, required, positions);
    if result.is_none() { invalidation_preserves(state.capture, state.world); }
}

pub proof fn incomplete_or_duplicate_inventory_cannot_seal(capture: RawCapture, world: InputWorld)
    requires
        capture_requirements(capture).to_set() != capture_context(capture).inputs.dom()
            || !capture_requirements(capture).no_duplicates()
            || capture_read_count(capture) != capture_requirements(capture).len(),
    ensures !can_seal(capture, world),
{}

pub proof fn unreadable_is_not_observed_absence(capture: RawCapture, world: InputWorld, key: InputKey)
    requires world.context.inputs.dom().contains(key),
        world.context.inputs[key] == ReadOutcome::Unreadable,
    ensures !can_seal(capture, world), readable(ReadOutcome::Absent),
        !readable(ReadOutcome::Unreadable),
{
    unreadable_cannot_be_current(world.context, key);
}

pub proof fn old_capture_cannot_seal_after_change(
    capture: RawCapture, world: InputWorld, context: ModelContext,
)
    requires capture_valid(capture, world),
    ensures !can_seal(capture, change_world(world, context)),
{}

pub proof fn aba_capture_requires_reobservation(
    capture: RawCapture, world: InputWorld, changed: ModelContext,
)
    requires capture_valid(capture, world),
    ensures !can_seal(capture, change_world(change_world(world, changed), world.context)),
{}

pub proof fn old_evidence_needs_read_time_refresh(evidence: &InputEvidence, world: InputWorld)
    requires evidence.valid_at(world),
    ensures !evidence.valid_at(advance_point(world)),
{}

/// A retained proof may reuse its old exact context only after a newly completed
/// observation transaction establishes that context at the current read point.
pub proof fn refresh_retained(
    retained: &InputEvidence, capture: RawCapture, world: InputWorld,
) -> (out: Option<InputEvidence>)
    requires capture_valid(capture, world),
    ensures
        out.is_some() == (can_seal(capture, world) && retained.view() == world.context
            && capture_begin_point(capture) > retained.point()),
        out.is_some() ==> out.unwrap().valid_at(world),
        out.is_some() ==> out.unwrap().view() == retained.view(),
        out.is_some() ==> out.unwrap().point() == world.point,
        out.is_some() ==> out.unwrap().observed_after(retained.point()),
{
    if capture.begin_point <= retained.point() { return None; }
    let fresh = seal_capture(capture, world);
    if fresh.is_none() { return None; }
    let evidence = fresh.unwrap();
    if evidence.view() != retained.view() { return None; }
    assert forall|i: int| #![trigger evidence.reads[i]] 0 <= i < evidence.reads.len()
        implies evidence.reads[i].point > retained.point() by {
        assert(evidence.reads[i] == capture.reads[i]);
    }
    Some(evidence)
}

/// Read every explicit required key in order in one unchanged modeled world.
/// This proves existence of useful sealed input evidence without postulating a
/// prevalidated token or an inventory-completeness Boolean.
proof fn read_required_prefix(world: InputWorld, required: Seq<InputKey>, count: nat) -> (capture: RawCapture)
    requires context_wf(world.context), required.len() > 0, required.no_duplicates(),
        required.to_set() == world.context.inputs.dom(), count <= required.len(),
        forall|i: int| 0 <= i < required.len() ==> required[i].role <= ROLE_RUNTIME,
    ensures capture_valid(capture, world), capture.base == world.context,
        capture.required == required, capture.original_epoch == world.epoch,
        capture.begin_point == world.point,
        !capture.invalidated, capture.reads.len() == count,
        forall|j: int| #![trigger capture.reads[j]] 0 <= j < count
            ==> capture.reads[j].key == required[j] && readable(capture.reads[j].outcome)
                && capture.reads[j].point == world.point,
    decreases count,
{
    if count == 0 {
        begin_capture_preserves(world, required);
        begin_capture(world, required)
    } else {
        let previous = read_required_prefix(world, required, (count - 1) as nat);
        let i = (count - 1) as int;
        let key = required[i];
        assert(required.contains(key));
        assert(world.context.inputs.dom().contains(key));
        assert(readable(read_at(world, key)));
        assert forall|j: int| 0 <= j < previous.reads.len()
            implies previous.reads[j].key != key by {
            assert(previous.reads[j].key == required[j]);
            assert(required[j] != required[i]);
        }
        observation_preserves(previous, world, key);
        let next = observe_input(previous, world, key);
        assert forall|j: int| #![trigger next.reads[j]] 0 <= j < count
            implies next.reads[j].key == required[j] && readable(next.reads[j].outcome)
                && next.reads[j].point == world.point by {
            if j < i { assert(next.reads[j] == previous.reads[j]); }
            else { assert(j == i); }
        }
        next
    }
}

pub proof fn complete_raw_capture(world: InputWorld, required: Seq<InputKey>) -> (capture: RawCapture)
    requires context_wf(world.context), required.len() > 0, required.no_duplicates(),
        required.to_set() == world.context.inputs.dom(),
        forall|i: int| 0 <= i < required.len() ==> required[i].role <= ROLE_RUNTIME,
    ensures capture_valid(capture, world), can_seal(capture, world),
        capture_context(capture) == world.context,
        capture_requirements(capture) == required,
        capture_begin_point(capture) == world.point,
        forall|i: int| #![trigger capture_observations(capture)[i]]
            0 <= i < capture_observations(capture).len()
                ==> capture_observations(capture)[i].point == world.point,
{
    let capture = read_required_prefix(world, required, required.len());
    assert(can_seal(capture, world));
    capture
}

pub proof fn capture_unchanged(world: InputWorld, required: Seq<InputKey>) -> (evidence: InputEvidence)
    requires context_wf(world.context), required.len() > 0, required.no_duplicates(),
        required.to_set() == world.context.inputs.dom(),
        forall|i: int| 0 <= i < required.len() ==> required[i].role <= ROLE_RUNTIME,
    ensures evidence.valid_at(world), evidence.view() == world.context,
        evidence.point() == world.point,
        forall|i: int| #![trigger evidence.observations()[i]]
            0 <= i < evidence.observations().len()
                ==> evidence.observations()[i].point == world.point,
        forall|cutoff: nat| cutoff < world.point ==> #[trigger] evidence.observed_after(cutoff),
{
    let capture = complete_raw_capture(world, required);
    let result = seal_capture(capture, world);
    let evidence = result.unwrap();
    assert forall|cutoff: nat| cutoff < world.point
        implies #[trigger] evidence.observed_after(cutoff) by {
        assert forall|i: int| #![trigger evidence.reads[i]] 0 <= i < evidence.reads.len()
            implies evidence.reads[i].point > cutoff by {
            assert(evidence.reads[i] == capture.reads[i]);
        }
    }
    evidence
}

/// Authentication exclusions affect implicit context selection only. Explicit
/// compiler/build-script declarations always bind the exact value or absence.
pub fn environment_is_bound(tool_input: bool, declared: bool, authentication_only: bool) -> (bound: bool)
    ensures bound == environment_binding_required(tool_input, declared, authentication_only),
{
    declared || (tool_input && !authentication_only)
}

pub open spec fn environment_binding_required(tool_input: bool, declared: bool, authentication_only: bool) -> bool {
    declared || (tool_input && !authentication_only)
}

pub proof fn declared_credentials_are_bound(tool_input: bool, authentication_only: bool)
    ensures environment_binding_required(tool_input, true, authentication_only),
{}

/// All concrete modeled events have a corresponding invariant-preserving step.
pub enum InputAction {
    Read(InputKey),
    Change(ModelContext),
    Tick,
    Invalidate,
    Reobserve,
    Learn(InputKey),
    Project(ModelContext, Seq<InputKey>, Seq<int>),
}

pub struct InputState {
    pub world: InputWorld,
    pub capture: RawCapture,
}

pub open spec fn input_initial(world: InputWorld, required: Seq<InputKey>) -> InputState {
    InputState { world, capture: begin_capture(world, required) }
}

pub open spec fn input_step(state: InputState, action: InputAction) -> InputState {
    match action {
        InputAction::Read(key) => {
            let world = advance_point(state.world);
            InputState { world, capture: observe_input(state.capture, world, key) }
        },
        InputAction::Change(context) => InputState {
            world: change_world(state.world, context), capture: state.capture },
        InputAction::Tick => InputState { world: advance_point(state.world), ..state },
        InputAction::Invalidate => InputState {
            capture: invalidate_capture(state.capture), ..state },
        InputAction::Reobserve => InputState {
            capture: reobserve(state.capture, state.world), ..state },
        InputAction::Learn(key) => InputState {
            capture: learn_requirement(state.capture, key), ..state },
        InputAction::Project(context, required, positions) => project_state(state, context, required, positions),
    }
}

pub proof fn every_input_step_preserves(state: InputState, action: InputAction)
    requires capture_valid(state.capture, state.world),
    ensures capture_valid(input_step(state, action).capture, input_step(state, action).world),
{
    match action {
        InputAction::Read(key) => {
            point_advance_preserves(state.capture, state.world);
            observation_preserves(state.capture, advance_point(state.world), key);
        },
        InputAction::Change(context) => world_change_preserves(state.capture, state.world, context),
        InputAction::Tick => point_advance_preserves(state.capture, state.world),
        InputAction::Invalidate => invalidation_preserves(state.capture, state.world),
        InputAction::Reobserve => reobservation_resets_reads(state.capture, state.world),
        InputAction::Learn(key) => learned_requirement_requires_reobservation(state.capture, state.world, key),
        InputAction::Project(context, required, positions) => projection_step_preserves(state, context, required, positions),
    }
}

pub open spec fn input_execution(state: InputState, actions: Seq<InputAction>) -> InputState
    decreases actions.len(),
{
    if actions.len() == 0 { state }
    else { input_step(input_execution(state, actions.drop_last()), actions.last()) }
}

pub proof fn all_input_executions_preserve(state: InputState, actions: Seq<InputAction>)
    requires capture_valid(state.capture, state.world),
    ensures capture_valid(input_execution(state, actions).capture, input_execution(state, actions).world),
    decreases actions.len(),
{
    if actions.len() > 0 {
        all_input_executions_preserve(state, actions.drop_last());
        every_input_step_preserves(input_execution(state, actions.drop_last()), actions.last());
    }
}

/// An arbitrary finite read/change/projection/reobservation execution can seal
/// only the exact context and input facts at its final observation point.
pub proof fn input_execution_seals_only_current(
    world: InputWorld, required: Seq<InputKey>, actions: Seq<InputAction>,
) -> (out: Option<InputEvidence>)
    ensures out.is_some() == can_seal(input_execution(input_initial(world, required), actions).capture,
            input_execution(input_initial(world, required), actions).world),
        out.is_some() ==> out.unwrap().valid_at(input_execution(input_initial(world, required), actions).world),
        out.is_some() ==> out.unwrap().view() == input_execution(input_initial(world, required), actions).world.context,
        out.is_some() ==> out.unwrap().point() == input_execution(input_initial(world, required), actions).world.point,
{
    let initial = input_initial(world, required);
    begin_capture_preserves(world, required);
    all_input_executions_preserve(initial, actions);
    let final_state = input_execution(initial, actions);
    seal_capture(final_state.capture, final_state.world)
}

pub open spec fn example_key(role: u8) -> InputKey {
    InputKey { role, lexical: seq![role] }
}

pub open spec fn example_requirements() -> Seq<InputKey> {
    seq![example_key(ROLE_SOURCE), example_key(ROLE_ANCESTOR), example_key(ROLE_GENERATED),
        example_key(ROLE_CONFIGURATION), example_key(ROLE_ENVIRONMENT), example_key(ROLE_RUNTIME)]
}

pub open spec fn example_input_context() -> ModelContext {
    ModelContext { request: seq![10u8], runtime: seq![11u8],
        scope: ScopeKey { whole: true, selection: Seq::empty() }, mode: Mode::Default,
        solver: SolverId { name: seq![12u8], version: seq![13u8], binary: seq![14u8], platform: seq![15u8] },
        inputs: Map::empty()
            .insert(example_key(ROLE_SOURCE), ReadOutcome::Present { resolved: seq![20u8], bytes: seq![21u8] })
            .insert(example_key(ROLE_ANCESTOR), ReadOutcome::Present { resolved: seq![22u8], bytes: Seq::empty() })
            .insert(example_key(ROLE_GENERATED), ReadOutcome::Present { resolved: seq![23u8], bytes: seq![24u8] })
            .insert(example_key(ROLE_CONFIGURATION), ReadOutcome::Absent)
            .insert(example_key(ROLE_ENVIRONMENT), ReadOutcome::Present { resolved: Seq::empty(), bytes: seq![25u8] })
            .insert(example_key(ROLE_RUNTIME), ReadOutcome::Present { resolved: seq![26u8], bytes: seq![27u8] }) }
}

pub proof fn example_input_context_is_well_formed()
    ensures context_wf(example_input_context()), example_requirements().len() == 6,
        example_requirements().no_duplicates(),
        example_requirements().to_set() == example_input_context().inputs.dom(),
        forall|i: int| 0 <= i < example_requirements().len()
            ==> example_requirements()[i].role <= ROLE_RUNTIME,
{
    let required = example_requirements();
    let context = example_input_context();
    assert(required.to_set() =~= context.inputs.dom());
}

/// A nonempty inventory containing every modeled role establishes sealed
/// evidence. A retained read starts a later real modeled read transaction.
pub proof fn lifecycle_success_and_retained_read_control()
{
    example_input_context_is_well_formed();
    let world = InputWorld { context: example_input_context(), epoch: 0, point: 0 };
    let required = example_requirements();
    let raw = complete_raw_capture(world, required);
    let sealed = seal_capture(raw, world);
    assert(sealed.is_some());
    let retained = sealed.unwrap();
    let later = advance_point(world);
    old_evidence_needs_read_time_refresh(&retained, world);
    let reread = complete_raw_capture(later, required);
    let current = refresh_retained(&retained, reread, later);
    assert(current.is_some());
    assert(current.unwrap().valid_at(later));
    point_advance_preserves(raw, world);
    let restamped = refresh_retained(&retained, raw, later);
    assert(restamped.is_none());
}

/// Mutation invalidates the old transaction even after exact restoration.
/// Fresh reads after the revert can reestablish the old exact input context.
pub proof fn mutation_aba_and_exact_revert_control()
{
    example_input_context_is_well_formed();
    let world = InputWorld { context: example_input_context(), epoch: 0, point: 0 };
    let required = example_requirements();
    let raw = complete_raw_capture(world, required);
    let sealed_original = seal_capture(raw, world);
    let retained = sealed_original.unwrap();
    let changed_context = ModelContext {
        inputs: world.context.inputs.insert(example_key(ROLE_ANCESTOR),
            ReadOutcome::Present { resolved: seq![99u8], bytes: Seq::empty() }),
        ..world.context
    };
    let changed = change_world(world, changed_context);
    let reverted = change_world(changed, world.context);
    world_change_preserves(raw, world, changed_context);
    world_change_preserves(raw, changed, world.context);
    aba_capture_requires_reobservation(raw, world, changed_context);
    assert(!can_seal(raw, reverted));
    let empty = reobserve(raw, reverted);
    reobservation_resets_reads(raw, reverted);
    assert(!can_seal(empty, reverted));
    let reread = complete_raw_capture(reverted, required);
    let restored = refresh_retained(&retained, reread, reverted);
    assert(restored.is_some());
    assert(restored.unwrap().view() == retained.view());
    assert(restored.unwrap().valid_at(reverted));
}

pub proof fn partial_read_and_unreadable_control()
{
    example_input_context_is_well_formed();
    let world = InputWorld { context: example_input_context(), epoch: 0, point: 0 };
    let required = example_requirements();
    let initial = begin_capture(world, required);
    begin_capture_preserves(world, required);
    observation_preserves(initial, world, required[0]);
    let partial = observe_input(initial, world, required[0]);
    assert(!can_seal(partial, world));
    let unreadable_context = ModelContext { inputs: world.context.inputs.insert(required[0],
        ReadOutcome::Unreadable), ..world.context };
    let unreadable_world = change_world(world, unreadable_context);
    let started = begin_capture(unreadable_world, required);
    begin_capture_preserves(unreadable_world, required);
    observation_preserves(started, unreadable_world, required[0]);
    let failed = observe_input(started, unreadable_world, required[0]);
    unreadable_is_not_observed_absence(failed, unreadable_world, required[0]);
    let sealed = seal_capture(failed, unreadable_world);
    assert(sealed.is_none());
}

pub proof fn projection_coverage_and_duplicate_control()
{
    example_input_context_is_well_formed();
    let world = InputWorld { context: example_input_context(), epoch: 0, point: 0 };
    let raw = complete_raw_capture(world, example_requirements());
    let required = seq![example_key(ROLE_SOURCE), example_key(ROLE_ANCESTOR)];
    let projected = ModelContext {
        scope: ScopeKey { whole: false, selection: seq![40u8] },
        inputs: Map::empty()
            .insert(required[0], world.context.inputs[required[0]])
            .insert(required[1], world.context.inputs[required[1]]),
        ..world.context
    };
    let selected = seq![0int, 1int];
    assert(required.no_duplicates());
    assert(required.to_set() =~= projected.inputs.dom());
    assert(context_projection(world.context, projected));
    assert(projectable(raw, world, projected, required, selected));
    let projected_result = project_capture(raw, world, projected, required, selected);
    let good = projected_result.unwrap();
    assert(can_seal(good.0, good.1));
    let sealed = seal_capture(good.0, good.1);
    assert(sealed.is_some());
    assert(!sealed.unwrap().view().scope.whole);
    let duplicate_positions = seq![0int, 0int];
    let duplicate = project_capture(raw, world, projected, required, duplicate_positions);
    assert(duplicate.is_none());
    let incomplete = project_capture(raw, world, projected, seq![required[0]], seq![0int]);
    assert(incomplete.is_none());
    let false_whole = ModelContext { scope: world.context.scope, ..projected };
    assert(world.context.inputs.dom().contains(example_key(ROLE_GENERATED)));
    assert(!false_whole.inputs.dom().contains(example_key(ROLE_GENERATED)));
    assert(world.context.inputs != false_whole.inputs);
    assert(!context_projection(world.context, false_whole));
    let missing_whole = project_capture(raw, world, false_whole, required, selected);
    assert(missing_whole.is_none());
    let changed_solver = ModelContext {
        solver: SolverId { binary: seq![99u8], ..world.context.solver }, ..world.context };
    assert(world.context.solver.binary[0] != changed_solver.solver.binary[0]);
    assert(world.context.solver != changed_solver.solver);
    solver_change_cannot_project(world.context, changed_solver);
    let wrong_runtime = project_capture(raw, world, changed_solver, example_requirements(),
        seq![0int, 1int, 2int, 3int, 4int, 5int]);
    assert(wrong_runtime.is_none());
}

pub fn credential_selection_control() {
    let implicit = environment_is_bound(true, false, true);
    assert(!implicit);
    let declared = environment_is_bound(true, true, true);
    assert(declared);
}

/// The inductive execution interface has a concrete nonempty accepting trace,
/// and the same trace rejects evidence when followed by an actual input change.
pub proof fn finite_execution_control() {
    let key = example_key(ROLE_SOURCE);
    let context = ModelContext {
        inputs: Map::empty().insert(key,
            ReadOutcome::Present { resolved: seq![20u8], bytes: seq![21u8] }),
        ..example_input_context()
    };
    let world = InputWorld { context, epoch: 0, point: 0 };
    let required = seq![key];
    let read_actions = seq![InputAction::Read(key)];
    reveal_with_fuel(input_execution, 3);
    let final_state = input_execution(input_initial(world, required), read_actions);
    assert(final_state.world.context == context);
    assert(final_state.world.epoch == 0 && final_state.world.point == 1);
    assert(final_state.capture == observe_input(begin_capture(world, required), advance_point(world), key));
    assert(required.to_set() =~= context.inputs.dom());
    assert(context_wf(context));
    assert(can_seal(final_state.capture, final_state.world));
    let accepted = input_execution_seals_only_current(world, required, read_actions);
    assert(accepted.is_some());
    assert(accepted.unwrap().point() == 1);
    let changed = ModelContext { inputs: context.inputs.insert(key,
        ReadOutcome::Present { resolved: seq![20u8], bytes: seq![99u8] }), ..context };
    let changed_actions = read_actions.push(InputAction::Change(changed));
    let refused = input_execution_seals_only_current(world, required, changed_actions);
    assert(refused.is_none());
}

fn main() {
    positive_controls();
    credential_selection_control();
    proof {
        lifecycle_success_and_retained_read_control();
        mutation_aba_and_exact_revert_control();
        partial_read_and_unreadable_control();
        projection_coverage_and_duplicate_control();
        finite_execution_control();
    }
}

}
