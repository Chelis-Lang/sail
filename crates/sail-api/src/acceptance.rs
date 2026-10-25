//! Positive Verus authority checked directly over the public report records.
//!
//! These predicates never infer a complete request from its selected-result
//! flags. The caller with the independently resolved request must additionally
//! establish [`requested_coverage`]. Currentness remains an input property:
//! a fresh failed verification can be Current without authorizing a repair.
#![allow(
    clippy::redundant_as_str,
    reason = "The pinned stock Verus specifies str::as_bytes, not String::as_bytes."
)]

use sail_acceptance::authority::{
    CompletionPolicy, RequestedTarget, argument_flag, copy_bytes, exact_bytes, owned_obligation,
    unique_obligations,
};
use sail_acceptance::{sail_code, sail_proof, sail_spec, sail_verified, sail_while};

/// Builds untrusted order candidates and invokes the checked authority factory.
pub use crate::acceptance_sort::{
    complete_report, complete_selection, complete_target, complete_whole_report,
};

use crate::output::{
    CrateReport, EngineEstablishment, EstablishmentName, FunctionReport, Report, SourceSummaryName,
    VerdictName,
};
use crate::project::{Currentness, ProjectResult, TargetResult};

#[cfg(sail_verus)]
use vstd::prelude::*;
#[cfg(sail_verus)]
use vstd::string::StringSliceAdditionalSpecFns;

sail_code! {
    /// Checked immutable report authority, bound to the actual report borrow.
    /// Its fields are private so deserialization and caller flags cannot mint it.
    pub struct CompleteReport<'a> {
        report: &'a Report,
        policy: CompletionPolicy,
    }

    /// Untrusted candidate ordering for one report's actual obligations.
    pub struct ReportOrder {
        /// Proposed sorted actual-ID indices.
        pub forward: Vec<usize>,
        /// Proposed inverse index certificate.
        pub inverse: Vec<usize>,
    }

    /// Positive authority for the selected targets supplied in this envelope.
    /// This does not invent an independently resolved request after storage.
    pub struct CompleteSelection<'a> {
        project: &'a ProjectResult,
    }

    impl CompleteSelection<'_> {
        /// Exact immutable project whose selected reports were checked.
        #[must_use]
        pub fn project(&self) -> &ProjectResult { self.project }
    }

    impl CompleteReport<'_> {
        /// The immutable actual report whose fields were checked.
        #[must_use]
        pub fn report(&self) -> &Report { self.report }

        /// The receipt or repair contract that was checked.
        #[must_use]
        pub fn policy(&self) -> CompletionPolicy { self.policy }
    }
}

