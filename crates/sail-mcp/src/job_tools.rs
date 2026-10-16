//! Nonblocking durable verification jobs, shared with the CLI and watch.

use serde_json::{Map, Value, json};

use crate::args::RunArgs;
use crate::run::{self, Cancel};
use crate::tools::{Context, ToolResult, project_dir};

#[cfg(test)]
#[path = "job_tools_tests.rs"]
mod tests;

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
    if let Some(result) =
        read_request(&arguments).and_then(|request| direct_read(ctx, project, &request, cancel))
    {
        return match result {
            Ok(response) => match response_value(response) {
                Ok(document) => ToolResult::ok(
                    "Verification job response; proof outcomes are in the completed result."
                        .to_owned(),
                    document,
                ),
                Err(error) => invalid_response(&error.to_string()),
            },
            Err(error) => read_error(error),
        };
    }
    match run::run(&ctx.config.sail, &arguments, project, cancel) {
        Ok(outcome) if outcome.exit_code == Some(0) => {
            match serde_json::from_slice::<Value>(&outcome.stdout) {
                Ok(document) if valid_response(&arguments, &document) => ToolResult::ok(
                    "Verification job response; proof outcomes are in the completed result."
                        .to_owned(),
                    document,
                ),
                Err(error) => ToolResult::error(
                    format!("SAIL0813: invalid job response: {error}"),
                    json!({"error":"invalid_response"}),
                ),
                Ok(_) => ToolResult::error(
                    "SAIL0813: expected a supported durable job response".to_owned(),
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

// Move the already-decoded arbitrary JSON graph rather than serialize it into
// a second owned Value. Exhaustive destructuring keeps schema growth visible.
fn response_value(response: sail_api::jobs::Response) -> serde_json::Result<Value> {
    let sail_api::jobs::Response::Result(document) = response else {
        return serde_json::to_value(response);
    };
    let sail_api::jobs::ResultDocument {
        repair,
        job,
        result,
    } = document;
    let mut body = Map::new();
    if let Some(repair) = repair {
        body.insert("repair".to_owned(), serde_json::to_value(repair)?);
    }
    body.insert("job".to_owned(), serde_json::to_value(job)?);
    body.insert("result".to_owned(), result.unwrap_or(Value::Null));
    Ok(Value::Object(Map::from_iter([
        ("response".to_owned(), Value::String("result".to_owned())),
        ("value".to_owned(), Value::Object(body)),
    ])))
}

pub(super) fn direct_read(
    ctx: &Context,
    project: &std::path::Path,
    request: &sail_api::jobs::Request,
    cancel: &Cancel,
) -> Option<Result<sail_api::jobs::Response, crate::job_clients::ReadError>> {
    ctx.jobs.as_ref()?.call(project, request, cancel)
}
fn invalid_response(message: &str) -> ToolResult {
    ToolResult::error(
        format!("SAIL0813: invalid job response: {message}"),
        json!({"error":"invalid_response"}),
    )
}
fn unsupported_response() -> ToolResult {
    ToolResult::error(
        "SAIL0813: expected a supported durable job response".to_owned(),
        json!({"error":"invalid_response"}),
    )
}
fn read_error(error: crate::job_clients::ReadError) -> ToolResult {
    match error {
        crate::job_clients::ReadError::InvalidResponse => unsupported_response(),
        crate::job_clients::ReadError::Cancelled => ToolResult::error(
            "SAIL0813: job read cancelled".to_owned(),
            json!({"error":"cancelled"}),
        ),
        crate::job_clients::ReadError::Operation(error) => ToolResult::error(
            format!("SAIL0813: job read failed: {error}"),
            json!({"error":"read_failed"}),
        ),
    }
}

fn read_request(args: &[String]) -> Option<sail_api::jobs::Request> {
    use sail_api::jobs::Request;
    if args.first()?.as_str() != "jobs" {
        return None;
    }
    let id = args.get(2)?.clone();
    match args.get(1)?.as_str() {
        "status" => Some(Request::Status { id }),
        "result" => Some(Request::Result { id }),
        _ => None,
    }
}

fn valid_response(arguments: &[String], document: &Value) -> bool {
    use sail_api::jobs::{Response, VERSION};
    if arguments
        .first()
        .is_some_and(|argument| argument == "verify")
    {
        return serde_json::from_value::<sail_api::jobs::Status>(document.clone())
            .is_ok_and(|status| status.version == VERSION);
    }
    match serde_json::from_value::<Response>(document.clone()) {
        Ok(Response::Result(result)) => {
            arguments
                .get(1)
                .is_some_and(|argument| argument == "result")
                && result.job.version == VERSION
                && arguments.get(2).is_some_and(|id| *id == result.job.id)
        }
        Ok(Response::Status(status)) => {
            arguments
                .get(1)
                .is_some_and(|argument| argument == "status" || argument == "cancel")
                && status.version == VERSION
                && arguments.get(2).is_some_and(|id| *id == status.id)
        }
        _ => false,
    }
}

/// A blocking compatibility call still uses the same durable worker as watch.
///
/// It is not forced fresh: like the CLI, a Current receipt for unchanged
/// inputs is reused, and its establishing report is returned.
pub(super) fn blocking(
    ctx: &Context,
    request: &RunArgs,
    project: &std::path::Path,
    cancel: &Cancel,
) -> Result<Option<(sail_api::jobs::Status, sail_api::project::ProjectResult)>, ToolResult> {
    let args = request.sail_args(&["verify", "--background", "--output", "json"]);
    let started = run::run(&ctx.config.sail, &args, project, cancel).map_err(|error| {
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
        if !crate::job_clients::response_matches(
            &sail_api::jobs::Request::Result {
                id: status.id.clone(),
            },
            &response,
        ) {
            return Err(invalid_response(
                "unexpected job identity, version or response",
            ));
        }
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

/// Requested targets' reports, fresh or reused. A reused target carries the
/// complete report of the run that established it.
pub(super) fn merged_report(
    result: &sail_api::project::ProjectResult,
) -> Option<sail_api::output::Report> {
    let mut targets = result.targets.iter().filter(|target| target.requested);
    let mut report = targets.next()?.report.clone();
    for target in targets {
        let next = &target.report;
        report.crates.extend(next.crates.clone());
        report.diagnostics.extend(next.diagnostics.clone());
        report
            .attribution
            .notes
            .extend(next.attribution.notes.clone());
        report.attribution.unmatched_sessions = report
            .attribution
            .unmatched_sessions
            .saturating_add(next.attribution.unmatched_sessions);
        if next.attribution.status != sail_api::output::AttributionStatus::Complete {
            report.attribution.status = sail_api::output::AttributionStatus::Partial;
        }
        report.summary.functions = report
            .summary
            .functions
            .saturating_add(next.summary.functions);
        report.summary.verified = report
            .summary
            .verified
            .saturating_add(next.summary.verified);
        report.summary.failed = report.summary.failed.saturating_add(next.summary.failed);
        report.summary.unknown = report.summary.unknown.saturating_add(next.summary.unknown);
        report.summary.provisional = report
            .summary
            .provisional
            .saturating_add(next.summary.provisional);
        report.summary.unstable = report
            .summary
            .unstable
            .saturating_add(next.summary.unstable);
    }
    report.exit_code = result.exit_code;
    Some(report)
}

pub(super) fn lookup(
    ctx: &Context,
    project: &std::path::Path,
    args: &crate::args::LookupArgs,
    models: bool,
    cancel: &Cancel,
) -> Option<ToolResult> {
    let listing = read_or_cli(
        ctx,
        project,
        &sail_api::jobs::Request::List { project_dir: None },
        &[
            "jobs".to_owned(),
            "list".to_owned(),
            "--output".to_owned(),
            "json".to_owned(),
        ],
        cancel,
    )
    .ok()?;
    let sail_api::jobs::Response::List(mut jobs) = listing else {
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
        let request = sail_api::jobs::Request::Result { id: job.id.clone() };
        let result = read_or_cli(
            ctx,
            project,
            &request,
            &[
                "jobs".to_owned(),
                "result".to_owned(),
                job.id,
                "--output".to_owned(),
                "json".to_owned(),
            ],
            cancel,
        )
        .ok()?;
        let sail_api::jobs::Response::Result(document) = result else {
            continue;
        };
        let Some(value) = document.result else {
            continue;
        };
        let Ok(result) = serde_json::from_value::<sail_api::project::ProjectResult>(value) else {
            continue;
        };
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
        if let Some(error) = missing_obligation(&functions, args, &document.job) {
            return Some(error);
        }
        let entries: Vec<_> = functions.iter().map(|(target, function)| {
            let obligations: Vec<_> = function.obligations.iter().flatten().filter(|obligation| args.obligation.as_ref().is_none_or(|id| id.split_once('#').is_some_and(|pair| obligation.id == format!("{}#{}", function.function, pair.1)))).collect();
            json!({"package_id":target.package_id,"target":target.target,"establishing_run":target.establishing_run,"reused":target.reused,"function":function.function,"verdict":crate::summary::verdict_name(crate::summary::effective_verdict(function)),"establishment":function.establishment,"evidence_tier":crate::summary::effective_evidence_tier(function),"mode":function.provenance.mode,"configuration":function.provenance.configuration,"function_report":function,"obligations":if models {serde_json::to_value(&obligations).unwrap_or(Value::Null)}else{Value::Null},"counterexamples":if models {serde_json::to_value(obligations.iter().filter_map(|obligation| obligation.counterexample.as_ref()).collect::<Vec<_>>()).unwrap_or(Value::Null)}else{Value::Null}})
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

fn read_or_cli(
    ctx: &Context,
    project: &std::path::Path,
    request: &sail_api::jobs::Request,
    args: &[String],
    cancel: &Cancel,
) -> std::io::Result<sail_api::jobs::Response> {
    if let Some(result) = direct_read(ctx, project, request, cancel) {
        return result.map_err(|error| match error {
            crate::job_clients::ReadError::Operation(error) => error,
            crate::job_clients::ReadError::InvalidResponse => {
                std::io::Error::other("SAIL0813: invalid job response")
            }
            crate::job_clients::ReadError::Cancelled => {
                std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled")
            }
        });
    }
    let result = run::run(&ctx.config.sail, args, project, cancel)?;
    let response = serde_json::from_slice(&result.stdout).map_err(std::io::Error::other)?;
    if result.exit_code != Some(0) || !crate::job_clients::response_matches(request, &response) {
        return Err(std::io::Error::other("SAIL0813: invalid job response"));
    }
    Ok(response)
}

fn missing_obligation(
    functions: &[(
        &sail_api::project::TargetResult,
        &sail_api::output::FunctionReport,
    )],
    args: &crate::args::LookupArgs,
    job: &sail_api::jobs::Status,
) -> Option<ToolResult> {
    let id = args.obligation.as_ref()?;
    let function = functions.first()?.1;
    let suffix = id.split_once('#').map_or("", |pair| pair.1);
    let canonical = format!("{}#{suffix}", function.function);
    if function
        .obligations
        .iter()
        .flatten()
        .any(|obligation| obligation.id == canonical)
    {
        None
    } else {
        Some(ToolResult::error(
            format!("No obligation {id} in the latest recorded function result."),
            json!({"error":"not_found","obligation":id,"job":job}),
        ))
    }
}

/// One line per requested target whose report was reused, naming the run that
/// established it (charter rule 7: confirmed results record their provenance).
/// A run without proof reuse (`SAIL3112`) says so first, with its reason.
pub(super) fn reuse_lines(result: &sail_api::project::ProjectResult) -> Vec<String> {
    result
        .no_reuse
        .iter()
        .map(no_reuse_line)
        .chain(
            result
                .targets
                .iter()
                .filter(|target| target.requested && target.reused)
                .map(|target| {
                    format!(
                        "{}: reused the complete report established by run {} for unchanged inputs.",
                        target.target, target.establishing_run
                    )
                }),
        )
        .collect()
}

/// The provenance line of a run without proof reuse.
fn no_reuse_line(no_reuse: &sail_api::project::NoReuse) -> String {
    let how = match no_reuse.verified_as {
        sail_api::project::VerifiedAs::PlainCargoVerus => {
            "verified as `cargo verus verify` does, in the project's own Cargo target, with Sail's solver cache; crates Cargo found up to date were not verified again and have no report"
        }
        sail_api::project::VerifiedAs::Managed => {
            "verified what Cargo rebuilt in Sail's target; targets Cargo found up to date have no report"
        }
    };
    format!(
        "No proof reuse ({}): {}; {how}. No proof receipt was reused or recorded, so this result is never current.",
        no_reuse.code, no_reuse.reason
    )
}
