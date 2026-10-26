//! Exact declared-input comparison and original observation guards.
//!
//! The same executable bodies run in Sail and in the direct stock-Verus gate.
//! Filesystem reads, digest production and compiler inventory discovery remain
//! explicit adapters; this module never accepts a caller-supplied Current flag.

#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    /// Strict lexicographic byte order, without hashing or normalization.
    pub open spec fn lex_at(a: Seq<u8>, b: Seq<u8>, i: int) -> bool {
        0 <= i <= a.len() && i <= b.len()
            && (forall|j: int| 0 <= j < i ==> a[j] == b[j])
            && (i == a.len() && i < b.len()
                || i < a.len() && i < b.len() && a[i] < b[i])
    }

    /// Lexicographic order has an explicit first-difference witness.
    pub open spec fn lex_less(a: Seq<u8>, b: Seq<u8>) -> bool {
        exists|i: int| lex_at(a, b, i)
    }

    /// One position agrees with its forward/inverse certificate.
    pub open spec fn permutation_at(forward: Seq<usize>, inverse: Seq<usize>, length: nat, i: int) -> bool {
        forward[i] < length && inverse[i] < length
            && inverse[forward[i] as int] == i
            && forward[inverse[i] as int] == i
    }

    /// A forward/inverse certificate explicitly accounts for every position.
    pub open spec fn valid_permutation(forward: Seq<usize>, inverse: Seq<usize>, length: nat) -> bool {
        forward.len() == length && inverse.len() == length
            && forall|i: int| 0 <= i < length ==> #[trigger] permutation_at(forward, inverse, length, i)
    }
}

sail_verified! {
    /// Compare every byte, proving exact sequence equality rather than a digest law.
    pub fn byte_eq(a: &[u8], b: &[u8]) -> (equal: bool)
        requires (true)
        ensures (equal == (a@ == b@))
    {
        if a.len() != b.len() { return false; }
        let mut i = 0;
        sail_while! { (i < a.len())
            invariant (i <= a.len(), a.len() == b.len(),
                forall|j: int| 0 <= j < i ==> a[j] == b[j])
            decreases (a.len() - i)
            {
                if a[i] != b[i] { return false; }
                i += 1;
            }
        }
        sail_proof! { assert(a@ =~= b@); }
        true
    }
}

sail_verified! {
    /// Strict byte order used to check a caller's proposed ordering certificate.
    pub fn byte_lt(a: &[u8], b: &[u8]) -> (less: bool)
        requires (true)
        ensures (less == lex_less(a@, b@))
    {
        sail_proof! { reveal(lex_less); }
        let mut i = 0;
        sail_while! { (i < a.len() && i < b.len())
            invariant (i <= a.len(), i <= b.len(),
                forall|j: int| 0 <= j < i ==> a[j] == b[j])
            decreases (a.len() - i)
            {
                if a[i] < b[i] {
                    sail_proof! { assert(lex_at(a@, b@, i as int)); assert(lex_less(a@, b@)); }
                    return true;
                }
                if a[i] > b[i] {
                    sail_proof! { if lex_less(a@, b@) {
                        let k = choose|k: int| lex_at(a@, b@, k);
                        if k < i { assert(a[k] == b[k]); }
                        else if k == i {} else { assert(a[i as int] == b[i as int]); }
                    } }
                    return false;
                }
                i += 1;
            }
        }
        let less = a.len() < b.len();
        sail_proof! {
            if less {
                assert(i == a.len());
                assert(lex_at(a@, b@, i as int));
                assert(lex_less(a@, b@));
            } else if lex_less(a@, b@) {
                let k = choose|k: int| lex_at(a@, b@, k);
                assert(i == b.len());
                assert(k < i);
                assert(a[k] == b[k]);
            }
        }
        less
    }
}

sail_verified! {
    /// Validate complete bijection data in linear time, without trusting a sorter.
    pub fn permutation(forward: &[usize], inverse: &[usize], length: usize) -> (valid: bool)
        requires (true)
        ensures (valid == valid_permutation(forward@, inverse@, length as nat))
    {
        if forward.len() != length || inverse.len() != length { return false; }
        let mut i = 0;
        sail_while! { (i < length)
            invariant (i <= length, forward.len() == length, inverse.len() == length,
                forall|j: int| 0 <= j < i ==> #[trigger] permutation_at(forward@, inverse@, length as nat, j))
            decreases (length - i)
            {
                let f = forward[i];
                let r = inverse[i];
                if f >= length || r >= length { sail_proof! {
                    assert(!permutation_at(forward@, inverse@, length as nat, i as int));
                    if valid_permutation(forward@, inverse@, length as nat) { assert(permutation_at(forward@, inverse@, length as nat, i as int)); }
                } return false; }
                if inverse[f] != i || forward[r] != i { sail_proof! {
                    assert(!permutation_at(forward@, inverse@, length as nat, i as int));
                    if valid_permutation(forward@, inverse@, length as nat) { assert(permutation_at(forward@, inverse@, length as nat, i as int)); }
                } return false; }
                sail_proof! { assert(permutation_at(forward@, inverse@, length as nat, i as int)); }
                i += 1;
            }
        }
        true
    }
}

sail_spec! {
    /// Strict byte ordering is transitive and therefore cannot hide duplicates.
    pub proof fn lex_less_transitive(a: Seq<u8>, b: Seq<u8>, c: Seq<u8>)
        requires lex_less(a, b), lex_less(b, c),
        ensures lex_less(a, c),
    {
        reveal(lex_less);
        let i = choose|i: int| lex_at(a, b, i);
        let k = choose|k: int| lex_at(b, c, k);
        let n = if i < k { i } else { k };
        assert forall|j: int| 0 <= j < n implies a[j] == c[j] by {}
        if i < k {
            assert(b[i] == c[i]);
        } else if k < i {
            assert(a[k] == b[k]);
        }
        assert(0 <= n <= a.len() && n <= c.len());
        assert(n == a.len() && n < c.len()
            || n < a.len() && n < c.len() && a[n] < c[n]);
        assert(lex_at(a, c, n));
        assert(lex_less(a, c));
    }
}

sail_code! {
    /// Content identities and alias destinations occupy distinct namespaces.
    pub struct InputFact<'a> {
        /// Stable role of this declared input (content or path binding).
        pub namespace: u8,
        /// Exact original encoded path or declared name.
        pub name: &'a [u8],
        /// Raw observed value; an error is never observed absence.
        pub value: InputValue<'a>,
    }

    /// A primitive input observation, before any equality authority exists.
    pub enum InputValue<'a> {
        /// Exact content identity or resolved-path bytes from the I/O adapter.
        Present(&'a [u8]),
        /// The primitive observation established absence.
        Absent,
        /// A primitive observation failed or was not accounted for.
        Unreadable,
    }

    /// Private comparison evidence borrows the exact compared inventories.
    pub struct Readback<'a> {
        expected: &'a [InputFact<'a>],
        observed: &'a [InputFact<'a>],
        before_order: &'a [usize],
        after_order: &'a [usize],
        equal: bool,
        changed: bool,
        unknown: bool,
    }
}

