//! Real public report records exercise the checked authority and caller surfaces.

use sail_acceptance::authority::CompletionPolicy;
use sail_api::acceptance::{
    complete_report, complete_report_ordered, complete_request, complete_target,
    complete_whole_report, focused_command, requested_coverage, requested_target,
    verification_enabled_command, verification_enabled_options,
};
use sail_api::output::{
    ErrorInfo, EstablishmentName, Report, SourceSummaryName, Summary, VerdictName,
};
use sail_api::project::{
    Currentness, Execution, ProjectResult, RequestTarget, ResolvedRequest, TargetResult,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn passing_report() -> Result<Report, Box<dyn std::error::Error>> {
    let mut report: Report = serde_json::from_str(include_str!(
        "../../sail-cli/tests/fixtures/attr/report.golden.json"
    ))?;
    report.exit_code = Some(0);
    report.error = None;
    for krate in &mut report.crates {
        krate
            .functions
            .retain(|f| f.verdict == VerdictName::Verified);
        krate.verus.errors = Some(0);
        krate.verus.success = Some(true);
        krate.verus.encountered_vir_error = Some(false);
    }
    report.summary = Summary::of(&report.crates);
    assert_ne!(report.crates[0].functions.len(), 0);
    Ok(report)
}

fn target(name: &str, report: Report) -> TargetResult {
    TargetResult {
        package_id: "cargo-package#1.0.0".into(),
        target: name.into(),
        kind: vec!["bin".into()],
        establishing_run: "establishing-run".into(),
        reused: false,
        scope_reuse: None,
        requested: true,
        complete: true,
        report,
        dependency_store: None,
        dependency_build: None,
    }
}

fn project(targets: Vec<TargetResult>) -> ProjectResult {
    ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: sail_api::project::SCHEMA_VERSION,
        run_id: "current-run".into(),
        workspace: "/workspace".into(),
        context_sha256: "context".into(),
        input_revision: "revision".into(),
        currentness: Currentness::Current,
        exit_code: Some(0),
        targets,
        execution: Execution::default(),
        notes: Vec::new(),
        error: None,
        engine: None,
        no_reuse: None,
    }
}

fn original_request() -> ResolvedRequest {
    ResolvedRequest {
        run_id: "current-run".into(),
        invocation: "/workspace".into(),
        workspace: "/workspace".into(),
        context_sha256: "context".into(),
        input_revision: "revision".into(),
        args: Vec::new(),
        engine: None,
        function: None,
        repair_package: String::new(),
        repair_target: None,
        repair_kind: String::new(),
        targets: vec![RequestTarget {
            package_id: "cargo-package#1.0.0".into(),
            target: "app".into(),
            kind: "bin".into(),
        }],
    }
}

#[test]
fn empty_human_footer_cannot_carry_verifier_text() {
    use sail_api::acceptance::empty_human_bytes;
    let footer = b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\n";
    assert!(empty_human_bytes(&[], footer));
    assert!(empty_human_bytes(
        &[],
        b"    Finished `release` profile [optimized] target(s) in 12m 59s\n"
    ));
    for stderr in [
        b"verification results:: 1 verified, 0 errors\n".as_slice(),
        b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\nverified\n",
        b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in .01s\n",
        b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s",
        b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 60s\n",
        b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\n\n",
    ] {
        assert!(!empty_human_bytes(&[], stderr));
    }
    assert!(!empty_human_bytes(b"verified", footer));
}

fn rendered_human_footer_preserves_prefix(
    admitted: &sail_api::acceptance::EmptyHumanOutput<'_, '_>,
) {
    assert_eq!(
        sail_api::acceptance::render_empty_human_output(admitted, b"12m 59s\n").as_deref(),
        Some(
            b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12m 59s\n"
                .as_slice()
        )
    );
    assert!(
        sail_api::acceptance::render_empty_human_output(admitted, b"0.01s\nverified\n").is_none()
    );
}

