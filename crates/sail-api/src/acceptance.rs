//! Positive Verus authority checked directly over the public report records.
//!
//! These predicates never infer a complete request from its selected-result
//! flags. The caller with the independently resolved request must additionally
//! establish [`requested_coverage`](crate::acceptance::requested_coverage). Currentness remains an input property:
//! a fresh failed verification can be Current without authorizing a repair.
//! The presentation helpers preserve the provisional override over raw fields;
//! they do not establish request, process, cache or whole-proof authority.
#![allow(
    clippy::redundant_as_str,
    reason = "The pinned stock Verus specifies str::as_bytes, not String::as_bytes."
)]

use sail_acceptance::authority::{
    CompletionPolicy, ObservedAttempt, RequestedTarget, argument_flag, copy_bytes, exact_bytes,
    observed_payload_matches, owned_obligation, unique_obligations,
};
use sail_acceptance::{sail_code, sail_method, sail_proof, sail_spec, sail_verified, sail_while};

/// Builds untrusted order candidates and invokes the checked authority factory.
pub use crate::acceptance_sort::{
    admit_declared_replay, bind_replayed_exact_partial_project, bind_replayed_project,
    complete_exact_partial_request, complete_exact_scope_report, complete_report, complete_request,
    complete_selection, complete_submitted_request, complete_target, complete_whole_report,
    completed_presentation, withhold_unbound_request_authority,
};

use crate::jobs::Mode;
use crate::output::{
    CrateReport, EngineEstablishment, ErrorInfo, EstablishmentName, FunctionReport, Report,
    SourceSummaryName, VerdictName,
};
use crate::project::{Currentness, EngineIdentity, ProjectResult, ResolvedRequest, TargetResult};

#[cfg(sail_verus)]
use vstd::prelude::*;
#[cfg(sail_verus)]
use vstd::string::StringSliceAdditionalSpecFns;

sail_spec! {
    /// Presentation only; confirmed establishment does not itself prove a verdict.
    pub open spec fn presentation_value(verdict: &VerdictName, establishment: &EstablishmentName) -> VerdictName {
        if *establishment is Provisional { VerdictName::Provisional } else { *verdict }
    }
}

sail_verified! {
    /// Provisional establishment always presents provisional; confirmed
    /// establishment preserves the supplied verdict without granting authority.
    pub fn presentation_verdict(verdict: &VerdictName, establishment: &EstablishmentName) -> (out: VerdictName)
        requires(true)
        ensures(out == presentation_value(verdict,establishment),
            *establishment is Provisional ==> out is Provisional,
            *establishment is Confirmed ==> out == *verdict)
    {
        match establishment {
            EstablishmentName::Provisional => VerdictName::Provisional,
            EstablishmentName::Confirmed => match verdict {
                VerdictName::Verified => VerdictName::Verified,
                VerdictName::Failed => VerdictName::Failed,
                VerdictName::Unknown => VerdictName::Unknown,
                VerdictName::Provisional => VerdictName::Provisional,
            },
        }
    }
}

sail_verified! {
    /// Applies the same presentation rule to the actual report's borrowed fields.
    /// Evidence tiers, provenance and all other report fields are untouched.
    pub fn present_function_verdict(report: &FunctionReport) -> (out: VerdictName)
        requires(true)
        ensures(out == presentation_value(&report.verdict,&report.establishment),
            report.establishment is Provisional ==> out is Provisional,
            report.establishment is Confirmed ==> out == report.verdict)
    { presentation_verdict(&report.verdict,&report.establishment) }
}

sail_code! {
    /// Process-local view of one original request/result observation. Data is
    /// always retained; optional declared whole-request evidence is separate.
    /// This is neither serializable authority nor a physical/origin witness.
    pub struct CompletedPresentation<'a> {
        project: &'a ProjectResult,
        request: Option<&'a ResolvedRequest>,
        whole: Option<CompleteRequest<'a>>,
    }

    impl<'a> CompletedPresentation<'a> {
        sail_method! {
            /// Exact original result, including all failure/partial/history data.
            pub fn project(&self) -> (out: &'a ProjectResult)
                requires(true) ensures(out == completed_presentation_project(self)) { self.project }
        }
        sail_method! {
            /// Exact paired original request; no report-selected reconstruction.
            pub fn request(&self) -> (out: Option<&'a ResolvedRequest>)
                requires(true) ensures(out == completed_presentation_request(self))
            { match &self.request { Some(request) => Some(*request), None => None } }
        }
        sail_method! {
            /// Optional existing CompleteRequest over supplied declared fields.
            /// This adds no origin/currentness observation; absence never
            /// withholds the recorded data projection.
            pub fn whole(&self) -> (out: Option<&CompleteRequest<'a>>)
                requires(true)
                ensures(out.is_some() == completed_presentation_whole(self).is_some(),
                    out.is_some() ==> *out.unwrap() == completed_presentation_whole(self).unwrap())
            { match &self.whole { Some(whole) => Some(whole), None => None } }
        }
    }

    /// Checked immutable report authority, bound to the actual report borrow.
    /// Its fields are private so deserialization and caller flags cannot mint it.
    pub struct CompleteReport<'a> {
        report: &'a Report,
        policy: CompletionPolicy,
    }

    /// Successful evidence for one exact partial invocation. This token cannot
    /// be used as whole-target or whole-request authority.
    pub struct ExactScopeReport<'a> {
        report: &'a Report,
        arguments: &'a [String],
    }

    /// Exact retained partial evidence for an independently resolved request.
    /// It supplies no whole-target or whole-request completion authority.
    pub struct ExactPartialRequest<'a> {
        request: &'a ResolvedRequest,
        project: &'a ProjectResult,
    }

    impl<'a> ExactPartialRequest<'a> {
        sail_method! {
            /// Borrows the original independently supplied partial request.
            pub fn request(&self) -> (out: &'a ResolvedRequest)
                requires(true) ensures(out == exact_partial_request(self)) { self.request }
        }
        sail_method! {
            /// Borrows the original project without changing scope or reports.
            pub fn project(&self) -> (out: &'a ProjectResult)
                requires(true) ensures(out == exact_partial_project(self)) { self.project }
        }
    }

    impl<'a> ExactScopeReport<'a> {
        sail_method! {
            /// Borrows the original report without changing its completion fields.
            pub fn report(&self) -> (out: &'a Report)
                requires(true) ensures(out == exact_scope_report(self)) { self.report }
        }
        sail_method! {
            /// Returns the exact ordered invocation arguments checked by the factory.
            pub fn arguments(&self) -> (out: &'a [String])
                requires(true) ensures(out == exact_scope_arguments(self)) { self.arguments }
        }
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

    /// Current whole-request authority bound to independently retained scope.
    pub struct CompleteRequest<'a> {
        request: &'a ResolvedRequest,
        project: &'a ProjectResult,
    }

    /// Private revision-linked declared readback for one supplied current
    /// request. Physical closure and request origin remain native obligations.
    pub struct ReplayReadback<'a, 'b> {
        request: &'a ResolvedRequest,
        readback: &'a sail_acceptance::inputs::GuardedReadback<'b>,
    }

    /// Checked correspondence with independently supplied native path bytes.
    /// This is not filesystem, loaded-image or complete input-domain authority.
    pub struct RequestPathBinding<'a> {
        request: &'a ResolvedRequest,
        invocation: &'a [u8],
        workspace: &'a [u8],
    }

    /// Admission of an exact retained replay over the supplied request,
    /// native-path and guarded-readback domain. This is not full service or
    /// physical compiler-input authority, nor a live process observation.
    pub struct DeclaredReplayAdmission<'a, 'b> {
        original: &'a ResolvedRequest,
        current: &'a ResolvedRequest,
        project: ProjectResult,
        paths: RequestPathBinding<'a>,
        readback: ReplayReadback<'a, 'b>,
    }

    /// Byte-only replay of a human Cargo no-op. This exposes neither a report
    /// nor any whole/partial proof token. Empty observed coverage cannot supply
    /// the independently requested, nonempty verification inventory.
    pub struct EmptyHumanOutput<'a, 'b> {
        original: &'a ResolvedRequest,
        current: &'a ResolvedRequest,
        observed: &'a ProjectResult,
        stderr: &'a [u8],
        paths: RequestPathBinding<'a>,
        readback: ReplayReadback<'a, 'b>,
    }

    impl<'a, 'b> EmptyHumanOutput<'a, 'b> {
        sail_method! {
            /// The only data projection: the exact validated Cargo footer.
            pub fn stderr(&self) -> (out: &'a [u8])
                requires(true) ensures(out == empty_human_stderr(self))
            { self.stderr }
        }
    }

    impl<'a, 'b> DeclaredReplayAdmission<'a, 'b> {
        sail_method! {
            /// Borrows the retained original request, without reminting it.
            pub fn original_request(&self) -> (out: &'a ResolvedRequest)
                requires(true) ensures(out == declared_replay_original(self)) { self.original }
        }
        sail_method! {
            /// Borrows exactly the independently supplied current request.
            pub fn request(&self) -> (out: &'a ResolvedRequest)
                requires(true) ensures(out == declared_replay_current(self)) { self.current }
        }
        sail_method! {
            /// Borrows the admitted view with its original reports intact.
            pub fn project(&self) -> (out: &ProjectResult)
                requires(true) ensures(*out == declared_replay_project(self)) { &self.project }
        }
        sail_method! {
            /// Borrows the exact native byte binding retained by admission.
            pub fn paths(&self) -> (out: &RequestPathBinding<'a>)
                requires(true) ensures(*out == declared_replay_paths(self)) { &self.paths }
        }
        sail_method! {
            /// Borrows the original declared-domain revision/readback relation.
            pub fn readback(&self) -> (out: &ReplayReadback<'a, 'b>)
                requires(true) ensures(*out == declared_replay_readback(self)) { &self.readback }
        }
    }

    impl<'a> RequestPathBinding<'a> {
        sail_method! {
            /// Borrows the exact request whose path fields were checked.
            pub fn request(&self) -> (out: &'a ResolvedRequest)
                requires(true) ensures(out == path_binding_request(self)) { self.request }
        }
        sail_method! {
            /// Returns the original native invocation bytes, without reencoding.
            pub fn invocation(&self) -> (out: &'a [u8])
                requires(true) ensures(out == path_binding_invocation(self)) { self.invocation }
        }
        sail_method! {
            /// Returns the original native workspace bytes, without reencoding.
            pub fn workspace(&self) -> (out: &'a [u8])
                requires(true) ensures(out == path_binding_workspace(self)) { self.workspace }
        }
    }

    impl<'a, 'b> ReplayReadback<'a, 'b> {
        sail_method! {
            /// Returns the exact current request bound by the checked factory.
            pub fn request(&self) -> (out: &'a ResolvedRequest)
                requires(true) ensures(out == replay_readback_request(self)) { self.request }
        }
        sail_method! {
            /// Borrows the original typed observation; no metadata remints it.
            pub fn observation(&self) -> (out: &'a sail_acceptance::inputs::GuardedReadback<'b>)
                requires(true) ensures(out == replay_readback_observation(self)) { self.readback }
        }
    }

    /// Pre-execution request correspondence, not a completed invocation origin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SubmissionKind {
        /// The original stock mode matches a captured stock backend selection.
        Stock,
        /// The interactive mode matches a captured interactive backend selection.
        Interactive,
        /// The exact original repair target/function selection was captured.
        Repair,
        /// An opt-in submission has a stock request; failure history is separate.
        StockForOptIn,
    }

    /// Exact original submission arguments and mode bound to captured request data.
    pub struct SubmissionBinding<'a> {
        arguments: &'a [String],
        mode: &'a Mode,
        request: &'a ResolvedRequest,
        kind: SubmissionKind,
    }

    /// Whole-request authority retaining the independent original submission borrow.
    pub struct SubmittedAuthority<'a> {
        arguments: &'a [String],
        mode: &'a Mode,
        request: &'a ResolvedRequest,
        project: &'a ProjectResult,
        kind: SubmissionKind,
    }

    impl SubmissionBinding<'_> {
        sail_method! {
            /// Exact original submitted arguments checked by the binding factory.
            #[must_use]
            pub fn arguments(&self) -> (out: &[String])
                requires(true) ensures(out == bound_arguments(self))
            { self.arguments }
        }
        sail_method! {
            /// Exact original selected mode checked by the binding factory.
            #[must_use]
            pub fn mode(&self) -> (out: &Mode)
                requires(true) ensures(out == bound_mode(self))
            { self.mode }
        }
        sail_method! {
            /// Exact captured request matched to the original submission.
            #[must_use]
            pub fn request(&self) -> (out: &ResolvedRequest)
                requires(true) ensures(out == bound_request(self))
            { self.request }
        }
        sail_method! {
            /// Derived request class; never a completion, origin or failure fact.
            #[must_use]
            pub fn kind(&self) -> (out: SubmissionKind)
                requires(true) ensures(out == bound_kind(self))
            { self.kind }
        }
    }

    impl SubmittedAuthority<'_> {
        sail_method! {
            /// Actual original submitted arguments, without interpreting a result header.
            #[must_use]
            pub fn arguments(&self) -> (out: &[String])
                requires(true) ensures(out == submitted_arguments(self))
            { self.arguments }
        }
        sail_method! {
            /// Actual original selected mode.
            #[must_use]
            pub fn mode(&self) -> (out: &Mode)
                requires(true) ensures(out == submitted_mode(self))
            { self.mode }
        }
        sail_method! {
            /// Actual captured request whose scope and options were checked.
            #[must_use]
            pub fn request(&self) -> (out: &ResolvedRequest)
                requires(true) ensures(out == submitted_request(self))
            { self.request }
        }
        sail_method! {
            /// Actual checked project, immutably bound to the original submission.
            #[must_use]
            pub fn project(&self) -> (out: &ProjectResult)
                requires(true) ensures(out == submitted_project(self))
            { self.project }
        }
        sail_method! {
            /// Derived request class, preserving the original mode distinction.
            #[must_use]
            pub fn kind(&self) -> (out: SubmissionKind)
                requires(true) ensures(out == submitted_kind(self))
            { self.kind }
        }
    }

    impl CompleteRequest<'_> {
        sail_method! {
            /// Original independently retained request checked by the factory.
            #[must_use]
            pub fn request(&self) -> (out: &ResolvedRequest)
                requires(true)
                ensures(out == request_authority_request(self))
            { self.request }
        }
        sail_method! {
            /// Actual project whose complete requested scope was checked.
            #[must_use]
            pub fn project(&self) -> (out: &ProjectResult)
                requires(true)
                ensures(out == request_authority_project(self))
            { self.project }
        }
    }

    impl CompleteSelection<'_> {
        sail_method! {
            /// Exact immutable project whose selected reports were checked.
            #[must_use]
            pub fn project(&self) -> (out: &ProjectResult)
                requires(true)
                ensures(out == selection_project(self))
            { self.project }
        }
    }

    impl CompleteReport<'_> {
        sail_method! {
            /// The immutable actual report whose fields were checked.
            #[must_use]
            pub fn report(&self) -> (out: &Report)
                requires(true)
                ensures(out == authority_report(self))
            { self.report }
        }

        sail_method! {
            /// The receipt or repair contract that was checked.
            #[must_use]
            pub fn policy(&self) -> (out: CompletionPolicy)
                requires(true)
                ensures(out == authority_policy(self))
            { self.policy }
        }
    }
}

sail_spec! {
    pub closed spec fn completed_presentation_project<'a>(view: &CompletedPresentation<'a>) -> &'a ProjectResult { view.project }
    pub closed spec fn completed_presentation_request<'a>(view: &CompletedPresentation<'a>) -> Option<&'a ResolvedRequest> { view.request }
    pub closed spec fn completed_presentation_whole<'a>(view: &CompletedPresentation<'a>) -> Option<CompleteRequest<'a>> { view.whole }
    pub open spec fn completed_presentation_valid(view: &CompletedPresentation) -> bool {
        completed_presentation_whole(view).is_some() ==> completed_presentation_request(view).is_some()
            && request_authority_request(&completed_presentation_whole(view).unwrap()) == completed_presentation_request(view).unwrap()
            && request_authority_project(&completed_presentation_whole(view).unwrap()) == completed_presentation_project(view)
            && request_authorized(completed_presentation_request(view).unwrap(),completed_presentation_project(view))
    }
}

