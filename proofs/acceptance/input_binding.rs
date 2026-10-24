//! Exact observed-input comparison and observation-point proof component.
//!
//! This root verifies a pure component, not Sail's filesystem adapter. Paths,
//! environment values and executable identities are represented by exact bytes.
//! Inventory completeness and concrete stamp semantics remain refinement work.

#![forbid(unsafe_code)]

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

fn main() {
    positive_controls();
}

}