#[test]
fn empty_human_replay_never_mints_requested_proof_authority() -> TestResult {
    use sail_acceptance::inputs::{InputFact, InputValue};
    use sail_api::acceptance::{
        admit_empty_human_output, bind_request_paths, empty_human_candidate,
    };
    let facts = [InputFact {
        namespace: 0,
        name: b"source",
        value: InputValue::Present(b"original bytes"),
    }];
    let footer = b"    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s\n";
    for variant in [
        "positive",
        "json",
        "stdout",
        "stderr",
        "original-owner",
        "current-scope",
        "stale",
        "paths",
        "compile-only",
        "focus",
        "report",
        "verifier",
        "reused",
        "empty-request",
        "exit",
        "historical",
    ] {
        let mut original = original_request();
        let mut observed = project(Vec::new());
        match variant {
            "compile-only" => original.args = vec!["--".into(), "--no-verify".into()],
            "focus" => original.args = vec!["--".into(), "--verify-module".into(), "m".into()],
            "report" => observed.targets.push(target("app", passing_report()?)),
            "verifier" => observed.execution.verifier_invocations = 1,
            "reused" => observed.execution.reused_targets = 1,
            "empty-request" => original.targets.clear(),
            "exit" => observed.exit_code = Some(1),
            "historical" => observed.currentness = Currentness::Historical,
            _ => (),
        }
        let mut current = original.clone();
        current.run_id = "new independently allocated request".into();
        if variant == "original-owner" {
            original.context_sha256.push('x');
        }
        if variant == "current-scope" {
            current.targets[0].target.push('x');
        }
        let mut path_request = current.clone();
        if variant == "paths" {
            path_request.workspace.push('x');
        }
        let paths = bind_request_paths(
            &path_request,
            path_request.invocation.as_bytes(),
            path_request.workspace.as_bytes(),
        )
        .ok_or("native path binding")?;
        let readback = replay_declared_readback(
            &facts,
            &facts,
            Some(b"original observation stamp"),
            if variant == "stale" {
                b"stale revision"
            } else {
                b"revision"
            },
        );
        let stdout = if variant == "stdout" {
            b"verified".as_slice()
        } else {
            &[]
        };
        let stderr = if variant == "stderr" {
            b"verified\n".as_slice()
        } else {
            footer.as_slice()
        };
        let admitted = admit_empty_human_output(
            &original,
            &current,
            &observed,
            variant != "json",
            stdout,
            stderr,
            Some(&paths),
            Some(&readback),
        );
        assert_eq!(admitted.is_some(), variant == "positive", "{variant}");
        if let Some(admitted) = admitted {
            assert_eq!(admitted.stderr(), footer);
            rendered_human_footer_preserves_prefix(&admitted);
            assert!(empty_human_candidate(&original, &observed, true));
            assert!(complete_request(&original, &observed).is_none());
            assert!(complete_request(&current, &observed).is_none());
            assert!(sail_api::acceptance::complete_selection(&observed).is_none());
        }
    }
    Ok(())
}

#[test]
fn original_request_binds_nonempty_scope_and_all_identity_fields() -> TestResult {
    let request = original_request();
    let actual = project(vec![target("app", passing_report()?)]);
    let authority = complete_request(&request, &actual).ok_or("valid nonempty request refused")?;
    assert!(std::ptr::eq(
        authority.request(),
        std::ptr::from_ref(&request)
    ));
    assert!(std::ptr::eq(
        authority.project(),
        std::ptr::from_ref(&actual)
    ));
    for field in [
        "run",
        "workspace",
        "context",
        "revision",
        "target",
        "omitted",
        "compile-only",
    ] {
        let mut wrong = request.clone();
        match field {
            "run" => wrong.run_id.push('x'),
            "workspace" => wrong.workspace.push('x'),
            "context" => wrong.context_sha256.push('x'),
            "revision" => wrong.input_revision.push('x'),
            "target" => wrong.targets[0].target.push('x'),
            "omitted" => wrong.targets.clear(),
            "compile-only" => wrong.args = vec!["--".into(), "--no-verify".into()],
            _ => unreachable!(),
        }
        assert!(complete_request(&wrong, &actual).is_none(), "{field}");
    }
    let mut missing = actual.clone();
    missing.targets.clear();
    assert!(complete_request(&request, &missing).is_none());
    for currentness in [Currentness::Historical, Currentness::Unknown] {
        let mut old = actual.clone();
        old.currentness = currentness;
        assert!(complete_request(&request, &old).is_none());
    }
    let mut refused = actual.clone();
    refused.targets[0].complete = false;
    assert!(complete_request(&request, &refused).is_none());
    let mut ambiguous = actual.clone();
    ambiguous.targets.push(ambiguous.targets[0].clone());
    assert!(complete_request(&request, &ambiguous).is_none());
    let mut zero = actual;
    for krate in &mut zero.targets[0].report.crates {
        krate.functions.clear();
        krate.verus.verified = Some(0);
    }
    zero.targets[0].report.summary = Summary::of(&zero.targets[0].report.crates);
    assert!(complete_request(&request, &zero).is_some());
    Ok(())
}