sail_verified! {
    /// Always retains the supplied pair. Only the existing checked factory may
    /// add declared whole-request evidence; refusal changes no data or label.
    pub fn completed_presentation_ordered<'a>(project: &'a ProjectResult,
        request: Option<&'a ResolvedRequest>, orders: &[ReportOrder]) -> (view: CompletedPresentation<'a>)
        requires(true)
        ensures(completed_presentation_project(&view) == project,
            completed_presentation_request(&view) == request, completed_presentation_valid(&view),
            request.is_none() ==> completed_presentation_whole(&view).is_none(),
            request.is_some() && small_positive_request(request.unwrap(),project)
                && orders@.len() == 1 && orders@[0].forward@.len() == 0 && orders@[0].inverse@.len() == 0
                ==> completed_presentation_whole(&view).is_some())
    {
        let whole = match request {
            Some(request) => complete_request_ordered(request,project,orders),
            None => None,
        };
        CompletedPresentation { project,request,whole }
    }
}

sail_verified! {
    /// Classifies a retained result from its original state and actual readback.
    /// A matching revision cannot upgrade Historical or Unknown establishment.
    pub fn retained_currentness(original: &Currentness, expected: &str,
        observed: Option<&str>, complete: bool) -> (out: crate::jobs::Currentness)
        requires(true)
        ensures((out is Current) == (*original is Current && complete
                && observed.is_some() && observed.unwrap().spec_bytes() == expected.spec_bytes()),
            *original is Historical ==> out is Historical,
            observed.is_some() && observed.unwrap().spec_bytes() != expected.spec_bytes()
                ==> out is Historical,
            out is Current ==> *original is Current)
    {
        if let Some(revision) = observed {
            if !exact_bytes(revision.as_bytes(), expected.as_bytes()) {
                return crate::jobs::Currentness::Historical;
            }
            if complete && matches!(original, Currentness::Current) {
                return crate::jobs::Currentness::Current;
            }
        }
        if matches!(original, Currentness::Historical) {
            crate::jobs::Currentness::Historical
        } else {
            crate::jobs::Currentness::Unknown
        }
    }
}

sail_verified! {
    /// Applies one selected freshness view without modifying proof evidence.
    /// Returns whether a formerly Current result needs the lost-readback note.
    pub fn apply_retained_currentness(project: &mut ProjectResult,
        view: &crate::jobs::Currentness) -> (lost: bool)
        requires(true)
        ensures(final(project).targets == old(project).targets,
            final(project).kind == old(project).kind,
            final(project).schema_version == old(project).schema_version,
            final(project).run_id == old(project).run_id,
            final(project).workspace == old(project).workspace,
            final(project).context_sha256 == old(project).context_sha256,
            final(project).input_revision == old(project).input_revision,
            final(project).exit_code == old(project).exit_code,
            final(project).error == old(project).error,
            final(project).engine == old(project).engine,
            (final(project).currentness is Current) == (*view is Current),
            (final(project).currentness is Historical) == (*view is Historical),
            (final(project).currentness is Unknown) == (*view is Unknown),
            lost == (old(project).currentness is Current && *view is Unknown))
    {
        let originally_current = matches!(project.currentness, Currentness::Current);
        project.currentness = match view {
            crate::jobs::Currentness::Current => Currentness::Current,
            crate::jobs::Currentness::Historical => Currentness::Historical,
            crate::jobs::Currentness::Unknown => Currentness::Unknown,
        };
        originally_current && matches!(view, crate::jobs::Currentness::Unknown)
    }
}

sail_spec! {
    broadcast use {vstd::seq_lib::group_seq_properties, vstd::seq::Seq::lemma_push_map_commute};
    pub open spec fn string_bytes(s: &String) -> Seq<u8> { vstd::utf8::encode_utf8(s@) }

    pub closed spec fn request_authority_request<'a>(a: &CompleteRequest<'a>) -> &'a ResolvedRequest { a.request }
    pub closed spec fn request_authority_project<'a>(a: &CompleteRequest<'a>) -> &'a ProjectResult { a.project }

    pub closed spec fn bound_request<'a>(a: &SubmissionBinding<'a>) -> &'a ResolvedRequest { a.request }
    pub closed spec fn bound_arguments<'a>(a: &SubmissionBinding<'a>) -> &'a [String] { a.arguments }
    pub closed spec fn bound_mode<'a>(a: &SubmissionBinding<'a>) -> &'a Mode { a.mode }
    pub closed spec fn bound_kind(a: &SubmissionBinding) -> SubmissionKind { a.kind }
    pub closed spec fn submitted_arguments<'a>(a: &SubmittedAuthority<'a>) -> &'a [String] { a.arguments }
    pub closed spec fn submitted_mode<'a>(a: &SubmittedAuthority<'a>) -> &'a Mode { a.mode }
    pub closed spec fn submitted_request<'a>(a: &SubmittedAuthority<'a>) -> &'a ResolvedRequest { a.request }
    pub closed spec fn submitted_project<'a>(a: &SubmittedAuthority<'a>) -> &'a ProjectResult { a.project }
    pub closed spec fn submitted_kind(a: &SubmittedAuthority) -> SubmissionKind { a.kind }

    pub open spec fn repair_selection_matches(r: &ResolvedRequest, package: Seq<u8>,
        target: &Option<String>, kind: Seq<u8>, function: Seq<u8>) -> bool {
        r.function.is_some() && string_bytes(&r.function.unwrap()) == function
            && string_bytes(&r.repair_package) == package && string_bytes(&r.repair_kind) == kind
            && match (&r.repair_target, target) {
                (None, None) => true,
                (Some(a), Some(b)) => string_bytes(a) == string_bytes(b),
                _ => false,
            }
    }

    pub open spec fn mode_correspondence(mode: &Mode, r: &ResolvedRequest, kind: SubmissionKind) -> bool {
        match mode {
            Mode::Stock => r.engine.is_none() && r.function.is_none() && kind is Stock,
            Mode::Interactive => r.function.is_none() && if r.engine.is_some() {
                interactive_engine(&r.engine) && kind is Interactive
            } else { kind is StockForOptIn },
            Mode::Repair(selection) => if repair_selection_matches(r, string_bytes(&selection.package), &selection.target, string_bytes(&selection.kind), string_bytes(&selection.function)) {
                interactive_engine(&r.engine) && kind is Repair
            } else { r.engine.is_none() && r.function.is_none() && kind is StockForOptIn },
        }
    }

    pub open spec fn interactive_engine(engine: &Option<EngineIdentity>) -> bool {
        match engine {
            Some(e) => string_bytes(&e.name) == b"interactive"@ && string_bytes(&e.manifest_sha256).len() > 0,
            None => false,
        }
    }

    pub open spec fn submission_correspondence(arguments: Seq<String>, mode: &Mode,
        r: &ResolvedRequest, kind: SubmissionKind) -> bool {
        r.args@.map_values(|s: String| string_bytes(&s)) == normalized_arguments(arguments, arguments.len())
            && mode_correspondence(mode, r, kind)
    }

    pub closed spec fn binding_valid(b: &SubmissionBinding) -> bool {
        submission_correspondence(b.arguments@, b.mode, b.request, b.kind)
    }

    pub open spec fn engine_identity_matches(a: &Option<EngineIdentity>, b: &Option<EngineIdentity>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => string_bytes(&x.name) == string_bytes(&y.name)
                && string_bytes(&x.manifest_sha256) == string_bytes(&y.manifest_sha256),
            _ => false,
        }
    }

    pub open spec fn request_header_matches(r: &ResolvedRequest, p: &ProjectResult) -> bool {
        string_bytes(&p.kind) == b"sail-project-result"@
            && p.schema_version == crate::project::SCHEMA_VERSION
            && string_bytes(&r.run_id).len() > 0
            && string_bytes(&r.context_sha256).len() > 0
            && string_bytes(&r.input_revision).len() > 0
            && string_bytes(&r.run_id) == string_bytes(&p.run_id)
            && string_bytes(&r.workspace) == string_bytes(&p.workspace)
            && string_bytes(&r.context_sha256) == string_bytes(&p.context_sha256)
            && string_bytes(&r.input_revision) == string_bytes(&p.input_revision)
            && engine_identity_matches(&r.engine, &p.engine)
    }

    pub open spec fn resolved_expected(r: &ResolvedRequest, es: Seq<RequestedTarget>) -> bool {
        es.len() == r.targets@.len() && forall |i: int| 0 <= i < es.len() ==>
            es[i].package@ == string_bytes(&r.targets@[i].package_id)
                && es[i].name@ == string_bytes(&r.targets@[i].target)
                && es[i].kind@ == string_bytes(&r.targets@[i].kind)
    }

    pub open spec fn resolved_coverage(r: &ResolvedRequest, p: &ProjectResult) -> bool {
        exists |es: Seq<RequestedTarget>| resolved_expected(r, es)
            && exact_requested_coverage(es, p.targets@)
    }

    pub open spec fn request_authorized(r: &ResolvedRequest, p: &ProjectResult) -> bool {
        request_header_matches(r, p) && resolved_coverage(r, p)
            && !partial_command(r.args@) && !verification_disabled(r.args@)
            && p.currentness is Current && p.exit_code == Some(0i32) && p.error.is_none()
            && (forall |i: int| 0 <= i < p.targets@.len() && p.targets@[i].requested ==>
                p.targets@[i].complete && p.targets@[i].scope_reuse.is_none()
                    && !partial_command(p.targets@[i].report.command@)
                    && successful_report(&p.targets@[i].report, CompletionPolicy::Whole))
    }

    pub open spec fn small_request_match(r: &ResolvedRequest, p: &ProjectResult) -> bool {
        request_header_matches(r, p) && r.targets@.len() == 1 && p.targets@.len() == 1
            && p.targets@[0].requested
            && string_bytes(&r.targets@[0].package_id) == string_bytes(&p.targets@[0].package_id)
            && string_bytes(&r.targets@[0].target) == string_bytes(&p.targets@[0].target)
            && exists |i: int| 0 <= i < p.targets@[0].kind@.len()
                && string_bytes(&r.targets@[0].kind) == string_bytes(&p.targets@[0].kind@[i])
    }

    pub open spec fn small_positive_request(r: &ResolvedRequest, p: &ProjectResult) -> bool {
        small_request_match(r, p) && !partial_command(r.args@) && !verification_disabled(r.args@)
            && p.currentness is Current && p.exit_code == Some(0i32) && p.error.is_none()
            && p.targets@[0].complete && p.targets@[0].scope_reuse.is_none()
            && !partial_command(p.targets@[0].report.command@)
            && small_positive_report(&p.targets@[0].report, CompletionPolicy::Whole)
    }

    pub proof fn single_target_coverage(es: Seq<RequestedTarget>, ts: Seq<TargetResult>)
        requires es.len() == 1, ts.len() == 1, ts[0].requested, target_match(&es[0], &ts[0]),
        ensures exact_requested_coverage(es, ts),
    {
        reveal(exact_requested_coverage);
        assert(request_covered(&es[0], ts)) by {
            assert(exists |i: int| 0 <= i < ts.len() && target_match(&es[0], &ts[i]) && ts[i].requested
                && forall |j: int| 0 <= j < ts.len() && target_match(&es[0], &ts[j]) ==> j == i);
        }
        assert(target_covered(&ts[0], es)) by {
            assert(exists |i: int| 0 <= i < es.len() && target_match(&es[i], &ts[0])
                && forall |j: int| 0 <= j < es.len() && target_match(&es[j], &ts[0]) ==> j == i);
        }
    }

    pub open spec fn normalized_arguments(args: Seq<String>, end: nat) -> Seq<Seq<u8>>
        recommends end <= args.len()
        decreases end
    {
        if end == 0 { Seq::empty() }
        else if string_bytes(&args[end - 1]) == b"--incremental"@
            && !(exists |i: int| 0 <= i < end - 1 && string_bytes(&args[i]) == b"--"@)
        { normalized_arguments(args, (end - 1) as nat) }
        else { normalized_arguments(args, (end - 1) as nat).push(string_bytes(&args[end - 1])) }
    }

    pub open spec fn repair_function_at(r: &ResolvedRequest, p: &ProjectResult,
        t: int, c: int, f: int) -> bool {
        r.function.is_some() && 0 <= t < p.targets@.len() && p.targets@[t].requested
            && 0 <= c < p.targets@[t].report.crates@.len()
            && 0 <= f < p.targets@[t].report.crates@[c].functions@.len()
            && string_bytes(&r.function.unwrap())
                == string_bytes(&p.targets@[t].report.crates@[c].functions@[f].function)
    }

    pub open spec fn repair_function_present(r: &ResolvedRequest, p: &ProjectResult) -> bool {
        exists |t: int, c: int, f: int| repair_function_at(r, p, t, c, f)
    }

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

    pub open spec fn verification_disabled(args: Seq<String>) -> bool {
        exists |i: int, j: int| 0 <= i < j < args.len()
            && string_bytes(&args[i]) == b"--"@
            && string_bytes(&args[j]) == b"--no-verify"@
    }

    pub open spec fn options_disable_verification(args: Seq<String>) -> bool {
        exists |i: int| 0 <= i < args.len() && string_bytes(&args[i]) == b"--no-verify"@
            && forall |j: int| 0 <= j < i ==> string_bytes(&args[j]) != b"--"@
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

    #[verifier::opaque]
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
            && !verification_disabled(r.command@)
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
    /// Refuses compile-only options in a Cargo command's forwarded Verus arguments.
    /// A successful zero-function document alone does not establish verification.
    pub fn verification_enabled_command(args: &[String]) -> (out: bool)
        requires(true)
        ensures(out == !verification_disabled(args@))
    {
        let mut separator = false;
        let mut i = 0usize;
        sail_while! { (i < args.len())
            invariant(i <= args.len(), !verification_disabled(args@.take(i as int)),
                separator == (exists |j: int| 0 <= j < i && string_bytes(&args@[j]) == b"--"@))
            decreases(args.len() - i)
            {
                let bytes = args[i].as_str().as_bytes();
                if separator && exact_bytes(bytes, b"--no-verify") { return false; }
                if exact_bytes(bytes, b"--") { separator = true; }
                i += 1;
            }
        }
        sail_proof! { assert(args@.take(i as int) =~= args@); }
        true
    }
}

