//! Actual original-target set operations for pre-execution request construction.
//!
//! These functions relate the real owned in-memory dictionary to its request
//! rows. They do not establish Cargo metadata or physical compiler-input closure.

use std::collections::{BTreeSet, btree_set::Iter};

use crate::acceptance::{
    RequestConstruction, construct_request_target, construct_resolved_request,
};
use crate::project::{EngineIdentity, RequestTarget, ResolvedRequest};
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

    /// Borrowed pre-execution header used with the actual original dictionary.
    /// This is construction data, not a second request or an authority token;
    /// only `ResolvedRequest` records the resulting request.
    pub struct OriginalRequestHeader<'a> {
        /// Run allocated before execution.
        pub run_id: &'a str,
        /// Original invocation rendering; its native byte binding is separate.
        pub invocation: &'a str,
        /// Original workspace rendering; its native byte binding is separate.
        pub workspace: &'a str,
        /// Independently resolved execution context.
        pub context: &'a str,
        /// Independently observed declared input revision.
        pub revision: &'a str,
        /// Actual managed arguments, before interpreting any verifier result.
        pub arguments: &'a [String],
        /// Original selected engine identity.
        pub engine: Option<EngineIdentity>,
        /// Original requested repair function, if any.
        pub function: Option<&'a str>,
        /// Original repair package or the empty non-repair value.
        pub repair_package: &'a str,
        /// Original repair binary, if any.
        pub repair_target: Option<&'a str>,
        /// Original repair kind or the empty non-repair value.
        pub repair_kind: &'a str,
    }
}

sail_spec! {
    pub open spec fn original_header_matches(header: &OriginalRequestHeader, request: &ResolvedRequest) -> bool {
        crate::acceptance::request_construction_matches(&RequestConstruction {
            run_id: header.run_id, invocation: header.invocation, workspace: header.workspace,
            context: header.context, revision: header.revision, arguments: header.arguments,
            engine: header.engine, function: header.function, repair_package: header.repair_package,
            repair_target: header.repair_target, repair_kind: header.repair_kind,
            targets: request.targets,
        }, request)
    }

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
        broadcast use vstd::laws_cmp::lemma_ref_obeys_cmp_spec;
        broadcast use vstd::std_specs::btree::axiom_increasing_seq_meaning;
        assert(vstd::laws_cmp::obeys_cmp::<&OriginalTargetKey>());
        assert forall |i: int, j: int| 0 <= i < j < keys.len() implies
            crate::request_order::key_order(&*keys[i], &*keys[j]) == std::cmp::Ordering::Less by {
            assert(keys[i].cmp_spec(&keys[j]) == std::cmp::Ordering::Less);
        }
    }

    /// Frames the real row-vector push through its exact key projection.
    pub proof fn projected_key_push(targets: Seq<RequestTarget>, target: RequestTarget)
        ensures projected_keys(targets.push(target)) == projected_keys(targets).push(
            crate::request_order::original_key_value((target.package_id, target.target, target.kind))),
    {
        assert(projected_keys(targets.push(target)) =~= projected_keys(targets).push(
            crate::request_order::original_key_value((target.package_id, target.target, target.kind))));
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
            (*final(iterator)).remaining() == if old(iterator).remaining().len() > 0 {
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
    pub fn project_original_targets(source: &OriginalTargets) -> (projection: Option<Vec<RequestTarget>>)
        requires(true)
        ensures(projection.is_some(), projection.is_some() ==> {
            let targets = projection.unwrap();
            targets.len() == source@.len() && projected_keys(targets@).to_set() == source@
                && projected_keys(targets@).no_duplicates() && ordered_keys(projected_keys(targets@))
        })
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
                    None => return None,
                };
                sail_proof! { projected_key_push(targets@, target); }
                targets.push(target);
            }
        }
        sail_proof! { assert(iterator.remaining().len() == 0); }
        Some(targets)
    }
}

sail_verified! {
    /// Constructs one request from its original borrowed header and the actual
    /// immutable dictionary. No caller supplies a target row list or success bit.
    /// Metadata enumeration, physical path identity and launch closure are separate.
    pub fn construct_original_request(header: OriginalRequestHeader<'_>, source: &OriginalTargets)
        -> (request: Option<ResolvedRequest>)
        requires(true)
        ensures(request.is_some(), request.is_some() ==> {
            let resolved = request.unwrap();
            original_header_matches(&header, &resolved)
                && resolved.args@ == header.arguments@
                && resolved.targets.len() == source@.len()
                && projected_keys(resolved.targets@).to_set() == source@
                && projected_keys(resolved.targets@).no_duplicates()
                && ordered_keys(projected_keys(resolved.targets@))
        })
    {
        let targets = match project_original_targets(source) {
            Some(targets) => targets,
            None => return None,
        };
        Some(construct_resolved_request(RequestConstruction {
            run_id: header.run_id,
            invocation: header.invocation,
            workspace: header.workspace,
            context: header.context,
            revision: header.revision,
            arguments: header.arguments,
            engine: header.engine,
            function: header.function,
            repair_package: header.repair_package,
            repair_target: header.repair_target,
            repair_kind: header.repair_kind,
            targets,
        }))
    }
}