#[test]
fn submitted_authority_keeps_the_original_arguments_and_mode() -> TestResult {
    use sail_api::acceptance::{SubmissionKind, bind_submission, complete_submitted_request};
    use sail_api::jobs::Mode;
    let request = original_request();
    let actual = project(vec![target("app", passing_report()?)]);
    let arguments = Vec::new();
    let mode = Mode::Stock;
    let authority = complete_submitted_request(&arguments, &mode, &request, &actual)
        .ok_or("valid submission refused")?;
    assert_eq!(authority.kind(), SubmissionKind::Stock);
    assert!(std::ptr::eq(
        authority.request(),
        std::ptr::from_ref(&request)
    ));
    assert!(std::ptr::eq(
        authority.project(),
        std::ptr::from_ref(&actual)
    ));
    assert!(std::ptr::eq(authority.mode(), std::ptr::from_ref(&mode)));
    let wrong_arguments = vec!["--features".into(), "different-proof-inputs".into()];
    assert!(bind_submission(&wrong_arguments, &mode, &request).is_none());
    let mut opt_in = request.clone();
    opt_in.engine = Some(sail_api::project::EngineIdentity {
        name: "interactive".into(),
        manifest_sha256: "manifest".into(),
    });
    assert!(bind_submission(&arguments, &Mode::Stock, &opt_in).is_none());
    assert_eq!(
        bind_submission(&arguments, &Mode::Interactive, &opt_in)
            .ok_or("interactive match refused")?
            .kind(),
        SubmissionKind::Interactive
    );
    let stock_for_opt_in = bind_submission(&arguments, &Mode::Interactive, &request)
        .ok_or("stock observation refused")?;
    assert_eq!(stock_for_opt_in.kind(), SubmissionKind::StockForOptIn);
    assert_ne!(stock_for_opt_in.kind(), SubmissionKind::Repair);
    opt_in.engine.as_mut().ok_or("missing engine")?.name = "unrecognized".into();
    assert!(bind_submission(&arguments, &Mode::Interactive, &opt_in).is_none());
    opt_in.engine.as_mut().ok_or("missing engine")?.name = "interactive".into();
    opt_in.function = Some("original".into());
    opt_in.repair_package = "app".into();
    opt_in.repair_kind = "lib".into();
    let repair = sail_api::repair::Selection {
        package: "app".into(),
        target: None,
        kind: "lib".into(),
        function: "original".into(),
    };
    assert_eq!(
        bind_submission(&arguments, &Mode::Repair(repair.clone()), &opt_in)
            .ok_or("matching repair refused")?
            .kind(),
        SubmissionKind::Repair
    );
    let mut swapped = repair;
    swapped.function = "other".into();
    assert!(bind_submission(&arguments, &Mode::Repair(swapped), &opt_in).is_none());
    Ok(())
}

#[test]
fn submitted_arguments_only_allow_the_documented_incremental_noop() {
    let strings = |values: &[&str]| values.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let submitted = strings(&[
        "--incremental",
        "-p",
        "app",
        "--",
        "--incremental",
        "--no-verify",
    ]);
    let actual = strings(&["-p", "app", "--", "--incremental", "--no-verify"]);
    assert!(sail_api::acceptance::submitted_arguments_match(
        &submitted, &actual
    ));
    assert!(!sail_api::acceptance::submitted_arguments_match(
        &submitted,
        &strings(&["-p", "app", "--", "--incremental"])
    ));
    assert!(!sail_api::acceptance::submitted_arguments_match(
        &submitted,
        &strings(&["-p", "other", "--", "--incremental", "--no-verify"])
    ));
    assert!(!sail_api::acceptance::submitted_arguments_match(
        &submitted,
        &strings(&["-p", "app", "--", "--no-verify"])
    ));
}

#[test]
fn effective_driver_tokens_cover_the_original_packed_value() {
    use sail_api::acceptance::{effective_driver_arguments_match, packed_driver_arguments_match};
    let strings = |values: &[&str]| values.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let separator = "__VERUS_DRIVER_ARGS_SEP__";
    assert!(packed_driver_arguments_match("--no-verify", &[]));
    assert!(packed_driver_arguments_match(separator, &strings(&[""])));
    let packed = format!("ignored{separator}--time{separator}{separator}--no-verify{separator}");
    assert!(packed_driver_arguments_match(
        &packed,
        &strings(&["--time", "", "--no-verify", ""])
    ));
    for candidate in [
        strings(&["--time", "", ""]),
        strings(&["--time", "", "--no-verify"]),
        strings(&["--time", "", "--verify", ""]),
        strings(&["--time", "--no-verify", "", ""]),
        strings(&["--time", "", "--no-verify", "", ""]),
    ] {
        assert!(!packed_driver_arguments_match(&packed, &candidate));
    }
    // Token-looking text in the ignored prefix cannot become an effective flag.
    assert!(!packed_driver_arguments_match(
        "--no-verify",
        &strings(&["--no-verify"])
    ));
    assert!(!packed_driver_arguments_match(
        &format!("{separator}a{separator}b"),
        &strings(&[&format!("a{separator}b")])
    ));
    let common = Some(format!("{separator}--time"));
    let package = Some(format!("{separator}--no-verify"));
    let actual = strings(&["--time", "--no-verify"]);
    assert!(effective_driver_arguments_match(
        &common, &package, 1, &actual
    ));
    assert!(!effective_driver_arguments_match(
        &common, &package, 0, &actual
    ));
    assert!(!effective_driver_arguments_match(
        &common, &package, 2, &actual
    ));
    assert!(!effective_driver_arguments_match(
        &common,
        &package,
        1,
        &strings(&["--time"])
    ));
    assert!(!effective_driver_arguments_match(
        &package, &common, 1, &actual
    ));
    assert!(!effective_driver_arguments_match(
        &None, &package, 1, &actual
    ));
}