sail_spec! {
    /// Exact identity of a declared key.
    pub open spec fn same_key(a: InputFact, b: InputFact) -> bool {
        a.namespace == b.namespace && a.name@ == b.name@
    }
    /// Strict declared-key ordering uses the namespace followed by raw bytes.
    pub open spec fn key_less(a: InputFact, b: InputFact) -> bool {
        a.namespace < b.namespace || a.namespace == b.namespace && lex_less(a.name@, b.name@)
    }
    /// Typed observed-value identity refuses unreadable even on both sides.
    pub open spec fn same_value(a: InputValue, b: InputValue) -> bool {
        match (a, b) {
            (InputValue::Present(left), InputValue::Present(right)) => left@ == right@,
            (InputValue::Absent, InputValue::Absent) => true,
            _ => false,
        }
    }
    /// A successful primitive observation has data or explicit absence.
    pub open spec fn value_readable(value: InputValue) -> bool {
        !matches!(value, InputValue::Unreadable)
    }
    /// The ordering certificate is complete and has unique declared keys.
    pub open spec fn ordered_facts(facts: Seq<InputFact>, order: Seq<usize>, inverse: Seq<usize>) -> bool {
        valid_permutation(order, inverse, facts.len())
            && forall|i: int| 1 <= i < order.len()
                ==> #[trigger] key_less(facts[order[i - 1] as int], facts[order[i] as int])
    }
    /// Membership requires exact key and readable value equality.
    pub open spec fn contains_fact(facts: Seq<InputFact>, fact: InputFact) -> bool {
        exists|i: int| #![trigger facts[i]] 0 <= i < facts.len() && same_key(facts[i], fact) && same_value(facts[i].value, fact.value)
    }
    /// Every original row survives, in both directions, independent of order.
    pub open spec fn same_inventory(before: Seq<InputFact>, after: Seq<InputFact>) -> bool {
        (forall|i: int| 0 <= i < before.len() ==> contains_fact(after, #[trigger] before[i]))
            && (forall|i: int| 0 <= i < after.len() ==> contains_fact(before, #[trigger] after[i]))
    }
    /// Exact sorted equality includes every raw entry, not only reported matches.
    pub open spec fn facts_match(before: Seq<InputFact>, after: Seq<InputFact>,
        before_order: Seq<usize>, after_order: Seq<usize>) -> bool {
        before.len() > 0 && before.len() == after.len()
            && before_order.len() == before.len() && after_order.len() == after.len()
            && forall|i: int| #![trigger before_order[i]] 0 <= i < before.len() ==>
                before_order[i] < before.len() && after_order[i] < after.len()
                && same_key(before[before_order[i] as int], after[after_order[i] as int])
                && same_value(before[before_order[i] as int].value, after[after_order[i] as int].value)
    }
}

sail_spec! {
    /// Distinct raw positions cannot denote the same declared key.
    pub open spec fn unique_input_keys(facts: Seq<InputFact>) -> bool {
        forall|i: int, j: int| 0 <= i < j < facts.len()
            ==> !same_key(#[trigger] facts[i], #[trigger] facts[j])
    }

    /// Namespace plus byte ordering is transitive.
    pub proof fn key_less_transitive(a: InputFact, b: InputFact, c: InputFact)
        requires key_less(a, b), key_less(b, c),
        ensures key_less(a, c),
    {
        if a.namespace == b.namespace && b.namespace == c.namespace {
            lex_less_transitive(a.name@, b.name@, c.name@);
        }
    }

    /// Strict key ordering cannot order equal names in one namespace.
    pub proof fn key_less_differs(a: InputFact, b: InputFact)
        requires key_less(a, b),
        ensures !same_key(a, b),
    {
        if a.namespace == b.namespace {
            reveal(lex_less);
            let i = choose|i: int| lex_at(a.name@, b.name@, i);
            if a.name@ == b.name@ { assert(a.name@.len() == b.name@.len()); }
        }
    }

    /// Adjacent strict ordering extends to any two certified ranks.
    pub proof fn ordered_pair_less(facts: Seq<InputFact>, order: Seq<usize>, inverse: Seq<usize>, a: int, b: int)
        requires ordered_facts(facts, order, inverse), 0 <= a < b < order.len(),
        ensures key_less(facts[order[a] as int], facts[order[b] as int]),
        decreases b - a,
    {
        assert(permutation_at(order, inverse, facts.len(), a));
        assert(permutation_at(order, inverse, facts.len(), b));
        if a < b - 1 {
            ordered_pair_less(facts, order, inverse, a, b - 1);
            key_less_transitive(facts[order[a] as int], facts[order[b - 1] as int], facts[order[b] as int]);
        }
    }

    /// Complete inverse certificates convert sorted uniqueness to raw uniqueness.
    pub proof fn ordered_keys_unique(facts: Seq<InputFact>, order: Seq<usize>, inverse: Seq<usize>)
        requires ordered_facts(facts, order, inverse),
        ensures unique_input_keys(facts),
    {
        assert forall|i: int, j: int| 0 <= i < j < facts.len()
            implies !same_key(#[trigger] facts[i], #[trigger] facts[j]) by {
            assert(permutation_at(order, inverse, facts.len(), i));
            assert(permutation_at(order, inverse, facts.len(), j));
            let a = inverse[i] as int;
            let b = inverse[j] as int;
            if a < b {
                ordered_pair_less(facts, order, inverse, a, b);
                key_less_differs(facts[i], facts[j]);
            } else if b < a {
                ordered_pair_less(facts, order, inverse, b, a);
                key_less_differs(facts[j], facts[i]);
            }
        }
    }
}

sail_verified! {
    /// Exact key comparison; the caller cannot replace a path by a hash label.
    pub fn input_key_eq(a: &InputFact<'_>, b: &InputFact<'_>) -> (equal: bool)
        requires (true)
        ensures (equal == same_key(*a, *b))
    {
        a.namespace == b.namespace && byte_eq(a.name, b.name)
    }
}

sail_verified! {
    /// Strict key ordering checked independently of the native index sorter.
    pub fn input_key_lt(a: &InputFact<'_>, b: &InputFact<'_>) -> (less: bool)
        requires (true)
        ensures (less == key_less(*a, *b))
    {
        a.namespace < b.namespace || a.namespace == b.namespace && byte_lt(a.name, b.name)
    }
}

sail_verified! {
    /// Compare one certified sorted position, checking its indexing proof here.
    pub fn ordered_key_less(facts: &[InputFact<'_>], order: &[usize], _inverse: &[usize],
        rank: usize, wanted: &InputFact<'_>) -> (less: bool)
        requires (ordered_facts(facts@, order@, _inverse@), rank < facts.len())
        ensures (less == key_less(facts[order[rank as int] as int], *wanted))
    {
        sail_proof! { assert(permutation_at(order@, _inverse@, facts.len() as nat, rank as int)); }
        input_key_lt(&facts[order[rank]], wanted)
    }
}

sail_verified! {
    /// Check all permutation positions and strict adjacent-key order.
    pub fn input_order(facts: &[InputFact<'_>], order: &[usize], inverse: &[usize]) -> (valid: bool)
        requires (true)
        ensures (valid == ordered_facts(facts@, order@, inverse@),
            valid ==> unique_input_keys(facts@))
    {
        if !permutation(order, inverse, facts.len()) { return false; }
        let mut i = 1;
        sail_while! { (i < order.len())
            invariant (1 <= i, i <= order.len() || order.len() == 0,
                valid_permutation(order@, inverse@, facts.len() as nat),
                forall|j: int| 1 <= j < i && j < order.len()
                    ==> #[trigger] key_less(facts[order[j - 1] as int], facts[order[j] as int]))
            decreases (order.len() + 1 - i)
            {
                let previous = order[i - 1];
                let current = order[i];
                sail_proof! {
                    assert(permutation_at(order@, inverse@, facts.len() as nat, (i - 1) as int));
                    assert(permutation_at(order@, inverse@, facts.len() as nat, i as int));
                }
                if !input_key_lt(&facts[previous], &facts[current]) { return false; }
                i += 1;
            }
        }
        sail_proof! { ordered_keys_unique(facts@, order@, inverse@); }
        true
    }
}

sail_verified! {
    /// Classify actual value data: zero equal, one changed, two unreadable.
    #[allow(clippy::cast_lossless, reason = "The primitive bool cast has an exact stock-Verus semantics; From<bool> does not export that law.")]
    pub fn value_difference(a: &InputValue<'_>, b: &InputValue<'_>) -> (difference: u8)
        requires (true)
        ensures (difference <= 2,
            (difference == 0) == same_value(*a, *b),
            (difference == 2) == (!value_readable(*a) || !value_readable(*b)))
    {
        match (a, b) {
            (InputValue::Unreadable, _) | (_, InputValue::Unreadable) => 2,
            (InputValue::Absent, InputValue::Absent) => 0,
            (InputValue::Present(left), InputValue::Present(right)) => {
                (!byte_eq(left, right)) as u8
            }
            _ => 1,
        }
    }
}

impl<'a> Readback<'a> {
    sail_method! {
        /// Exact retained expected inventory, with every declared entry intact.
        pub fn expected(&self) -> (facts: &'a [InputFact<'a>])
            requires (true)
            ensures (facts@ == self.expected_view())
        { self.expected }
    }
    sail_method! {
        /// Exact retained primitive observations.
        pub fn observed(&self) -> (facts: &'a [InputFact<'a>])
            requires (true)
            ensures (facts@ == self.observed_view())
        { self.observed }
    }
    sail_method! {
        /// Validated candidate ordering of the expected entries.
        pub fn expected_order(&self) -> (order: &'a [usize])
            requires (true)
            ensures (order@ == self.expected_order_view())
        { self.before_order }
    }
    sail_method! {
        /// Validated candidate ordering of the primitive observation entries.
        pub fn observed_order(&self) -> (order: &'a [usize])
            requires (true)
            ensures (order@ == self.observed_order_view())
        { self.after_order }
    }
    sail_impl_spec! {
        /// Exact expected facts retained by the comparison.
        pub closed spec fn expected_view(&self) -> Seq<InputFact<'a>> { self.expected@ }
        /// Exact raw observations retained by the comparison.
        pub closed spec fn observed_view(&self) -> Seq<InputFact<'a>> { self.observed@ }
        /// Candidate expected ordering retained by the comparison.
        pub closed spec fn expected_order_view(&self) -> Seq<usize> { self.before_order@ }
        /// Candidate observed ordering retained by the comparison.
        pub closed spec fn observed_order_view(&self) -> Seq<usize> { self.after_order@ }
        /// This exact comparison established equality of the declared inputs.
        pub closed spec fn equal_view(&self) -> bool { self.equal }
        /// The compared immutable inventories and positions match exactly.
        pub closed spec fn matching_inputs(&self) -> bool {
            facts_match(self.expected@, self.observed@, self.before_order@, self.after_order@)
        }
        /// At least one concrete difference was observed.
        pub closed spec fn changed_view(&self) -> bool { self.changed }
        /// A comparison or input observation could not be established.
        pub closed spec fn unknown_view(&self) -> bool { self.unknown }
    }
    sail_method! {
        /// Whether this comparison established current input identity, not a proof verdict.
        pub fn is_equal(&self) -> (equal: bool)
            requires (true)
            ensures (equal == self.equal_view())
        { self.equal }
    }
    sail_method! {
        /// Whether an actual key/value difference was observed.
        pub fn changed(&self) -> (changed: bool)
            requires (true)
            ensures (changed == self.changed_view())
        { self.changed }
    }
    sail_method! {
        /// Whether some required observation or inventory check is unavailable.
        pub fn unknown(&self) -> (unknown: bool)
            requires (true)
            ensures (unknown == self.unknown_view())
        { self.unknown }
    }
}

sail_verified! {
    /// Compare complete typed readback inventories and retain immutable evidence.
    /// Unknown rows remain unknown; a detected difference retains precedence so
    /// fresh negative/historical results keep the existing Currentness semantics.
    pub fn compare_readback<'a>(expected: &'a [InputFact<'a>], observed: &'a [InputFact<'a>],
        before_order: &'a [usize], before_inverse: &[usize],
        after_order: &'a [usize], after_inverse: &[usize]) -> (result: Readback<'a>)
        requires (true)
        ensures (result.expected_view() == expected@, result.observed_view() == observed@,
            result.equal_view() == (!result.changed_view() && !result.unknown_view()),
            result.expected_order_view() == before_order@, result.observed_order_view() == after_order@,
            result.equal_view() == (expected.len() > 0
                && ordered_facts(expected@, before_order@, before_inverse@)
                && ordered_facts(observed@, after_order@, after_inverse@)
                && facts_match(expected@, observed@, before_order@, after_order@)),
            result.equal_view() ==> result.matching_inputs(),
            result.equal_view() ==> same_inventory(expected@, observed@),
            result.equal_view() ==> unique_input_keys(expected@) && unique_input_keys(observed@))
    {
        if expected.is_empty() || !input_order(expected, before_order, before_inverse)
            || !input_order(observed, after_order, after_inverse) {
            return Readback { expected, observed, before_order, after_order,
                equal: false, changed: false, unknown: true };
        }
        if expected.len() != observed.len() {
            return Readback { expected, observed, before_order, after_order,
                equal: false, changed: true, unknown: false };
        }
        let mut changed = false;
        let mut unknown = false;
        let mut i = 0;
        sail_while! { (i < expected.len())
            invariant (i <= expected.len(), expected.len() == observed.len(), expected.len() > 0,
                ordered_facts(expected@, before_order@, before_inverse@),
                ordered_facts(observed@, after_order@, after_inverse@),
                unique_input_keys(expected@), unique_input_keys(observed@),
                (!changed && !unknown) == (forall|j: int| #![trigger before_order[j]] 0 <= j < i ==>
                    before_order[j] < expected.len() && after_order[j] < observed.len() && same_key(expected[before_order[j] as int], observed[after_order[j] as int])
                    && same_value(expected[before_order[j] as int].value, observed[after_order[j] as int].value)))
            decreases (expected.len() - i)
            {
                sail_proof! {
                    assert(permutation_at(before_order@, before_inverse@, expected.len() as nat, i as int));
                    assert(permutation_at(after_order@, after_inverse@, observed.len() as nat, i as int));
                }
                let before = &expected[before_order[i]];
                let after = &observed[after_order[i]];
                if !input_key_eq(before, after) { changed = true; }
                let difference = value_difference(&before.value, &after.value);
                if difference == 1 { changed = true; }
                if difference == 2 { unknown = true; }
                i += 1;
            }
        }
        sail_proof! {
            if !changed && !unknown {
                assert(facts_match(expected@, observed@, before_order@, after_order@));
                assert forall|j: int| 0 <= j < expected.len() implies contains_fact(observed@, #[trigger] expected[j]) by {
                    assert(permutation_at(before_order@, before_inverse@, expected.len() as nat, j));
                    let rank = before_inverse[j];
                    assert(before_order[rank as int] == j);
                    assert(same_key(expected[j], observed[after_order[rank as int] as int]));
                    assert(contains_fact(observed@, expected[j]));
                }
                assert forall|j: int| 0 <= j < observed.len() implies contains_fact(expected@, #[trigger] observed[j]) by {
                    assert(permutation_at(after_order@, after_inverse@, observed.len() as nat, j));
                    let rank = after_inverse[j];
                    assert(after_order[rank as int] == j);
                    assert(same_key(expected[before_order[rank as int] as int], observed[j]));
                    assert(contains_fact(expected@, observed[j]));
                }
            }
        }
        Readback { expected, observed, before_order, after_order,
            equal: !changed && !unknown, changed, unknown }
    }
}

sail_code! {
    /// An unsealed before-read observation; no currentness can be obtained yet.
    pub struct OriginalCapture {
        before: Option<Vec<u8>>,
    }

    /// Original capture stamp bound to the revision whose reads it bracketed.
    #[derive(Debug)]
    pub struct OriginalGuard {
        stamp: Vec<u8>,
        revision: Vec<u8>,
    }

    /// Exact declared readback and its original bracketing capture together.
    /// This token supplies no independent compiler-closure or OS snapshot claim.
    pub struct GuardedReadback<'a> {
        comparison: Readback<'a>,
        guard: Option<OriginalGuard>,
        stamp_changed: bool,
    }
}

sail_spec! {
    /// Available primitive stamps are compared as exact byte strings.
    pub open spec fn optional_stamp_equal(a: Option<&[u8]>, b: Option<&[u8]>) -> bool {
        match (a, b) {
            (Some(left), Some(right)) => left@ == right@,
            _ => false,
        }
    }
}

sail_verified! {
    /// Copy exact identity bytes into privately held capture evidence.
    pub fn copy_bytes(bytes: &[u8]) -> (copied: Vec<u8>)
        requires (true)
        ensures (copied@ == bytes@)
    {
        let mut copied = Vec::new();
        let mut i = 0;
        sail_while! { (i < bytes.len())
            invariant (i <= bytes.len(), copied@ == bytes@.subrange(0, i as int))
            decreases (bytes.len() - i)
            { copied.push(bytes[i]); i += 1; }
        }
        sail_proof! { assert(bytes@.subrange(0, bytes.len() as int) =~= bytes@); }
        copied
    }
}

impl OriginalCapture {
    sail_impl_spec! {
        /// The before-read primitive observation, including unavailability.
        pub closed spec fn before_view(&self) -> Option<Seq<u8>> {
            match self.before { Some(bytes) => Some(bytes@), None => None }
        }
    }
    sail_method! {
        /// Begin before content reads; an unavailable stamp remains unsealable.
        #[allow(clippy::manual_map, reason = "Explicit Option variants preserve the identical executable proof branches.")]
        pub fn begin(before: Option<&[u8]>) -> (capture: OriginalCapture)
            requires (true)
            ensures (capture.before_view() == match before { Some(bytes) => Some(bytes@), None => None })
        {
            let before = match before {
                Some(bytes) => Some(copy_bytes(bytes)),
                None => None,
            };
            OriginalCapture { before }
        }
    }
    sail_method! {
        /// Seal only when the after-read primitive stamp is the original stamp.
        /// The adapter supplies the revision produced by those intervening reads.
        #[allow(clippy::question_mark, reason = "Explicit early returns preserve the identical executable proof branches.")]
        pub fn finish(self, after: Option<&[u8]>, revision: &[u8]) -> (guard: Option<OriginalGuard>)
            requires (true)
            ensures (guard.is_some() == match (self.before_view(), after) {
                (Some(before), Some(after)) => before == after@, _ => false },
                guard.is_some() ==> guard.unwrap().revision_view() == revision@,
                guard.is_some() ==> Some(guard.unwrap().stamp_view()) == self.before_view())
        {
            let Some(before) = self.before else { return None; };
            let Some(after) = after else { return None; };
            if !byte_eq(&before, after) { return None; }
            Some(OriginalGuard { stamp: before, revision: copy_bytes(revision) })
        }
    }
}

impl OriginalGuard {
    sail_impl_spec! {
        /// Immutable original capture stamp.
        pub closed spec fn stamp_view(&self) -> Seq<u8> { self.stamp@ }
        /// Revision bound when the original content read completed.
        pub closed spec fn revision_view(&self) -> Seq<u8> { self.revision@ }
    }
    sail_method! {
        /// Reuse needs the bound revision, the original supplied stamp and a
        /// present primitive observation still equal to that original stamp.
        pub fn retained(&self, revision: &[u8], supplied: Option<&[u8]>, now: Option<&[u8]>) -> (current: bool)
            requires (true)
            ensures (current == (revision@ == self.revision_view()
                && match (supplied, now) {
                    (Some(supplied), Some(now)) => supplied@ == self.stamp_view() && now@ == self.stamp_view(),
                    _ => false,
                }))
        {
            let Some(supplied) = supplied else { return false; };
            let Some(now) = now else { return false; };
            byte_eq(revision, &self.revision) && byte_eq(supplied, &self.stamp) && byte_eq(now, &self.stamp)
        }
    }
}

impl OriginalGuard {
    sail_method! {
        /// Preserve the original capture identity when cloning a snapshot.
        #[must_use]
        pub fn duplicate(&self) -> (guard: Self)
            requires (true)
            ensures (guard.stamp_view() == self.stamp_view(), guard.revision_view() == self.revision_view())
        { OriginalGuard { stamp: copy_bytes(&self.stamp), revision: copy_bytes(&self.revision) } }
    }
}

impl<'a> GuardedReadback<'a> {
    sail_impl_spec! {
        /// The exact complete typed comparison retained by this transaction.
        pub closed spec fn comparison_view(&self) -> Readback<'a> { self.comparison }
        /// Original/final observation evidence, present only after sealing.
        pub closed spec fn guard_view(&self) -> Option<OriginalGuard> { self.guard }
        /// A concrete input or guard difference was observed.
        pub closed spec fn changed_view(&self) -> bool { self.comparison.changed_view() || self.stamp_changed }
        /// A required comparison or stamp could not be established.
        pub closed spec fn unknown_view(&self) -> bool { self.comparison.unknown_view() || self.guard.is_none() }
        /// This batch may return the original complete revision.
        pub open spec fn unchanged_view(&self) -> bool { !self.changed_view() && !self.unknown_view() }
    }
    sail_method! {
        /// Borrow the original immutable typed comparison, preserving its domain.
        pub fn comparison(&self) -> (comparison: &Readback<'a>)
            requires (true)
            ensures (comparison.expected_view() == self.comparison_view().expected_view(),
                comparison.observed_view() == self.comparison_view().observed_view())
        { &self.comparison }
    }
    sail_method! {
        /// Bind a usable typed readback to the revision sealed around those
        /// reads. This does not establish physical compiler-input completeness.
        pub fn matches_revision(&self, revision: &[u8]) -> (matches: bool)
            requires (true)
            ensures (matches == (self.unchanged_view() && self.guard_view().is_some()
                && self.guard_view().unwrap().revision_view() == revision@))
        {
            if self.changed() || self.unknown() { return false; }
            match &self.guard {
                Some(guard) => byte_eq(revision, &guard.revision),
                None => false,
            }
        }
    }
    sail_method! {
        /// An actual difference retains precedence for Historical results.
        pub fn changed(&self) -> (changed: bool)
            requires (true) ensures (changed == self.changed_view())
        { self.comparison.changed() || self.stamp_changed }
    }
    sail_method! {
        /// Unavailable evidence cannot establish the original complete revision.
        pub fn unknown(&self) -> (unknown: bool)
            requires (true) ensures (unknown == self.unknown_view())
        { self.comparison.unknown() || self.guard.is_none() }
    }
}

sail_verified! {
    /// Compare the exact declared read domain and seal its original capture.
    /// Input equality and present equal endpoint stamps are both necessary;
    /// neither supplies compiler-closure completeness or OS linearization.
    #[allow(clippy::too_many_arguments, reason = "Both exact inventories, checked permutation proposals and original/final observations are explicit.")]
    pub fn guard_readback<'a>(expected: &'a [InputFact<'a>], observed: &'a [InputFact<'a>],
        before_order: &'a [usize], before_inverse: &[usize],
        after_order: &'a [usize], after_inverse: &[usize],
        capture: OriginalCapture, after: Option<&[u8]>, revision: &[u8]) -> (result: GuardedReadback<'a>)
        requires (true)
        ensures (result.comparison_view().expected_view() == expected@,
            result.comparison_view().observed_view() == observed@,
            result.comparison_view().equal_view() == (expected.len() > 0
                && ordered_facts(expected@, before_order@, before_inverse@)
                && ordered_facts(observed@, after_order@, after_inverse@)
                && facts_match(expected@, observed@, before_order@, after_order@)),
            result.comparison_view().equal_view() == (!result.comparison_view().changed_view() && !result.comparison_view().unknown_view()),
            result.changed_view() == (result.comparison_view().changed_view()
                || match (capture.before_view(), after) { (Some(before), Some(after)) => before != after@, _ => false }),
            result.unknown_view() == (result.comparison_view().unknown_view() || result.guard_view().is_none()),
            result.guard_view().is_some() == match (capture.before_view(), after) { (Some(before), Some(after)) => before == after@, _ => false },
            result.unchanged_view() == (result.comparison_view().equal_view()
                && match (capture.before_view(), after) { (Some(before), Some(after)) => before == after@, _ => false }),
            result.unchanged_view() ==> same_inventory(expected@, observed@)
                && expected.len() > 0 && unique_input_keys(expected@) && unique_input_keys(observed@),
            result.unchanged_view() ==> result.guard_view().is_some()
                && result.guard_view().unwrap().revision_view() == revision@
                && Some(result.guard_view().unwrap().stamp_view()) == capture.before_view()
                && Some(result.guard_view().unwrap().stamp_view()) == match after { Some(bytes) => Some(bytes@), None => None })
    {
        let comparison = compare_readback(expected, observed, before_order, before_inverse,
            after_order, after_inverse);
        let stamp_changed = match (&capture.before, after) {
            (Some(before), Some(after)) => !byte_eq(before, after),
            _ => false,
        };
        let guard = capture.finish(after, revision);
        GuardedReadback { comparison, guard, stamp_changed }
    }
}

sail_spec! {
    /// Every requested selected fact is an exact readable fact of the source.
    pub open spec fn covered_selection(source: Seq<InputFact>, selected: Seq<InputFact>) -> bool {
        selected.len() > 0 && forall|i: int| 0 <= i < selected.len()
            ==> contains_fact(source, #[trigger] selected[i])
    }

    /// A sorted cursor that skipped only smaller keys cannot pass a member.
    pub proof fn ordered_lookup_stops_at_member(source: Seq<InputFact>, order: Seq<usize>,
        inverse: Seq<usize>, wanted: InputFact, cursor: int)
        requires ordered_facts(source, order, inverse), 0 <= cursor <= source.len(),
            contains_fact(source, wanted),
            forall|k: int| 0 <= k < cursor ==> key_less(source[#[trigger] order[k] as int], wanted),
            cursor < source.len() ==> !key_less(source[order[cursor] as int], wanted),
        ensures cursor < source.len(), same_key(source[order[cursor] as int], wanted),
            same_value(source[order[cursor] as int].value, wanted.value),
    {
        let position = choose|p: int| 0 <= p < source.len() && same_key(source[p], wanted)
            && same_value(source[p].value, wanted.value);
        assert(permutation_at(order, inverse, source.len(), position));
        let rank = inverse[position] as int;
        if rank < cursor {
            assert(key_less(source[position], wanted));
            key_less_differs(source[position], wanted);
        } else if cursor < rank {
            ordered_pair_less(source, order, inverse, cursor, rank);
            assert(key_less(source[order[cursor] as int], wanted));
        }
    }
}

sail_verified! {
    /// Validate every selected key/value against the original observation.
    /// Both candidate orderings must account for every entry exactly once.
    #[allow(clippy::too_many_arguments, reason = "Both complete permutation certificates are explicit and independently checked.")]
    pub fn selection_covered(source: &[InputFact<'_>], selected: &[InputFact<'_>],
        source_order: &[usize], source_inverse: &[usize],
        selected_order: &[usize], selected_inverse: &[usize]) -> (covered: bool)
        requires (true)
        ensures (covered == (ordered_facts(source@, source_order@, source_inverse@)
                && ordered_facts(selected@, selected_order@, selected_inverse@)
                && covered_selection(source@, selected@)),
            covered ==> unique_input_keys(source@) && unique_input_keys(selected@))
    {
        if selected.is_empty() || !input_order(source, source_order, source_inverse)
            || !input_order(selected, selected_order, selected_inverse) { return false; }
        let mut i = 0;
        let mut j = 0;
        sail_while! { (i < selected.len())
            invariant (i <= selected.len(), j <= source.len(), selected.len() > 0,
                ordered_facts(source@, source_order@, source_inverse@),
                ordered_facts(selected@, selected_order@, selected_inverse@),
                unique_input_keys(source@), unique_input_keys(selected@),
                i < selected.len() ==> forall|k: int| 0 <= k < j ==>
                    key_less(source[#[trigger] source_order[k] as int], selected[selected_order[i as int] as int]),
                forall|k: int| 0 <= k < i ==> contains_fact(source@, #[trigger] selected[selected_order[k] as int]))
            decreases (selected.len() - i)
            {
                sail_proof! { assert(permutation_at(selected_order@, selected_inverse@, selected.len() as nat, i as int)); }
                let wanted = &selected[selected_order[i]];
                sail_while! { (j < source.len() && ordered_key_less(source, source_order, source_inverse, j, wanted))
                    invariant (j <= source.len(), i < selected.len(),
                        ordered_facts(source@, source_order@, source_inverse@),
                        forall|k: int| 0 <= k < j ==> key_less(source[#[trigger] source_order[k] as int], *wanted))
                    decreases (source.len() - j)
                    {
                        sail_proof! { assert(permutation_at(source_order@, source_inverse@, source.len() as nat, j as int)); }
                        j += 1;
                    }
                }
                sail_proof! {
                    if covered_selection(source@, selected@) {
                        assert(contains_fact(source@, *wanted));
                        ordered_lookup_stops_at_member(source@, source_order@, source_inverse@, *wanted, j as int);
                    }
                }
                if j >= source.len() { return false; }
                sail_proof! { assert(permutation_at(source_order@, source_inverse@, source.len() as nat, j as int)); }
                let original = &source[source_order[j]];
                if !input_key_eq(original, wanted) || value_difference(&original.value, &wanted.value) != 0 {
                    return false;
                }
                sail_proof! { assert(contains_fact(source@, *wanted)); }
                sail_proof! {
                    if i + 1 < selected.len() {
                        ordered_pair_less(selected@, selected_order@, selected_inverse@, i as int, i as int + 1);
                        assert forall|k: int| 0 <= k < j implies
                            key_less(source[#[trigger] source_order[k] as int], selected[selected_order[i as int + 1] as int]) by {
                            key_less_transitive(source[source_order[k] as int], *wanted, selected[selected_order[i as int + 1] as int]);
                        }
                    }
                }
                i += 1;
            }
        }
        sail_proof! {
            assert forall|k: int| 0 <= k < selected.len() implies contains_fact(source@, #[trigger] selected[k]) by {
                assert(permutation_at(selected_order@, selected_inverse@, selected.len() as nat, k));
                let rank = selected_inverse[k];
                assert(selected_order[rank as int] == k);
            }
        }
        true
    }
}

impl OriginalGuard {
    sail_method! {
        /// A projection keeps the original stamp and binds its exact selected
        /// facts to a new revision. It does not establish currentness by itself.
        #[allow(clippy::too_many_arguments, reason = "The source identity and both checked full-index certificates are distinct evidence.")]
        pub fn project(&self, source_revision: &[u8], revision: &[u8],
            source: &[InputFact<'_>], selected: &[InputFact<'_>],
            source_order: &[usize], source_inverse: &[usize],
            selected_order: &[usize], selected_inverse: &[usize]) -> (guard: Option<OriginalGuard>)
            requires (true)
            ensures (guard.is_some() ==> source_revision@ == self.revision_view(),
                guard.is_some() ==> covered_selection(source@, selected@),
                guard.is_some() ==> guard.unwrap().stamp_view() == self.stamp_view(),
                guard.is_some() ==> guard.unwrap().revision_view() == revision@)
        {
            if !byte_eq(source_revision, &self.revision) || !selection_covered(source, selected,
                source_order, source_inverse, selected_order, selected_inverse) { return None; }
            Some(OriginalGuard { stamp: copy_bytes(&self.stamp), revision: copy_bytes(revision) })
        }
    }
}

sail_spec! {
    /// The primitive observation contains bytes, rather than absence or error.
    pub open spec fn value_present(value: InputValue) -> bool {
        matches!(value, InputValue::Present(_))
    }
    /// A required name has a complete present primitive observation.
    pub open spec fn contains_present(facts: Seq<InputFact>, wanted: InputFact) -> bool {
        exists|i: int| #![trigger facts[i]] 0 <= i < facts.len()
            && same_key(facts[i], wanted) && value_present(facts[i].value)
    }
    /// The complete declared file inventory was observed, without inferred reads.
    pub open spec fn complete_present_inventory(required: Seq<InputFact>, observed: Seq<InputFact>) -> bool {
        required.len() == observed.len()
            && forall|i: int| 0 <= i < required.len() ==> contains_present(observed, #[trigger] required[i])
    }
}

sail_spec! {
    /// Exact executable acceptance condition for keyed dep-info/read coverage.
    pub open spec fn ordered_present_reads(required: Seq<InputFact>, observed: Seq<InputFact>,
        required_order: Seq<usize>, required_inverse: Seq<usize>,
        observed_order: Seq<usize>, observed_inverse: Seq<usize>) -> bool {
        required.len() == observed.len()
            && ordered_facts(required, required_order, required_inverse)
            && ordered_facts(observed, observed_order, observed_inverse)
            && forall|i: int| #![trigger required_order[i]] 0 <= i < required.len() ==>
                same_key(required[required_order[i] as int], observed[observed_order[i] as int])
                && value_present(observed[observed_order[i] as int].value)
    }
}

sail_verified! {
    /// Certify every declared file was actually represented by a successful read.
    /// A missing, unreadable, absent or duplicate row cannot establish coverage.
    pub fn reads_complete(required: &[InputFact<'_>], observed: &[InputFact<'_>],
        required_order: &[usize], required_inverse: &[usize],
        observed_order: &[usize], observed_inverse: &[usize]) -> (complete: bool)
        requires (true)
        ensures (complete == ordered_present_reads(required@, observed@, required_order@, required_inverse@,
                observed_order@, observed_inverse@),
            complete ==> complete_present_inventory(required@, observed@),
            complete ==> unique_input_keys(required@) && unique_input_keys(observed@))
    {
        if required.len() != observed.len()
            || !input_order(required, required_order, required_inverse)
            || !input_order(observed, observed_order, observed_inverse) { return false; }
        let mut i = 0;
        sail_while! { (i < required.len())
            invariant (i <= required.len(), required.len() == observed.len(),
                ordered_facts(required@, required_order@, required_inverse@),
                ordered_facts(observed@, observed_order@, observed_inverse@),
                unique_input_keys(required@), unique_input_keys(observed@),
                forall|j: int| #![trigger required_order[j]] 0 <= j < i ==>
                    same_key(required[required_order[j] as int], observed[observed_order[j] as int])
                    && value_present(observed[observed_order[j] as int].value),
                forall|j: int| 0 <= j < i ==> contains_present(observed@, #[trigger] required[required_order[j] as int]))
            decreases (required.len() - i)
            {
                sail_proof! {
                    assert(permutation_at(required_order@, required_inverse@, required.len() as nat, i as int));
                    assert(permutation_at(observed_order@, observed_inverse@, observed.len() as nat, i as int));
                }
                let wanted = &required[required_order[i]];
                let actual = &observed[observed_order[i]];
                if !input_key_eq(wanted, actual) || !matches!(actual.value, InputValue::Present(_)) {
                    sail_proof! {
                        assert(!same_key(required[required_order[i as int] as int], observed[observed_order[i as int] as int])
                            || !value_present(observed[observed_order[i as int] as int].value));
                    }
                    return false;
                }
                sail_proof! {
                    assert(same_key(required[required_order[i as int] as int], observed[observed_order[i as int] as int]));
                    assert(value_present(observed[observed_order[i as int] as int].value));
                    assert(contains_present(observed@, *wanted));
                    assert forall|j: int| #![trigger required_order[j]] 0 <= j < i + 1 implies
                        same_key(required[required_order[j] as int], observed[observed_order[j] as int])
                            && value_present(observed[observed_order[j] as int].value) by {
                        assert(permutation_at(required_order@, required_inverse@, required.len() as nat, j));
                        assert(permutation_at(observed_order@, observed_inverse@, observed.len() as nat, j));
                        if j < i {
                            assert(same_key(required[required_order[j] as int], observed[observed_order[j] as int]));
                        }
                    }
                }
                i += 1;
            }
        }
        sail_proof! {
            assert forall|j: int| 0 <= j < required.len() implies contains_present(observed@, #[trigger] required[j]) by {
                assert(permutation_at(required_order@, required_inverse@, required.len() as nat, j));
                let rank = required_inverse[j];
                assert(required_order[rank as int] == j);
            }
        }
        true
    }
}

sail_code! {
    /// One raw private-journal event. Parsing and physical delivery are adapters.
    #[derive(Debug)]
    pub struct ProviderEvent {
        /// Zero: intent, one: actual start, two: artifact needing dep-info,
        /// three: terminal, four: build-script or custom-build frame.
        pub kind: u8,
        /// The adapter process that declared the child before spawning it.
        pub producer: u32,
        /// Actual child PID; zero only in the pre-spawn intent.
        pub child: u32,
        /// Observed retained-frame count, meaningful in the terminal event.
        pub frames: usize,
        /// Actual normal-exit wait result, meaningful in the terminal event.
        pub exited: bool,
    }

    /// Checked completion of one declared provider; never a compiler-closure claim.
    #[derive(Debug)]
    pub struct ProviderSeal {
        producer: u32,
        child: u32,
        frames: usize,
    }
}

sail_spec! {
    /// Exact event order, provenance and terminal count for a completed provider.
    pub open spec fn provider_trace(events: Seq<ProviderEvent>) -> bool {
        events.len() >= 3
            && events[0].kind == 0 && events[0].producer != 0 && events[0].child == 0
            && events[1].kind == 1 && events[1].producer == events[0].producer && events[1].child != 0
            && events.last().kind == 3 && events.last().exited
            && events.last().producer == events[0].producer && events.last().child == events[1].child
            && events.last().frames == events.len() - 3
            && forall|i: int| 2 <= i < events.len() - 1 ==> #[trigger] provider_frame(events, i)
    }

    /// Every intervening frame belongs to the actual started child.
    pub open spec fn provider_frame(events: Seq<ProviderEvent>, i: int) -> bool {
        (events[i].kind == 2 || events[i].kind == 4) && events[i].producer == events[0].producer && events[i].child == events[1].child
    }
}

sail_verified! {
    /// Seal only a complete intent/start/frames/normal-exit trace.
    pub fn seal_provider(events: &[ProviderEvent]) -> (seal: Option<ProviderSeal>)
        requires (true)
        ensures (seal.is_some() == provider_trace(events@),
            seal.is_some() ==> seal.unwrap().producer_view() == events[0].producer
                && seal.unwrap().child_view() == events[1].child
                && seal.unwrap().frames_view() == events.len() - 3)
    {
        if events.len() < 3 { return None; }
        let intent = &events[0];
        let start = &events[1];
        let end = &events[events.len() - 1];
        if intent.kind != 0 || intent.producer == 0 || intent.child != 0
            || start.kind != 1 || start.producer != intent.producer || start.child == 0
            || end.kind != 3 || !end.exited || end.producer != intent.producer
            || end.child != start.child || end.frames != events.len() - 3 { return None; }
        let mut i = 2;
        sail_while! { (i < events.len() - 1)
            invariant (2 <= i <= events.len() - 1, events.len() >= 3,
                intent.producer == events[0].producer, start.child == events[1].child,
                forall|j: int| 2 <= j < i ==> #[trigger] provider_frame(events@, j))
            decreases (events.len() - 1 - i)
            {
                let frame = &events[i];
                if (frame.kind != 2 && frame.kind != 4) || frame.producer != intent.producer || frame.child != start.child {
                    sail_proof! { assert(!provider_frame(events@, i as int)); }
                    return None;
                }
                i += 1;
            }
        }
        Some(ProviderSeal { producer: intent.producer, child: start.child, frames: end.frames })
    }
}

impl ProviderSeal {
    sail_method! {
        /// Adapter process whose intent is sealed.
        #[must_use]
        pub fn producer(&self) -> (producer: u32)
            requires (true) ensures (producer == self.producer_view())
        { self.producer }
    }
    sail_method! {
        /// Actual started child whose frames are sealed.
        #[must_use]
        pub fn child(&self) -> (child: u32)
            requires (true) ensures (child == self.child_view())
        { self.child }
    }
    sail_method! {
        /// Number of retained frames represented by this seal.
        #[must_use]
        pub fn frames(&self) -> (frames: usize)
            requires (true) ensures (frames == self.frames_view())
        { self.frames }
    }
    sail_impl_spec! {
        /// Producer whose pre-spawn intent is present in the checked trace.
        pub closed spec fn producer_view(&self) -> u32 { self.producer }
        /// Actual started child named by every checked frame and terminal.
        pub closed spec fn child_view(&self) -> u32 { self.child }
        /// Exact observed-frame count.
        pub closed spec fn frames_view(&self) -> usize { self.frames }
    }
}

sail_code! {
    /// A dep-info read carrying its own actual artifact-frame identity.
    #[derive(Debug)]
    pub struct ArtifactRead<'a> {
        /// Adapter that declared the child.
        pub producer: u32,
        /// Actual child responsible for the artifact.
        pub child: u32,
        /// Position of this artifact in that child's checked event trace.
        pub frame: usize,
        /// Actual dep-info bytes; absent for unreadable/unusable observations.
        pub raw: Option<&'a [u8]>,
    }
}

sail_spec! {
    /// One successful typed read names this exact required artifact position.
    pub open spec fn artifact_read_at(events: Seq<ProviderEvent>, read: ArtifactRead, frame: int) -> bool {
        2 <= frame < events.len() - 1 && events[frame].kind == 2
            && read.producer == events[0].producer && read.child == events[1].child
            && read.frame == frame && read.raw.is_some()
    }

    /// The independent outcome ledger covers every required artifact once and
    /// contains no results for other artifacts, children or exempt frames.
    pub open spec fn artifact_read_coverage(events: Seq<ProviderEvent>, reads: Seq<ArtifactRead>) -> bool {
        (forall|r: int| 0 <= r < reads.len() ==> #[trigger] artifact_read_at(events, reads[r], reads[r].frame as int))
            && (forall|r: int, t: int| 0 <= r < t < reads.len() ==> #[trigger] reads[r].frame < #[trigger] reads[t].frame)
            && (forall|frame: int| 2 <= frame < events.len() - 1 && events[frame].kind == 2 ==>
                exists|r: int| 0 <= r < reads.len() && #[trigger] reads[r].frame == frame)
    }
}

sail_spec! {
    /// A concrete prefix consumed exactly its earlier artifact reads.
    pub open spec fn artifact_prefix(events: Seq<ProviderEvent>, reads: Seq<ArtifactRead>, frame: int, r: int) -> bool {
        2 <= frame <= events.len() - 1 && 0 <= r <= reads.len()
            && (forall|j: int| 0 <= j < r ==> #[trigger] artifact_read_at(events, reads[j], reads[j].frame as int)
                && reads[j].frame < frame)
            && (forall|j: int, k: int| 0 <= j < k < r ==> #[trigger] reads[j].frame < #[trigger] reads[k].frame)
            && (forall|f: int| 2 <= f < frame && events[f].kind == 2 ==>
                exists|j: int| 0 <= j < r && #[trigger] reads[j].frame == f)
    }

    /// The actual scan adds exactly the current required frame, or no read for
    /// an exempt frame. This lemma is a concrete inductive state transition.
    pub proof fn artifact_prefix_step(events: Seq<ProviderEvent>, reads: Seq<ArtifactRead>, frame: int, before: int, after: int)
        requires provider_trace(events), artifact_prefix(events, reads, frame, before), frame < events.len() - 1,
            after == before + if events[frame].kind == 2 { 1int } else { 0int },
            events[frame].kind == 2 ==> before < reads.len() && artifact_read_at(events, reads[before], frame),
        ensures artifact_prefix(events, reads, frame + 1, after),
    {
        assert forall|j: int| 0 <= j < after implies
            #[trigger] artifact_read_at(events, reads[j], reads[j].frame as int) && reads[j].frame < frame + 1 by {
            if j == before { assert(artifact_read_at(events, reads[j], frame)); }
            else { assert(artifact_read_at(events, reads[j], reads[j].frame as int)); }
        }
        assert forall|j: int, k: int| 0 <= j < k < after implies #[trigger] reads[j].frame < #[trigger] reads[k].frame by {
            if k == before { assert(artifact_read_at(events, reads[j], reads[j].frame as int)); }
            else { assert(reads[j].frame < reads[k].frame); }
        }
        assert forall|f: int| 2 <= f < frame + 1 && events[f].kind == 2 implies
            exists|j: int| 0 <= j < after && #[trigger] reads[j].frame == f by {
            if f == frame { assert(reads[before].frame == f); }
            else {
                let j = choose|j: int| 0 <= j < before && reads[j].frame == f;
                assert(0 <= j < after);
            }
        }
    }

    /// Exact global coverage determines the next actual artifact outcome.
    pub proof fn artifact_next(events: Seq<ProviderEvent>, reads: Seq<ArtifactRead>, frame: int, r: int)
        requires provider_trace(events), artifact_read_coverage(events, reads), artifact_prefix(events, reads, frame, r),
        ensures frame == events.len() - 1 ==> r == reads.len(),
            frame < events.len() - 1 && events[frame].kind == 2 ==>
                r < reads.len() && artifact_read_at(events, reads[r], frame),
    {
        if r < reads.len() {
            assert(artifact_read_at(events, reads[r], reads[r].frame as int));
            if reads[r].frame < frame {
                let j = choose|j: int| 0 <= j < r && reads[j].frame == reads[r].frame;
                assert(reads[j].frame < reads[r].frame);
            }
        }
        if frame == events.len() - 1 {
            if r < reads.len() { assert(reads[r].frame < frame); }
        } else if events[frame].kind == 2 {
            let k = choose|k: int| 0 <= k < reads.len() && reads[k].frame == frame;
            if k < r { assert(artifact_read_at(events, reads[k], reads[k].frame as int)); assert(reads[k].frame < frame); }
            if r < k { assert(reads[r].frame < reads[k].frame); }
            assert(k == r);
            assert(artifact_read_at(events, reads[r], frame));
        }
    }
}

sail_verified! {
    /// Seal only a completed provider whose every actual required artifact has
    /// exactly one matching successful dep-info outcome. Sorting is untrusted.
    #[allow(clippy::question_mark, reason = "Explicit early returns preserve the identical executable proof branches.")]
    pub fn seal_compiler_provider(events: &[ProviderEvent], reads: &[ArtifactRead<'_>]) -> (seal: Option<ProviderSeal>)
        requires (true)
        ensures (seal.is_some() == (provider_trace(events@) && artifact_read_coverage(events@, reads@)))
    {
        let seal = seal_provider(events);
        if seal.is_none() { return None; }
        let mut frame = 2usize;
        let mut r = 0usize;
        sail_while! { (frame < events.len() - 1)
            invariant (provider_trace(events@), artifact_prefix(events@, reads@, frame as int, r as int))
            decreases (events.len() - 1 - frame)
            {
                sail_proof! {
                    if artifact_read_coverage(events@, reads@) { artifact_next(events@, reads@, frame as int, r as int); }
                }
                if events[frame].kind == 2 {
                    if r >= reads.len() { return None; }
                    let read = &reads[r];
                    if read.producer != events[0].producer || read.child != events[1].child
                        || read.frame != frame || read.raw.is_none() { return None; }
                    sail_proof! { assert(artifact_read_at(events@, reads[r as int], frame as int)); }
                    r += 1;
                }
                sail_proof! {
                    let before = if events[frame as int].kind == 2 { r as int - 1 } else { r as int };
                    artifact_prefix_step(events@, reads@, frame as int, before, r as int);
                }
                frame += 1;
            }
        }
        sail_proof! { if artifact_read_coverage(events@, reads@) { artifact_next(events@, reads@, frame as int, r as int); } }
        if r != reads.len() { return None; }
        seal
    }
}

sail_code! {
    /// Raw attributes observed from a pathname or the owned opened file.
    /// Equality is exact; interpreting these fields is an explicit OS adapter.
    #[derive(Debug)]
    pub struct ObjectStamp {
        /// Filesystem device identity.
        pub device: u64,
        /// Open object identity within that filesystem.
        pub object: u64,
        /// Reported byte length, checked for native address-size representability.
        pub length: usize,
        /// Raw permissions and file-kind mode bits.
        pub mode: u32,
        /// Modification timestamp seconds.
        pub modified_seconds: i64,
        /// Modification timestamp fractional nanoseconds.
        pub modified_nanos: i64,
        /// Change timestamp seconds.
        pub changed_seconds: i64,
        /// Change timestamp fractional nanoseconds.
        pub changed_nanos: i64,
        /// One is a regular file; all other kinds refuse this read protocol.
        pub kind: u8,
    }

    /// Owned read state bound to the original opened object's attributes.
    #[derive(Debug)]
    pub struct OpenedRead {
        original: ObjectStamp,
        consumed: usize,
        ended: bool,
        failed: bool,
    }

    /// The exact successful chunk supplied to this read transaction.
    #[derive(Debug)]
    pub struct ReadChunk<'a> {
        bytes: &'a [u8],
        device: u64,
        object: u64,
    }

    /// Complete EOF observation with the original opened/path object preserved.
    #[derive(Debug)]
    pub struct ReadSeal {
        original: ObjectStamp,
        consumed: usize,
    }
}

sail_verified! {
    /// Compare every structured object attribute without digest assumptions.
    pub fn same_object_stamp(a: &ObjectStamp, b: &ObjectStamp) -> (equal: bool)
        requires (true)
        ensures (equal == (*a == *b))
    {
        a.device == b.device && a.object == b.object && a.length == b.length
            && a.mode == b.mode && a.modified_seconds == b.modified_seconds
            && a.modified_nanos == b.modified_nanos && a.changed_seconds == b.changed_seconds
            && a.changed_nanos == b.changed_nanos && a.kind == b.kind
    }
}

impl OpenedRead {
    sail_impl_spec! {
        /// Original path/open-object observation; never replaced by a later stamp.
        pub closed spec fn original_view(&self) -> ObjectStamp { self.original }
        /// Exact accumulated lengths of accepted chunks.
        pub closed spec fn consumed_view(&self) -> usize { self.consumed }
        /// Whether EOF was observed, including a refused premature EOF.
        pub closed spec fn ended_view(&self) -> bool { self.ended }
        /// A refusal is sticky across all later transitions.
        pub closed spec fn failed_view(&self) -> bool { self.failed }
        /// Constructor-preserved arithmetic/file-kind invariant.
        pub closed spec fn well_formed(&self) -> bool {
            self.original.kind == 1 && self.consumed <= self.original.length
        }
    }
    sail_method! {
        /// Begin only when the requested pathname and actual opened handle agree.
        pub fn begin(original: ObjectStamp, opened: &ObjectStamp) -> (read: Option<Self>)
            requires (true)
            ensures (read.is_some() == (original.kind == 1 && original == *opened),
                read.is_some() ==> read.unwrap().well_formed()
                    && read.unwrap().original_view() == original
                    && read.unwrap().consumed_view() == 0 && !read.unwrap().ended_view()
                    && !read.unwrap().failed_view())
        {
            if original.kind != 1 || !same_object_stamp(&original, opened) { return None; }
            Some(OpenedRead { original, consumed: 0, ended: false, failed: false })
        }
    }
    sail_method! {
        /// Return only exact chunk bytes, retaining the same opened object.
        pub fn chunk<'a>(mut self, bytes: &'a [u8]) -> (out: (Self, Option<ReadChunk<'a>>))
            requires (self.well_formed())
            ensures (out.0.well_formed(), out.0.original_view() == self.original_view(),
                out.0.ended_view() == self.ended_view(),
                out.1.is_some() == (!self.failed_view() && !self.ended_view()
                    && bytes.len() > 0 && bytes.len() <= self.original_view().length - self.consumed_view()),
                out.1.is_some() ==> out.0.consumed_view() == self.consumed_view() + bytes.len()
                    && !out.0.failed_view() && out.1.unwrap().bytes_view() == bytes@
                    && out.1.unwrap().owner_view() == (self.original_view().device, self.original_view().object),
                out.1.is_none() ==> out.0.consumed_view() == self.consumed_view() && out.0.failed_view())
        {
            if self.failed || self.ended || bytes.is_empty() || bytes.len() > self.original.length - self.consumed {
                self.failed = true;
                return (self, None);
            }
            self.consumed += bytes.len();
            let chunk = ReadChunk { bytes, device: self.original.device, object: self.original.object };
            (self, Some(chunk))
        }
    }
    sail_method! {
        /// A short read is allowed; only EOF at exactly the original length seals.
        #[must_use]
        pub fn eof(mut self) -> (read: Self)
            requires (self.well_formed())
            ensures (read.well_formed(), read.original_view() == self.original_view(),
                read.consumed_view() == self.consumed_view(), read.ended_view(),
                read.failed_view() == (self.failed_view() || self.ended_view()
                    || self.consumed_view() != self.original_view().length))
        {
            if self.ended || self.consumed != self.original.length { self.failed = true; }
            self.ended = true;
            self
        }
    }
    sail_method! {
        /// Failure cannot be repaired by EOF or newly matching attributes.
        #[must_use]
        pub fn fail(mut self) -> (read: Self)
            requires (self.well_formed())
            ensures (read.well_formed(), read.original_view() == self.original_view(),
                read.consumed_view() == self.consumed_view(), read.ended_view() == self.ended_view(),
                read.failed_view())
        { self.failed = true; self }
    }
    sail_method! {
        /// Final path and handle observations must equal the original stamp.
        pub fn finish(self, handle_after: &ObjectStamp, path_after: &ObjectStamp) -> (seal: Option<ReadSeal>)
            requires (self.well_formed())
            ensures (seal.is_some() == (!self.failed_view() && self.ended_view()
                && self.consumed_view() == self.original_view().length
                && *handle_after == self.original_view() && *path_after == self.original_view()),
                seal.is_some() ==> seal.unwrap().original_view() == self.original_view()
                    && seal.unwrap().consumed_view() == self.original_view().length)
        {
            if self.failed || !self.ended || self.consumed != self.original.length
                || !same_object_stamp(&self.original, handle_after)
                || !same_object_stamp(&self.original, path_after) { return None; }
            Some(ReadSeal { original: self.original, consumed: self.consumed })
        }
    }
}

impl ReadChunk<'_> {
    sail_impl_spec! {
        /// Exact bytes supplied by the actual chunk observation.
        pub closed spec fn bytes_view(&self) -> Seq<u8> { self.bytes@ }
        /// Original opened object to which this chunk was admitted.
        pub closed spec fn owner_view(&self) -> (u64, u64) { (self.device, self.object) }
    }
    sail_method! {
        /// The hasher consumes this exact returned slice, not a copied label.
        pub fn bytes(&self) -> (bytes: &[u8])
            requires (true) ensures (bytes@ == self.bytes_view())
        { self.bytes }
    }
    sail_method! {
        /// Actual observed object identity associated with these bytes.
        pub fn owner(&self) -> (owner: (u64, u64))
            requires (true) ensures (owner == self.owner_view())
        { (self.device, self.object) }
    }
}

impl ReadSeal {
    sail_method! {
        /// Both the proposed cache key and actual requested pathname must still
        /// name the original observed object at their checked observation points.
        pub fn matches_binding(&self, canonical: &ObjectStamp, requested: &ObjectStamp) -> (matches: bool)
            requires (true)
            ensures (matches == (*canonical == self.original_view() && *requested == self.original_view()))
        { self.matches(canonical) && self.matches(requested) }
    }
    sail_method! {
        /// A cache key's actual object observation must match this original read.
        pub fn matches(&self, observed: &ObjectStamp) -> (matches: bool)
            requires (true) ensures (matches == (*observed == self.original_view()))
        { same_object_stamp(&self.original, observed) }
    }
    sail_impl_spec! {
        /// Unmodified original object attributes backing this seal.
        pub closed spec fn original_view(&self) -> ObjectStamp { self.original }
        /// Complete number of accepted bytes.
        pub closed spec fn consumed_view(&self) -> usize { self.consumed }
    }
    sail_method! {
        /// Total bytes accounted through the exact chunk factories.
        pub fn bytes(&self) -> (bytes: usize)
            requires (true) ensures (bytes == self.consumed_view())
        { self.consumed }
    }
    sail_method! {
        /// Original observed opened object, preserved through EOF and validation.
        pub fn owner(&self) -> (owner: (u64, u64))
            requires (true) ensures (owner == (self.original_view().device, self.original_view().object))
        { (self.original.device, self.original.object) }
    }
}

sail_spec! {
    /// Rustc's dep-info spelling for one environment-name byte.
    pub open spec fn encoded_environment_byte(byte: u8) -> Seq<u8> {
        if byte == 10 { seq![92u8, 110u8] }
        else if byte == 13 { seq![92u8, 114u8] }
        else if byte == 92 { seq![92u8, 92u8] }
        else { seq![byte] }
    }

    /// Dep-info escapes only carriage return, newline and backslash in names.
    pub open spec fn encoded_environment_name(name: Seq<u8>) -> Seq<u8>
        decreases name.len()
    {
        if name.len() == 0 { Seq::empty() }
        else { encoded_environment_name(name.drop_last()) + encoded_environment_byte(name.last()) }
    }

    /// Every encoded prefix remains a prefix after encoding the remaining name.
    pub proof fn encoded_environment_prefix(name: Seq<u8>, length: int)
        requires 0 <= length <= name.len()
        ensures encoded_environment_name(name.take(length)).len() <= encoded_environment_name(name).len(),
            encoded_environment_name(name).take(encoded_environment_name(name.take(length)).len() as int)
                == encoded_environment_name(name.take(length))
        decreases name.len() - length
    {
        if length == name.len() {
            assert(name.take(length) == name);
        } else {
            encoded_environment_prefix(name.drop_last(), length);
            assert(name.take(length) == name.drop_last().take(length));
        }
    }
}

sail_verified! {
    /// Check a decoder proposal against the exact emitted dep-info name bytes.
    /// Whitespace is an ordinary name byte and is never trimmed.
    pub fn environment_name_matches(encoded: &[u8], name: &[u8]) -> (matches: bool)
        requires (true)
        ensures (matches == (name.len() > 0
            && (forall|j: int| 0 <= j < name.len() ==> name[j] != 0 && name[j] != 61)
            && encoded@ == encoded_environment_name(name@)))
    {
        if name.is_empty() { return false; }
        let mut i: usize = 0;
        let mut j: usize = 0;
        sail_while! { (j < name.len())
            invariant (i <= encoded.len(), j <= name.len(),
                encoded@.take(i as int) == encoded_environment_name(name@.take(j as int)),
                forall|k: int| 0 <= k < j ==> name[k] != 0 && name[k] != 61)
            decreases (name.len() - j)
            {
                sail_proof! {
                    encoded_environment_prefix(name@, j as int + 1);
                    assert(name@.take(j as int + 1).drop_last() == name@.take(j as int));
                    if encoded@ == encoded_environment_name(name@) {
                        assert(encoded@.take(encoded_environment_name(name@.take(j as int + 1)).len() as int)
                            == encoded_environment_name(name@.take(j as int + 1)));
                    }
                }
                let byte = name[j];
                if byte == 0 || byte == 61 { return false; }
                if byte == 10 || byte == 13 || byte == 92 {
                    sail_proof! {
                        assert(encoded_environment_name(name@.take(j as int)).len() == i);
                        if encoded@ == encoded_environment_name(name@) {
                            assert(encoded_environment_name(name@.take(j as int + 1))
                                == encoded@.take(i as int) + encoded_environment_byte(byte));
                            assert(i as int + 2 <= encoded.len());
                            assert(encoded@.take(i as int + 2) == encoded@.take(i as int) + encoded_environment_byte(byte));
                            assert(encoded@.take(i as int + 2)[i as int] == encoded[i as int]);
                            assert(encoded[i as int] == 92);
                        }
                    }
                    if encoded.len() - i < 2 || encoded[i] != 92 { return false; }
                    let escaped = if byte == 10 { 110 } else if byte == 13 { 114 } else { 92 };
                    sail_proof! {
                        if encoded@ == encoded_environment_name(name@) {
                            assert(encoded_environment_byte(byte) == seq![92u8, escaped]);
                            assert(encoded@.take(i as int + 2) == encoded@.take(i as int) + seq![92u8, escaped]);
                            assert(encoded@.take(i as int + 2)[i as int + 1] == encoded[i as int + 1]);
                            assert(encoded[i as int + 1] == escaped);
                        }
                    }
                    if encoded[i + 1] != escaped { return false; }
                    sail_proof! {
                        assert(name@.take(j as int + 1).drop_last() == name@.take(j as int));
                        assert(encoded@.take(i as int + 2) == encoded@.take(i as int) + seq![92u8, escaped]);
                    }
                    i += 2;
                } else {
                    sail_proof! {
                        assert(encoded_environment_name(name@.take(j as int)).len() == i);
                        if encoded@ == encoded_environment_name(name@) {
                            assert(encoded_environment_name(name@.take(j as int + 1)) == encoded@.take(i as int) + seq![byte]);
                            assert(i < encoded.len());
                            assert(encoded@.take(i as int + 1) == encoded@.take(i as int) + seq![byte]);
                            assert(encoded@.take(i as int + 1)[i as int] == encoded[i as int]);
                            assert(encoded[i as int] == byte);
                        }
                    }
                    if i == encoded.len() || encoded[i] != byte { return false; }
                    sail_proof! {
                        assert(name@.take(j as int + 1).drop_last() == name@.take(j as int));
                        assert(encoded@.take(i as int + 1) == encoded@.take(i as int) + seq![byte]);
                    }
                    i += 1;
                }
                j += 1;
            }
        }
        sail_proof! {
            assert(name@.take(j as int) == name@);
            if i == encoded.len() { assert(encoded@.take(i as int) == encoded@); }
        }
        i == encoded.len()
    }
}

sail_spec! {
    /// Rustc's dep-info spelling for one dependency-name byte.
    pub open spec fn encoded_dependency_byte(byte: u8) -> Seq<u8> {
        if byte == 32 { seq![92u8, 32u8] }
        else { seq![byte] }
    }

    /// Rustc filename encoding escapes only ASCII spaces; every other byte stays literal.
    pub open spec fn encoded_dependency_name(name: Seq<u8>) -> Seq<u8>
        decreases name.len()
    {
        if name.len() == 0 { Seq::empty() }
        else { encoded_dependency_name(name.drop_last()) + encoded_dependency_byte(name.last()) }
    }

    /// Every encoded prefix remains a prefix after encoding the remaining name.
    pub proof fn encoded_dependency_prefix(name: Seq<u8>, length: int)
        requires 0 <= length <= name.len()
        ensures encoded_dependency_name(name.take(length)).len() <= encoded_dependency_name(name).len(),
            encoded_dependency_name(name).take(encoded_dependency_name(name.take(length)).len() as int)
                == encoded_dependency_name(name.take(length))
        decreases name.len() - length
    {
        if length == name.len() {
            assert(name.take(length) == name);
        } else {
            encoded_dependency_prefix(name.drop_last(), length);
            assert(name.take(length) == name.drop_last().take(length));
        }
    }
}

sail_verified! {
    /// Check a decoder proposal against the exact emitted dep-info name bytes.
    /// Tabs, non-ASCII whitespace, hashes, colons and backslashes are literal.
    /// Multiline and NUL paths are outside this supported token representation.
    pub fn dependency_filename_matches(encoded: &[u8], name: &[u8]) -> (matches: bool)
        requires (true)
        ensures (matches == (name.len() > 0
            && (forall|j: int| 0 <= j < name.len() ==> name[j] != 0 && name[j] != 10 && name[j] != 13)
            && encoded@ == encoded_dependency_name(name@)))
    {
        if name.is_empty() { return false; }
        let mut i: usize = 0;
        let mut j: usize = 0;
        sail_while! { (j < name.len())
            invariant (i <= encoded.len(), j <= name.len(),
                encoded@.take(i as int) == encoded_dependency_name(name@.take(j as int)),
                forall|k: int| 0 <= k < j ==> name[k] != 0 && name[k] != 10 && name[k] != 13)
            decreases (name.len() - j)
            {
                sail_proof! {
                    encoded_dependency_prefix(name@, j as int + 1);
                    assert(name@.take(j as int + 1).drop_last() == name@.take(j as int));
                    if encoded@ == encoded_dependency_name(name@) {
                        assert(encoded@.take(encoded_dependency_name(name@.take(j as int + 1)).len() as int)
                            == encoded_dependency_name(name@.take(j as int + 1)));
                    }
                }
                let byte = name[j];
                if byte == 0 || byte == 10 || byte == 13 { return false; }
                if byte == 32 {
                    sail_proof! {
                        assert(encoded_dependency_name(name@.take(j as int)).len() == i);
                        if encoded@ == encoded_dependency_name(name@) {
                            assert(encoded_dependency_name(name@.take(j as int + 1))
                                == encoded@.take(i as int) + encoded_dependency_byte(byte));
                            assert(i as int + 2 <= encoded.len());
                            assert(encoded@.take(i as int + 2) == encoded@.take(i as int) + encoded_dependency_byte(byte));
                            assert(encoded@.take(i as int + 2)[i as int] == encoded[i as int]);
                            assert(encoded[i as int] == 92);
                        }
                    }
                    if encoded.len() - i < 2 || encoded[i] != 92 { return false; }
                    let escaped = 32;
                    sail_proof! {
                        if encoded@ == encoded_dependency_name(name@) {
                            assert(encoded_dependency_byte(byte) == seq![92u8, escaped]);
                            assert(encoded@.take(i as int + 2) == encoded@.take(i as int) + seq![92u8, escaped]);
                            assert(encoded@.take(i as int + 2)[i as int + 1] == encoded[i as int + 1]);
                            assert(encoded[i as int + 1] == escaped);
                        }
                    }
                    if encoded[i + 1] != escaped { return false; }
                    sail_proof! {
                        assert(name@.take(j as int + 1).drop_last() == name@.take(j as int));
                        assert(encoded@.take(i as int + 2) == encoded@.take(i as int) + seq![92u8, escaped]);
                    }
                    i += 2;
                } else {
                    sail_proof! {
                        assert(encoded_dependency_name(name@.take(j as int)).len() == i);
                        if encoded@ == encoded_dependency_name(name@) {
                            assert(encoded_dependency_name(name@.take(j as int + 1)) == encoded@.take(i as int) + seq![byte]);
                            assert(i < encoded.len());
                            assert(encoded@.take(i as int + 1) == encoded@.take(i as int) + seq![byte]);
                            assert(encoded@.take(i as int + 1)[i as int] == encoded[i as int]);
                            assert(encoded[i as int] == byte);
                        }
                    }
                    if i == encoded.len() || encoded[i] != byte { return false; }
                    sail_proof! {
                        assert(name@.take(j as int + 1).drop_last() == name@.take(j as int));
                        assert(encoded@.take(i as int + 1) == encoded@.take(i as int) + seq![byte]);
                    }
                    i += 1;
                }
                j += 1;
            }
        }
        sail_proof! {
            assert(name@.take(j as int) == name@);
            if i == encoded.len() { assert(encoded@.take(i as int) == encoded@); }
        }
        i == encoded.len()
    }
}

sail_spec! {
    /// Optional exact in-memory primitive outcomes used by a constructive
    /// vector provider. The native filesystem provider has no prediction.
    pub struct ScriptedReadFile {
        pub key: Seq<u8>,
        pub stamp: ObjectStamp,
        pub bytes: Seq<u8>,
    }

    pub open spec fn valid_read_script(file: ScriptedReadFile, key: Seq<u8>) -> bool {
        file.key == key && file.stamp.kind == 1 && file.stamp.length == file.bytes.len()
            && file.bytes.len() <= usize::MAX
    }

    pub enum FileObservation {
        Path { key: Seq<u8>, stamp: ObjectStamp },
        Open { key: Seq<u8>, owner: (u64, u64) },
        Handle { owner: (u64, u64), stamp: ObjectStamp },
        Bytes { owner: (u64, u64), bytes: Seq<u8> },
        Eof { owner: (u64, u64) },
        Failure { operation: u8 },
    }
    pub open spec fn positive_owned_read(event: FileObservation, owner: (u64, u64)) -> bool {
        match event { FileObservation::Bytes { owner: actual, bytes } => actual == owner && bytes.len() > 0, _ => false }
    }
}

sail_trait! {
    /// Narrow physical-I/O and hash primitives for the checked read transaction.
    /// Production uses only an explicitly source-bound private native provider;
    /// arbitrary trait implementations supply no filesystem authority.
    pub trait FileReadHost<Handle, Hasher, Digest, Failure> {
        spec {
            spec fn scripted_file(&self) -> Option<ScriptedReadFile>;
            spec fn scripted_handle(handle: &Handle) -> Option<ScriptedReadFile>;
            spec fn key(&self) -> Seq<u8>;
            spec fn observations(&self) -> Seq<FileObservation>;
            spec fn owner(handle: &Handle) -> (u64, u64);
            spec fn observed_bytes(handle: &Handle) -> Seq<u8>;
            spec fn hashed_bytes(hasher: &Hasher) -> Seq<u8>;
            spec fn digest_of(bytes: Seq<u8>) -> Digest;
        }
        /// Observe the actual requested pathname at this operation.
        ///
        /// # Errors
        /// Returns the provider failure when pathname metadata is unavailable.
        fn path_stamp(&mut self) -> (result: Result<ObjectStamp, Failure>)
            requires (true)
            ensures (final(self).key() == old(self).key(),
                final(self).scripted_file() == old(self).scripted_file(),
                final(self).observations() == old(self).observations().push(match result {
                    Ok(stamp) => FileObservation::Path { key: old(self).key(), stamp },
                    Err(_) => FileObservation::Failure { operation: 0 },
                }),
                match old(self).scripted_file() { Some(file) => valid_read_script(file, old(self).key())
                    ==> result == Ok(file.stamp), None => true });
        /// Open the requested path and begin this handle observation history.
        ///
        /// # Errors
        /// Returns the actual provider open failure.
        fn open(&mut self) -> (result: Result<Handle, Failure>)
            requires (true)
            ensures (final(self).key() == old(self).key(),
                final(self).scripted_file() == old(self).scripted_file(),
                final(self).observations() == old(self).observations().push(match result {
                    Ok(handle) => FileObservation::Open { key: old(self).key(), owner: Self::owner(&handle) },
                    Err(_) => FileObservation::Failure { operation: 1 },
                }),
                match result { Ok(handle) => Self::observed_bytes(&handle) == Seq::<u8>::empty(), Err(_) => true },
                match old(self).scripted_file() { Some(file) => valid_read_script(file, old(self).key())
                    ==> match result { Ok(handle) => Self::scripted_handle(&handle) == Some(file), Err(_) => false }, None => true });
        /// Observe metadata of this exact owned handle, preserving its owner.
        ///
        /// # Errors
        /// Returns the provider failure when handle metadata is unavailable.
        fn handle_stamp(&mut self, handle: &Handle) -> (result: Result<ObjectStamp, Failure>)
            requires (true)
            ensures (final(self).key() == old(self).key(),
                final(self).scripted_file() == old(self).scripted_file(),
                final(self).observations() == old(self).observations().push(match result {
                    Ok(stamp) => FileObservation::Handle { owner: Self::owner(handle), stamp },
                    Err(_) => FileObservation::Failure { operation: 2 },
                }),
                match result { Ok(stamp) => (stamp.device, stamp.object) == Self::owner(handle), Err(_) => true },
                match old(self).scripted_file() { Some(file) => valid_read_script(file, old(self).key()) && Self::scripted_handle(handle) == Some(file)
                    ==> result == Ok(file.stamp), None => true });
        /// Return the actual observed buffer prefix from this same handle.
        ///
        /// # Errors
        /// Returns the actual read failure without authorizing a completed read.
        fn read(&mut self, handle: &mut Handle, buffer: &mut [u8]) -> (result: Result<usize, Failure>)
            requires (old(buffer).len() > 0)
            ensures (final(self).key() == old(self).key(),
                final(self).scripted_file() == old(self).scripted_file(),
                final(self).observations() == old(self).observations().push(match result {
                    Ok(count) => if count == 0 { FileObservation::Eof { owner: Self::owner(old(handle)) } }
                        else { FileObservation::Bytes { owner: Self::owner(old(handle)), bytes: final(buffer)@.take(count as int) } },
                    Err(_) => FileObservation::Failure { operation: 3 },
                }),
                final(buffer).len() == old(buffer).len(),
                match result { Ok(count) => count <= final(buffer).len()
                && Self::owner(final(handle)) == Self::owner(old(handle))
                && Self::observed_bytes(final(handle)) == Self::observed_bytes(old(handle))
                    + final(buffer)@.take(count as int), Err(_) => true },
                match old(self).scripted_file() { Some(file) => valid_read_script(file, old(self).key())
                    && Self::scripted_handle(old(handle)) == Some(file) && old(buffer).len() > 0
                    && Self::observed_bytes(old(handle)).len() <= file.bytes.len()
                    && Self::observed_bytes(old(handle)) == file.bytes.take(Self::observed_bytes(old(handle)).len() as int)
                    ==> match result { Ok(count) => Self::scripted_handle(final(handle)) == Some(file)
                        && count <= file.bytes.len() - Self::observed_bytes(old(handle)).len()
                        && (count == 0) == (Self::observed_bytes(old(handle)).len() == file.bytes.len())
                        && final(buffer)@.take(count as int) == file.bytes.subrange(Self::observed_bytes(old(handle)).len() as int,
                            Self::observed_bytes(old(handle)).len() as int + count as int), Err(_) => false }, None => true });
        /// Begin one empty hash stream.
        fn hash_start(&mut self) -> (result: Hasher)
            requires (true) ensures (Self::hashed_bytes(&result) == Seq::<u8>::empty(),
                final(self).key() == old(self).key(),
                final(self).scripted_file() == old(self).scripted_file(), final(self).observations() == old(self).observations());
        /// Feed precisely the supplied bytes to the same hash state.
        fn hash_update(&mut self, hasher: &mut Hasher, bytes: &[u8]) -> (result: ())
            requires (Self::hashed_bytes(old(hasher)).len() + bytes.len() <= usize::MAX)
            ensures (Self::hashed_bytes(final(hasher)) == Self::hashed_bytes(old(hasher)) + bytes@,
                final(self).key() == old(self).key(),
                final(self).scripted_file() == old(self).scripted_file(), final(self).observations() == old(self).observations());
        /// Finalize the forward hash function of the supplied byte stream.
        fn hash_finish(&mut self, hasher: Hasher) -> (result: Digest)
            requires (true) ensures (result == Self::digest_of(Self::hashed_bytes(&hasher)),
                final(self).key() == old(self).key(),
                final(self).scripted_file() == old(self).scripted_file(), final(self).observations() == old(self).observations());
        /// Construct the ordinary conservative observation error.
        fn changed(&self) -> (result: Failure)
            requires (true) ensures (true);
    }
}

sail_code! {
    /// A checked read/hash transaction under its source-bound primitive contract.
    pub struct CompletedRead<Handle, Digest> {
        handle: Handle,
        digest: Digest,
        seal: ReadSeal,
    }
}
impl<Handle, Digest> CompletedRead<Handle, Digest> {
    sail_impl_spec! {
        /// The retained owned handle and its observed transaction history.
        pub closed spec fn handle_view(&self) -> Handle { self.handle }
        /// The exact hash result returned by the primitive provider.
        pub closed spec fn digest_view(&self) -> Digest { self.digest }
        /// The immutable original observation seal.
        pub closed spec fn seal_view(&self) -> ReadSeal { self.seal }
    }
    sail_method! {
        /// Extract the digest and original seal, dropping the owned handle now.
        pub fn into_parts(self) -> (result: (Digest, ReadSeal))
            requires (true)
            ensures (result.0 == self.digest_view(), result.1 == self.seal_view())
        {
            let Self { handle: _handle, digest, seal } = self;
            (digest, seal)
        }
    }
}

sail_verified! {
    /// Read, hash and seal one adopted owned handle using exact primitive results.
    ///
    /// # Errors
    /// Preserves provider failures and refuses empty buffers, incomplete reads
    /// or metadata observations that disagree with the original object.
    pub fn read_opened_digest<Host, Handle, Hasher, Digest, Failure>(host: &mut Host,
        input: Handle, original: ObjectStamp, buffer: &mut [u8]) -> (result: Result<CompletedRead<Handle, Digest>, Failure>)
        where (Host: FileReadHost<Handle, Hasher, Digest, Failure>)
        requires (Host::observed_bytes(&input) == Seq::<u8>::empty())
        ensures (final(buffer).len() == old(buffer).len(), match result { Ok(done) => original.kind == 1 && done.seal_view().original_view() == original
            && done.seal_view().consumed_view() == original.length
            && Host::observed_bytes(&done.handle_view()).len() == original.length
            && done.digest_view() == Host::digest_of(Host::observed_bytes(&done.handle_view()))
            && Host::owner(&done.handle_view()) == (original.device, original.object)
            && Host::owner(&done.handle_view()) == Host::owner(&input)
            && final(host).key() == old(host).key()
            && final(host).observations().len() >= old(host).observations().len() + 4
            && final(host).observations().take(old(host).observations().len() as int) == old(host).observations()
            && final(host).observations()[old(host).observations().len() as int] == (FileObservation::Handle { owner: (original.device, original.object), stamp: original })
            && final(host).observations()[final(host).observations().len() as int - 3] == (FileObservation::Eof { owner: (original.device, original.object) })
            && final(host).observations()[final(host).observations().len() as int - 2] == (FileObservation::Handle { owner: (original.device, original.object), stamp: original })
            && final(host).observations().last() == (FileObservation::Path { key: old(host).key(), stamp: original })
            && forall|i: int| old(host).observations().len() + 1 <= i < final(host).observations().len() - 3
                ==> positive_owned_read(final(host).observations()[i], (original.device, original.object)), Err(_) => true },
            match old(host).scripted_file() { Some(file) => valid_read_script(file, old(host).key())
                && Host::scripted_handle(&input) == Some(file) && original == file.stamp && old(buffer).len() > 0
                ==> match result { Ok(done) => Host::observed_bytes(&done.handle_view()) == file.bytes,
                    Err(_) => false }, None => true })
    {
        let mut handle = input;
        if buffer.is_empty() { return Err(host.changed()); }
        let opened = host.handle_stamp(&handle)?;
        let Some(mut read) = OpenedRead::begin(original, &opened) else { return Err(host.changed()); };
        let mut hasher = host.hash_start();
        let mut eof = false;
        sail_while! { (!eof)
            invariant (buffer.len() > 0, buffer.len() == old(buffer).len(),
                host.scripted_file() == old(host).scripted_file(),
                match old(host).scripted_file() { Some(file) => valid_read_script(file, old(host).key())
                    && Host::scripted_handle(&input) == Some(file) && original == file.stamp
                    ==> Host::scripted_handle(&handle) == Some(file) && !read.failed_view()
                        && Host::observed_bytes(&handle) == file.bytes.take(read.consumed_view() as int)
                        && (eof ==> read.consumed_view() == file.bytes.len()), None => true },
                host.key() == old(host).key(),
                host.observations().len() >= old(host).observations().len() + 1 + if eof { 1int } else { 0int },
                host.observations().take(old(host).observations().len() as int) == old(host).observations(),
                host.observations()[old(host).observations().len() as int] == (FileObservation::Handle { owner: (original.device, original.object), stamp: original }),
                eof ==> host.observations().last() == (FileObservation::Eof { owner: (original.device, original.object) }),
                forall|i: int| old(host).observations().len() + 1 <= i < host.observations().len() - if eof { 1int } else { 0int }
                    ==> positive_owned_read(host.observations()[i], (original.device, original.object)),
                Host::owner(&handle) == (original.device, original.object),
                Host::owner(&handle) == Host::owner(&input),
                read.well_formed(), read.original_view() == original,
                Host::observed_bytes(&handle) == Host::hashed_bytes(&hasher),
                Host::observed_bytes(&handle).len() == read.consumed_view(),
                eof == read.ended_view(), !eof ==> !read.failed_view())
            decreases (original.length as int - read.consumed_view() as int, if eof { 0int } else { 1int })
            {
                let count = match host.read(&mut handle, buffer) {
                    Ok(count) => count,
                    Err(error) => { let _ = read.fail(); return Err(error); }
                };
                if count == 0 {
                    read = read.eof();
                    eof = true;
                } else {
                    let (next, chunk) = read.chunk(&buffer[..count]);
                    sail_proof! {
                        if chunk.is_some() {
                            assert(next.consumed_view() == read.consumed_view() + count);
                            assert(next.consumed_view() > read.consumed_view());
                        }
                    }
                    read = next;
                    let Some(chunk) = chunk else { return Err(host.changed()); };
                    host.hash_update(&mut hasher, chunk.bytes());
                }
            }
        }
        let handle_after = host.handle_stamp(&handle)?;
        let path_after = host.path_stamp()?;
        let Some(seal) = read.finish(&handle_after, &path_after) else { return Err(host.changed()); };
        let digest = host.hash_finish(hasher);
        Ok(CompletedRead { handle, digest, seal })
    }
}

sail_verified! {
    /// Observe, open, read and seal the requested path through one owned object.
    ///
    /// # Errors
    /// Preserves path/open/read failures and refuses incomplete or changed
    /// observations without returning a digest seal.
    pub fn open_read_digest<Host, Handle, Hasher, Digest, Failure>(host: &mut Host,
        buffer: &mut [u8]) -> (result: Result<CompletedRead<Handle, Digest>, Failure>)
        where (Host: FileReadHost<Handle, Hasher, Digest, Failure>)
        requires (true)
        ensures (final(buffer).len() == old(buffer).len(), match result { Ok(done) => done.seal_view().original_view().kind == 1
            && done.seal_view().consumed_view() == done.seal_view().original_view().length
            && Host::observed_bytes(&done.handle_view()).len() == done.seal_view().original_view().length
            && done.digest_view() == Host::digest_of(Host::observed_bytes(&done.handle_view()))
            && Host::owner(&done.handle_view()) == (done.seal_view().original_view().device, done.seal_view().original_view().object)
            && final(host).key() == old(host).key()
            && final(host).observations().len() >= old(host).observations().len() + 6
            && final(host).observations().take(old(host).observations().len() as int) == old(host).observations()
            && final(host).observations()[old(host).observations().len() as int] == (FileObservation::Path { key: old(host).key(), stamp: done.seal_view().original_view() })
            && final(host).observations()[old(host).observations().len() as int + 1] == (FileObservation::Open { key: old(host).key(), owner: Host::owner(&done.handle_view()) })
            && final(host).observations()[old(host).observations().len() as int + 2] == (FileObservation::Handle { owner: Host::owner(&done.handle_view()), stamp: done.seal_view().original_view() })
            && final(host).observations()[final(host).observations().len() as int - 3] == (FileObservation::Eof { owner: Host::owner(&done.handle_view()) })
            && final(host).observations()[final(host).observations().len() as int - 2] == (FileObservation::Handle { owner: Host::owner(&done.handle_view()), stamp: done.seal_view().original_view() })
            && final(host).observations().last() == (FileObservation::Path { key: old(host).key(), stamp: done.seal_view().original_view() })
            && forall|i: int| old(host).observations().len() + 3 <= i < final(host).observations().len() - 3
                ==> positive_owned_read(final(host).observations()[i], Host::owner(&done.handle_view())), Err(_) => true },
            match old(host).scripted_file() { Some(file) => valid_read_script(file, old(host).key()) && old(buffer).len() > 0
                ==> match result { Ok(done) => Host::observed_bytes(&done.handle_view()) == file.bytes,
                    Err(_) => false }, None => true })
    {
        let original = host.path_stamp()?;
        let handle = host.open()?;
        sail_proof! {
            assert(host.observations().len() == old(host).observations().len() + 2);
            assert(host.observations() == old(host).observations()
                .push(FileObservation::Path { key: old(host).key(), stamp: original })
                .push(FileObservation::Open { key: old(host).key(), owner: Host::owner(&handle) }));
        }
        let result = read_opened_digest::<Host, Handle, Hasher, Digest, Failure>(host, handle, original, buffer);
        sail_proof! {
            if let Ok(done) = &result {
                assert(done.seal_view().original_view() == original);
                assert(host.observations().take(old(host).observations().len() as int + 2) == old(host).observations()
                    .push(FileObservation::Path { key: old(host).key(), stamp: original })
                    .push(FileObservation::Open { key: old(host).key(), owner: Host::owner(&done.handle_view()) }));
                assert(host.observations().take(old(host).observations().len() as int + 2)[old(host).observations().len() as int]
                    == host.observations()[old(host).observations().len() as int]);
                assert(host.observations().take(old(host).observations().len() as int + 2)[old(host).observations().len() as int + 1]
                    == host.observations()[old(host).observations().len() as int + 1]);
                assert(host.observations()[old(host).observations().len() as int] == (FileObservation::Path { key: old(host).key(), stamp: original }));
                assert(host.observations()[old(host).observations().len() as int + 1] == (FileObservation::Open { key: old(host).key(), owner: Host::owner(&done.handle_view()) }));
                assert forall|i: int| old(host).observations().len() + 3 <= i < host.observations().len() - 3
                    implies positive_owned_read(host.observations()[i], Host::owner(&done.handle_view())) by {
                    assert(positive_owned_read(host.observations()[i], (original.device, original.object)));
                }
            }
        }
        result
    }
}

sail_trait! {
    /// Only native ownership identity comparison, not a process success oracle.
    /// Production uses a thin `Arc::ptr_eq` adapter, never a compiler-domain premise.
    pub trait OwnerIdentity<Owner> {
        spec { spec fn identity(owner: &Owner) -> int; }
        /// Compare exactly these two opaque ownership leases.
        fn same_owner(&self, original: &Owner, actual: &Owner) -> (same: bool)
            requires (true)
            ensures (same == (Self::identity(original) == Self::identity(actual)));
    }
}
