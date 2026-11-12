//! Read-only effort measurements from retained jobs or result documents.

use std::io::Write;
use std::process::ExitCode;

use sail_api::health::View;
use sail_api::jobs::{Client, Currentness, Lifecycle, Request, Response};

pub(crate) const USAGE: &str = "\
Read observed proof effort without starting verification.

USAGE:
    sail health [JOB | --file RESULT.json] [--limit N] [--output json]
                [--baseline NAME | --before JOB]

Without JOB or --file, reads the newest completed job for the current directory.
The default limit is 20 functions (1..1000); omitted entries are counted.
Reports original verdicts, provenance, finite check effort, missing coverage,
unlimited checks and resource exhaustion. More than half the requested finite
rlimit is near the limit (ADR 0009). No repeated-run stability or unmeasured
fragility is inferred. A file's currentness is the value recorded in that file.
An earlier job or immutable baseline is shown separately, with its own identity
and configuration. No solver runs and no new proof is established.
";

struct Options {
    job: Option<String>,
    file: Option<String>,
    baseline: Option<String>,
    before: Option<String>,
    limit: usize,
    json: bool,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        job: None,
        file: None,
        baseline: None,
        before: None,
        limit: 20,
        json: false,
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let key = if arg == "--output=json" || arg == "--json" {
            "--output"
        } else {
            arg
        };
        if key.starts_with('-') && !seen.insert(key) {
            return Err(format!("duplicate option {key}"));
        }
        match arg.as_str() {
            "--file" => options.file = Some(iter.next().ok_or("--file needs a path")?.clone()),
            "--baseline" => {
                options.baseline = Some(iter.next().ok_or("--baseline needs a name")?.clone());
            }
            "--before" => options.before = Some(iter.next().ok_or("--before needs a job")?.clone()),
            "--limit" => {
                options.limit = iter
                    .next()
                    .ok_or("--limit needs a number")?
                    .parse()
                    .map_err(|_| "--limit must be a whole number between 1 and 1000")?;
            }
            "--output=json" | "--json" => options.json = true,
            "--output" if iter.next().map(String::as_str) == Some("json") => options.json = true,
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg}")),
            _ if options.job.is_none() => options.job = Some(arg.clone()),
            _ => return Err("expected at most one job identity".into()),
        }
    }
    if options.job.is_some() && options.file.is_some()
        || options.before.is_some() && options.baseline.is_some()
        || !(1..=1000).contains(&options.limit)
    {
        return Err("choose JOB or --file, at most one earlier view, and a limit 1..1000".into());
    }
    Ok(options)
}

fn latest(client: &Client) -> Result<String, String> {
    let directory = std::env::current_dir()
        .and_then(|path| path.canonicalize())
        .map_err(|error| error.to_string())?;
    let Response::List(jobs) = crate::product_cli::call(
        client,
        &Request::List {
            project_dir: Some(directory),
        },
    )?
    else {
        return Err("unexpected retained job list response".into());
    };
    jobs.into_iter().filter(|job| job.state == Lifecycle::Completed)
        .max_by(|left, right| (left.submitted_at_ms, &left.id).cmp(&(right.submitted_at_ms, &right.id)))
        .map(|job| job.id)
        .ok_or_else(|| "no completed job for this directory; run sail verify --background first, or use --file RESULT.json".into())
}

fn job(client: &Client, id: &str, limit: usize) -> Result<View, String> {
    let Response::Result(document) =
        crate::product_cli::call(client, &Request::Result { id: id.into() })?
    else {
        return Err("unexpected retained job result response".into());
    };
    if document.job.id != id || document.job.state != Lifecycle::Completed {
        return Err("health requires the named completed job; use sail jobs status JOB".into());
    }
    let result = serde_json::from_value(
        document
            .result
            .ok_or("job has no completed project result")?,
    )
    .map_err(|error| error.to_string())?;
    let mut view = View::of(&result, limit)?;
    view.job_id = Some(id.into());
    match document.job.currentness {
        Currentness::Unknown => view.currentness = sail_api::project::Currentness::Unknown,
        Currentness::Historical if view.currentness == sail_api::project::Currentness::Current => {
            view.currentness = sail_api::project::Currentness::Historical;
        }
        Currentness::Historical | Currentness::Current => {}
    }
    Ok(view)
}

fn read(options: &Options) -> Result<View, String> {
    let needs_service =
        options.file.is_none() || options.baseline.is_some() || options.before.is_some();
    let client = needs_service
        .then(crate::jobs::connect_or_start)
        .transpose()?;
    let mut view = if let Some(file) = &options.file {
        View::of(&crate::ci_report::document(Some(file))?, options.limit)?
    } else {
        let client = client.as_ref().ok_or("missing job service")?;
        let id = options.job.clone().map_or_else(|| latest(client), Ok)?;
        job(client, &id, options.limit)?
    };
    if let Some(name) = &options.baseline {
        let Response::Baseline(baseline) = crate::product_cli::call(
            client.as_ref().ok_or("missing baseline service")?,
            &Request::GetBaseline { name: name.clone() },
        )?
        else {
            return Err("unexpected baseline response".into());
        };
        if baseline.name != *name
            || baseline.version != sail_api::evidence::VERSION
            || baseline.result.currentness != sail_api::project::Currentness::Historical
            || baseline.job_id.is_empty()
        {
            return Err(
                "immutable baseline identity, version or historical currentness is invalid".into(),
            );
        }
        let mut earlier = View::of(&baseline.result, options.limit)?;
        earlier.job_id = Some(baseline.job_id);
        view.baseline_name = Some(name.clone());
        view.baseline = Some(Box::new(earlier));
    } else if let Some(id) = &options.before {
        view.baseline = Some(Box::new(job(
            client.as_ref().ok_or("missing job service")?,
            id,
            options.limit,
        )?));
    }
    Ok(view)
}