#[test]
fn repair_selection_preserves_raw_package_and_exact_function_identity() {
    let mut request = original_request();
    request.function = Some("app::lemma".into());
    request.repair_package = "app@1.0.0".into();
    request.repair_target = Some("app-bin".into());
    request.repair_kind = "bin".into();
    let selected = Some("app-bin".to_owned());
    assert!(sail_api::acceptance::repair_request_matches(
        &request,
        "app@1.0.0",
        &selected,
        "bin",
        "app::lemma"
    ));
    for (package, target, kind, function) in [
        ("cargo-package#1.0.0", selected.clone(), "bin", "app::lemma"),
        ("app@1.0.0", Some("other-bin".into()), "bin", "app::lemma"),
        ("app@1.0.0", selected.clone(), "lib", "app::lemma"),
        ("app@1.0.0", selected, "bin", "app::other"),
    ] {
        assert!(!sail_api::acceptance::repair_request_matches(
            &request, package, &target, kind, function
        ));
    }
}

#[test]
fn repair_requires_its_function_in_actual_requested_full_records() -> TestResult {
    let mut request = original_request();
    request.function = Some("input::Counter::get".into());
    let mut actual = project(vec![target("app", passing_report()?)]);
    assert!(complete_request(&request, &actual).is_some());
    assert!(sail_api::acceptance::requested_function_present(
        &request, &actual
    ));
    request.function = Some("input::does_not_exist".into());
    assert!(
        complete_request(&request, &actual).is_some(),
        "whole-target proof remains independent of repair selection"
    );
    assert!(!sail_api::acceptance::requested_function_present(
        &request, &actual
    ));
    request.function = Some("input::Counter::get".into());
    actual.targets[0].requested = false;
    assert!(
        !sail_api::acceptance::requested_function_present(&request, &actual),
        "a dependency is not the requested repair target"
    );
    Ok(())
}

#[test]
fn actual_success_and_solver_free_crates_mint_authority() -> TestResult {
    let report = passing_report()?;
    assert!(complete_report(&report, CompletionPolicy::Receipt).is_some());
    assert!(complete_report(&report, CompletionPolicy::Repair).is_some());
    assert_eq!(report.invariant_violations(), Vec::<String>::new());

    let mut zero = report;
    for krate in &mut zero.crates {
        krate.functions.clear();
        krate.verus.verified = Some(0);
    }
    zero.summary = Summary::of(&zero.crates);
    assert!(complete_whole_report(&zero).is_some());
    zero.crates[0].verus.success = None;
    assert!(complete_report(&zero, CompletionPolicy::Receipt).is_some());
    assert!(complete_whole_report(&zero).is_none());
    assert!(complete_report(&zero, CompletionPolicy::Repair).is_none());
    Ok(())
}

#[test]
fn compile_only_success_is_distinct_from_legitimate_zero_function_verification() -> TestResult {
    let mut report = passing_report()?;
    for krate in &mut report.crates {
        krate.functions.clear();
        krate.verus.verified = Some(0);
    }
    report.summary = Summary::of(&report.crates);
    assert!(complete_whole_report(&report).is_some());
    report.command = vec!["--".into(), "--no-verify".into()];
    assert!(!verification_enabled_command(&report.command));
    for policy in [
        CompletionPolicy::Receipt,
        CompletionPolicy::Whole,
        CompletionPolicy::Repair,
    ] {
        assert!(complete_report(&report, policy).is_none());
    }
    let full = project(vec![target("app", report)]);
    assert_eq!(full.currentness, Currentness::Current);
    assert!(!sail_api::repair::accepted(&full));
    assert!(verification_enabled_command(&["--no-verify".into()]));
    assert!(verification_enabled_command(&[
        "--".into(),
        "--no-verify=false".into()
    ]));
    assert!(!verification_enabled_options(&["--no-verify".into()]));
    assert!(verification_enabled_options(&[
        "--".into(),
        "--no-verify".into()
    ]));
    Ok(())
}