sail_verified! {
    /// Checks decoded effective Verus options before their Rust-argument separator.
    pub fn verification_enabled_options(args: &[String]) -> (out: bool)
        requires(true)
        ensures(out == !options_disable_verification(args@))
    {
        let mut i = 0usize;
        sail_while! { (i < args.len())
            invariant(i <= args.len(),
                forall |j: int| 0 <= j < i ==> string_bytes(&args@[j]) != b"--"@
                    && string_bytes(&args@[j]) != b"--no-verify"@)
            decreases(args.len() - i)
            {
                let bytes = args[i].as_str().as_bytes();
                if exact_bytes(bytes, b"--") {
                    sail_proof! {
                        assert(string_bytes(&args@[i as int]) == b"--"@);
                        reveal(options_disable_verification);
                        if options_disable_verification(args@) {
                            let k = choose |k: int| 0 <= k < args@.len()
                                && string_bytes(&args@[k]) == b"--no-verify"@
                                && forall |j: int| 0 <= j < k ==> string_bytes(&args@[j]) != b"--"@;
                            if k < i { assert(string_bytes(&args@[k]) != b"--no-verify"@); }
                            else if k > i { assert(string_bytes(&args@[i as int]) != b"--"@); }
                            else { assert(string_bytes(&args@[k]).len() == 2); }
                        }
                        assert(!options_disable_verification(args@));
                    }
                    return true;
                }
                if exact_bytes(bytes, b"--no-verify") { return false; }
                i += 1;
            }
        }
        true
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
    /// Compares actual selected engine identities without a caller validity flag.
    pub fn same_engine(a: &Option<EngineIdentity>, b: &Option<EngineIdentity>) -> (out: bool)
        requires(true)
        ensures(out == engine_identity_matches(a, b))
    {
        match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => exact_bytes(x.name.as_str().as_bytes(), y.name.as_str().as_bytes())
                && exact_bytes(x.manifest_sha256.as_str().as_bytes(), y.manifest_sha256.as_str().as_bytes()),
            _ => false,
        }
    }
}

sail_verified! {
    /// Checks only the CLI's documented no-op incremental-flag removal.
    /// Forwarded verifier arguments, including every option after `--`, stay exact.
    pub fn submitted_arguments_match(submitted: &[String], actual: &[String]) -> (out: bool)
        requires(true)
        ensures(out ==> actual@.map_values(|s: String| string_bytes(&s))
            == normalized_arguments(submitted@, submitted@.len()),
            submitted@.len() == 0 && actual@.len() == 0 ==> out)
    {
        let mut i = 0usize;
        let mut j = 0usize;
        let mut forwarded = false;
        sail_while! { (i < submitted.len())
            invariant(i <= submitted.len(), j <= actual.len(),
                actual@.take(j as int).map_values(|s: String| string_bytes(&s))
                    == normalized_arguments(submitted@, i as nat),
                forwarded == (exists |k: int| 0 <= k < i && string_bytes(&submitted@[k]) == b"--"@))
            decreases(submitted.len() - i)
            {
                if forwarded || !exact_bytes(submitted[i].as_str().as_bytes(), b"--incremental") {
                    if j == actual.len() || !exact_bytes(submitted[i].as_str().as_bytes(), actual[j].as_str().as_bytes()) {
                        return false;
                    }
                    j += 1;
                }
                if exact_bytes(submitted[i].as_str().as_bytes(), b"--") { forwarded = true; }
                i += 1;
            }
        }
        j == actual.len()
    }
}

sail_verified! {
    /// Binds an actual captured repair focus to the original exact selection.
    pub fn repair_request_matches(request: &ResolvedRequest, package: &str,
        target: &Option<String>, kind: &str, function: &str) -> (out: bool)
        requires(true)
        ensures(out == repair_selection_matches(request, vstd::utf8::encode_utf8(package@),
            target, vstd::utf8::encode_utf8(kind@), vstd::utf8::encode_utf8(function@)))
    {
        let same_function = match &request.function {
            Some(actual) => exact_bytes(actual.as_str().as_bytes(), function.as_bytes()),
            None => false,
        };
        same_function && exact_bytes(request.repair_package.as_str().as_bytes(), package.as_bytes())
            && exact_bytes(request.repair_kind.as_str().as_bytes(), kind.as_bytes())
            && match (&request.repair_target, target) {
                (None, None) => true,
                (Some(a), Some(b)) => exact_bytes(a.as_str().as_bytes(), b.as_str().as_bytes()),
                _ => false,
            }
    }
}

sail_verified! {
    /// Checks the actual original/output headers before reasoning about coverage.
    pub fn request_headers_match(request: &ResolvedRequest, project: &ProjectResult) -> (out: bool)
        requires(true)
        ensures(out == request_header_matches(request, project))
    {
        if !exact_bytes(project.kind.as_str().as_bytes(), b"sail-project-result")
            || project.schema_version != crate::project::SCHEMA_VERSION
            || request.run_id.is_empty() || request.context_sha256.is_empty() || request.input_revision.is_empty()
            || !exact_bytes(request.run_id.as_str().as_bytes(), project.run_id.as_str().as_bytes())
            || !exact_bytes(request.workspace.as_str().as_bytes(), project.workspace.as_str().as_bytes())
            || !exact_bytes(request.context_sha256.as_str().as_bytes(), project.context_sha256.as_str().as_bytes())
            || !exact_bytes(request.input_revision.as_str().as_bytes(), project.input_revision.as_str().as_bytes())
            || !same_engine(&request.engine, &project.engine) { return false; }
        true
    }
}

sail_verified! {
    /// Checks the explicitly named interactive backend identity in actual fields.
    pub fn interactive_engine_matches(engine: &Option<EngineIdentity>) -> (out: bool)
        requires(true) ensures(out == interactive_engine(engine))
    {
        match engine {
            Some(e) => exact_bytes(e.name.as_str().as_bytes(), b"interactive") && !e.manifest_sha256.is_empty(),
            None => false,
        }
    }
}

sail_verified! {
    /// Derives request correspondence from actual original-mode and request records.
    pub fn classify_submission_mode(mode: &Mode, request: &ResolvedRequest) -> (out: Option<SubmissionKind>)
        requires(true)
        ensures(out.is_some() ==> mode_correspondence(mode, request, out.unwrap()),
            mode is Stock && request.engine.is_none() && request.function.is_none() ==> out.is_some())
    {
        let kind = match mode {
            Mode::Stock => {
                if request.engine.is_some() || request.function.is_some() { return None; }
                SubmissionKind::Stock
            }
            Mode::Interactive => {
                if request.function.is_some() { return None; }
                if request.engine.is_some() {
                    if !interactive_engine_matches(&request.engine) { return None; }
                    SubmissionKind::Interactive
                } else { SubmissionKind::StockForOptIn }
            }
            Mode::Repair(selection) => {
                if repair_request_matches(request, &selection.package, &selection.target, &selection.kind, &selection.function) {
                    if !interactive_engine_matches(&request.engine) { return None; }
                    SubmissionKind::Repair
                } else if request.engine.is_none() && request.function.is_none() {
                    SubmissionKind::StockForOptIn
                } else { return None; }
            }
        };
        Some(kind)
    }
}

sail_verified! {
    /// Binds independent original arguments and mode to the actual captured request.
    #[allow(clippy::question_mark, reason = "Keep explicit refusal branches in the source shared with stock Verus.")]
    pub fn bind_submission<'a>(arguments: &'a [String], mode: &'a Mode,
        request: &'a ResolvedRequest) -> (out: Option<SubmissionBinding<'a>>)
        requires(true)
        ensures(out.is_some() ==> binding_valid(&out.unwrap()),
            out.is_some() ==> bound_arguments(&out.unwrap()) == arguments,
            out.is_some() ==> bound_mode(&out.unwrap()) == mode,
            out.is_some() ==> bound_request(&out.unwrap()) == request,
            arguments@.len() == 0 && request.args@.len() == 0 && mode is Stock
                && request.engine.is_none() && request.function.is_none() ==> out.is_some())
    {
        if !submitted_arguments_match(arguments, &request.args) { return None; }
        let Some(kind) = classify_submission_mode(mode, request) else { return None; };
        Some(SubmissionBinding { arguments, mode, request, kind })
    }
}

sail_verified! {
    /// Establishes whole authority for the actual independent original submission.
    #[allow(clippy::question_mark, reason = "Keep explicit refusal branches in the source shared with stock Verus.")]
    pub fn complete_submitted_request_ordered<'a>(arguments: &'a [String], mode: &'a Mode,
        request: &'a ResolvedRequest, project: &'a ProjectResult, orders: &[ReportOrder])
        -> (out: Option<SubmittedAuthority<'a>>)
        requires(true)
        ensures(out.is_some() ==> request_authorized(request, project),
            out.is_some() ==> submission_correspondence(arguments@, mode, request, submitted_kind(&out.unwrap())),
            out.is_some() ==> submitted_arguments(&out.unwrap()) == arguments,
            out.is_some() ==> submitted_mode(&out.unwrap()) == mode,
            out.is_some() ==> submitted_request(&out.unwrap()) == request,
            out.is_some() ==> submitted_project(&out.unwrap()) == project,
            arguments@.len() == 0 && request.args@.len() == 0 && mode is Stock
                && request.engine.is_none() && request.function.is_none()
                && small_positive_request(request, project) && orders@.len() == 1
                && orders@[0].forward@.len() == 0 && orders@[0].inverse@.len() == 0 ==> out.is_some())
    {
        let Some(binding) = bind_submission(arguments, mode, request) else { return None; };
        if complete_request_ordered(request, project, orders).is_none() { return None; }
        Some(SubmittedAuthority { arguments, mode, request, project, kind: binding.kind })
    }
}

sail_verified! {
    /// Checks the retained pre-execution identity and independently resolved scope.
    /// A matching failed or compile-only observation is not positive proof authority.
    pub fn request_corresponds(request: &ResolvedRequest, project: &ProjectResult) -> (out: bool)
        requires(true)
        ensures(out ==> request_header_matches(request, project) && resolved_coverage(request, project),
            small_request_match(request, project) ==> out)
    {
        if !request_headers_match(request, project) { return false; }
        let mut expected: Vec<RequestedTarget> = Vec::new();
        let mut i = 0usize;
        sail_while! { (i < request.targets.len())
            invariant(i <= request.targets.len(), expected.len() == i,
                forall |j: int| 0 <= j < i ==>
                    expected@[j].package@ == string_bytes(&request.targets@[j].package_id)
                        && expected@[j].name@ == string_bytes(&request.targets@[j].target)
                        && expected@[j].kind@ == string_bytes(&request.targets@[j].kind))
            decreases(request.targets.len() - i)
            {
                let target = &request.targets[i];
                expected.push(requested_target(&target.package_id, &target.target, &target.kind));
                i += 1;
            }
        }
        sail_proof! {
            if small_request_match(request, project) {
                assert(expected@.len() == 1);
                assert(target_match(&expected@[0], &project.targets@[0]));
                single_target_coverage(expected@, project.targets@);
            }
        }
        if !requested_coverage(&expected, &project.targets) { return false; }
        sail_proof! {
            assert(exact_requested_coverage(expected@, project.targets@));
            reveal(exact_requested_coverage);
            assert(resolved_expected(request, expected@));
            assert(resolved_coverage(request, project));
        }
        true
    }
}

sail_verified! {
    /// Finds the original repair function in actual requested full-report records.
    /// This presence check alone confers no verdict or Currentness authority.
    pub fn requested_function_present(request: &ResolvedRequest, project: &ProjectResult) -> (out: bool)
        requires(true)
        ensures(out == repair_function_present(request, project))
    {
        let Some(name) = &request.function else { return false; };
        let mut t = 0usize;
        sail_while! { (t < project.targets.len())
            invariant(t <= project.targets.len(), request.function.is_some(),
                string_bytes(name) == string_bytes(&request.function.unwrap()),
                forall |a: int, b: int, d: int| 0 <= a < t ==> !repair_function_at(request, project, a, b, d))
            decreases(project.targets.len() - t)
            {
                if project.targets[t].requested {
                    let mut c = 0usize;
                    sail_while! { (c < project.targets[t].report.crates.len())
                        invariant(t < project.targets.len(), project.targets@[t as int].requested,
                            request.function.is_some(), string_bytes(name) == string_bytes(&request.function.unwrap()),
                            c <= project.targets@[t as int].report.crates@.len(),
                            forall |b: int, d: int| 0 <= b < c ==> !repair_function_at(request, project, t as int, b, d))
                        decreases(project.targets@[t as int].report.crates@.len() - c)
                        {
                            let mut f = 0usize;
                            sail_while! { (f < project.targets[t].report.crates[c].functions.len())
                                invariant(t < project.targets.len(), project.targets@[t as int].requested,
                                    request.function.is_some(), string_bytes(name) == string_bytes(&request.function.unwrap()),
                                    c < project.targets@[t as int].report.crates@.len(),
                                    f <= project.targets@[t as int].report.crates@[c as int].functions@.len(),
                                    forall |d: int| 0 <= d < f ==> !repair_function_at(request, project, t as int, c as int, d))
                                decreases(project.targets@[t as int].report.crates@[c as int].functions@.len() - f)
                                {
                                    if exact_bytes(name.as_str().as_bytes(), project.targets[t].report.crates[c].functions[f].function.as_str().as_bytes()) {
                                        sail_proof! { assert(repair_function_at(request, project, t as int, c as int, f as int)); }
                                        return true;
                                    }
                                    f += 1;
                                }
                            }
                            c += 1;
                        }
                    }
                }
                t += 1;
            }
        }
        false
    }
}

