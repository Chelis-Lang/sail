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

sail_spec! {
    /// Every requested selected fact is an exact readable fact of the source.
    pub open spec fn covered_selection(source: Seq<InputFact>, selected: Seq<InputFact>) -> bool {
        selected.len() > 0 && forall|i: int| 0 <= i < selected.len()
            ==> contains_fact(source, #[trigger] selected[i])
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
        ensures (covered ==> covered_selection(source@, selected@),
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
                forall|k: int| 0 <= k < i ==> contains_fact(source@, #[trigger] selected[selected_order[k] as int]))
            decreases (selected.len() - i)
            {
                sail_proof! { assert(permutation_at(selected_order@, selected_inverse@, selected.len() as nat, i as int)); }
                let wanted = &selected[selected_order[i]];
                sail_while! { (j < source.len())
                    invariant (j <= source.len(), i < selected.len(),
                        ordered_facts(source@, source_order@, source_inverse@))
                    decreases (source.len() - j)
                    {
                        sail_proof! { assert(permutation_at(source_order@, source_inverse@, source.len() as nat, j as int)); }
                        if !input_key_lt(&source[source_order[j]], wanted) { break; }
                        j += 1;
                    }
                }
                if j >= source.len() { return false; }
                sail_proof! { assert(permutation_at(source_order@, source_inverse@, source.len() as nat, j as int)); }
                let original = &source[source_order[j]];
                if !input_key_eq(original, wanted) || value_difference(&original.value, &wanted.value) != 0 {
                    return false;
                }
                sail_proof! { assert(contains_fact(source@, *wanted)); }
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

sail_verified! {
    /// Certify every declared file was actually represented by a successful read.
    /// A missing, unreadable, absent or duplicate row cannot establish coverage.
    pub fn reads_complete(required: &[InputFact<'_>], observed: &[InputFact<'_>],
        required_order: &[usize], required_inverse: &[usize],
        observed_order: &[usize], observed_inverse: &[usize]) -> (complete: bool)
        requires (true)
        ensures (complete ==> complete_present_inventory(required@, observed@),
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
                forall|j: int| 0 <= j < i ==> contains_present(observed@, #[trigger] required[required_order[j] as int]))
            decreases (required.len() - i)
            {
                sail_proof! {
                    assert(permutation_at(required_order@, required_inverse@, required.len() as nat, i as int));
                    assert(permutation_at(observed_order@, observed_inverse@, observed.len() as nat, i as int));
                }
                let wanted = &required[required_order[i]];
                let actual = &observed[observed_order[i]];
                if !input_key_eq(wanted, actual) || !matches!(actual.value, InputValue::Present(_)) { return false; }
                sail_proof! { assert(contains_present(observed@, *wanted)); }
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