#[test]
fn nonempty_record_projection_has_a_constructive_positive_control() -> TestResult {
    let mut report = passing_report()?;
    report.crates.truncate(1);
    report.crates[0].functions.truncate(1);
    report.crates[0].verus.verified = Some(1);
    let function = &mut report.crates[0].functions[0];
    function.obligations = None;
    function.provenance.source = SourceSummaryName::Unrecorded;
    function.provenance.checks_solved = 0;
    function.provenance.checks_served = 0;
    function.provenance.checks_unrecorded = 0;
    report.summary = Summary::of(&report.crates);
    assert_eq!(report.summary.functions, 1);
    assert_eq!(report.invariant_violations(), Vec::<String>::new());
    assert!(complete_report_ordered(&report, CompletionPolicy::Receipt, &[], &[]).is_some());
    Ok(())
}

#[test]
fn actual_function_records_override_forged_success_counts() -> TestResult {
    let original = passing_report()?;
    for case in 0..5 {
        let mut report = original.clone();
        let f = &mut report.crates[0].functions[0];
        match case {
            0 => f.verdict = VerdictName::Failed,
            1 => f.establishment = EstablishmentName::Provisional,
            2 => f.evidence_tier = None,
            3 => f.provenance.checks_unrecorded = 1,
            _ => report.summary.verified += 1,
        }
        assert!(complete_report(&report, CompletionPolicy::Receipt).is_none());
    }
    Ok(())
}

#[test]
fn missing_exit_errors_and_malformed_headers_never_mint_authority() -> TestResult {
    let original = passing_report()?;
    for case in 0..7 {
        let mut report = original.clone();
        match case {
            0 => report.exit_code = None,
            1 => report.exit_code = Some(1),
            2 => report.schema_version += 1,
            3 => {
                report.error = Some(ErrorInfo {
                    code: "SAIL3107".into(),
                    message: "failure".into(),
                });
            }
            4 => report.crates.clear(),
            5 => report.crates[0].verus.errors = None,
            _ => report.crates[0].verus.encountered_vir_error = Some(true),
        }
        assert!(complete_report(&report, CompletionPolicy::Receipt).is_none());
    }
    Ok(())
}

#[test]
fn every_actual_obligation_is_checked_and_sort_candidates_are_untrusted() -> TestResult {
    let mut report = passing_report()?;
    let first = report.crates[0].functions[0].obligations.clone();
    let Some(mut ids) = first else {
        return Err("fixture has no obligations".into());
    };
    assert_ne!(ids.len(), 0);
    ids.push(ids[0].clone());
    report.crates[0].functions[0].obligations = Some(ids);
    assert!(complete_report(&report, CompletionPolicy::Receipt).is_none());

    let mut report = passing_report()?;
    if let Some(ids) = &mut report.crates[0].functions[0].obligations {
        ids[0].id = "unrelated::owner#Function-Def/0".into();
    }
    assert!(complete_report(&report, CompletionPolicy::Receipt).is_none());

    let report = passing_report()?;
    assert!(complete_report_ordered(&report, CompletionPolicy::Receipt, &[], &[]).is_none());
    assert!(
        complete_report_ordered(&report, CompletionPolicy::Receipt, &[usize::MAX], &[0]).is_none()
    );
    assert!(complete_report(&report, CompletionPolicy::Receipt).is_some());
    Ok(())
}

#[test]
fn actual_forwarded_scope_blocks_whole_target_promotion() -> TestResult {
    for spelling in [
        "--verify-function=lemma",
        "--verify-module",
        "--verify-root",
        "--verify-only-module=m",
    ] {
        let mut report = passing_report()?;
        report.command = vec!["--".into(), spelling.into()];
        assert!(focused_command(&report.command));
        assert!(complete_report(&report, CompletionPolicy::Receipt).is_some());
        assert!(complete_whole_report(&report).is_none());
        assert!(!sail_api::repair::accepted(&project(vec![target(
            "app", report
        )])));
    }
    assert!(!focused_command(&[
        "--verify-function=before-separator".into()
    ]));
    assert!(!focused_command(&[
        "--".into(),
        "--verify-functional=near-prefix".into()
    ]));
    Ok(())
}