sail_verified! {
    /// Mints Current whole-request authority from an independent request and actual reports.
    pub fn complete_request_ordered<'a>(request: &'a ResolvedRequest, project: &'a ProjectResult,
        orders: &[ReportOrder]) -> (out: Option<CompleteRequest<'a>>)
        requires(true)
        ensures(out.is_some() ==> request_authorized(request, project),
            out.is_some() ==> request_authority_request(&out.unwrap()) == request,
            out.is_some() ==> request_authority_project(&out.unwrap()) == project,
            small_positive_request(request, project) && orders@.len() == 1
                && orders@[0].forward@.len() == 0 && orders@[0].inverse@.len() == 0 ==> out.is_some())
    {
        if !request_corresponds(request, project) || focused_command(&request.args)
            || !verification_enabled_command(&request.args)
            || !matches!(project.currentness, Currentness::Current)
            || project.exit_code != Some(0) || project.error.is_some()
            || orders.len() != project.targets.len() { return None; }
        let mut i = 0usize;
        sail_while! { (i < project.targets.len())
            invariant(i <= project.targets.len(), orders.len() == project.targets.len(),
                forall |j: int| 0 <= j < i && project.targets@[j].requested ==>
                    project.targets@[j].complete && project.targets@[j].scope_reuse.is_none()
                        && !partial_command(project.targets@[j].report.command@)
                        && successful_report(&project.targets@[j].report, CompletionPolicy::Whole))
            decreases(project.targets.len() - i)
            {
                if project.targets[i].requested && complete_target_ordered(&project.targets[i],
                    &orders[i].forward, &orders[i].inverse).is_none() { return None; }
                i += 1;
            }
        }
        Some(CompleteRequest { request, project })
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
            out.is_some() ==> authority_report(&out.unwrap()) == &target.report,
            target.complete && target.scope_reuse.is_none() && !partial_command(target.report.command@)
                && small_positive_report(&target.report, CompletionPolicy::Whole)
                && forward@.len() == 0 && inverse@.len() == 0 ==> out.is_some())
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
            out.is_some() ==> authority_report(&out.unwrap()) == report,
            !partial_command(report.command@) && small_positive_report(report, CompletionPolicy::Whole)
                && forward@.len() == 0 && inverse@.len() == 0 ==> out.is_some())
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
        ensures(out.is_some() == request_covered(e, ts@))
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
        ensures(out.is_some() == target_covered(t, es@))
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
        ensures(out == exact_requested_coverage(expected@, targets@))
    {
        sail_proof! { reveal(exact_requested_coverage); }
        if expected.is_empty() { return false; }
        let mut i = 0usize;
        sail_while! { (i < expected.len())
            invariant(i <= expected.len(),
                forall |j: int| 0 <= j < i ==> request_covered(&expected@[j], targets@))
            decreases(expected.len() - i)
            {
                sail_proof! { reveal(exact_requested_coverage); }
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
                sail_proof! { reveal(exact_requested_coverage); }
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
        if !verification_enabled_command(&report.command) { return false; }
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
                    assert forall |a: int, b: int| 0 <= a < i + 1
                        && 0 <= b < report.crates@.take(i as int + 1)[a].functions@.len()
                        implies #[trigger] owned_function_ids(&report.crates@.take(i as int + 1)[a].functions@[b]) by {
                        if a < i {
                            assert(report.crates@.take(i as int)[a] == report.crates@[a]);
                            assert(all_owned_ids(report.crates@.take(i as int)));
                            assert(owned_function_ids(&report.crates@.take(i as int)[a].functions@[b]));
                        } else {
                            assert(a == i && c == &report.crates@[a]);
                            assert(b < c.functions@.len() && j == c.functions@.len());
                            assert(owned_function_ids(&c.functions@[b]));
                        }
                    }
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

sail_verified! {
    /// Binds live report input to sealed process observations, preserving fresh
    /// output while recording missing history as retained-authority refusal.
    pub fn require_process_observation(report: &mut Report, observed: Option<&ObservedAttempt>,
        stdout: &[u8], stderr: &[u8], exit: Option<i32>, unavailable: ErrorInfo) -> (out: bool)
        requires(observed.is_some() ==> sail_acceptance::authority::observation_complete(
            &sail_acceptance::authority::observed_running(observed.unwrap())))
        ensures(out == (observed.is_some()
            && sail_acceptance::authority::running_stdout(&sail_acceptance::authority::observed_running(observed.unwrap())).unwrap()@ == stdout@
            && sail_acceptance::authority::running_stderr(&sail_acceptance::authority::observed_running(observed.unwrap())).unwrap()@ == stderr@
            && sail_acceptance::authority::running_exit(&sail_acceptance::authority::observed_running(observed.unwrap())).unwrap() == exit),
            final(report).error.is_some() == (old(report).error.is_some() || !out),
            old(report).error.is_some() ==> final(report).error == old(report).error,
            final(report).command == old(report).command, final(report).schema_version == old(report).schema_version,
            final(report).exit_code == old(report).exit_code, final(report).summary == old(report).summary,
            final(report).crates == old(report).crates, final(report).engine == old(report).engine)
    {
        let matched = match observed {
            Some(observed) => observed_payload_matches(observed, stdout, stderr, exit),
            None => false,
        };
        if !matched && report.error.is_none() { report.error = Some(unavailable); }
        matched
    }
}

sail_spec! {
    pub open spec fn driver_delimiter(raw: Seq<u8>, at: int) -> bool {
        0 <= at && at + b"__VERUS_DRIVER_ARGS_SEP__"@.len() <= raw.len()
            && raw.subrange(at, at + b"__VERUS_DRIVER_ARGS_SEP__"@.len() as int) == b"__VERUS_DRIVER_ARGS_SEP__"@
    }

    pub open spec fn first_driver_delimiter(raw: Seq<u8>, start: int, at: int) -> bool {
        start <= at && driver_delimiter(raw, at)
            && forall |i: int| start <= i < at ==> !driver_delimiter(raw, i)
    }

    pub open spec fn driver_token_span(raw: Seq<u8>, token: &String, delimiter: int, end: int) -> bool {
        let start = delimiter + b"__VERUS_DRIVER_ARGS_SEP__"@.len();
        driver_delimiter(raw, delimiter) && start <= end <= raw.len()
            && string_bytes(token) == raw.subrange(start, end)
            && forall |i: int| start <= i < end ==> !driver_delimiter(raw, i)
    }

    pub open spec fn driver_token_bounds(raw: Seq<u8>, tokens: Seq<String>, count: int, end: int, bounds: Seq<int>) -> bool {
        0 <= count <= tokens.len() && bounds.len() == count + 1
            && first_driver_delimiter(raw, 0, bounds[0]) && bounds.last() == end
            && forall |i: int| 0 <= i < count ==> driver_token_span(raw, &tokens[i], bounds[i], bounds[i + 1])
    }

    pub open spec fn driver_token_prefix(raw: Seq<u8>, tokens: Seq<String>, count: int, end: int) -> bool {
        exists |bounds: Seq<int>| driver_token_bounds(raw, tokens, count, end, bounds)
    }

    /// Exact non-overlapping delimiter semantics of the pinned driver's split/skip.
    pub open spec fn packed_driver_tokens(raw: Seq<u8>, tokens: Seq<String>) -> bool {
        if tokens.len() == 0 {
            forall |i: int| 0 <= i <= raw.len() ==> !driver_delimiter(raw, i)
        } else {
            driver_token_prefix(raw, tokens, tokens.len() as int, raw.len() as int)
        }
    }

    pub open spec fn driver_source_tokens(source: &Option<String>, tokens: Seq<String>) -> bool {
        match source { None => tokens.len() == 0, Some(value) => packed_driver_tokens(string_bytes(value), tokens) }
    }

    pub open spec fn driver_option_sources(common: &Option<String>, package: &Option<String>,
        arguments: Seq<String>, boundary: int) -> bool {
        0 <= boundary <= arguments.len() && driver_source_tokens(common, arguments.take(boundary))
            && driver_source_tokens(package, arguments.skip(boundary))
    }
}

sail_verified! {
    /// Recognizes the exact packed-driver separator at a bounded byte offset.
    pub fn driver_delimiter_at(raw: &[u8], at: usize) -> (out: bool)
        requires(true)
        ensures(out == driver_delimiter(raw@, at as int))
    {
        let delimiter: &[u8] = b"__VERUS_DRIVER_ARGS_SEP__";
        if at > raw.len() || raw.len() - at < delimiter.len() { return false; }
        exact_bytes(&raw[at..at + delimiter.len()], delimiter)
    }
}

sail_verified! {
    /// Finds the first separator, proving that no effective token was skipped.
    pub fn next_driver_delimiter(raw: &[u8], start: usize) -> (out: Option<usize>)
        requires(start <= raw@.len())
        ensures(out.is_some() ==> first_driver_delimiter(raw@, start as int, out.unwrap() as int),
            out.is_none() ==> forall |i: int| start <= i <= raw@.len() ==> !driver_delimiter(raw@, i))
    {
        let mut at = start;
        sail_while! { (at < raw.len())
            invariant(start <= at <= raw.len(),
                forall |i: int| start <= i < at ==> !driver_delimiter(raw@, i))
            decreases(raw.len() - at)
            {
                if driver_delimiter_at(raw, at) { return Some(at); }
                at += 1;
            }
        }
        None
    }
}

sail_verified! {
    /// Validates decoded driver-token candidates against the exact original value.
    /// The ignored leading prefix, empty tokens and trailing token are preserved.
    pub fn packed_driver_arguments_match(value: &str, tokens: &[String]) -> (out: bool)
        requires(true)
        ensures(out ==> packed_driver_tokens(vstd::utf8::encode_utf8(value@), tokens@),
            vstd::utf8::encode_utf8(value@).len() < b"__VERUS_DRIVER_ARGS_SEP__"@.len()
                && tokens@.len() == 0 ==> out,
            vstd::utf8::encode_utf8(value@) == b"__VERUS_DRIVER_ARGS_SEP__"@
                && tokens@.len() == 1 && string_bytes(&tokens@[0]).len() == 0 ==> out)
    {
        let raw = value.as_bytes();
        let raw_length = raw.len();
        let delimiter: &[u8] = b"__VERUS_DRIVER_ARGS_SEP__";
        let Some(first) = next_driver_delimiter(raw, 0) else {
            sail_proof! { if raw@ == delimiter@ { assert(driver_delimiter(raw@, 0)); } }
            return tokens.is_empty();
        };
        let mut at = first;
        let mut index = 0usize;
        sail_proof! { assert(driver_token_prefix(raw@, tokens@, 0, first as int)) by {
            let bounds = seq![first as int];
            assert(bounds.len() == 1 && bounds.last() == first);
            assert(driver_token_bounds(raw@, tokens@, 0, first as int, bounds));
        } }
        sail_while! { (index < tokens.len())
            invariant(index <= tokens.len(), driver_delimiter(raw@, at as int),
                raw@ == vstd::utf8::encode_utf8(value@), delimiter@ == b"__VERUS_DRIVER_ARGS_SEP__"@, delimiter.len() == 25,
                raw@.len() == raw_length,
                driver_token_prefix(raw@, tokens@, index as int, at as int),
                raw@ == delimiter@ && tokens@.len() == 1 && string_bytes(&tokens@[0]).len() == 0 ==> index == 0)
            decreases(tokens.len() - index)
            {
                sail_proof! { assert(at as int + delimiter@.len() <= raw@.len()); }
                let start = at + delimiter.len();
                let next = next_driver_delimiter(raw, start);
                let end = match next { Some(found) => found, None => raw_length };
                sail_proof! {
                    if raw@ == delimiter@ && tokens@.len() == 1 && string_bytes(&tokens@[0]).len() == 0 {
                        assert(at == 0 && start == raw@.len() && end == raw@.len());
                        assert(raw@.subrange(start as int, end as int) =~= Seq::<u8>::empty());
                        assert(string_bytes(&tokens@[index as int]) =~= Seq::<u8>::empty());
                    }
                }
                if !exact_bytes(tokens[index].as_str().as_bytes(), &raw[start..end]) { return false; }
                sail_proof! {
                    let before = choose |bounds: Seq<int>| driver_token_bounds(raw@, tokens@, index as int, at as int, bounds);
                    let bounds = before.push(end as int);
                    assert forall |i: int| 0 <= i < index + 1
                        implies driver_token_span(raw@, &tokens@[i], bounds[i], bounds[i + 1]) by {
                        if i == index { assert(driver_token_span(raw@, &tokens@[i], bounds[i], bounds[i + 1])); }
                    }
                    assert(driver_token_bounds(raw@, tokens@, index as int + 1, end as int, bounds));
                    assert(driver_token_prefix(raw@, tokens@, index as int + 1, end as int));
                }
                index += 1;
                match next {
                    None => {
                        if index != tokens.len() { return false; }
                        return true;
                    }
                    Some(found) => { at = found; }
                }
            }
        }
        false
    }
}

sail_verified! {
    /// Validates all effective tokens in common-then-current-package order.
    /// The proposed split index is untrusted, including when a source is absent.
    pub fn effective_driver_arguments_match(common: &Option<String>, package: &Option<String>,
        boundary: usize, arguments: &[String]) -> (out: bool)
        requires(true)
        ensures(out ==> driver_option_sources(common, package, arguments@, boundary as int),
            common.is_none() && package.is_none() && boundary == 0 && arguments@.len() == 0 ==> out)
    {
        if boundary > arguments.len() { return false; }
        let first = &arguments[..boundary];
        let second = &arguments[boundary..];
        match common {
            None => { if !first.is_empty() { return false; } }
            Some(value) => { if !packed_driver_arguments_match(value.as_str(), first) { return false; } }
        }
        match package {
            None => { if !second.is_empty() { return false; } }
            Some(value) => { if !packed_driver_arguments_match(value.as_str(), second) { return false; } }
        }
        true
    }
}

sail_spec! {
    /// Exact retained target data, allowing only loss of its completion label.
    pub open spec fn same_target_except_completion(a: &TargetResult, b: &TargetResult) -> bool {
        a.package_id == b.package_id && a.target == b.target && a.kind == b.kind
            && a.establishing_run == b.establishing_run && a.reused == b.reused
            && a.requested == b.requested && a.scope_reuse == b.scope_reuse && a.report == b.report
    }

    /// All captured project data except freshness and completion labels stays fixed.
    pub open spec fn same_project_except_freshness_and_completion(a: &ProjectResult, b: &ProjectResult) -> bool {
        a.kind == b.kind && a.schema_version == b.schema_version && a.run_id == b.run_id
            && a.workspace == b.workspace && a.context_sha256 == b.context_sha256
            && a.input_revision == b.input_revision
            && a.exit_code == b.exit_code && a.error == b.error && a.engine == b.engine
            && a.targets@.len() == b.targets@.len()
            && forall |i: int| 0 <= i < a.targets@.len() ==>
                same_target_except_completion(&a.targets@[i], &b.targets@[i])
    }

    /// Request refusal additionally preserves the original freshness observation.
    pub open spec fn same_project_except_completion(a: &ProjectResult, b: &ProjectResult) -> bool {
        same_project_except_freshness_and_completion(a, b) && a.currentness == b.currentness
    }
}

sail_verified! {
    /// Rechecks original request authority before retained completion can be shown.
    /// Refusal clears labels while preserving the recorded reports and currentness.
    pub fn withhold_unbound_request_authority_ordered(project: &mut ProjectResult,
        request: Option<&ResolvedRequest>, orders: &[ReportOrder]) -> (corresponding: bool)
        requires(true)
        ensures(same_project_except_completion(old(project), final(project)),
            forall |i: int| 0 <= i < final(project).targets@.len() ==>
                final(project).targets@[i].complete ==> old(project).targets@[i].complete,
            final(project).currentness is Current && final(project).exit_code == Some(0i32)
                && (exists |i: int| 0 <= i < final(project).targets@.len()
                    && final(project).targets@[i].complete) ==>
                request.is_some() && request_authorized(request.unwrap(), final(project)),
            request.is_none() ==> !corresponding,
            request.is_some() && small_positive_request(request.unwrap(), old(project))
                && orders@.len() == 1 && orders@[0].forward@.len() == 0
                && orders@[0].inverse@.len() == 0 ==>
                corresponding && *final(project) == *old(project))
    {
        let corresponding = match request {
            Some(request) => request_corresponds(request, project),
            None => false,
        };
        let admitted = match request {
            Some(request) => corresponding
                && verification_enabled_command(&request.args)
                && !focused_command(&request.args)
                && (!matches!(project.currentness, Currentness::Current)
                    || project.exit_code != Some(0)
                    || complete_request_ordered(request, project, orders).is_some()),
            None => false,
        };
        if admitted {
            return corresponding;
        }
        let mut i = 0usize;
        sail_while! { (i < project.targets.len())
            invariant(i <= project.targets.len(),
                same_project_except_completion(old(project), project),
                forall |j: int| 0 <= j < i ==> !project.targets@[j].complete,
                forall |j: int| i <= j < project.targets@.len() ==>
                    project.targets@[j].complete == old(project).targets@[j].complete)
            decreases(project.targets.len() - i)
            {
                project.targets[i].complete = false;
                i += 1;
            }
        }
        corresponding
    }
}

sail_spec! {
    pub closed spec fn replay_readback_request<'a, 'b>(r: &ReplayReadback<'a, 'b>) -> &'a ResolvedRequest { r.request }
    pub closed spec fn replay_readback_observation<'a, 'b>(r: &ReplayReadback<'a, 'b>) -> &'a sail_acceptance::inputs::GuardedReadback<'b> { r.readback }
    pub open spec fn replay_readback_valid(r: &ReplayReadback) -> bool {
        let readback = replay_readback_observation(r);
        readback.unchanged_view() && readback.guard_view().is_some()
            && readback.guard_view().unwrap().revision_view() == string_bytes(&replay_readback_request(r).input_revision)
    }

    pub open spec fn replay_optional_text(a: &Option<String>, b: &Option<String>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => string_bytes(x) == string_bytes(y),
            _ => false,
        }
    }
    pub open spec fn replay_target_inventory(a: Seq<crate::project::RequestTarget>, b: Seq<crate::project::RequestTarget>) -> bool {
        a.len() == b.len() && forall |i: int| 0 <= i < a.len() ==>
            string_bytes(&a[i].package_id) == string_bytes(&b[i].package_id)
                && string_bytes(&a[i].target) == string_bytes(&b[i].target)
                && string_bytes(&a[i].kind) == string_bytes(&b[i].kind)
    }
    /// Exact resolved execution request except the separately allocated view ID.
    pub open spec fn replay_request_payload(current: &ResolvedRequest, original: &ResolvedRequest) -> bool {
        string_bytes(&current.run_id).len() > 0 && string_bytes(&original.run_id).len() > 0
            && string_bytes(&current.run_id) != string_bytes(&original.run_id)
            && string_bytes(&current.invocation) == string_bytes(&original.invocation)
            && string_bytes(&current.workspace) == string_bytes(&original.workspace)
            && string_bytes(&current.context_sha256) == string_bytes(&original.context_sha256)
            && string_bytes(&current.input_revision) == string_bytes(&original.input_revision)
            && current.args@.len() == original.args@.len()
            && current.args@.map_values(|s: String| string_bytes(&s)) == original.args@.map_values(|s: String| string_bytes(&s))
            && engine_identity_matches(&current.engine, &original.engine)
            && replay_optional_text(&current.function, &original.function)
            && string_bytes(&current.repair_package) == string_bytes(&original.repair_package)
            && replay_optional_text(&current.repair_target, &original.repair_target)
            && string_bytes(&current.repair_kind) == string_bytes(&original.repair_kind)
            && replay_target_inventory(current.targets@, original.targets@)
    }
    /// The replay changes only the current view identity in captured proof fields.
    pub open spec fn same_replay_evidence(a: &ProjectResult, b: &ProjectResult) -> bool {
        a.kind == b.kind && a.schema_version == b.schema_version
            && a.workspace == b.workspace && a.context_sha256 == b.context_sha256
            && a.input_revision == b.input_revision && a.currentness == b.currentness
            && a.exit_code == b.exit_code && a.targets == b.targets
            && a.error == b.error && a.engine == b.engine
    }
}

sail_verified! {
    /// Compares optional original request text, preserving absence versus empty.
    pub fn same_replay_optional_text(a: &Option<String>, b: &Option<String>) -> (out: bool)
        requires(true) ensures(out == replay_optional_text(a, b))
    {
        match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => exact_bytes(x.as_str().as_bytes(), y.as_str().as_bytes()),
            _ => false,
        }
    }
}