sail_spec! {
    broadcast use {vstd::seq_lib::group_seq_properties, vstd::seq::Seq::lemma_push_map_commute};
    pub open spec fn string_bytes(s: &String) -> Seq<u8> { vstd::utf8::encode_utf8(s@) }

    pub open spec fn focused_argument(bytes: Seq<u8>) -> bool {
        sail_acceptance::authority::flag_matches(bytes, b"--verify-module"@)
            || sail_acceptance::authority::flag_matches(bytes, b"--verify-only-module"@)
            || sail_acceptance::authority::flag_matches(bytes, b"--verify-function"@)
            || sail_acceptance::authority::flag_matches(bytes, b"--verify-root"@)
    }

    pub open spec fn partial_command(args: Seq<String>) -> bool {
        exists |i: int, j: int| 0 <= i <= j < args.len()
            && string_bytes(&args[i]) == b"--"@ && focused_argument(string_bytes(&args[j]))
    }

    pub open spec fn target_match(e: &RequestedTarget, t: &TargetResult) -> bool {
        e.package@ == string_bytes(&t.package_id) && e.name@ == string_bytes(&t.target)
            && exists |i: int| 0 <= i < t.kind@.len() && e.kind@ == string_bytes(&t.kind@[i])
    }

    pub open spec fn request_covered(e: &RequestedTarget, ts: Seq<TargetResult>) -> bool {
        exists |i: int| 0 <= i < ts.len() && target_match(e, &ts[i]) && ts[i].requested
            && forall |j: int| 0 <= j < ts.len() && target_match(e, &ts[j]) ==> j == i
    }

    pub open spec fn target_covered(t: &TargetResult, es: Seq<RequestedTarget>) -> bool {
        exists |i: int| 0 <= i < es.len() && target_match(&es[i], t)
            && forall |j: int| 0 <= j < es.len() && target_match(&es[j], t) ==> j == i
    }

    pub open spec fn exact_requested_coverage(es: Seq<RequestedTarget>, ts: Seq<TargetResult>) -> bool {
        es.len() > 0
            && (forall |i: int| 0 <= i < es.len() ==> request_covered(&es[i], ts))
            && (forall |i: int| 0 <= i < ts.len() && ts[i].requested ==> target_covered(&ts[i], es))
    }

    pub open spec fn same_target_identity(a: &TargetResult, b: &TargetResult) -> bool {
        string_bytes(&a.package_id) == string_bytes(&b.package_id)
            && string_bytes(&a.target) == string_bytes(&b.target)
            && a.kind@.len() == b.kind@.len()
            && (forall |i: int| 0 <= i < a.kind@.len() ==> string_bytes(&a.kind@[i]) == string_bytes(&b.kind@[i]))
    }

    pub open spec fn supplied_selection_authorized(p: &ProjectResult) -> bool {
        p.currentness is Current && p.exit_code == Some(0i32) && p.error.is_none()
            && (exists |i: int| 0 <= i < p.targets@.len() && p.targets@[i].requested)
            && (forall |i: int| 0 <= i < p.targets@.len() && p.targets@[i].requested ==>
                p.targets@[i].complete && p.targets@[i].scope_reuse.is_none()
                    && !partial_command(p.targets@[i].report.command@)
                    && successful_report(&p.targets@[i].report, CompletionPolicy::Repair))
            && (forall |i: int, j: int| 0 <= i < j < p.targets@.len()
                && p.targets@[i].requested && p.targets@[j].requested ==>
                    !same_target_identity(&p.targets@[i], &p.targets@[j]))
    }

    pub closed spec fn selection_project<'a>(a: &CompleteSelection<'a>) -> &'a ProjectResult { a.project }
    pub closed spec fn selection_authority_valid(a: &CompleteSelection) -> bool {
        supplied_selection_authorized(a.project)
    }

    pub open spec fn provenance_valid(f: &FunctionReport) -> bool {
        let p = &f.provenance;
        match p.source {
            SourceSummaryName::Fresh => p.checks_solved > 0 && p.checks_served == 0 && p.checks_unrecorded == 0,
            SourceSummaryName::Cache => p.checks_solved == 0 && p.checks_served > 0 && p.checks_unrecorded == 0,
            SourceSummaryName::Mixed => p.checks_solved > 0 && p.checks_served > 0 && p.checks_unrecorded == 0,
            SourceSummaryName::Unrecorded => p.checks_unrecorded > 0
                || (p.checks_solved == 0 && p.checks_served == 0 && p.checks_unrecorded == 0),
        }
    }

    pub open spec fn confirmed(f: &FunctionReport) -> bool {
        f.verdict is Verified && f.establishment is Confirmed
            && f.evidence_tier.is_some() && provenance_valid(f)
    }

    pub open spec fn crate_completed(c: &CrateReport, policy: CompletionPolicy) -> bool {
        c.verus.errors == Some(0u64) && c.verus.encountered_vir_error != Some(true)
            && match policy {
                CompletionPolicy::Receipt => c.verus.success != Some(false),
                CompletionPolicy::Whole | CompletionPolicy::Repair => c.verus.success == Some(true),
            }
    }

    pub open spec fn function_count(cs: Seq<CrateReport>) -> nat
        decreases cs.len()
    {
        if cs.len() == 0 { 0 } else { function_count(cs.drop_last()) + cs.last().functions@.len() }
    }

    pub open spec fn unstable_functions(fs: Seq<FunctionReport>) -> nat
        decreases fs.len()
    {
        if fs.len() == 0 { 0 } else {
            unstable_functions(fs.drop_last()) + if fs.last().unstable { 1nat } else { 0nat }
        }
    }

    pub open spec fn unstable_count(cs: Seq<CrateReport>) -> nat
        decreases cs.len()
    {
        if cs.len() == 0 { 0 } else {
            unstable_count(cs.drop_last()) + unstable_functions(cs.last().functions@)
        }
    }

    pub open spec fn obligation_ids(f: &FunctionReport) -> Seq<Seq<u8>> {
        match f.obligations {
            Some(os) => os@.map_values(|o: crate::output::ObligationReport| string_bytes(&o.id)),
            None => Seq::empty(),
        }
    }

    pub open spec fn function_ids(fs: Seq<FunctionReport>) -> Seq<Seq<u8>>
        decreases fs.len()
    {
        if fs.len() == 0 { Seq::empty() } else {
            function_ids(fs.drop_last()) + obligation_ids(&fs.last())
        }
    }

    pub open spec fn report_ids(cs: Seq<CrateReport>) -> Seq<Seq<u8>>
        decreases cs.len()
    {
        if cs.len() == 0 { Seq::empty() } else {
            report_ids(cs.drop_last()) + function_ids(cs.last().functions@)
        }
    }

    pub open spec fn owned_function_ids(f: &FunctionReport) -> bool {
        forall |i: int| 0 <= i < obligation_ids(f).len() ==>
            obligation_ids(f)[i].len() > string_bytes(&f.function).len()
                && obligation_ids(f)[i][string_bytes(&f.function).len() as int] == 35u8
                && forall |j: int| 0 <= j < string_bytes(&f.function).len() ==>
                    obligation_ids(f)[i][j] == string_bytes(&f.function)[j]
    }

    pub open spec fn all_owned_ids(cs: Seq<CrateReport>) -> bool {
        forall |i: int, j: int| 0 <= i < cs.len() && 0 <= j < cs[i].functions@.len() ==>
            owned_function_ids(&cs[i].functions@[j])
    }

    pub open spec fn distinct_ids(ids: Seq<Seq<u8>>) -> bool {
        forall |i: int, j: int| 0 <= i < j < ids.len() ==> ids[i] != ids[j]
    }

    pub open spec fn successful_records(r: &Report, policy: CompletionPolicy) -> bool {
        r.schema_version == crate::output::SCHEMA_VERSION && r.exit_code == Some(0i32) && r.error.is_none()
            && r.crates@.len() > 0
            && (forall |i: int| 0 <= i < r.crates@.len() ==> crate_completed(&r.crates@[i], policy))
            && (forall |i: int, j: int| 0 <= i < r.crates@.len() && 0 <= j < r.crates@[i].functions@.len()
                ==> confirmed(&r.crates@[i].functions@[j]))
            && r.summary.functions == function_count(r.crates@)
            && r.summary.verified == function_count(r.crates@)
            && r.summary.failed == 0 && r.summary.unknown == 0 && r.summary.provisional == 0
            && r.summary.unstable == unstable_count(r.crates@)
            && (policy is Repair ==> match r.engine {
                None => true,
                Some(engine) => engine.established_by is Engine,
            })
    }

    pub open spec fn successful_report(r: &Report, policy: CompletionPolicy) -> bool {
        successful_records(r, policy) && all_owned_ids(r.crates@)
            && distinct_ids(report_ids(r.crates@))
    }

    pub open spec fn small_positive_report(r: &Report, policy: CompletionPolicy) -> bool {
        successful_records(r, policy) && all_owned_ids(r.crates@)
            && r.crates@.len() == 1 && r.crates@[0].functions@.len() <= 1
            && report_ids(r.crates@).len() == 0
    }

    pub closed spec fn authority_report<'a>(a: &CompleteReport<'a>) -> &'a Report { a.report }
    pub closed spec fn authority_policy(a: &CompleteReport) -> CompletionPolicy { a.policy }

    pub closed spec fn report_authority_valid(a: &CompleteReport) -> bool {
        successful_report(a.report, a.policy)
    }
}