#[test]
fn exact_request_coverage_preserves_failed_siblings_and_colliding_names() -> TestResult {
    let good = target("foo-bar", passing_report()?);
    let mut bad = target("foo_bar", passing_report()?);
    bad.complete = false;
    bad.report.exit_code = Some(1);
    let targets = vec![good, bad];
    let expected: Vec<_> = targets
        .iter()
        .map(|t| requested_target(&t.package_id, &t.target, "bin"))
        .collect();
    assert!(requested_coverage(&expected, &targets));
    assert!(!sail_api::repair::accepted(&project(targets.clone())));
    assert!(!requested_coverage(&expected, &[targets[0].clone()]));
    assert!(!requested_coverage(
        &expected,
        &[targets[0].clone(), targets[0].clone(), targets[1].clone()]
    ));
    let mut unselected = targets;
    unselected[1].requested = false;
    assert!(!requested_coverage(&expected, &unselected));
    Ok(())
}

#[test]
fn selected_authority_refuses_duplicate_partial_and_stale_envelopes() -> TestResult {
    let good = target("app", passing_report()?);
    assert!(complete_target(&good).is_some());
    let full = project(vec![good.clone()]);
    assert!(sail_api::repair::accepted(&full));
    for case in 0..4 {
        let mut changed = full.clone();
        match case {
            0 => changed.currentness = Currentness::Historical,
            1 => changed.currentness = Currentness::Unknown,
            2 => changed.targets.push(good.clone()),
            _ => changed.targets[0].scope_reuse = Some(vec!["--".into(), "--verify-root".into()]),
        }
        assert!(!sail_api::repair::accepted(&changed));
    }
    Ok(())
}

fn replay_declared_readback<'a>(
    expected: &'a [sail_acceptance::inputs::InputFact<'a>],
    observed: &'a [sail_acceptance::inputs::InputFact<'a>],
    stamp: Option<&[u8]>,
    revision: &[u8],
) -> sail_acceptance::inputs::GuardedReadback<'a> {
    sail_acceptance::inputs::guard_readback(
        expected,
        observed,
        &[0],
        &[0],
        &[0],
        &[0],
        sail_acceptance::inputs::OriginalCapture::begin(Some(b"original observation stamp")),
        stamp,
        revision,
    )
}

#[test]
fn replay_transfer_requires_independent_requests_and_checked_readback() -> TestResult {
    use sail_acceptance::inputs::{InputFact, InputValue};
    let expected = [InputFact {
        namespace: 0,
        name: b"declared fixture source",
        value: InputValue::Present(b"original exact bytes"),
    }];
    for variant in [
        "positive",
        "changed-guard",
        "missing-guard",
        "changed-revision",
        "changed-input",
        "unreadable-input",
        "swapped-original",
        "changed-current-scope",
    ] {
        let observed = [InputFact {
            namespace: 0,
            name: b"declared fixture source",
            value: match variant {
                "changed-input" => InputValue::Present(b"changed exact bytes"),
                "unreadable-input" => InputValue::Unreadable,
                _ => InputValue::Present(b"original exact bytes"),
            },
        }];
        let stamp = match variant {
            "changed-guard" => Some(b"another observation stamp".as_slice()),
            "missing-guard" => None,
            _ => Some(b"original observation stamp".as_slice()),
        };
        let revision = if variant == "changed-revision" {
            b"another revision".as_slice()
        } else {
            b"revision".as_slice()
        };
        let readback = replay_declared_readback(&expected, &observed, stamp, revision);
        let mut actual = project(vec![target("app", passing_report()?)]);
        let original_project = actual.clone();
        let mut original = original_request();
        let mut current = original.clone();
        current.run_id = "independently allocated current view".into();
        if variant == "swapped-original" {
            original.workspace = "/another-original-workspace".into();
        }
        if variant == "changed-current-scope" {
            current.targets[0].target = "another-target".into();
        }
        let accepted = sail_api::acceptance::bind_replayed_project(
            &mut actual,
            &original,
            &current,
            &readback,
        );
        assert_eq!(accepted, variant == "positive", "{variant}");
        let mut expected_project = original_project;
        if accepted {
            expected_project.run_id.clone_from(&current.run_id);
            assert!(complete_request(&current, &actual).is_some());
        }
        assert_eq!(
            actual, expected_project,
            "all reports, original establishing identities and fields stay intact: {variant}"
        );
    }
    Ok(())
}