sail_verified! {
    /// Compares complete ordered requested targets using their exact raw fields.
    pub fn same_replay_targets(a: &[crate::project::RequestTarget], b: &[crate::project::RequestTarget]) -> (out: bool)
        requires(true) ensures(out == replay_target_inventory(a@, b@))
    {
        if a.len() != b.len() { return false; }
        let mut i = 0usize;
        sail_while! { (i < a.len())
            invariant(i <= a.len(), a.len() == b.len(),
                forall |j: int| 0 <= j < i ==> string_bytes(&a@[j].package_id) == string_bytes(&b@[j].package_id)
                    && string_bytes(&a@[j].target) == string_bytes(&b@[j].target)
                    && string_bytes(&a@[j].kind) == string_bytes(&b@[j].kind))
            decreases(a.len() - i)
            {
                if !exact_bytes(a[i].package_id.as_str().as_bytes(), b[i].package_id.as_str().as_bytes())
                    || !exact_bytes(a[i].target.as_str().as_bytes(), b[i].target.as_str().as_bytes())
                    || !exact_bytes(a[i].kind.as_str().as_bytes(), b[i].kind.as_str().as_bytes()) { return false; }
                i += 1;
            }
        }
        true
    }
}

sail_verified! {
    /// Compares original request argument bytes without normalization or flag removal.
    pub fn same_replay_arguments(a: &[String], b: &[String]) -> (out: bool)
        requires(true)
        ensures(out == (a@.map_values(|s: String| string_bytes(&s)) == b@.map_values(|s: String| string_bytes(&s))),
            out ==> a@.len() == b@.len(),
            out ==> partial_command(a@) == partial_command(b@),
            out ==> verification_disabled(a@) == verification_disabled(b@))
    {
        if a.len() != b.len() { return false; }
        let mut i = 0usize;
        sail_while! { (i < a.len())
            invariant(i <= a.len(), a.len() == b.len(),
                forall |j: int| 0 <= j < i ==> string_bytes(&a@[j]) == string_bytes(&b@[j]))
            decreases(a.len() - i)
            {
                sail_proof! {
                    assert(a@.map_values(|s: String| string_bytes(&s))[i as int] == string_bytes(&a@[i as int]));
                    assert(b@.map_values(|s: String| string_bytes(&s))[i as int] == string_bytes(&b@[i as int]));
                }
                if !exact_bytes(a[i].as_str().as_bytes(), b[i].as_str().as_bytes()) { return false; }
                i += 1;
            }
        }
        sail_proof! { assert(a@.map_values(|s: String| string_bytes(&s)) =~= b@.map_values(|s: String| string_bytes(&s))); }
        true
    }
}

sail_verified! {
    /// Matches an independently supplied current request against the retained
    /// original before either result or run ID can supply its expected scope.
    pub fn replay_requests_match(current: &ResolvedRequest, original: &ResolvedRequest) -> (out: bool)
        requires(true) ensures(out == replay_request_payload(current, original))
    {
        !current.run_id.is_empty() && !original.run_id.is_empty()
            && !exact_bytes(current.run_id.as_str().as_bytes(), original.run_id.as_str().as_bytes())
            && exact_bytes(current.invocation.as_str().as_bytes(), original.invocation.as_str().as_bytes())
            && exact_bytes(current.workspace.as_str().as_bytes(), original.workspace.as_str().as_bytes())
            && exact_bytes(current.context_sha256.as_str().as_bytes(), original.context_sha256.as_str().as_bytes())
            && exact_bytes(current.input_revision.as_str().as_bytes(), original.input_revision.as_str().as_bytes())
            && same_replay_arguments(&current.args, &original.args)
            && same_engine(&current.engine, &original.engine)
            && same_replay_optional_text(&current.function, &original.function)
            && exact_bytes(current.repair_package.as_str().as_bytes(), original.repair_package.as_str().as_bytes())
            && same_replay_optional_text(&current.repair_target, &original.repair_target)
            && exact_bytes(current.repair_kind.as_str().as_bytes(), original.repair_kind.as_str().as_bytes())
            && same_replay_targets(&current.targets, &original.targets)
    }
}

sail_verified! {
    /// Seals a request-linked observation only from its actual guarded readback
    /// and exact original bound revision. This is not whole-proof authority.
    pub fn seal_replay_readback<'a, 'b>(request: &'a ResolvedRequest,
        readback: &'a sail_acceptance::inputs::GuardedReadback<'b>) -> (out: Option<ReplayReadback<'a, 'b>>)
        requires(true)
        ensures(out.is_some() == (readback.unchanged_view() && readback.guard_view().is_some()
                && readback.guard_view().unwrap().revision_view() == string_bytes(&request.input_revision)),
            out.is_some() ==> replay_readback_valid(&out.unwrap()),
            out.is_some() ==> replay_readback_request(&out.unwrap()) == request,
            out.is_some() ==> replay_readback_observation(&out.unwrap()) == readback)
    {
        if !readback.matches_revision(request.input_revision.as_str().as_bytes()) { return None; }
        Some(ReplayReadback { request, readback })
    }
}

sail_spec! {
    pub proof fn replay_request_preserves_modes(current: &ResolvedRequest, original: &ResolvedRequest)
        requires replay_request_payload(current, original),
        ensures partial_command(current.args@) == partial_command(original.args@),
            verification_disabled(current.args@) == verification_disabled(original.args@),
    {
        assert(current.args@.len() == original.args@.len());
        assert forall |i: int| 0 <= i < current.args@.len() implies
            string_bytes(&current.args@[i]) == string_bytes(&original.args@[i]) by {
            assert(current.args@.map_values(|s: String| string_bytes(&s))[i]
                == original.args@.map_values(|s: String| string_bytes(&s))[i]);
        }
    }

    pub proof fn replay_request_preserves_coverage(current: &ResolvedRequest, original: &ResolvedRequest, project: &ProjectResult)
        requires replay_request_payload(current, original), resolved_coverage(original, project),
        ensures resolved_coverage(current, project),
    {
        let es = choose |es: Seq<RequestedTarget>| resolved_expected(original, es)
            && exact_requested_coverage(es, project.targets@);
        assert forall |i: int| 0 <= i < es.len() implies
            es[i].package@ == string_bytes(&current.targets@[i].package_id)
                && es[i].name@ == string_bytes(&current.targets@[i].target)
                && es[i].kind@ == string_bytes(&current.targets@[i].kind) by {
            assert(string_bytes(&original.targets@[i].package_id) == string_bytes(&current.targets@[i].package_id));
        }
        assert(resolved_expected(current, es));
    }

    // Keep the constructive premise opaque at callers so a broken always-false
    // implementation produces an ordinary counterexample without searching the
    // nested report representation. Its exact original meaning is unchanged.
    #[verifier::opaque]
    pub open spec fn replay_constructive_request(original: &ResolvedRequest, project: &ProjectResult,
        current: &ResolvedRequest, orders: &[ReportOrder], readback: &sail_acceptance::inputs::GuardedReadback) -> bool {
        small_positive_request(original, project) && replay_request_payload(current, original)
            && readback.unchanged_view() && readback.guard_view().is_some()
            && readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision)
            && orders@.len() == 1 && orders@[0].forward@.len() == 0 && orders@[0].inverse@.len() == 0
    }

    pub proof fn replay_constructive_request_definition(original: &ResolvedRequest, project: &ProjectResult,
        current: &ResolvedRequest, orders: &[ReportOrder], readback: &sail_acceptance::inputs::GuardedReadback)
        ensures replay_constructive_request(original, project, current, orders, readback)
            == (small_positive_request(original, project) && replay_request_payload(current, original)
                && readback.unchanged_view() && readback.guard_view().is_some()
                && readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision)
                && orders@.len() == 1 && orders@[0].forward@.len() == 0 && orders@[0].inverse@.len() == 0),
    { reveal(replay_constructive_request); }
}

