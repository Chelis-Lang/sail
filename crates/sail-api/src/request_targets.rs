//! Actual original-target set operations for pre-execution request construction.
//!
//! These functions relate the real owned in-memory dictionary to its request
//! rows. They do not establish Cargo metadata or physical compiler-input closure.

use std::collections::{BTreeSet, btree_set::Iter};

use crate::acceptance::construct_request_target;
use crate::project::RequestTarget;
use crate::request_order::OriginalTargetKey;
#[cfg(sail_verus)]
use crate::request_order::original_target_key_order_is_lawful;
use sail_acceptance::{sail_code, sail_proof, sail_spec, sail_verified, sail_while};

#[cfg(sail_verus)]
use vstd::prelude::*;
#[cfg(sail_verus)]
use vstd::std_specs::cmp::OrdSpec;
#[cfg(sail_verus)]
use vstd::std_specs::iter::IteratorSpec;

sail_code! {
    /// The actual pre-execution target dictionary; keys retain original Strings.
    /// It is an in-memory record domain, not a physical compiler-input token.
    pub type OriginalTargets = BTreeSet<OriginalTargetKey>;
}

sail_spec! {
    pub open spec fn projected_keys(targets: Seq<RequestTarget>) -> Seq<OriginalTargetKey> {
        targets.map_values(|target: RequestTarget| crate::request_order::original_key_value(
            (target.package_id, target.target, target.kind)))
    }

    pub open spec fn ordered_keys(keys: Seq<OriginalTargetKey>) -> bool {
        forall |i: int, j: int| 0 <= i < j < keys.len() ==>
            crate::request_order::key_order(&keys[i], &keys[j]) == std::cmp::Ordering::Less
    }

    pub proof fn join_original_head(prefix: Seq<OriginalTargetKey>, remaining: Seq<&OriginalTargetKey>)
        requires remaining.len() > 0,
        ensures prefix.push(*remaining[0]) + remaining.drop_first().unref()
            == prefix + remaining.unref(),
    {
        assert(prefix.push(*remaining[0]) + remaining.drop_first().unref()
            =~= prefix + remaining.unref());
    }

    pub proof fn original_iterator_order(keys: Seq<&OriginalTargetKey>)
        requires vstd::std_specs::btree::increasing_seq(keys),
        ensures ordered_keys(keys.unref()),
    {
        original_target_key_order_is_lawful();
        assert(vstd::laws_cmp::obeys_cmp::<&OriginalTargetKey>());
        assert forall |i: int, j: int| 0 <= i < j < keys.len() implies
            crate::request_order::key_order(&*keys[i], &*keys[j]) == std::cmp::Ordering::Less by {
            assert(keys[i].cmp_spec(&keys[j]) == std::cmp::Ordering::Less);
        }
    }
}

sail_verified! {
    /// Inserts the actual selected metadata triple under the checked original
    /// byte order. No count or caller completeness flag constructs the domain.
    pub fn insert_original_target(targets: &mut OriginalTargets, original: (String, String, String))
        -> (inserted: bool)
        requires(true)
        ensures(final(targets)@ == old(targets)@.insert(crate::request_order::original_key_value(original)),
            inserted == !old(targets)@.contains(crate::request_order::original_key_value(original)))
    {
        sail_proof! { original_target_key_order_is_lawful(); }
        targets.insert(OriginalTargetKey::new(original))
    }
}

sail_verified! {
    /// Decodes only the actual standard iterator outcome and copies that
    /// returned entry. No caller supplies a success or completeness token.
    fn next_original_target(iterator: &mut Iter<'_, OriginalTargetKey>) -> (target: Option<RequestTarget>)
        requires(true)
        ensures(target.is_some() == (old(iterator).remaining().len() > 0),
            final(iterator).remaining() == if old(iterator).remaining().len() > 0 {
                old(iterator).remaining().drop_first()
            } else { old(iterator).remaining() },
            target.is_some() ==> crate::request_order::original_key_value((target.unwrap().package_id,
                target.unwrap().target, target.unwrap().kind)) == *old(iterator).remaining()[0])
    {
        match iterator.next() {
            Some(original) => Some(construct_request_target(original.as_tuple())),
            None => None,
        }
    }
}

sail_verified! {
    /// Projects every entry of the actual immutable original dictionary once,
    /// in its native order. This proves no upstream Cargo-selection completeness.
    pub fn project_original_targets(source: &OriginalTargets) -> (targets: Vec<RequestTarget>)
        requires(true)
        ensures(targets.len() == source@.len(), projected_keys(targets@).to_set() == source@,
            projected_keys(targets@).no_duplicates(), ordered_keys(projected_keys(targets@)))
    {
        sail_proof! { original_target_key_order_is_lawful(); }
        let total = source.len();
        let mut iterator = source.iter();
        sail_proof! { original_iterator_order(iterator.remaining()); }
        let mut targets: Vec<RequestTarget> = Vec::with_capacity(total);
        sail_while! { (targets.len() < total)
            invariant(targets.len() <= total,
                iterator.remaining().len() + targets.len() == total,
                (projected_keys(targets@) + iterator.remaining().unref()).to_set() == source@,
                (projected_keys(targets@) + iterator.remaining().unref()).no_duplicates(),
                ordered_keys(projected_keys(targets@) + iterator.remaining().unref()))
            decreases(total - targets.len())
            {
                sail_proof! { join_original_head(projected_keys(targets@), iterator.remaining()); }
                let target = match next_original_target(&mut iterator) {
                    Some(target) => target,
                    None => return targets,
                };
                targets.push(target);
            }
        }
        sail_proof! { assert(iterator.remaining().len() == 0); }
        targets
    }
}

#[cfg(test)]
mod tests {
    use super::{OriginalTargets, insert_original_target, project_original_targets};
    use std::collections::BTreeSet;

    #[test]
    fn actual_dictionary_projection_retains_old_order_scope_and_duplicates() {
        let original: Vec<(String, String, String)> = vec![
            ("package-b".into(), "sibling".into(), "bin".into()),
            ("package-a".into(), "original".into(), "lib".into()),
            ("package-a".into(), "original".into(), "bin".into()),
            ("package-a".into(), "original".into(), "lib".into()),
            ("λ".into(), "target\0".into(), "lib".into()),
        ];
        let mut source = OriginalTargets::new();
        for original in &original {
            insert_original_target(&mut source, original.clone());
        }
        let projected = project_original_targets(&source);
        let actual: Vec<_> = projected
            .into_iter()
            .map(|target| (target.package_id, target.target, target.kind))
            .collect();
        let expected: BTreeSet<_> = original.into_iter().collect();
        assert_eq!(actual, expected.into_iter().collect::<Vec<_>>());
        assert!(project_original_targets(&OriginalTargets::new()).is_empty());
    }
}