#[test]
fn replay_request_fidelity_checks_every_original_execution_field() {
    let original = original_request();
    for field in [
        "positive",
        "run",
        "same-run",
        "invocation",
        "workspace",
        "context",
        "revision",
        "arguments",
        "engine",
        "function",
        "repair-package",
        "repair-target",
        "repair-kind",
        "target-package",
        "target-name",
        "target-kind",
        "omitted-target",
    ] {
        let mut current = original.clone();
        current.run_id = "new independently allocated view".into();
        match field {
            "run" => current.run_id.clear(),
            "same-run" => current.run_id.clone_from(&original.run_id),
            "invocation" => current.invocation = "/other-invocation".into(),
            "workspace" => current.workspace = "/other-workspace".into(),
            "context" => current.context_sha256 = "other-context".into(),
            "revision" => current.input_revision = "other-revision".into(),
            "arguments" => current.args.push("--rlimit=100".into()),
            "engine" => {
                current.engine = Some(sail_api::project::EngineIdentity {
                    name: "interactive".into(),
                    manifest_sha256: "other-engine".into(),
                });
            }
            "function" => current.function = Some(String::new()),
            "repair-package" => current.repair_package = "other-package".into(),
            "repair-target" => current.repair_target = Some(String::new()),
            "repair-kind" => current.repair_kind = "lib".into(),
            "target-package" => current.targets[0].package_id = "other-package".into(),
            "target-name" => current.targets[0].target = "other-target".into(),
            "target-kind" => current.targets[0].kind = "lib".into(),
            "omitted-target" => current.targets.clear(),
            _ => (),
        }
        assert_eq!(
            sail_api::acceptance::replay_requests_match(&current, &original),
            field == "positive",
            "{field}"
        );
    }
}

#[test]
fn replay_keeps_enabled_zero_function_proofs_and_refuses_compile_only() -> TestResult {
    use sail_acceptance::inputs::{InputFact, InputValue};
    let facts = [InputFact {
        namespace: 0,
        name: b"declared fixture source",
        value: InputValue::Present(b"source bytes"),
    }];
    let readback = replay_declared_readback(
        &facts,
        &facts,
        Some(b"original observation stamp"),
        b"revision",
    );
    let mut report = passing_report()?;
    for krate in &mut report.crates {
        krate.functions.clear();
        krate.verus.verified = Some(0);
    }
    report.summary = Summary::of(&report.crates);
    let mut actual = project(vec![target("app", report)]);
    let original = original_request();
    let mut current = original.clone();
    current.run_id = "new independent view".into();
    assert!(sail_api::acceptance::bind_replayed_project(
        &mut actual,
        &original,
        &current,
        &readback
    ));
    assert_eq!(actual.targets[0].report.summary.functions, 0);
    assert_eq!(actual.targets[0].establishing_run, "establishing-run");
    let before = actual.clone();
    let mut disabled = current.clone();
    disabled.run_id = "another independent view".into();
    disabled.args = vec!["--".into(), "--no-verify".into()];
    assert!(!sail_api::acceptance::bind_replayed_project(
        &mut actual,
        &current,
        &disabled,
        &readback
    ));
    assert_eq!(actual, before);
    Ok(())
}

#[test]
fn request_paths_bind_exact_bytes_and_reject_lossy_replacement_aliases() -> TestResult {
    use sail_api::acceptance::bind_request_paths;
    for invocation_changed in [true, false] {
        let mut request = original_request();
        let invalid_native: &[u8] = b"/workspace/non-utf8-\xff";
        let replacement = String::from_utf8_lossy(invalid_native).into_owned();
        if invocation_changed {
            request.invocation = replacement;
        } else {
            request.workspace = replacement;
        }
        let invocation = if invocation_changed {
            invalid_native
        } else {
            request.invocation.as_bytes()
        };
        let workspace = if invocation_changed {
            request.workspace.as_bytes()
        } else {
            invalid_native
        };
        assert!(bind_request_paths(&request, invocation, workspace).is_none());

        // The real UTF-8 replacement-name path is a different native byte name.
        // Matching it is legitimate; matching the invalid-byte alias is not.
        let invocation = request.invocation.as_bytes();
        let workspace = request.workspace.as_bytes();
        let binding =
            bind_request_paths(&request, invocation, workspace).ok_or("exact UTF-8 paths")?;
        assert_eq!(binding.request(), &request);
        assert_eq!(binding.invocation(), invocation);
        assert_eq!(binding.workspace(), workspace);
    }
    Ok(())
}