sail_verified! {
    /// Compares every field of a stored Cargo target identity.
    pub fn same_target(a: &TargetResult, b: &TargetResult) -> (out: bool)
        requires(true)
        ensures(out == same_target_identity(a, b))
    {
        if !exact_bytes(a.package_id.as_str().as_bytes(), b.package_id.as_str().as_bytes())
            || !exact_bytes(a.target.as_str().as_bytes(), b.target.as_str().as_bytes())
            || a.kind.len() != b.kind.len() { return false; }
        let mut i = 0usize;
        sail_while! { (i < a.kind.len())
            invariant(i <= a.kind.len(), a.kind.len() == b.kind.len(),
                string_bytes(&a.package_id) == string_bytes(&b.package_id),
                string_bytes(&a.target) == string_bytes(&b.target),
                forall |j: int| 0 <= j < i ==> string_bytes(&a.kind@[j]) == string_bytes(&b.kind@[j]))
            decreases(a.kind.len() - i)
            {
                if !exact_bytes(a.kind[i].as_str().as_bytes(), b.kind[i].as_str().as_bytes()) { return false; }
                i += 1;
            }
        }
        sail_proof! {
            assert(a.kind@.map_values(|s: String| string_bytes(&s)) =~=
                b.kind@.map_values(|s: String| string_bytes(&s)));
        }
        true
    }
}