fn human(view: &View, out: &mut dyn Write) -> std::io::Result<()> {
    writeln!(
        out,
        "{}: {:?}; {} functions, {} omitted",
        view.run_id, view.currentness, view.total_functions, view.omitted
    )?;
    writeln!(
        out,
        "job {:?}; workspace {}; revision {}; configuration {}; original exit {:?}",
        view.job_id, view.workspace, view.input_revision, view.context_sha256, view.exit_code
    )?;
    for entry in &view.entries {
        let peak = entry.health.peak.map_or_else(
            || "unrecorded".into(),
            |value| format!("{}/{}", value.used, value.limit),
        );
        writeln!(
            out,
            "{} [{}; {} {}] {:?}/{:?}: peak {peak}; {} near limit, {} exhausted, {} unrecorded, {} unlimited{}",
            entry.function,
            entry.target.package_id,
            entry.target.name,
            entry.target.kind.join(","),
            entry.verdict,
            entry.establishment,
            entry.health.near_limit_checks,
            entry.health.exhausted_checks,
            entry.health.unrecorded_checks,
            entry.health.unlimited_checks,
            if entry.health.attributed {
                ""
            } else {
                "; attribution unavailable"
            }
        )?;
        writeln!(
            out,
            "  established {}; reused={}; source={:?} ({} solved, {} served, {} unrecorded); mode={}; configuration={:?}; engine={:?}; unstable={}; fragility={:?}",
            entry.establishing_run,
            entry.reused,
            entry.provenance.source,
            entry.provenance.checks_solved,
            entry.provenance.checks_served,
            entry.provenance.checks_unrecorded,
            entry.provenance.mode,
            entry.provenance.configuration,
            entry.provenance.engine,
            entry.unstable,
            entry.fragility
        )?;
    }
    if let Some(error) = &view.error {
        writeln!(out, "{}: {}", error.code, error.message)?;
    }
    if let Some(baseline) = &view.baseline {
        writeln!(
            out,
            "Reference evidence (baseline {:?}):",
            view.baseline_name
        )?;
        human(baseline, out)?;
    }
    Ok(())
}

pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let mut action = || -> Result<(), String> {
        let options = options(args)?;
        let view = read(&options)?;
        if options.json {
            serde_json::to_writer_pretty(&mut *out, &view).map_err(|error| error.to_string())?;
            writeln!(out).map_err(|error| error.to_string())
        } else {
            human(&view, out).map_err(|error| error.to_string())
        }
    };
    match action() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(err, "error[SAIL0906]: sail health: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_health_preserves_package_context_provenance_and_reused_origin()
    -> Result<(), Box<dyn std::error::Error>> {
        use sail_api::project::{Execution, ProjectResult, TargetResult};
        let report =
            serde_json::from_str(include_str!("../tests/fixtures/attr/report.golden.json"))?;
        let target = TargetResult {
            package_id: "package-a".into(),
            target: "same".into(),
            kind: vec!["lib".into()],
            establishing_run: "original-proof".into(),
            reused: true,
            scope_reuse: None,
            requested: true,
            complete: false,
            report,
            dependency_store: None,
            dependency_build: None,
        };
        let mut other = target.clone();
        other.package_id = "package-b".into();
        let project = ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: 2,
            run_id: "run".into(),
            workspace: "recorded-workspace".into(),
            context_sha256: "recorded-context".into(),
            input_revision: "recorded-revision".into(),
            currentness: sail_api::project::Currentness::Historical,
            exit_code: Some(1),
            targets: vec![target, other],
            execution: Execution::default(),
            notes: vec![],
            error: None,
            engine: None,
            no_reuse: None,
        };
        let view = View::of(&project, 100)?;
        let mut output = Vec::new();
        human(&view, &mut output)?;
        let text = String::from_utf8(output)?;
        for fragment in [
            "package-a",
            "package-b",
            "recorded-workspace",
            "recorded-context",
            "recorded-revision",
            "original-proof",
            "reused=true",
            "source=Fresh",
            "mode=default",
            "original exit Some(1)",
        ] {
            assert!(text.contains(fragment), "{fragment}: {text}");
        }
        Ok(())
    }

    #[test]
    fn health_options_refuse_ambiguous_sources_bounds_and_repeated_flags() {
        for args in [
            vec!["job", "--file", "result.json"],
            vec!["job", "other-job"],
            vec!["--limit", "0"],
            vec!["--limit", "1001"],
            vec!["--limit", "-1"],
            vec!["--output", "text"],
            vec!["--output=json", "--json"],
            vec!["--file", "a", "--file", "b"],
            vec!["job", "--baseline", "base", "--before", "other"],
        ] {
            assert!(options(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        let parsed = options(
            &[
                "job",
                "--limit",
                "1000",
                "--baseline",
                "base",
                "--output",
                "json",
            ]
            .map(str::to_owned),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(parsed.job.as_deref(), Some("job"));
        assert_eq!(parsed.baseline.as_deref(), Some("base"));
        assert_eq!(parsed.limit, 1000);
        assert!(parsed.json);
        assert!(options(&[]).is_ok());
    }
}