sail_verified! {
    /// Binds a retained whole proof to an independent current request and actual
    /// typed guarded readback. It changes only the view ID; reports and original
    /// establishing identities remain intact. Native request production and
    /// complete physical compiler-input observation remain separate obligations.
    pub fn bind_replayed_project_ordered(project: &mut ProjectResult, original: &ResolvedRequest,
        current: &ResolvedRequest, orders: &[ReportOrder], readback: &sail_acceptance::inputs::GuardedReadback)
        -> (accepted: bool)
        requires(true)
        ensures(same_replay_evidence(old(project), final(project)),
            accepted ==> replay_request_payload(current, original),
            accepted ==> request_authorized(original, old(project)),
            accepted ==> request_authorized(current, final(project)),
            accepted ==> readback.unchanged_view() && readback.guard_view().is_some()
                && readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision),
            accepted ==> string_bytes(&final(project).run_id) == string_bytes(&current.run_id),
            !accepted ==> *final(project) == *old(project),
            replay_constructive_request(original, old(project), current, orders, readback) ==> accepted)
    {
        sail_proof! { reveal(replay_constructive_request); }
        if !replay_requests_match(current, original) { return false; }
        sail_proof! { replay_request_preserves_modes(current, original); }
        let Some(sealed) = seal_replay_readback(current, readback) else { return false; };
        let current = sealed.request();
        if !verification_enabled_command(&current.args) || focused_command(&current.args)
            || complete_request_ordered(original, project, orders).is_none() { return false; }
        sail_proof! {
            assert(request_authorized(original, project));
            replay_request_preserves_coverage(current, original, project);
        }
        let current_run = current.run_id.clone();
        project.run_id = current_run;
        sail_proof! {
            assert(request_header_matches(current, project));
            assert(request_authorized(current, project));
        }
        true
    }
}

sail_spec! {
    pub closed spec fn exact_scope_report<'a>(scope: &ExactScopeReport<'a>) -> &'a Report { scope.report }
    pub closed spec fn exact_scope_arguments<'a>(scope: &ExactScopeReport<'a>) -> &'a [String] { scope.arguments }

    /// Exact recorded partial scope and successful receipt-policy evidence.
    /// Neither an absent nor a true whole-crate summary widens this scope.
    pub open spec fn exact_scope_evidence(report: &Report, arguments: Seq<String>) -> bool {
        partial_command(arguments) && !verification_disabled(arguments)
            && arguments.len() == report.command@.len()
            && arguments.map_values(|s: String| string_bytes(&s))
                == report.command@.map_values(|s: String| string_bytes(&s))
            && partial_command(report.command@) && report.engine.is_none()
            && successful_report(report, CompletionPolicy::Receipt)
    }

    pub open spec fn exact_scope_authority_valid(scope: &ExactScopeReport) -> bool {
        exact_scope_evidence(exact_scope_report(scope), exact_scope_arguments(scope)@)
    }

    // An opaque exact definition keeps an always-refuse mutation's
    // counterexample independent of nested report representation search.
    #[verifier::opaque]
    pub open spec fn small_exact_scope_candidate(report: &Report, arguments: Seq<String>,
        forward: Seq<usize>, inverse: Seq<usize>) -> bool {
        partial_command(arguments) && arguments.len() == report.command@.len()
            && arguments.map_values(|s: String| string_bytes(&s))
                == report.command@.map_values(|s: String| string_bytes(&s))
            && report.engine.is_none() && small_positive_report(report, CompletionPolicy::Receipt)
            && forward.len() == 0 && inverse.len() == 0
    }

    pub proof fn small_exact_scope_candidate_definition(report: &Report, arguments: Seq<String>,
        forward: Seq<usize>, inverse: Seq<usize>)
        ensures small_exact_scope_candidate(report, arguments, forward, inverse)
            == (partial_command(arguments) && arguments.len() == report.command@.len()
                && arguments.map_values(|s: String| string_bytes(&s))
                    == report.command@.map_values(|s: String| string_bytes(&s))
                && report.engine.is_none() && small_positive_report(report, CompletionPolicy::Receipt)
                && forward.len() == 0 && inverse.len() == 0),
    { reveal(small_exact_scope_candidate); }

    /// A partial scope token never satisfies the whole-target scope guard.
    pub proof fn exact_scope_cannot_authorize_whole(report: &Report, arguments: Seq<String>)
        requires exact_scope_evidence(report, arguments),
        ensures partial_command(report.command@),
            !( !partial_command(report.command@) && successful_report(report, CompletionPolicy::Whole) ),
    { }
}

sail_verified! {
    /// Validates one exact partial invocation over the actual report records.
    /// Original process/history admission and request-level scope coverage are
    /// separate native obligations. No report or scope field is rewritten.
    #[allow(clippy::question_mark, reason = "The explicit admission branch is shared with stock Verus and its guard-removal control.")]
    pub fn complete_exact_scope_report_ordered<'a>(report: &'a Report, arguments: &'a [String],
        forward: &[usize], inverse: &[usize]) -> (out: Option<ExactScopeReport<'a>>)
        requires(true)
        ensures(out.is_some() ==> exact_scope_authority_valid(&out.unwrap()),
            out.is_some() ==> exact_scope_evidence(report, arguments@),
            out.is_some() ==> exact_scope_report(&out.unwrap()) == report,
            out.is_some() ==> exact_scope_arguments(&out.unwrap()) == arguments,
            small_exact_scope_candidate(report, arguments@, forward@, inverse@) ==> out.is_some())
    {
        sail_proof! { reveal(small_exact_scope_candidate); }
        if !focused_command(arguments) || !same_replay_arguments(arguments, &report.command)
            || report.engine.is_some() { return None; }
        if complete_report_ordered(report, CompletionPolicy::Receipt, forward, inverse).is_none() {
            return None;
        }
        Some(ExactScopeReport { report, arguments })
    }
}

sail_spec! {
    pub open spec fn exact_partial_arguments(a: Seq<String>, b: Seq<String>) -> bool {
        a.len() == b.len() && a.map_values(|s: String| string_bytes(&s))
            == b.map_values(|s: String| string_bytes(&s))
    }

    /// This partial transfer covers plain stock invocations, not repair or
    /// source-engine selections with their own execution/acceptance rules.
    pub open spec fn plain_partial_request(request: &ResolvedRequest) -> bool {
        partial_command(request.args@) && !verification_disabled(request.args@)
            && request.engine.is_none() && request.function.is_none()
            && string_bytes(&request.repair_package).len() == 0 && request.repair_target.is_none()
            && string_bytes(&request.repair_kind).len() == 0
    }

    /// Retained exact-scope reuse, never a claim about complete-target reuse.
    pub open spec fn exact_partial_target(target: &TargetResult, arguments: Seq<String>) -> bool {
        target.requested && !target.complete && !target.reused
            && string_bytes(&target.establishing_run).len() > 0 && target.scope_reuse.is_some()
            && exact_partial_arguments(target.scope_reuse.unwrap()@, arguments)
            && exact_scope_evidence(&target.report, arguments)
    }

    pub open spec fn exact_partial_request_authorized(request: &ResolvedRequest, project: &ProjectResult) -> bool {
        request_header_matches(request, project) && resolved_coverage(request, project)
            && plain_partial_request(request) && project.currentness is Current
            && project.exit_code == Some(0i32) && project.error.is_none()
            && forall |i: int| 0 <= i < project.targets@.len() && project.targets@[i].requested ==>
                exact_partial_target(&project.targets@[i], request.args@)
    }

    pub closed spec fn exact_partial_request<'a>(scope: &ExactPartialRequest<'a>) -> &'a ResolvedRequest { scope.request }
    pub closed spec fn exact_partial_project<'a>(scope: &ExactPartialRequest<'a>) -> &'a ProjectResult { scope.project }
    pub open spec fn exact_partial_request_valid(scope: &ExactPartialRequest) -> bool {
        exact_partial_request_authorized(exact_partial_request(scope), exact_partial_project(scope))
    }

    #[verifier::opaque]
    pub open spec fn small_exact_partial_target(target: &TargetResult, arguments: Seq<String>,
        forward: Seq<usize>, inverse: Seq<usize>) -> bool {
        target.requested && !target.complete && !target.reused
            && string_bytes(&target.establishing_run).len() > 0 && target.scope_reuse.is_some()
            && exact_partial_arguments(target.scope_reuse.unwrap()@, arguments)
            && small_exact_scope_candidate(&target.report, arguments, forward, inverse)
    }

    pub proof fn small_exact_partial_target_definition(target: &TargetResult, arguments: Seq<String>,
        forward: Seq<usize>, inverse: Seq<usize>)
        ensures small_exact_partial_target(target, arguments, forward, inverse)
            == (target.requested && !target.complete && !target.reused
                && string_bytes(&target.establishing_run).len() > 0 && target.scope_reuse.is_some()
                && exact_partial_arguments(target.scope_reuse.unwrap()@, arguments)
                && small_exact_scope_candidate(&target.report, arguments, forward, inverse)),
    { reveal(small_exact_partial_target); }

    #[verifier::opaque]
    pub open spec fn small_exact_partial_request(request: &ResolvedRequest, project: &ProjectResult,
        orders: Seq<ReportOrder>) -> bool {
        small_request_match(request, project) && plain_partial_request(request)
            && project.currentness is Current && project.exit_code == Some(0i32) && project.error.is_none()
            && orders.len() == 1
            && small_exact_partial_target(&project.targets@[0], request.args@, orders[0].forward@, orders[0].inverse@)
    }

    pub proof fn small_exact_partial_request_definition(request: &ResolvedRequest, project: &ProjectResult,
        orders: Seq<ReportOrder>)
        ensures small_exact_partial_request(request, project, orders)
            == (small_request_match(request, project) && plain_partial_request(request)
                && project.currentness is Current && project.exit_code == Some(0i32) && project.error.is_none()
                && orders.len() == 1
                && small_exact_partial_target(&project.targets@[0], request.args@, orders[0].forward@, orders[0].inverse@)),
    { reveal(small_exact_partial_request); }

    /// Partial request evidence cannot satisfy the existing whole-request contract.
    pub proof fn exact_partial_request_excludes_whole(request: &ResolvedRequest, project: &ProjectResult)
        requires exact_partial_request_authorized(request, project),
        ensures !request_authorized(request, project), !supplied_selection_authorized(project),
            forall |i: int| 0 <= i < project.targets@.len() && project.targets@[i].requested ==>
                !project.targets@[i].complete && !project.targets@[i].reused && project.targets@[i].scope_reuse.is_some(),
    {
        if supplied_selection_authorized(project) {
            let i = choose |i: int| 0 <= i < project.targets@.len() && project.targets@[i].requested;
            assert(exact_partial_target(&project.targets@[i], request.args@));
            assert(!project.targets@[i].complete);
        }
    }
}

sail_verified! {
    /// Recognizes the exact plain partial request domain of this transfer.
    pub fn plain_partial_request_matches(request: &ResolvedRequest) -> (out: bool)
        requires(true) ensures(out == plain_partial_request(request))
    {
        focused_command(&request.args) && verification_enabled_command(&request.args)
            && request.engine.is_none() && request.function.is_none()
            && request.repair_package.is_empty() && request.repair_target.is_none() && request.repair_kind.is_empty()
    }
}

sail_verified! {
    /// Validates a retained requested target's exact partial-scope fields and
    /// original report without minting complete-target authority.
    #[allow(clippy::question_mark, reason = "Keep explicit checked refusal branches in the source shared with stock Verus.")]
    pub fn complete_exact_partial_target_ordered<'a>(target: &'a TargetResult, arguments: &'a [String],
        forward: &[usize], inverse: &[usize]) -> (out: Option<ExactScopeReport<'a>>)
        requires(true)
        ensures(out.is_some() ==> exact_partial_target(target, arguments@),
            out.is_some() ==> exact_scope_authority_valid(&out.unwrap()),
            out.is_some() ==> exact_scope_report(&out.unwrap()) == &target.report,
            out.is_some() ==> exact_scope_arguments(&out.unwrap()) == arguments,
            small_exact_partial_target(target, arguments@, forward@, inverse@) ==> out.is_some())
    {
        sail_proof! { reveal(small_exact_partial_target); }
        if !target.requested || target.complete || target.reused || target.establishing_run.is_empty() {
            return None;
        }
        let Some(scope) = &target.scope_reuse else { return None; };
        if !same_replay_arguments(scope, arguments) { return None; }
        complete_exact_scope_report_ordered(&target.report, arguments, forward, inverse)
    }
}

sail_verified! {
    /// Checks independent original target coverage and each retained partial
    /// report. Original process/history admission remains a native obligation.
    pub fn complete_exact_partial_request_ordered<'a>(request: &'a ResolvedRequest, project: &'a ProjectResult,
        orders: &[ReportOrder]) -> (out: Option<ExactPartialRequest<'a>>)
        requires(true)
        ensures(out.is_some() ==> exact_partial_request_authorized(request, project),
            out.is_some() ==> exact_partial_request_valid(&out.unwrap()),
            out.is_some() ==> exact_partial_request(&out.unwrap()) == request,
            out.is_some() ==> exact_partial_project(&out.unwrap()) == project,
            small_exact_partial_request(request, project, orders@) ==> out.is_some())
    {
        sail_proof! { reveal(small_exact_partial_request); }
        if !request_corresponds(request, project) || !plain_partial_request_matches(request)
            || !matches!(project.currentness, Currentness::Current) || project.exit_code != Some(0)
            || project.error.is_some() || orders.len() != project.targets.len() { return None; }
        let mut i = 0usize;
        sail_while! { (i < project.targets.len())
            invariant(i <= project.targets.len(), orders.len() == project.targets.len(),
                small_exact_partial_request(request, project, orders@) ==>
                    project.targets@.len() == 1 && orders@.len() == 1
                        && small_exact_partial_target(&project.targets@[0], request.args@,
                            orders@[0].forward@, orders@[0].inverse@),
                forall |j: int| 0 <= j < i && project.targets@[j].requested ==>
                    exact_partial_target(&project.targets@[j], request.args@))
            decreases(project.targets.len() - i)
            {
                sail_proof! {
                    if small_exact_partial_request(request, project, orders@) {
                        assert(i == 0);
                        assert(small_exact_partial_target(&project.targets@[i as int], request.args@,
                            orders@[i as int].forward@, orders@[i as int].inverse@));
                    }
                }
                if project.targets[i].requested && complete_exact_partial_target_ordered(&project.targets[i],
                    &request.args, &orders[i].forward, &orders[i].inverse).is_none() { return None; }
                i += 1;
            }
        }
        Some(ExactPartialRequest { request, project })
    }
}