sail_verified! {
    /// Mints selected-target authority using actual reports and arbitrary index candidates.
    pub fn complete_selection_ordered<'a>(project: &'a ProjectResult, orders: &[ReportOrder])
        -> (out: Option<CompleteSelection<'a>>)
        requires(true)
        ensures(out.is_some() ==> supplied_selection_authorized(project),
            out.is_some() ==> selection_project(&out.unwrap()) == project,
            out.is_some() ==> selection_authority_valid(&out.unwrap()))
    {
        if !matches!(project.currentness, Currentness::Current) || project.exit_code != Some(0)
            || project.error.is_some() || orders.len() != project.targets.len() { return None; }
        let mut found = false;
        let mut i = 0usize;
        sail_while! { (i < project.targets.len())
            invariant(i <= project.targets.len(), orders.len() == project.targets.len(),
                found == (exists |j: int| 0 <= j < i && project.targets@[j].requested),
                forall |j: int| 0 <= j < i && project.targets@[j].requested ==>
                    project.targets@[j].complete && project.targets@[j].scope_reuse.is_none()
                        && !partial_command(project.targets@[j].report.command@)
                        && successful_report(&project.targets@[j].report, CompletionPolicy::Repair),
                forall |j: int, k: int| 0 <= j < k < i
                    && project.targets@[j].requested && project.targets@[k].requested ==>
                        !same_target_identity(&project.targets@[j], &project.targets@[k]))
            decreases(project.targets.len() - i)
            {
                let t = &project.targets[i];
                if t.requested {
                    if !t.complete || t.scope_reuse.is_some() || focused_command(&t.report.command)
                        || complete_report_ordered(&t.report, CompletionPolicy::Repair,
                            &orders[i].forward, &orders[i].inverse).is_none() { return None; }
                    let mut j = 0usize;
                    sail_while! { (j < i)
                        invariant(j <= i, i < project.targets.len(),
                            forall |k: int| 0 <= k < j && project.targets@[k].requested ==>
                                !same_target_identity(&project.targets@[k], t))
                        decreases(i - j)
                        {
                            if project.targets[j].requested && same_target(&project.targets[j], t) { return None; }
                            j += 1;
                        }
                    }
                    found = true;
                }
                i += 1;
            }
        }
        if !found { return None; }
        Some(CompleteSelection { project })
    }
}

sail_verified! {
    /// Revalidates a target's positive presentation against its actual whole report.
    pub fn complete_target_ordered<'a>(target: &'a TargetResult, forward: &[usize], inverse: &[usize])
        -> (out: Option<CompleteReport<'a>>)
        requires(true)
        ensures(out.is_some() ==> target.complete && target.scope_reuse.is_none(),
            out.is_some() ==> successful_report(&target.report, CompletionPolicy::Whole),
            out.is_some() ==> !partial_command(target.report.command@),
            out.is_some() ==> authority_report(&out.unwrap()) == &target.report)
    {
        if !target.complete || target.scope_reuse.is_some() { return None; }
        complete_whole_report_ordered(&target.report, forward, inverse)
    }
}

sail_verified! {
    /// Whole-target authority additionally checks actual forwarded scope arguments.
    pub fn complete_whole_report_ordered<'a>(report: &'a Report, forward: &[usize], inverse: &[usize])
        -> (out: Option<CompleteReport<'a>>)
        requires(true)
        ensures(out.is_some() ==> successful_report(report, CompletionPolicy::Whole),
            out.is_some() ==> !partial_command(report.command@),
            out.is_some() ==> authority_report(&out.unwrap()) == report)
    {
        if focused_command(&report.command) { return None; }
        complete_report_ordered(report, CompletionPolicy::Whole, forward, inverse)
    }
}

