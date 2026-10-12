//! Reader-latency probe for the M7 reader gate (`docs/validation/m7.md`).
//!
//! Four clients read the Status and complete Result of one completed, Current
//! job through the product's `sail_api::jobs::Client` while another job is
//! running. It prints p50/p95/p99 per operation and exits nonzero unless every
//! sample is Current, the active job stays running throughout, and both p95
//! values are below 200 ms.
//!
//! `cargo run --release -p sail-cli --example job_read_bench -- --socket S --job DONE --active-job RUNNING [--out FILE]`
#![forbid(unsafe_code)]

use sail_api::jobs::{self, Client, Currentness, Lifecycle, Request, Response};
use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

const CLIENTS: usize = 4;
const SAMPLES: usize = 200;
const P95_LIMIT_MS: f64 = 200.0;

struct Args {
    socket: PathBuf,
    job: String,
    active_job: String,
    out: Option<PathBuf>,
}

#[derive(serde::Serialize)]
struct Sample {
    client: usize,
    operation: &'static str,
    wall_ms: f64,
    current: bool,
    active: bool,
}

fn parse(mut values: impl Iterator<Item = String>) -> io::Result<Args> {
    let (mut socket, mut job, mut active_job, mut out) = (None, None, None, None);
    while let Some(name) = values.next() {
        let value = values
            .next()
            .ok_or_else(|| io::Error::other(format!("missing value for {name}")))?;
        let slot = match name.as_str() {
            "--socket" => &mut socket,
            "--job" => &mut job,
            "--active-job" => &mut active_job,
            "--out" => &mut out,
            _ => return Err(io::Error::other(format!("unknown option {name}"))),
        };
        if slot.replace(value).is_some() {
            return Err(io::Error::other(format!("repeated option {name}")));
        }
    }
    let missing = |name: &str| io::Error::other(format!("missing {name}"));
    let args = Args {
        socket: socket.ok_or_else(|| missing("--socket"))?.into(),
        job: job.ok_or_else(|| missing("--job"))?,
        active_job: active_job.ok_or_else(|| missing("--active-job"))?,
        out: out.map(PathBuf::from),
    };
    if args.job == args.active_job {
        return Err(io::Error::other("--job and --active-job must differ"));
    }
    Ok(args)
}

fn status(client: &Client, id: &str) -> io::Result<jobs::Status> {
    match client.call(&Request::Status { id: id.to_owned() })? {
        Response::Status(status) => Ok(status),
        other => Err(io::Error::other(format!("unexpected response {other:?}"))),
    }
}

fn current(status: &jobs::Status, id: &str, revision: &str) -> bool {
    status.id == id
        && status.state == Lifecycle::Completed
        && status.currentness == Currentness::Current
        && status.error.is_none()
        && status.input_revision.as_deref() == Some(revision)
}

fn running(client: &Client, id: &str) -> bool {
    status(client, id).is_ok_and(|s| s.state == Lifecycle::Running)
}

fn sample(client: &Client, args: &Args, revision: &str, id: usize, index: usize) -> Sample {
    let operation = if index.is_multiple_of(2) {
        "status"
    } else {
        "result"
    };
    let before = running(client, &args.active_job);
    let request = if operation == "status" {
        Request::Status {
            id: args.job.clone(),
        }
    } else {
        Request::Result {
            id: args.job.clone(),
        }
    };
    // The clock covers the complete frame read and typed decode.
    let start = Instant::now();
    let response = client.call(&request);
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    let ok = match response {
        Ok(Response::Status(status)) => current(&status, &args.job, revision),
        Ok(Response::Result(document)) => {
            current(&document.job, &args.job, revision)
                && document.result.as_ref().is_some_and(|result| {
                    result.get("currentness").and_then(|c| c.as_str()) == Some("current")
                })
        }
        _ => false,
    };
    Sample {
        client: id,
        operation,
        wall_ms,
        current: ok,
        active: before && running(client, &args.active_job),
    }
}

/// Nearest-rank quantile, so p95 keeps every tail sample.
fn quantile(sorted: &[f64], percent: usize) -> f64 {
    sorted[(percent * sorted.len()).div_ceil(100).max(1) - 1]
}

fn run(args: &Args) -> io::Result<bool> {
    let client = Client::new(jobs::socket_path(&args.socket));
    let started = Instant::now();
    while !running(&client, &args.active_job) {
        if started.elapsed() > Duration::from_secs(600) {
            return Err(io::Error::other("the active job never started running"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let done = status(&client, &args.job)?;
    let revision = done
        .input_revision
        .clone()
        .filter(|revision| current(&done, &args.job, revision))
        .ok_or_else(|| io::Error::other("the read job is not completed and Current"))?;
    let barrier = std::sync::Barrier::new(CLIENTS);
    let samples: Vec<Sample> = std::thread::scope(|scope| {
        let (client, barrier, revision) = (&client, &barrier, revision.as_str());
        let handles: Vec<_> = (0..CLIENTS)
            .map(|id| {
                scope.spawn(move || {
                    barrier.wait();
                    (0..SAMPLES / CLIENTS)
                        .map(|n| sample(client, args, revision, id, id * SAMPLES / CLIENTS + n))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| io::Error::other("reader panicked"))
            })
            .collect::<io::Result<Vec<_>>>()
    })?
    .into_iter()
    .flatten()
    .collect();
    let mut passed = samples.len() == SAMPLES && samples.iter().all(|s| s.current && s.active);
    for operation in ["status", "result"] {
        let mut values: Vec<f64> = samples
            .iter()
            .filter(|s| s.operation == operation)
            .map(|s| s.wall_ms)
            .collect();
        values.sort_by(f64::total_cmp);
        let p95 = quantile(&values, 95);
        passed &= p95 < P95_LIMIT_MS;
        println!(
            "{operation}: n={} p50={:.1} p95={p95:.1} p99={:.1} ms",
            values.len(),
            quantile(&values, 50),
            quantile(&values, 99)
        );
    }
    let invalid = samples.iter().filter(|s| !(s.current && s.active)).count();
    println!("invalid={invalid} passed={passed}");
    if let Some(out) = &args.out {
        std::fs::write(
            out,
            serde_json::to_vec_pretty(&samples).map_err(io::Error::other)?,
        )?;
    }
    Ok(passed)
}

fn main() -> std::process::ExitCode {
    match parse(std::env::args().skip(1)).and_then(|args| run(&args)) {
        Ok(true) => std::process::ExitCode::SUCCESS,
        Ok(false) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("job_read_bench: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
