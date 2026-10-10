use super::{
    args::Args,
    process::{self, Process},
};
use sail_api::{
    jobs::{self, Client, Currentness, Lifecycle, Request, Response, Status},
    project::ProjectResult,
};
use serde::Serialize;
use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

const SAMPLES: usize = 200;
const CLIENTS: usize = 4;
#[derive(Serialize)]
struct Sample {
    client_id: usize,
    operation: &'static str,
    wall_ms: f64,
    current: bool,
    error: Option<String>,
    active_before: bool,
    active_after: bool,
    verifiers_before: Vec<Process>,
    verifiers_after: Vec<Process>,
}
#[derive(Serialize)]
struct Summary {
    count: usize,
    median_ms: f64,
    p95_ms: f64,
    max_ms: f64,
}
#[derive(Serialize)]
struct Evidence<'a> {
    schema_version: u32,
    client: &'static str,
    execution_environment: &'a str,
    clients: usize,
    deadline_seconds: u32,
    job: &'a str,
    active_job: &'a str,
    active_worker: Process,
    input_revision: String,
    establishment_ms: f64,
    establishment_reads: usize,
    samples: Vec<Sample>,
    summary: std::collections::BTreeMap<&'static str, Summary>,
    passed: bool,
}
fn status(client: &Client, job: &str) -> io::Result<Status> {
    match client.call(&Request::Status { id: job.to_owned() })? {
        Response::Status(status) => Ok(status),
        _ => Err(io::Error::other("unexpected status response")),
    }
}
fn result(client: &Client, job: &str) -> io::Result<jobs::ResultDocument> {
    match client.call(&Request::Result { id: job.to_owned() })? {
        Response::Result(document) => Ok(document),
        _ => Err(io::Error::other("unexpected result response")),
    }
}
fn valid_status(status: &Status, job: &str, revision: &str) -> bool {
    status.id == job
        && status.state == Lifecycle::Completed
        && status.currentness == Currentness::Current
        && status.error.is_none()
        && status.input_revision.as_deref() == Some(revision)
}
fn valid_project(project: &ProjectResult) -> bool {
    project.kind == "sail-project-result"
        && project.schema_version == sail_api::project::SCHEMA_VERSION
        && project.currentness == sail_api::project::Currentness::Current
        && project.error.is_none()
        && !project.targets.is_empty()
        && project
            .targets
            .iter()
            .all(|t| t.complete && t.report.error.is_none())
}
fn full(document: &jobs::ResultDocument) -> io::Result<ProjectResult> {
    // The real product Client has already decoded the entire bounded response,
    // including every report and repair field. This extra complete typed check
    // never projects away functions or replaces transfer/decode with a seal.
    serde_json::from_value(
        document
            .result
            .clone()
            .ok_or_else(|| io::Error::other("missing full result"))?,
    )
    .map_err(io::Error::other)
}
fn valid_result(document: &jobs::ResultDocument, expected: &ProjectResult, job: &str) -> bool {
    valid_status(&document.job, job, &expected.input_revision)
        && full(document).is_ok_and(|p| valid_project(&p) && p == *expected)
        && document
            .repair
            .as_ref()
            .is_none_or(|r| r.batch.full == *expected && r.accepted == r.batch.accepted())
}
fn establish(client: &Client, job: &str) -> io::Result<(ProjectResult, usize, f64)> {
    let start = Instant::now();
    let mut reads = 0;
    while start.elapsed() < Duration::from_secs(600) {
        let document = result(client, job)?;
        reads += 1;
        if let Ok(project) = full(&document)
            && valid_project(&project)
            && valid_result(&document, &project, job)
        {
            return Ok((project, reads, start.elapsed().as_secs_f64() * 1000.0));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(io::Error::other(
        "completed current full evidence never established",
    ))
}
fn sample(
    client: &Client,
    args: &Args,
    owner: &Process,
    expected: &ProjectResult,
    index: usize,
    client_id: usize,
) -> io::Result<Sample> {
    let active_before = status(client, &args.active_job)?.state == Lifecycle::Running;
    let verifiers_before = process::verifiers(&process::table()?, owner);
    let start = Instant::now();
    let operation = if index.is_multiple_of(2) {
        "status"
    } else {
        "result"
    };
    // The clock includes the actual product Client's complete frame read and
    // typed response decode; validation and process sampling are outside it,
    // exactly as in the independent Python census.
    let response = client.call(&if operation == "status" {
        Request::Status {
            id: args.job.clone(),
        }
    } else {
        Request::Result {
            id: args.job.clone(),
        }
    })?;
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    let verifiers_after = process::verifiers(&process::table()?, owner);
    let active_after = status(client, &args.active_job)?.state == Lifecycle::Running;
    let current = match response {
        Response::Status(status) => valid_status(&status, &args.job, &expected.input_revision),
        Response::Result(document) => valid_result(&document, expected, &args.job),
        _ => false,
    };
    Ok(Sample {
        client_id,
        operation,
        wall_ms,
        current,
        error: None,
        active_before,
        active_after,
        verifiers_before,
        verifiers_after,
    })
}
fn summarize(samples: &[Sample], operation: &str) -> io::Result<Summary> {
    let mut values: Vec<_> = samples
        .iter()
        .filter(|s| s.operation == operation)
        .map(|s| s.wall_ms)
        .collect();
    values.sort_by(f64::total_cmp);
    let count = values.len();
    if count == 0 {
        return Err(io::Error::other("no operation samples"));
    }
    let median_ms = if count.is_multiple_of(2) {
        values[count / 2 - 1].midpoint(values[count / 2])
    } else {
        values[count / 2]
    };
    Ok(Summary {
        count,
        median_ms,
        p95_ms: values[(95 * count).div_ceil(100) - 1],
        max_ms: values[count - 1],
    })
}
fn collect_samples(
    client: &Client,
    args: &Args,
    owner: &Process,
    expected: &ProjectResult,
) -> io::Result<Vec<Sample>> {
    let barrier = Arc::new(std::sync::Barrier::new(CLIENTS));
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CLIENTS)
            .map(|client_id| {
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    let mut samples = Vec::new();
                    barrier.wait();
                    for offset in 0..SAMPLES / CLIENTS {
                        let index = client_id * (SAMPLES / CLIENTS) + offset;
                        let started = Instant::now();
                        let measured = sample(client, args, owner, expected, index, client_id)
                            .unwrap_or_else(|error| Sample {
                                client_id,
                                operation: if index.is_multiple_of(2) {
                                    "status"
                                } else {
                                    "result"
                                },
                                wall_ms: started.elapsed().as_secs_f64() * 1000.0,
                                current: false,
                                error: Some(error.to_string()),
                                active_before: false,
                                active_after: false,
                                verifiers_before: Vec::new(),
                                verifiers_after: Vec::new(),
                            });
                        samples.push(measured);
                    }
                    Ok::<_, io::Error>(samples)
                })
            })
            .collect();
        handles
            .into_iter()
            .try_fold(Vec::new(), |mut output, handle| {
                output.extend(
                    handle
                        .join()
                        .map_err(|_| io::Error::other("reader thread failed"))??,
                );
                Ok::<_, io::Error>(output)
            })
    })
}
pub(super) fn run(args: &Args) -> io::Result<bool> {
    if !cfg!(target_os = "linux") || std::env::var_os("CI").is_some() {
        return Err(io::Error::other(
            "the real reader census is Linux and local-only",
        ));
    }
    let client = Client::new(jobs::socket_path(&args.socket));
    let active = status(&client, &args.active_job)?;
    if active.state != Lifecycle::Running {
        return Err(io::Error::other("selected active job is not running"));
    }
    let owner = process::owner(args.worker, &active.project_dir)?;
    let (expected, establishment_reads, establishment_ms) = establish(&client, &args.job)?;
    let samples = collect_samples(&client, args, &owner, &expected)?;
    let summary = std::collections::BTreeMap::from([
        ("status", summarize(&samples, "status")?),
        ("result", summarize(&samples, "result")?),
    ]);
    let passed = samples.len() == SAMPLES
        && samples.iter().all(|s| {
            s.current
                && s.active_before
                && s.active_after
                && !s.verifiers_before.is_empty()
                && !s.verifiers_after.is_empty()
        })
        && summary.values().all(|s| s.p95_ms < 200.0);
    let evidence = Evidence {
        schema_version: 1,
        client: "sail_api::jobs::Client; full typed response",
        execution_environment: &args.environment,
        clients: CLIENTS,
        deadline_seconds: 5,
        job: &args.job,
        active_job: &args.active_job,
        active_worker: owner,
        input_revision: expected.input_revision,
        establishment_ms,
        establishment_reads,
        samples,
        summary,
        passed,
    };
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &args.out,
        serde_json::to_vec_pretty(&evidence).map_err(io::Error::other)?,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence.summary).map_err(io::Error::other)?
    );
    println!("passed={passed}");
    Ok(passed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_state_both_revision_and_currentness_are_mandatory() {
        let mut status = Status {
            version: 1,
            id: "done".into(),
            project_dir: "project".into(),
            args: vec![],
            state: Lifecycle::Completed,
            submitted_at_ms: 0,
            started_at_ms: Some(0),
            finished_at_ms: Some(1),
            exit_code: Some(0),
            input_revision: Some("revision".into()),
            currentness: Currentness::Current,
            error: None,
        };
        assert!(valid_status(&status, "done", "revision"));
        assert!(!valid_status(&status, "other", "revision"));
        assert!(!valid_status(&status, "done", "different"));
        status.state = Lifecycle::Running;
        assert!(!valid_status(&status, "done", "revision"));
        status.state = Lifecycle::Completed;
        status.currentness = Currentness::Unknown;
        assert!(!valid_status(&status, "done", "revision"));
        status.currentness = Currentness::Current;
        status.error = Some("broken".into());
        assert!(!valid_status(&status, "done", "revision"));
    }
    #[test]
    fn full_result_keeps_targets_and_rejects_project_revision_or_error_changes() {
        let report: sail_api::output::Report = serde_json::from_value(serde_json::json!({
            "schema_version":1,"sail_version":"fixture","mode":"default","command":[],
            "exit_code":0,"solver":null,"cache":{"daemon":false},
            "attribution":{"status":"unavailable","notes":[],"unmatched_sessions":0},
            "crates":[],"diagnostics":[],"error":null,
            "summary":{"functions":0,"verified":0,"failed":0,"unknown":0,"provisional":0,"unstable":0}
        })).unwrap_or_else(|e|unreachable!("fixture:{e}"));
        let expected = ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: 1,
            run_id: "run".into(),
            workspace: "workspace".into(),
            context_sha256: "context".into(),
            input_revision: "revision".into(),
            currentness: sail_api::project::Currentness::Current,
            exit_code: Some(0),
            execution: sail_api::project::Execution::default(),
            notes: vec![],
            error: None,
            targets: vec![sail_api::project::TargetResult {
                package_id: "package".into(),
                target: "target".into(),
                kind: vec!["lib".into()],
                establishing_run: "run".into(),
                reused: false,
                requested: true,
                complete: true,
                report,
            }],
        };
        let status = Status {
            version: 1,
            id: "done".into(),
            project_dir: "workspace".into(),
            args: vec![],
            state: Lifecycle::Completed,
            submitted_at_ms: 0,
            started_at_ms: Some(0),
            finished_at_ms: Some(1),
            exit_code: Some(0),
            input_revision: Some("revision".into()),
            currentness: Currentness::Current,
            error: None,
        };
        let mut document = jobs::ResultDocument {
            job: status,
            result: serde_json::to_value(&expected).ok(),
            repair: None,
        };
        assert!(valid_result(&document, &expected, "done"));
        document.job.input_revision = Some("other".into());
        assert!(!valid_result(&document, &expected, "done"));
        document.job.input_revision = Some("revision".into());
        for case in ["revision", "target", "report", "error", "currentness"] {
            let mut changed = expected.clone();
            match case {
                "revision" => changed.input_revision = "other".into(),
                "target" => changed.targets[0].complete = false,
                "report" => changed.targets[0].report.mode = "different-mode".into(),
                "error" => {
                    changed.error = Some(sail_api::output::ErrorInfo {
                        code: "SAIL0000".into(),
                        message: "failure".into(),
                    });
                }
                _ => changed.currentness = sail_api::project::Currentness::Historical,
            }
            document.result = serde_json::to_value(changed).ok();
            assert!(!valid_result(&document, &expected, "done"), "case:{case}");
        }
    }
    #[test]
    fn p95_keeps_all_tail_samples() {
        let samples: Vec<_> = (1..=100)
            .map(|n| Sample {
                client_id: 0,
                operation: "status",
                wall_ms: f64::from(n),
                current: true,
                error: None,
                active_before: true,
                active_after: true,
                verifiers_before: vec![],
                verifiers_after: vec![],
            })
            .collect();
        let summary = summarize(&samples, "status").unwrap_or_else(|e| unreachable!("fixture:{e}"));
        assert!((summary.p95_ms - 95.0).abs() < f64::EPSILON);
        assert_eq!(summary.count, 100);
        assert!((summary.max_ms - 100.0).abs() < f64::EPSILON);
    }
}