sail_verified! {
    /// Preserves exact strings from the independently resolved Cargo selection.
    pub fn requested_target(package: &str, name: &str, kind: &str) -> (out: RequestedTarget)
        requires(true)
        ensures(out.package@ == package.spec_bytes(), out.name@ == name.spec_bytes(),
            out.kind@ == kind.spec_bytes())
    {
        RequestedTarget {
            package: copy_bytes(package.as_bytes()),
            name: copy_bytes(name.as_bytes()),
            kind: copy_bytes(kind.as_bytes()),
        }
    }
}

sail_verified! {
    /// Matches exact Cargo package/name/kind, preserving normalized-name collisions.
    pub fn target_matches(e: &RequestedTarget, t: &TargetResult) -> (out: bool)
        requires(true)
        ensures(out == target_match(e, t))
    {
        if !exact_bytes(&e.package, t.package_id.as_str().as_bytes())
            || !exact_bytes(&e.name, t.target.as_str().as_bytes()) { return false; }
        let mut i = 0usize;
        sail_while! { (i < t.kind.len())
            invariant(i <= t.kind.len(), e.package@ == string_bytes(&t.package_id),
                e.name@ == string_bytes(&t.target),
                forall |j: int| 0 <= j < i ==> e.kind@ != string_bytes(&t.kind@[j]))
            decreases(t.kind.len() - i)
            {
                if exact_bytes(&e.kind, t.kind[i].as_str().as_bytes()) {
                    sail_proof! { assert(target_match(e, t)); }
                    return true;
                }
                i += 1;
            }
        }
        false
    }
}

sail_verified! {
    fn unique_observed(e: &RequestedTarget, ts: &[TargetResult]) -> (out: Option<usize>)
        requires(true)
        ensures(out.is_some() ==> request_covered(e, ts@))
    {
        let mut found: Option<usize> = None;
        let mut i = 0usize;
        sail_while! { (i < ts.len())
            invariant(i <= ts.len(),
                found.is_some() ==> found.unwrap() < i && target_match(e, &ts@[found.unwrap() as int])
                    && ts@[found.unwrap() as int].requested,
                forall |j: int| 0 <= j < i && target_match(e, &ts@[j]) ==>
                    found.is_some() && j == found.unwrap())
            decreases(ts.len() - i)
            {
                if target_matches(e, &ts[i]) {
                    if found.is_some() || !ts[i].requested { return None; }
                    found = Some(i);
                }
                i += 1;
            }
        }
        sail_proof! { if found.is_some() { assert(request_covered(e, ts@)); } }
        found
    }
}

sail_verified! {
    fn unique_requested(t: &TargetResult, es: &[RequestedTarget]) -> (out: Option<usize>)
        requires(true)
        ensures(out.is_some() ==> target_covered(t, es@))
    {
        let mut found: Option<usize> = None;
        let mut i = 0usize;
        sail_while! { (i < es.len())
            invariant(i <= es.len(),
                found.is_some() ==> found.unwrap() < i && target_match(&es@[found.unwrap() as int], t),
                forall |j: int| 0 <= j < i && target_match(&es@[j], t) ==>
                    found.is_some() && j == found.unwrap())
            decreases(es.len() - i)
            {
                if target_matches(&es[i], t) {
                    if found.is_some() { return None; }
                    found = Some(i);
                }
                i += 1;
            }
        }
        sail_proof! { if found.is_some() { assert(target_covered(t, es@)); } }
        found
    }
}

sail_verified! {
    /// Establishes one-to-one coverage of the independently resolved request.
    /// Failed targets without artifacts must still appear as observations.
    pub fn requested_coverage(expected: &[RequestedTarget], targets: &[TargetResult]) -> (out: bool)
        requires(true)
        ensures(out ==> exact_requested_coverage(expected@, targets@))
    {
        if expected.is_empty() { return false; }
        let mut i = 0usize;
        sail_while! { (i < expected.len())
            invariant(i <= expected.len(),
                forall |j: int| 0 <= j < i ==> request_covered(&expected@[j], targets@))
            decreases(expected.len() - i)
            {
                if unique_observed(&expected[i], targets).is_none() { return false; }
                i += 1;
            }
        }
        let mut j = 0usize;
        sail_while! { (j < targets.len())
            invariant(j <= targets.len(),
                forall |k: int| 0 <= k < targets@.len() && k < j && targets@[k].requested
                    ==> target_covered(&targets@[k], expected@),
                forall |k: int| 0 <= k < expected@.len() ==> request_covered(&expected@[k], targets@))
            decreases(targets.len() - j)
            {
                if targets[j].requested && unique_requested(&targets[j], expected).is_none() { return false; }
                j += 1;
            }
        }
        true
    }
}