sail_spec! {
    pub proof fn replay_preserves_plain_partial_request(current: &ResolvedRequest, original: &ResolvedRequest)
        requires replay_request_payload(current, original), plain_partial_request(original),
        ensures plain_partial_request(current),
    { replay_request_preserves_modes(current, original); }

    pub proof fn replay_preserves_partial_target(current: &ResolvedRequest, original: &ResolvedRequest, target: &TargetResult)
        requires replay_request_payload(current, original), exact_partial_target(target, original.args@),
        ensures exact_partial_target(target, current.args@),
    { replay_request_preserves_modes(current, original); }

    pub proof fn replay_preserves_partial_coverage(current: &ResolvedRequest, original: &ResolvedRequest, project: &ProjectResult)
        requires replay_request_payload(current, original), exact_partial_request_authorized(original, project),
        ensures plain_partial_request(current), resolved_coverage(current, project),
            forall |i: int| 0 <= i < project.targets@.len() && project.targets@[i].requested ==>
                exact_partial_target(&project.targets@[i], current.args@),
    {
        replay_preserves_plain_partial_request(current, original);
        replay_request_preserves_coverage(current, original, project);
        assert forall |i: int| 0 <= i < project.targets@.len() && project.targets@[i].requested implies
            exact_partial_target(&project.targets@[i], current.args@) by {
            replay_preserves_partial_target(current, original, &project.targets@[i]);
        }
    }

    #[verifier::opaque]
    pub open spec fn small_exact_partial_transfer(original: &ResolvedRequest, project: &ProjectResult,
        current: &ResolvedRequest, orders: Seq<ReportOrder>, readback: &sail_acceptance::inputs::GuardedReadback) -> bool {
        small_exact_partial_request(original, project, orders) && replay_request_payload(current, original)
            && readback.unchanged_view() && readback.guard_view().is_some()
            && readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision)
    }

    pub proof fn small_exact_partial_transfer_definition(original: &ResolvedRequest, project: &ProjectResult,
        current: &ResolvedRequest, orders: Seq<ReportOrder>, readback: &sail_acceptance::inputs::GuardedReadback)
        ensures small_exact_partial_transfer(original, project, current, orders, readback)
            == (small_exact_partial_request(original, project, orders) && replay_request_payload(current, original)
                && readback.unchanged_view() && readback.guard_view().is_some()
                && readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision)),
    { reveal(small_exact_partial_transfer); }

    /// Concrete one-target premises used by the constructive refinement
    /// witnesses. They expose the complete premise rather than assuming an
    /// opaque positive-authority flag; zero or one confirmed function is valid.
    pub open spec fn single_exact_partial_request_example(request: &ResolvedRequest, project: &ProjectResult) -> bool {
        small_request_match(request, project) && plain_partial_request(request)
            && project.currentness is Current && project.exit_code == Some(0i32) && project.error.is_none()
            && project.targets@[0].requested && !project.targets@[0].complete && !project.targets@[0].reused
            && string_bytes(&project.targets@[0].establishing_run).len() > 0 && project.targets@[0].scope_reuse.is_some()
            && exact_partial_arguments(project.targets@[0].scope_reuse.unwrap()@, request.args@)
            && exact_partial_arguments(request.args@, project.targets@[0].report.command@)
            && project.targets@[0].report.engine.is_none()
            && small_positive_report(&project.targets@[0].report, CompletionPolicy::Receipt)
    }

    pub proof fn single_exact_partial_request_reaches_factory(request: &ResolvedRequest, project: &ProjectResult,
        orders: Seq<ReportOrder>)
        requires single_exact_partial_request_example(request, project), orders.len() == 1,
            orders[0].forward@.len() == 0, orders[0].inverse@.len() == 0,
        ensures small_exact_partial_request(request, project, orders),
    {
        small_exact_scope_candidate_definition(&project.targets@[0].report, request.args@,
            orders[0].forward@, orders[0].inverse@);
        small_exact_partial_target_definition(&project.targets@[0], request.args@,
            orders[0].forward@, orders[0].inverse@);
        small_exact_partial_request_definition(request, project, orders);
    }
}

sail_verified! {
    /// Transfers an exact retained partial request to an independently supplied
    /// current request and guarded readback, preserving original scope/history
    /// fields. Native unchanged-call replay supplies its independent current
    /// request and retains separate original producer/history admission.
    pub fn bind_replayed_exact_partial_project_ordered(project: &mut ProjectResult, original: &ResolvedRequest,
        current: &ResolvedRequest, orders: &[ReportOrder], readback: &sail_acceptance::inputs::GuardedReadback)
        -> (accepted: bool)
        requires(true)
        ensures(same_replay_evidence(old(project), final(project)),
            accepted ==> replay_request_payload(current, original),
            accepted ==> exact_partial_request_authorized(original, old(project)),
            accepted ==> exact_partial_request_authorized(current, final(project)),
            accepted ==> !request_authorized(current, final(project)),
            accepted ==> !supplied_selection_authorized(final(project)),
            accepted ==> readback.unchanged_view() && readback.guard_view().is_some()
                && readback.guard_view().unwrap().revision_view() == string_bytes(&current.input_revision),
            accepted ==> string_bytes(&final(project).run_id) == string_bytes(&current.run_id),
            !accepted ==> *final(project) == *old(project),
            small_exact_partial_transfer(original, old(project), current, orders@, readback) ==> accepted)
    {
        sail_proof! { reveal(small_exact_partial_transfer); }
        if !replay_requests_match(current, original) { return false; }
        let Some(sealed) = seal_replay_readback(current, readback) else { return false; };
        let current = sealed.request();
        if complete_exact_partial_request_ordered(original, project, orders).is_none() { return false; }
        sail_proof! { replay_preserves_partial_coverage(current, original, project); }
        if !plain_partial_request_matches(current) { return false; }
        let current_run = current.run_id.clone();
        project.run_id = current_run;
        sail_proof! {
            assert(request_header_matches(current, project));
            assert(exact_partial_request_authorized(current, project));
            exact_partial_request_excludes_whole(current, project);
        }
        true
    }
}

sail_spec! {
    /// Equality with an independently supplied current canonical invocation.
    /// Actual filesystem resolution remains the native caller's obligation.
    pub open spec fn retained_invocation_corresponds(current: Option<&str>, request: &ResolvedRequest) -> bool {
        current.is_some() && current.unwrap().spec_bytes() == string_bytes(&request.invocation)
    }
}

sail_verified! {
    /// Checks the retained original submission against its captured request and
    /// a separately observed current invocation identity. A stored request or
    /// previously successful cache lookup cannot supply this observation.
    #[allow(clippy::question_mark, reason = "Keep explicit checked refusal branches in the source shared with stock Verus.")]
    pub fn bind_retained_submission<'a>(arguments: &'a [String], mode: &'a Mode,
        request: &'a ResolvedRequest, current_invocation: Option<&str>) -> (out: Option<SubmissionBinding<'a>>)
        requires(true)
        ensures(out.is_some() ==> binding_valid(&out.unwrap()),
            out.is_some() ==> bound_arguments(&out.unwrap()) == arguments,
            out.is_some() ==> bound_mode(&out.unwrap()) == mode,
            out.is_some() ==> bound_request(&out.unwrap()) == request,
            out.is_some() ==> retained_invocation_corresponds(current_invocation, request),
            arguments@.len() == 0 && request.args@.len() == 0 && mode is Stock
                && request.engine.is_none() && request.function.is_none()
                && retained_invocation_corresponds(current_invocation, request) ==> out.is_some())
    {
        let Some(current) = current_invocation else { return None; };
        if !exact_bytes(current.as_bytes(), request.invocation.as_str().as_bytes()) { return None; }
        bind_submission(arguments, mode, request)
    }
}

sail_spec! {
    pub closed spec fn path_binding_request<'a>(binding: &RequestPathBinding<'a>) -> &'a ResolvedRequest { binding.request }
    pub closed spec fn path_binding_invocation<'a>(binding: &RequestPathBinding<'a>) -> &'a [u8] { binding.invocation }
    pub closed spec fn path_binding_workspace<'a>(binding: &RequestPathBinding<'a>) -> &'a [u8] { binding.workspace }

    pub open spec fn request_paths_correspond(request: &ResolvedRequest, invocation: Seq<u8>, workspace: Seq<u8>) -> bool {
        string_bytes(&request.invocation) == invocation && string_bytes(&request.workspace) == workspace
    }

    pub open spec fn request_path_binding_valid(binding: &RequestPathBinding) -> bool {
        request_paths_correspond(path_binding_request(binding), path_binding_invocation(binding)@,
            path_binding_workspace(binding)@)
    }

    /// Observed records and scope evidence remain intact except the positive
    /// whole/partial labels which the path guard may withhold.
    pub open spec fn same_project_except_path_authority(a: &ProjectResult, b: &ProjectResult) -> bool {
        a.kind == b.kind && a.schema_version == b.schema_version && a.run_id == b.run_id
            && a.workspace == b.workspace && a.context_sha256 == b.context_sha256
            && a.input_revision == b.input_revision && a.currentness == b.currentness
            && a.exit_code == b.exit_code && a.error == b.error && a.engine == b.engine
            && a.targets@.len() == b.targets@.len()
            && forall |i: int| 0 <= i < a.targets@.len() ==>
                a.targets@[i].package_id == b.targets@[i].package_id
                    && a.targets@[i].target == b.targets@[i].target && a.targets@[i].kind == b.targets@[i].kind
                    && a.targets@[i].establishing_run == b.targets@[i].establishing_run
                    && a.targets@[i].reused == b.targets@[i].reused && a.targets@[i].requested == b.targets@[i].requested
                    && a.targets@[i].report == b.targets@[i].report
                    && (b.targets@[i].complete ==> a.targets@[i].complete)
                    && (b.targets@[i].scope_reuse.is_some() ==> b.targets@[i].scope_reuse == a.targets@[i].scope_reuse)
    }
}

sail_verified! {
    /// Binds request Strings to the original native bytes supplied separately
    /// by the caller. A lossy encoder or different path spelling cannot pass.
    pub fn bind_request_paths<'a>(request: &'a ResolvedRequest, invocation: &'a [u8], workspace: &'a [u8])
        -> (out: Option<RequestPathBinding<'a>>)
        requires(true)
        ensures(out.is_some() == request_paths_correspond(request, invocation@, workspace@),
            out.is_some() ==> request_path_binding_valid(&out.unwrap()),
            out.is_some() ==> path_binding_request(&out.unwrap()) == request,
            out.is_some() ==> path_binding_invocation(&out.unwrap()) == invocation,
            out.is_some() ==> path_binding_workspace(&out.unwrap()) == workspace)
    {
        if !exact_bytes(invocation, request.invocation.as_str().as_bytes())
            || !exact_bytes(workspace, request.workspace.as_str().as_bytes()) { return None; }
        Some(RequestPathBinding { request, invocation, workspace })
    }
}

sail_verified! {
    /// Keeps authority only when the independently retained native path bytes
    /// still match this request. Missing/mismatched bindings clear whole and
    /// exact-partial labels, preserving reports, identities and input currentness.
    #[allow(clippy::collapsible_if, reason = "Keep the checked path-binding branches shared with stock Verus unchanged.")]
    pub fn withhold_unmatched_request_paths(project: &mut ProjectResult, request: &ResolvedRequest,
        paths: Option<&RequestPathBinding>) -> (matched: bool)
        requires(true)
        ensures(same_project_except_path_authority(old(project), final(project)),
            matched ==> paths.is_some() && request_paths_correspond(request,
                path_binding_invocation(paths.unwrap())@, path_binding_workspace(paths.unwrap())@),
            matched ==> *final(project) == *old(project),
            !matched ==> forall |i: int| 0 <= i < final(project).targets@.len() ==>
                !final(project).targets@[i].complete && final(project).targets@[i].scope_reuse.is_none(),
            paths.is_some() && request_paths_correspond(request,
                path_binding_invocation(paths.unwrap())@, path_binding_workspace(paths.unwrap())@) ==> matched)
    {
        if let Some(binding) = paths {
            if bind_request_paths(request, binding.invocation(), binding.workspace()).is_some() { return true; }
        }
        let mut i = 0usize;
        sail_while! { (i < project.targets.len())
            invariant(i <= project.targets.len(), project.targets@.len() == old(project).targets@.len(),
                same_project_except_path_authority(old(project), project),
                forall |j: int| 0 <= j < i ==> !project.targets@[j].complete && project.targets@[j].scope_reuse.is_none(),
                forall |j: int| i <= j < project.targets@.len() ==>
                    project.targets@[j].complete == old(project).targets@[j].complete
                        && project.targets@[j].scope_reuse == old(project).targets@[j].scope_reuse)
            decreases(project.targets.len() - i)
            {
                project.targets[i].complete = false;
                project.targets[i].scope_reuse = None;
                i += 1;
            }
        }
        false
    }
}

sail_spec! {
    /// Exact requested coverage is nonvacuous: it contains a requested target.
    pub proof fn resolved_coverage_has_requested_target(request: &ResolvedRequest, project: &ProjectResult)
        requires resolved_coverage(request, project),
        ensures exists |i: int| 0 <= i < project.targets@.len() && project.targets@[i].requested,
    {
        let expected = choose |expected: Seq<RequestedTarget>| resolved_expected(request, expected)
            && exact_requested_coverage(expected, project.targets@);
        reveal(exact_requested_coverage);
        assert(expected.len() > 0);
        assert(request_covered(&expected[0], project.targets@));
        let i = choose |i: int| 0 <= i < project.targets@.len() && target_match(&expected[0], &project.targets@[i])
            && project.targets@[i].requested
            && forall |j: int| 0 <= j < project.targets@.len() && target_match(&expected[0], &project.targets@[j]) ==> j == i;
        assert(project.targets@[i].requested);
    }
}