#[test]
fn missing_or_swapped_native_paths_withhold_whole_and_partial_labels() -> TestResult {
    use sail_api::acceptance::{
        bind_request_paths, complete_exact_partial_request, withhold_unmatched_request_paths,
    };
    for partial in [false, true] {
        let mut request = original_request();
        let mut report = passing_report()?;
        if partial {
            request.args = vec!["--".into(), "--verify-module".into(), "m".into()];
            report.command.clone_from(&request.args);
            for krate in &mut report.crates {
                krate.verus.success = None;
            }
        }
        let mut actual = project(vec![target("app", report)]);
        if partial {
            actual.targets[0].complete = false;
            actual.targets[0].scope_reuse = Some(request.args.clone());
            assert!(complete_exact_partial_request(&request, &actual).is_some());
        } else {
            assert!(complete_request(&request, &actual).is_some());
        }
        let before = actual.clone();
        let binding =
            bind_request_paths(&request, b"/workspace", b"/workspace").ok_or("original paths")?;
        assert!(withhold_unmatched_request_paths(
            &mut actual,
            &request,
            Some(&binding)
        ));
        assert_eq!(actual, before);

        assert!(!withhold_unmatched_request_paths(
            &mut actual,
            &request,
            None
        ));
        let mut expected = before.clone();
        expected.targets[0].complete = false;
        expected.targets[0].scope_reuse = None;
        assert_eq!(actual, expected);
        assert!(complete_request(&request, &actual).is_none());
        assert!(complete_exact_partial_request(&request, &actual).is_none());

        let mut swapped = request.clone();
        swapped.invocation = "/different-invocation".into();
        let mut actual = before;
        assert!(!withhold_unmatched_request_paths(
            &mut actual,
            &swapped,
            Some(&binding)
        ));
        assert_eq!(actual, expected);
    }
    Ok(())
}

/// Declared value-domain composition only: these reports and input facts are
/// fixtures, not physical compiler/solver observations or service qualification.
#[test]
fn declared_replay_conjoins_scope_paths_association_and_guarded_inputs() -> TestResult {
    use sail_acceptance::inputs::{InputFact, InputValue};
    use sail_api::acceptance::{
        admit_declared_replay, bind_request_paths, present_declared_replay,
    };
    let facts = [InputFact {
        namespace: 0,
        name: b"declared nonempty fixture source",
        value: InputValue::Present(b"source bytes"),
    }];
    for partial in [false, true] {
        for variant in [
            "positive",
            "missing-paths",
            "missing-readback",
            "swapped-paths",
            "missing-guard",
            "changed-guard",
            "changed-revision",
            "swapped-original",
            "changed-scope",
            "missing-report-authority",
        ] {
            let mut original = original_request();
            let mut report = passing_report()?;
            if partial {
                original.args = vec!["--".into(), "--verify-module".into(), "m".into()];
                report.command.clone_from(&original.args);
                for krate in &mut report.crates {
                    krate.verus.success = None;
                }
            }
            let mut candidate = project(vec![target("app", report)]);
            if partial {
                candidate.targets[0].complete = false;
                candidate.targets[0].scope_reuse = Some(original.args.clone());
            }
            let mut current = original.clone();
            current.run_id = "independently allocated view".into();
            let mut path_request = current.clone();
            match variant {
                "swapped-paths" => path_request.invocation = "/different-native-invocation".into(),
                "swapped-original" => original.context_sha256 = "different-original-context".into(),
                "changed-scope" => current.args.push("different-current-scope".into()),
                "missing-report-authority" if partial => candidate.targets[0].scope_reuse = None,
                "missing-report-authority" => candidate.targets[0].complete = false,
                _ => (),
            }
            let paths = bind_request_paths(
                &path_request,
                path_request.invocation.as_bytes(),
                path_request.workspace.as_bytes(),
            )
            .ok_or("independent fixture native paths")?;
            let guard = match variant {
                "missing-guard" => None,
                "changed-guard" => Some(b"fixture guard".as_slice()),
                _ => Some(b"original observation stamp".as_slice()),
            };
            let revision = if variant == "changed-revision" {
                b"other revision".as_slice()
            } else {
                b"revision".as_slice()
            };
            let readback = replay_declared_readback(&facts, &facts, guard, revision);
            let mut expected = candidate.clone();
            expected.run_id.clone_from(&current.run_id);
            let admitted = admit_declared_replay(
                candidate,
                &original,
                &current,
                (variant != "missing-paths").then_some(&paths),
                (variant != "missing-readback").then_some(&readback),
            );
            assert_eq!(
                admitted.is_some(),
                variant == "positive",
                "partial={partial} {variant}"
            );
            if let Some(admitted) = admitted {
                assert_eq!(admitted.request(), &current);
                assert_eq!(admitted.project(), &expected);
                let view = present_declared_replay(admitted);
                assert_eq!(
                    view, expected,
                    "presentation preserves every admitted field"
                );
                assert_eq!(view.targets[0].establishing_run, "establishing-run");
                assert_eq!(complete_request(&current, &view).is_some(), !partial);
                assert_eq!(
                    sail_api::acceptance::complete_exact_partial_request(&current, &view).is_some(),
                    partial
                );
            }
        }
    }
    Ok(())
}