sail_verified! {
    /// Recognizes the actual forwarded Verus selectors that make a run partial.
    pub fn focused_command(args: &[String]) -> (out: bool)
        requires(true)
        ensures(out == partial_command(args@))
    {
        let mut separator = false;
        let mut i = 0usize;
        sail_while! { (i < args.len())
            invariant(i <= args.len(), !partial_command(args@.take(i as int)),
                separator == (exists |j: int| 0 <= j < i && string_bytes(&args@[j]) == b"--"@))
            decreases(args.len() - i)
            {
                let bytes = args[i].as_str().as_bytes();
                if exact_bytes(bytes, b"--") { separator = true; }
                if separator && (argument_flag(bytes, b"--verify-module")
                    || argument_flag(bytes, b"--verify-only-module")
                    || argument_flag(bytes, b"--verify-function")
                    || argument_flag(bytes, b"--verify-root")) { return true; }
                i += 1;
            }
        }
        sail_proof! { assert(args@.take(i as int) =~= args@); }
        false
    }
}

sail_verified! {
    /// Checks arbitrary candidate index data before minting actual report authority.
    /// Candidate production and sorting need no correctness assumption.
    #[allow(clippy::question_mark, reason = "Keep the explicit checked refusal branch in the shared source.")]
    pub fn complete_report_ordered<'a>(report: &'a Report, policy: CompletionPolicy,
        forward: &[usize], inverse: &[usize]) -> (out: Option<CompleteReport<'a>>)
        requires(true)
        ensures(out.is_some() ==> successful_report(report, policy),
            out.is_some() ==> authority_report(&out.unwrap()) == report,
            out.is_some() ==> authority_policy(&out.unwrap()) == policy,
            out.is_some() ==> report_authority_valid(&out.unwrap()),
            small_positive_report(report, policy) && forward@.len() == 0 && inverse@.len() == 0
                ==> out.is_some())
    {
        if !completed_report_records(report, policy) { return None; }
        let Some(ids) = collect_report_ids(report) else { return None; };
        if !unique_obligations(&ids, forward, inverse) { return None; }
        Some(CompleteReport { report, policy })
    }
}

sail_verified! {
    /// Checks confirmed verdict, evidence tier and actual provenance counts.
    pub fn confirmed_function(f: &FunctionReport) -> (out: bool)
        requires(true)
        ensures(out == confirmed(f))
    {
        let p = &f.provenance;
        let provenance = match p.source {
            SourceSummaryName::Fresh => p.checks_solved > 0 && p.checks_served == 0 && p.checks_unrecorded == 0,
            SourceSummaryName::Cache => p.checks_solved == 0 && p.checks_served > 0 && p.checks_unrecorded == 0,
            SourceSummaryName::Mixed => p.checks_solved > 0 && p.checks_served > 0 && p.checks_unrecorded == 0,
            SourceSummaryName::Unrecorded => p.checks_unrecorded > 0
                || (p.checks_solved == 0 && p.checks_served == 0 && p.checks_unrecorded == 0),
        };
        matches!(f.verdict, VerdictName::Verified)
            && matches!(f.establishment, EstablishmentName::Confirmed)
            && f.evidence_tier.is_some()
            && provenance
    }
}