sail_spec! {
    pub closed spec fn declared_replay_original<'a, 'b>(a: &DeclaredReplayAdmission<'a, 'b>) -> &'a ResolvedRequest { a.original }
    pub closed spec fn declared_replay_current<'a, 'b>(a: &DeclaredReplayAdmission<'a, 'b>) -> &'a ResolvedRequest { a.current }
    pub closed spec fn declared_replay_project(a: &DeclaredReplayAdmission) -> ProjectResult { a.project }
    pub closed spec fn declared_replay_paths<'a, 'b>(a: &DeclaredReplayAdmission<'a, 'b>) -> RequestPathBinding<'a> { a.paths }
    pub closed spec fn declared_replay_readback<'a, 'b>(a: &DeclaredReplayAdmission<'a, 'b>) -> ReplayReadback<'a, 'b> { a.readback }

    /// Conjunction over supplied immutable evidence. There is deliberately no
    /// complete physical input domain, solver-inventory or launch assertion.
    pub open spec fn declared_replay_valid(a: &DeclaredReplayAdmission) -> bool {
        let current = declared_replay_current(a);
        let project = declared_replay_project(a);
        let paths = declared_replay_paths(a);
        let readback = declared_replay_readback(a);
        replay_request_payload(current, declared_replay_original(a))
            && request_path_binding_valid(&paths) && path_binding_request(&paths) == current
            && replay_readback_valid(&readback) && replay_readback_request(&readback) == current
            && if partial_command(current.args@) {
                exact_partial_request_authorized(current, &project)
                    && !request_authorized(current, &project) && !supplied_selection_authorized(&project)
            } else { request_authorized(current, &project) }
    }
}

sail_verified! {
    /// Composes checked request/report association, native byte identity and
    /// supplied guarded readback. Missing witnesses refuse the replay candidate.
    /// Existing full whole/partial binders retain their original checks and
    /// constructive contracts. This does not admit a full service proof.
    #[allow(clippy::question_mark, reason = "Keep explicit refusal branches in the source shared with stock Verus.")]
    pub fn admit_declared_replay_ordered<'a, 'b>(mut project: ProjectResult,
        original: &'a ResolvedRequest, current: &'a ResolvedRequest, orders: &[ReportOrder],
        paths: Option<&RequestPathBinding<'a>>,
        readback: Option<&'a sail_acceptance::inputs::GuardedReadback<'b>>)
        -> (out: Option<DeclaredReplayAdmission<'a, 'b>>)
        requires(true)
        ensures(out.is_some() ==> declared_replay_valid(&out.unwrap()),
            out.is_some() ==> declared_replay_original(&out.unwrap()) == original,
            out.is_some() ==> declared_replay_current(&out.unwrap()) == current,
            out.is_some() ==> same_replay_evidence(&project, &declared_replay_project(&out.unwrap())),
            out.is_some() ==> paths.is_some() && readback.is_some(),
            out.is_some() ==> replay_readback_observation(&declared_replay_readback(&out.unwrap())) == readback.unwrap(),
            out.is_some() ==> string_bytes(&declared_replay_project(&out.unwrap()).run_id) == string_bytes(&current.run_id),
            paths.is_some() && readback.is_some()
                && request_paths_correspond(current, path_binding_invocation(paths.unwrap())@,
                    path_binding_workspace(paths.unwrap())@)
                && !partial_command(current.args@)
                && replay_constructive_request(original, &project, current, orders, readback.unwrap()) ==> out.is_some(),
            paths.is_some() && readback.is_some()
                && request_paths_correspond(current, path_binding_invocation(paths.unwrap())@,
                    path_binding_workspace(paths.unwrap())@)
                && partial_command(current.args@)
                && small_exact_partial_transfer(original, &project, current, orders@, readback.unwrap()) ==> out.is_some())
    {
        let Some(paths) = paths else { return None; };
        let Some(checked_paths) = bind_request_paths(current, paths.invocation(), paths.workspace()) else { return None; };
        let Some(readback) = readback else { return None; };
        // Reveal only the original constructive premises; no positive field or
        // replacement assumption supplies missing evidence.
        sail_proof! {
            reveal(replay_constructive_request);
            reveal(small_exact_partial_transfer);
        }
        let Some(checked_readback) = seal_replay_readback(current, readback) else { return None; };
        let accepted = if focused_command(&current.args) {
            bind_replayed_exact_partial_project_ordered(&mut project, original, current, orders, readback)
        } else {
            bind_replayed_project_ordered(&mut project, original, current, orders, readback)
        };
        if !accepted { return None; }
        Some(DeclaredReplayAdmission { original, current, project, paths: checked_paths, readback: checked_readback })
    }
}

sail_verified! {
    /// Consumes the admitted declared-domain view without changing any field.
    /// It grants no stronger Current, physical-input or full-service authority.
    pub fn present_declared_replay(admitted: DeclaredReplayAdmission) -> (out: ProjectResult)
        requires(declared_replay_valid(&admitted))
        ensures(out == declared_replay_project(&admitted))
    { admitted.project }
}

sail_spec! {
    pub open spec fn decimal_byte(b: u8) -> bool { b'0' <= b <= b'9' }

    pub open spec fn cargo_duration_tail(bytes: Seq<u8>, end: int) -> bool {
        0 < end <= bytes.len()
            && (forall |i: int| 0 <= i < end ==> decimal_byte(bytes[i]))
            && ((bytes.len() - end == 5 && bytes[end] == b'.'
                    && decimal_byte(bytes[end + 1]) && decimal_byte(bytes[end + 2])
                    && bytes[end + 3] == b's' && bytes[end + 4] == b'\n')
                || (bytes.len() - end == 6 && bytes[end] == b'm' && bytes[end + 1] == b' '
                    && b'0' <= bytes[end + 2] <= b'5' && decimal_byte(bytes[end + 3])
                    && bytes[end + 4] == b's' && bytes[end + 5] == b'\n'))
    }

    pub open spec fn cargo_duration(bytes: Seq<u8>) -> bool {
        exists |end: int| cargo_duration_tail(bytes, end)
    }

    pub open spec fn cargo_footer_prefix(bytes: Seq<u8>, prefix: Seq<u8>) -> bool {
        prefix.len() <= bytes.len() && bytes.take(prefix.len() as int) == prefix
            && cargo_duration(bytes.skip(prefix.len() as int))
    }

    /// A deliberately strict plain Cargo footer grammar, with no arbitrary
    /// diagnostic text or ANSI escape path. Other renderings run normally.
    pub open spec fn cargo_footer(bytes: Seq<u8>) -> bool {
        cargo_footer_prefix(bytes, b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in "@)
            || cargo_footer_prefix(bytes, b"    Finished `dev` profile [unoptimized] target(s) in "@)
            || cargo_footer_prefix(bytes, b"    Finished `release` profile [optimized] target(s) in "@)
            || cargo_footer_prefix(bytes, b"    Finished `release` profile [optimized + debuginfo] target(s) in "@)
    }

    pub open spec fn empty_human_observation(r: &ResolvedRequest, p: &ProjectResult, human: bool) -> bool {
        human && request_header_matches(r, p) && r.targets@.len() > 0 && p.targets@.len() == 0
            && !partial_command(r.args@) && !verification_disabled(r.args@)
            && !options_disable_verification(r.args@)
            && r.engine.is_none() && p.engine.is_none() && r.function.is_none()
            && string_bytes(&r.repair_package).len() == 0 && r.repair_target.is_none()
            && string_bytes(&r.repair_kind).len() == 0
            && p.currentness is Current && p.exit_code == Some(0i32)
            && p.error.is_none() && p.no_reuse.is_none()
            && p.execution.verifier_invocations == 0 && p.execution.reused_targets == 0
    }

    pub closed spec fn empty_human_stderr<'a, 'b>(a: &EmptyHumanOutput<'a, 'b>) -> &'a [u8] { a.stderr }
    pub closed spec fn empty_human_valid(a: &EmptyHumanOutput) -> bool {
        empty_human_observation(a.original, a.observed, true)
            && replay_request_payload(a.current, a.original) && cargo_footer(a.stderr@)
            && request_path_binding_valid(&a.paths) && path_binding_request(&a.paths) == a.current
            && replay_readback_valid(&a.readback) && replay_readback_request(&a.readback) == a.current
            && !request_authorized(a.original, a.observed)
            && !request_authorized(a.current, a.observed)
            && !supplied_selection_authorized(a.observed)
    }
}

sail_verified! {
    /// Checks one complete numeric Cargo duration, including its final newline.
    pub fn cargo_duration_only(bytes: &[u8]) -> (out: bool)
        requires(true) ensures(out == cargo_duration(bytes@))
    {
        let mut i = 0usize;
        sail_while! { (i < bytes.len() && b'0' <= bytes[i] && bytes[i] <= b'9')
            invariant(i <= bytes.len(), forall |j: int| 0 <= j < i ==> decimal_byte(bytes@[j]))
            decreases(bytes.len() - i)
            { i += 1; }
        }
        sail_proof! {
            if cargo_duration(bytes@) {
                let end = choose |end: int| cargo_duration_tail(bytes@, end);
                if end < i { assert(decimal_byte(bytes@[end])); }
                if i < end { assert(decimal_byte(bytes@[i as int])); }
                assert(end == i);
            }
        }
        if i == 0 { return false; }
        if bytes.len() - i == 5 {
            bytes[i] == b'.' && b'0' <= bytes[i + 1] && bytes[i + 1] <= b'9'
                && b'0' <= bytes[i + 2] && bytes[i + 2] <= b'9'
                && bytes[i + 3] == b's' && bytes[i + 4] == b'\n'
        } else if bytes.len() - i == 6 {
            bytes[i] == b'm' && bytes[i + 1] == b' '
                && b'0' <= bytes[i + 2] && bytes[i + 2] <= b'5'
                && b'0' <= bytes[i + 3] && bytes[i + 3] <= b'9'
                && bytes[i + 4] == b's' && bytes[i + 5] == b'\n'
        } else { false }
    }
}

sail_verified! {
    /// Checks the complete prefix and duration without trimming or substring matches.
    pub fn cargo_footer_with_prefix(bytes: &[u8], prefix: &[u8]) -> (out: bool)
        requires(true) ensures(out == cargo_footer_prefix(bytes@, prefix@))
    {
        if prefix.len() > bytes.len() { return false; }
        exact_bytes(&bytes[..prefix.len()], prefix) && cargo_duration_only(&bytes[prefix.len()..])
    }
}

sail_verified! {
    /// Accepts only empty stdout and one fully parsed ordinary Cargo footer.
    pub fn empty_human_bytes(stdout: &[u8], stderr: &[u8]) -> (out: bool)
        requires(true) ensures(out == (stdout@.len() == 0 && cargo_footer(stderr@)))
    {
        stdout.is_empty() && (
            cargo_footer_with_prefix(stderr, b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in ")
            || cargo_footer_with_prefix(stderr, b"    Finished `dev` profile [unoptimized] target(s) in ")
            || cargo_footer_with_prefix(stderr, b"    Finished `release` profile [optimized] target(s) in ")
            || cargo_footer_with_prefix(stderr, b"    Finished `release` profile [optimized + debuginfo] target(s) in "))
    }
}

sail_verified! {
    /// Checks an original human no-op observation. This deliberately does not
    /// call a complete-request factory or derive a verdict from exit zero.
    pub fn empty_human_candidate(request: &ResolvedRequest, project: &ProjectResult, human: bool) -> (out: bool)
        requires(true) ensures(out == empty_human_observation(request, project, human))
    {
        human && request_headers_match(request, project) && !request.targets.is_empty() && project.targets.is_empty()
            && !focused_command(&request.args) && verification_enabled_command(&request.args)
            && verification_enabled_options(&request.args)
            && request.engine.is_none() && project.engine.is_none() && request.function.is_none()
            && request.repair_package.is_empty() && request.repair_target.is_none() && request.repair_kind.is_empty()
            && matches!(project.currentness, Currentness::Current) && project.exit_code == Some(0)
            && project.error.is_none() && project.no_reuse.is_none()
            && project.execution.verifier_invocations == 0 && project.execution.reused_targets == 0
    }
}

sail_verified! {
    /// Composes exact original/current requests, native path bytes, guarded
    /// revision readback and byte-only output. The only public data accessor is
    /// stderr; no report, refreshed result, or proof token can be obtained.
    #[allow(clippy::too_many_arguments, reason = "Each original witness remains explicit at this admission boundary.")]
    #[allow(clippy::question_mark, reason = "Keep explicit refusal branches in the source shared with stock Verus.")]
    pub fn admit_empty_human_output<'a, 'b>(original: &'a ResolvedRequest, current: &'a ResolvedRequest,
        observed: &'a ProjectResult, human: bool, stdout: &[u8], stderr: &'a [u8],
        paths: Option<&RequestPathBinding<'a>>, readback: Option<&'a sail_acceptance::inputs::GuardedReadback<'b>>)
        -> (out: Option<EmptyHumanOutput<'a, 'b>>)
        requires(true)
        ensures(out.is_some() ==> empty_human_valid(&out.unwrap()),
            out.is_some() ==> human && stdout@.len() == 0,
            out.is_some() ==> empty_human_stderr(&out.unwrap()) == stderr,
            out.is_some() ==> empty_human_observation(original, observed, human)
                && replay_request_payload(current, original),
            out.is_some() == (empty_human_observation(original, observed, human)
                && replay_request_payload(current, original) && stdout@.len() == 0 && cargo_footer(stderr@)
                && paths.is_some() && request_paths_correspond(current,
                    path_binding_invocation(paths.unwrap())@, path_binding_workspace(paths.unwrap())@)
                && readback.is_some() && readback.unwrap().unchanged_view() && readback.unwrap().guard_view().is_some()
                && readback.unwrap().guard_view().unwrap().revision_view() == string_bytes(&current.input_revision)))
    {
        if !empty_human_candidate(original, observed, human) || !replay_requests_match(current, original)
            || !empty_human_bytes(stdout, stderr) { return None; }
        let Some(paths) = paths else { return None; };
        let Some(paths) = bind_request_paths(current, paths.invocation(), paths.workspace()) else { return None; };
        let Some(readback) = readback else { return None; };
        let Some(readback) = seal_replay_readback(current, readback) else { return None; };
        sail_proof! {
            if resolved_coverage(original, observed) { resolved_coverage_has_requested_target(original, observed); }
            if resolved_coverage(current, observed) { resolved_coverage_has_requested_target(current, observed); }
            assert(!supplied_selection_authorized(observed));
        }
        Some(EmptyHumanOutput { original, current, observed, stderr, paths, readback })
    }
}
