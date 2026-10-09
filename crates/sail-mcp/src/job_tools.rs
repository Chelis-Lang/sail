//! Nonblocking durable verification jobs, shared with the CLI and watch.

use serde_json::{Map, Value, json};

use crate::args::RunArgs;
use crate::run::{self, Cancel};
use crate::tools::{Context, ToolResult, project_dir};

pub(super) fn definitions(run_schema: &Value) -> Vec<Value> {
    let id_schema = json!({"type":"object","additionalProperties":false,"required":["job_id"],"properties":{"job_id":{"type":"string","description":"Durable job id returned by start_verification."}}});
    vec![
        json!({"name":"start_verification","title":"Start workspace verification","description":"Submit persistent incremental workspace verification and return immediately. Successful Cargo targets reuse complete proof receipts; results retain original establishing provenance. Use verification_status and verification_result while you continue working. Portfolio and fragility use their existing explicit tools.","inputSchema":run_schema,"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
        json!({"name":"verification_status","title":"Verification job status","description":"Read durable job lifecycle without waiting for a verifier. Lifecycle is distinct from proof outcome.","inputSchema":id_schema,"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"verification_result","title":"Verification job result","description":"Read complete per-target reports, establishing provenance, currentness and counterexamples from a retained job. Running jobs return status without blocking. Historical results apply only to their earlier input revision.","inputSchema":id_schema,"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"cancel_verification","title":"Cancel verification job","description":"Cancel a queued or running job and its complete verifier process group. Completed results remain available.","inputSchema":id_schema,"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
    ]
}

pub(super) fn call(
    ctx: &Context,
    name: &str,
    args: &Map<String, Value>,
    cancel: &Cancel,
) -> ToolResult {
    if name == "start_verification" {
        return start(ctx, args, cancel);
    }
    if args.len() != 1 || !args.contains_key("job_id") {
        return ToolResult::invalid("expected exactly one argument: job_id");
    }
    let Some(id) = args["job_id"]
        .as_str()
        .filter(|id| !id.is_empty() && !id.contains('\0') && !id.starts_with('-'))
    else {
        return ToolResult::invalid(
            "job_id must be a nonempty string returned by start_verification",
        );
    };
    let operation = match name {
        "verification_status" => "status",
        "verification_result" => "result",
        "cancel_verification" => "cancel",
        _ => return ToolResult::invalid("unknown job tool"),
    };
    execute(
        ctx,
        &["jobs".to_owned(), operation.to_owned(), id.to_owned()],
        &ctx.config.cwd,
        cancel,
    )
}
fn start(ctx: &Context, args: &Map<String, Value>, cancel: &Cancel) -> ToolResult {
    let request = match RunArgs::parse(args) {
        Ok(request) => request,
        Err(error) => return ToolResult::invalid(&error),
    };
    if request.measure_fragility || request.portfolio.is_some() {
        return ToolResult::invalid(
            "persistent incremental jobs support default verification; use verify or fragility for explicit solver experiments",
        );
    }
    let project = match project_dir(ctx, request.project_dir.as_deref()) {
        Ok(project) => project,
        Err(error) => return error,
    };
    let arguments = request.sail_args(&["verify", "--background", "--output", "json"]);
    execute(ctx, &arguments, &project, cancel)
}
fn execute(
    ctx: &Context,
    arguments: &[String],
    project: &std::path::Path,
    cancel: &Cancel,
) -> ToolResult {
    let mut arguments = arguments.to_vec();
    if arguments.first().is_some_and(|argument| argument == "jobs") {
        arguments.extend(["--output".to_owned(), "json".to_owned()]);
    }
    match run::run(&ctx.config.sail, &arguments, project, cancel) {
        Ok(outcome) if outcome.exit_code == Some(0) => {
            match serde_json::from_slice::<Value>(&outcome.stdout) {
                Ok(document) => ToolResult::ok(
                    "Verification job response; proof outcomes are in the completed result."
                        .to_owned(),
                    document,
                ),
                Err(error) => ToolResult::error(
                    format!("SAIL0813: invalid job response: {error}"),
                    json!({"error":"invalid_response"}),
                ),
            }
        }
        Ok(outcome) => ToolResult::error(
            format!("SAIL0813: job operation failed: {}", outcome.stderr_tail),
            json!({"exit_code":outcome.exit_code}),
        ),
        Err(error) => ToolResult::error(
            format!("SAIL0813: cannot contact project service: {error}"),
            json!({"error":"service_unavailable"}),
        ),
    }
}

/// A blocking compatibility call still uses the same durable worker as watch.
pub(super) fn blocking(
    ctx: &Context,
    request: &RunArgs,
    project: &std::path::Path,
    cancel: &Cancel,
) -> Result<Option<(sail_api::jobs::Status, sail_api::project::ProjectResult)>, ToolResult> {
    let args = request.sail_args(&["verify", "--background", "--output", "json"]);
    let started = run::run_with_environment(
        &ctx.config.sail,
        &args,
        project,
        cancel,
        &[("SAIL_MANAGED_FORCE", "1")],
    )
    .map_err(|error| {
        ToolResult::error(
            format!("SAIL0813: {error}"),
            json!({"error":"service_unavailable"}),
        )
    })?;
    let Ok(status) = serde_json::from_slice::<sail_api::jobs::Status>(&started.stdout) else {
        // Older installations support the original report protocol only.
        if serde_json::from_slice::<sail_api::output::Report>(&started.stdout).is_ok() {
            return Ok(None);
        }
        return Err(ToolResult::error(
            format!(
                "SAIL0813: cannot submit verification: {}",
                started.stderr_tail
            ),
            json!({"error":"submission_failed"}),
        ));
    };
    let poll = [
        "jobs".to_owned(),
        "result".to_owned(),
        status.id.clone(),
        "--output".to_owned(),
        "json".to_owned(),
    ];
    loop {
        if cancel.is_cancelled() {
            let args = ["jobs".to_owned(), "cancel".to_owned(), status.id.clone()];
            let _ = run::run(&ctx.config.sail, &args, project, &Cancel::default());
            return Err(ToolResult::error(
                "Verification cancelled.".to_owned(),
                json!({"job_id":status.id,"cancelled":true}),
            ));
        }
        let outcome =
            run::run(&ctx.config.sail, &poll, project, &Cancel::default()).map_err(|error| {
                ToolResult::error(format!("SAIL0813: {error}"), json!({"job_id":status.id}))
            })?;
        let response = serde_json::from_slice::<sail_api::jobs::Response>(&outcome.stdout)
            .map_err(|error| {
                ToolResult::error(
                    format!("SAIL0813: invalid result: {error}"),
                    json!({"job_id":status.id}),
                )
            })?;
        if let sail_api::jobs::Response::Result(document) = response {
            if let Some(result) = document.result {
                let project = serde_json::from_value(result).map_err(|error| {
                    ToolResult::error(
                        format!("SAIL0813: invalid project result: {error}"),
                        json!({"job_id":status.id}),
                    )
                })?;
                return Ok(Some((document.job, project)));
            }
            if document.job.state.terminal() {
                return Err(ToolResult::error(
                    format!(
                        "Verification job {:?}: {}",
                        document.job.state,
                        document.job.error.unwrap_or_default()
                    ),
                    json!({"job_id":status.id,"state":document.job.state}),
                ));
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub(super) fn merged_report(
    result: &sail_api::project::ProjectResult,
) -> Option<sail_api::output::Report> {
    let mut targets = result
        .targets
        .iter()
        .filter(|target| target.requested && !target.reused);
    let mut report = targets.next()?.report.clone();
    for target in targets {
        let next = &target.report;
        report.crates.extend(next.crates.clone());
        report.diagnostics.extend(next.diagnostics.clone());
        report
            .attribution
            .notes
            .extend(next.attribution.notes.clone());
        report.attribution.unmatched_sessions += next.attribution.unmatched_sessions;
        if next.attribution.status != sail_api::output::AttributionStatus::Complete {
            report.attribution.status = sail_api::output::AttributionStatus::Partial;
        }
        report.summary.functions += next.summary.functions;
        report.summary.verified += next.summary.verified;
        report.summary.failed += next.summary.failed;
        report.summary.unknown += next.summary.unknown;
        report.summary.provisional += next.summary.provisional;
        report.summary.unstable += next.summary.unstable;
    }
    report.exit_code = result.exit_code;
    Some(report)
}

pub(super) fn lookup(
    ctx: &Context,
    project: &std::path::Path,
    args: &crate::args::LookupArgs,
    models: bool,
) -> Option<ToolResult> {
    let cancel = Cancel::default();
    let listing = run::run(
        &ctx.config.sail,
        &[
            "jobs".to_owned(),
            "list".to_owned(),
            "--output".to_owned(),
            "json".to_owned(),
        ],
        project,
        &cancel,
    )
    .ok()?;
    let sail_api::jobs::Response::List(mut jobs) = serde_json::from_slice(&listing.stdout).ok()?
    else {
        return None;
    };
    jobs.retain(|job| {
        job.project_dir == project && job.state == sail_api::jobs::Lifecycle::Completed
    });
    jobs.sort_by_key(|job| std::cmp::Reverse(job.submitted_at_ms));
    let query = args.function.as_deref().or_else(|| {
        args.obligation
            .as_deref()
            .and_then(|obligation| obligation.split_once('#').map(|pair| pair.0))
    });
    for job in jobs {
        let result = run::run(
            &ctx.config.sail,
            &[
                "jobs".to_owned(),
                "result".to_owned(),
                job.id,
                "--output".to_owned(),
                "json".to_owned(),
            ],
            project,
            &cancel,
        )
        .ok()?;
        let sail_api::jobs::Response::Result(document) =
            serde_json::from_slice(&result.stdout).ok()?
        else {
            continue;
        };
        let result: sail_api::project::ProjectResult =
            serde_json::from_value(document.result?).ok()?;
        let functions: Vec<_> = result
            .targets
            .iter()
            .filter(|target| target.requested)
            .flat_map(|target| {
                target.report.crates.iter().flat_map(move |krate| {
                    krate
                        .functions
                        .iter()
                        .map(move |function| (target, function))
                })
            })
            .filter(|(_, function)| {
                query.is_none_or(|query| {
                    function.function == query || function.function.ends_with(&format!("::{query}"))
                })
            })
            .collect();
        if functions.is_empty() {
            continue;
        }
        if query.is_some() && functions.len() > 1 {
            return Some(ToolResult::error("Ambiguous function; select its complete reported path and target through verification_result.".to_owned(), json!({"error":"ambiguous","job":document.job,"functions":functions.iter().map(|(_, function)| &function.function).collect::<Vec<_>>()})));
        }
        let entries: Vec<_> = functions.iter().map(|(target, function)| {
            let obligations: Vec<_> = function.obligations.iter().flatten().filter(|obligation| args.obligation.as_ref().is_none_or(|id| *id == obligation.id)).collect();
            json!({"package_id":target.package_id,"target":target.target,"establishing_run":target.establishing_run,"reused":target.reused,"function":function,"counterexamples":if models {serde_json::to_value(obligations.iter().filter_map(|obligation| obligation.counterexample.as_ref()).collect::<Vec<_>>()).unwrap_or(Value::Null)}else{Value::Null}})
        }).collect();
        return Some(ToolResult::ok(
            format!(
                "{} function result(s) from job {}; currentness {:?}.",
                entries.len(),
                document.job.id,
                document.job.currentness
            ),
            json!({"project_dir":project,"job":document.job,"functions":entries}),
        ));
    }
    None
}