sail_verified! {
    /// Counts only independently checked confirmed functions.
    fn confirmed_counts(c: &CrateReport) -> (out: Option<(u64, u64)>)
        requires(true)
        ensures(out.is_some() ==> out.unwrap().0 == c.functions@.len(),
            out.is_some() ==> out.unwrap().1 == unstable_functions(c.functions@),
            out.is_some() ==> forall |i: int| 0 <= i < c.functions@.len() ==> confirmed(&c.functions@[i]),
            (c.functions@.len() <= 1 && forall |i: int| 0 <= i < c.functions@.len() ==> confirmed(&c.functions@[i]))
                ==> out.is_some())
    {
        if c.functions.len() as u128 > u128::from(u64::MAX) { return None; }
        let mut i = 0usize;
        let mut unstable = 0u64;
        sail_while! { (i < c.functions.len())
            invariant(i <= c.functions.len(), c.functions.len() <= u64::MAX,
                unstable <= i, unstable == unstable_functions(c.functions@.take(i as int)),
                forall |j: int| 0 <= j < i ==> confirmed(&c.functions@[j]))
            decreases(c.functions.len() - i)
            {
                if !confirmed_function(&c.functions[i]) { return None; }
                if c.functions[i].unstable { unstable += 1; }
                sail_proof! {
                    assert(c.functions@.take(i as int + 1).drop_last() == c.functions@.take(i as int));
                    assert(c.functions@.take(i as int + 1).last() == c.functions@[i as int]);
                    reveal(unstable_functions);
                }
                i += 1;
            }
        }
        sail_proof! { assert(c.functions@.take(i as int) =~= c.functions@); }
        Some((c.functions.len() as u64, unstable))
    }
}

sail_verified! {
    /// Checks exact report fields and summaries; a zero-function crate is valid.
    fn completed_report_records(report: &Report, policy: CompletionPolicy) -> (out: bool)
        requires(true)
        ensures(out ==> successful_records(report, policy),
            (successful_records(report, policy) && report.crates@.len() == 1
                && report.crates@[0].functions@.len() <= 1) ==> out)
    {
        if report.schema_version != crate::output::SCHEMA_VERSION || report.exit_code != Some(0) || report.error.is_some()
            || report.crates.is_empty() { return false; }
        let engine_allowed = match (policy, &report.engine) {
            (CompletionPolicy::Repair, Some(engine)) => matches!(engine.established_by, EngineEstablishment::Engine),
            _ => true,
        };
        if !engine_allowed { return false; }
        let mut i = 0usize;
        let mut functions = 0u64;
        let mut unstable = 0u64;
        sail_while! { (i < report.crates.len())
            invariant(i <= report.crates.len(),
                functions == function_count(report.crates@.take(i as int)),
                unstable == unstable_count(report.crates@.take(i as int)),
                forall |j: int| 0 <= j < i ==> crate_completed(&report.crates@[j], policy),
                forall |j: int, k: int| 0 <= j < i && 0 <= k < report.crates@[j].functions@.len()
                    ==> confirmed(&report.crates@[j].functions@[k]))
            decreases(report.crates.len() - i)
            {
                let c = &report.crates[i];
                if !completed_crate(c, policy) { return false; }
                let Some(counts) = confirmed_counts(c) else { return false; };
                if u64::MAX - functions < counts.0 || u64::MAX - unstable < counts.1 {
                    return false;
                }
                functions += counts.0;
                unstable += counts.1;
                sail_proof! {
                    assert(report.crates@.take(i as int + 1).drop_last() == report.crates@.take(i as int));
                    assert(report.crates@.take(i as int + 1).last() == report.crates@[i as int]);
                    reveal(function_count);
                    reveal(unstable_count);
                }
                i += 1;
            }
        }
        sail_proof! { assert(report.crates@.take(i as int) =~= report.crates@); }
        report.summary.functions == functions && report.summary.verified == functions
            && report.summary.failed == 0 && report.summary.unknown == 0 && report.summary.provisional == 0
            && report.summary.unstable == unstable
    }
}

sail_verified! {
    /// Appends every actual obligation ID after checking its exact ownership.
    fn append_obligations(f: &FunctionReport, seen: &mut Vec<Vec<u8>>) -> (out: bool)
        requires(true)
        ensures(out == owned_function_ids(f),
            out ==> final(seen)@.map_values(|v: Vec<u8>| v@)
                == old(seen)@.map_values(|v: Vec<u8>| v@) + obligation_ids(f))
    {
        if let Some(os) = &f.obligations {
            let mut i = 0usize;
            sail_while! { (i < os.len())
                invariant(i <= os.len(),
                    obligation_ids(f) == os@.map_values(|o: crate::output::ObligationReport| string_bytes(&o.id)),
                    seen@.map_values(|v: Vec<u8>| v@) == old(seen)@.map_values(|v: Vec<u8>| v@) + obligation_ids(f).take(i as int),
                    forall |j: int| 0 <= j < i ==> obligation_ids(f)[j].len() > string_bytes(&f.function).len()
                        && obligation_ids(f)[j][string_bytes(&f.function).len() as int] == 35u8
                        && forall |k: int| 0 <= k < string_bytes(&f.function).len() ==>
                            obligation_ids(f)[j][k] == string_bytes(&f.function)[k])
                decreases(os.len() - i)
                {
                    let id = os[i].id.as_str().as_bytes();
                    let name = f.function.as_str().as_bytes();
                    if !owned_obligation(name, id) {
                        sail_proof! {
                            assert(obligation_ids(f)[i as int] == id@);
                            if owned_function_ids(f) {
                                assert(obligation_ids(f)[i as int].len() > string_bytes(&f.function).len());
                                assert(sail_acceptance::authority::owned_id(name, id));
                            }
                        }
                        return false;
                    }
                    seen.push(copy_bytes(id));
                    sail_proof! {
                        assert(obligation_ids(f).take(i as int + 1)
                            == obligation_ids(f).take(i as int).push(obligation_ids(f)[i as int]));
                        assert(seen@.map_values(|v: Vec<u8>| v@) =~=
                            old(seen)@.map_values(|v: Vec<u8>| v@) + obligation_ids(f).take(i as int + 1));
                    }
                    i += 1;
                }
            }
        }
        true
    }
}

sail_verified! {
    /// Collects every actual obligation, checking its exact owning function.
    pub fn collect_report_ids(report: &Report) -> (out: Option<Vec<Vec<u8>>>)
        requires(true)
        ensures(out.is_some() == all_owned_ids(report.crates@),
            out.is_some() ==> out.unwrap()@.map_values(|v: Vec<u8>| v@) == report_ids(report.crates@))
    {
        let mut seen: Vec<Vec<u8>> = Vec::new();
        let mut i = 0usize;
        sail_while! { (i < report.crates.len())
            invariant(i <= report.crates.len(),
                seen@.map_values(|v: Vec<u8>| v@) == report_ids(report.crates@.take(i as int)),
                all_owned_ids(report.crates@.take(i as int)))
            decreases(report.crates.len() - i)
            {
                let c = &report.crates[i];
                let mut j = 0usize;
                sail_while! { (j < c.functions.len())
                    invariant(j <= c.functions.len(), i < report.crates.len(), c == &report.crates@[i as int],
                        seen@.map_values(|v: Vec<u8>| v@)
                            == report_ids(report.crates@.take(i as int)) + function_ids(c.functions@.take(j as int)),
                                all_owned_ids(report.crates@.take(i as int)),
                        forall |k: int| 0 <= k < j ==> owned_function_ids(&c.functions@[k]))
                    decreases(c.functions.len() - j)
                    {
                        if !append_obligations(&c.functions[j], &mut seen) {
                            sail_proof! {
                                if all_owned_ids(report.crates@) {
                                    assert(owned_function_ids(&report.crates@[i as int].functions@[j as int]));
                                }
                            }
                            return None;
                        }
                        sail_proof! {
                            assert(c.functions@.take(j as int + 1).drop_last() == c.functions@.take(j as int));
                            assert(c.functions@.take(j as int + 1).last() == c.functions@[j as int]);
                            reveal(function_ids);
                        }
                        j += 1;
                    }
                }
                sail_proof! {
                    assert(c.functions@.take(j as int) =~= c.functions@);
                    assert(report.crates@.take(i as int + 1).drop_last() == report.crates@.take(i as int));
                    assert(report.crates@.take(i as int + 1).last() == report.crates@[i as int]);
                    reveal(report_ids);
                }
                i += 1;
            }
        }
        sail_proof! { assert(report.crates@.take(i as int) =~= report.crates@); }
        Some(seen)
    }
}

sail_verified! {
    /// Checks real crate summary records, preserving solver-free success.
    pub fn completed_crate(c: &CrateReport, policy: CompletionPolicy) -> (out: bool)
        requires(true)
        ensures(out == crate_completed(c, policy))
    {
        c.verus.errors == Some(0) && c.verus.encountered_vir_error != Some(true)
            && match policy {
                CompletionPolicy::Receipt => c.verus.success != Some(false),
                CompletionPolicy::Whole | CompletionPolicy::Repair => c.verus.success == Some(true),
            }
    }
}
