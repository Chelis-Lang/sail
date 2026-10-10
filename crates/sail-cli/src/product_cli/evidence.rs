//! Evidence commands make exact identities explicit.
use sail_api::jobs::{Request, Response};
use sail_api::proof::Target;
use sail_api::repair::Reference;
use std::io::Write;
use std::process::ExitCode;

pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let mut run = || -> Result<(), String> {
        let (json, rest) = output(args)?;
        let request = request(&rest)?;
        let response = super::call(&crate::jobs::connect_or_start()?, &request)?;
        let text = if json {
            serde_json::to_string_pretty(&response).map_err(|error| error.to_string())?
        } else {
            human(&response)
        };
        writeln!(out, "{text}").map_err(|error| error.to_string())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => super::error(&message, err),
    }
}
fn output(args: &[String]) -> Result<(bool, Vec<String>), String> {
    let (mut json, mut rest) = (false, Vec::new());
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--output" => {
                if iter.next().map(String::as_str) != Some("json") {
                    return Err("expected --output json".into());
                }
                json = true;
            }
            "--json" | "--output=json" => json = true,
            _ => rest.push(arg.clone()),
        }
    }
    Ok((json, rest))
}
fn request(args: &[String]) -> Result<Request, String> {
    match args.first().map(String::as_str) {
        Some("explain") => explain(&args[1..]),
        Some("baseline") => match args.get(1).map(String::as_str) {
            Some("list") if args.len() == 2 => Ok(Request::Baselines),
            Some("save") if args.len() == 5 && args[3] == "--job" => Ok(Request::SaveBaseline {
                id: args[4].clone(),
                name: args[2].clone(),
            }),
            _ => Err("usage: baseline save NAME --job JOB | baseline list".into()),
        },
        Some("compare") => match args {
            [_, before, after] => Ok(Request::Compare {
                before: Reference::Job(before.clone()),
                after: after.clone(),
            }),
            [_, after, flag, name] if flag == "--baseline" => Ok(Request::Compare {
                before: Reference::Baseline(name.clone()),
                after: after.clone(),
            }),
            _ => Err("usage: compare BEFORE_JOB AFTER_JOB | compare JOB --baseline NAME".into()),
        },
        _ => Err("unknown evidence command".into()),
    }
}
fn explain(args: &[String]) -> Result<Request, String> {
    let id = args
        .first()
        .filter(|id| !id.starts_with('-'))
        .ok_or("explain requires JOB")?
        .clone();
    let (mut package_id, mut name, mut kind, mut function, mut obligation) =
        (None, None, None, None, None);
    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("{arg} requires a value"))?
            .clone();
        let slot = match arg.as_str() {
            "--package-id" => &mut package_id,
            "--target" => &mut name,
            "--kind" => &mut kind,
            "--function" => &mut function,
            "--obligation" => &mut obligation,
            _ => return Err(format!("unknown explanation option {arg}")),
        };
        if slot.replace(value).is_some() {
            return Err(format!("duplicate explanation option {arg}"));
        }
    }
    let kind = kind.ok_or("explain requires --kind lib|bin")?;
    if kind != "lib" && kind != "bin" {
        return Err("explain kind must be lib or bin".into());
    }
    Ok(Request::Explain {
        id,
        target: Target {
            package_id: package_id.ok_or("explain requires --package-id")?,
            name: name.ok_or("explain requires --target")?,
            kind: vec![kind],
        },
        function: function.ok_or("explain requires --function")?,
        obligation,
    })
}
fn human(response: &Response) -> String {
    match response {
        Response::Explanation(explanation) => format!(
            "{} {} {:?}: {:?}; established by {} ({:?})\n{}",
            explanation.target.name,
            explanation.function.function,
            explanation.target.kind,
            explanation.function.verdict,
            explanation.establishing_run,
            explanation.currentness,
            explanation
                .run_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.rendered.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        ),
        Response::Baseline(baseline) => format!(
            "Saved immutable baseline {} from job {} at revision {}",
            baseline.name, baseline.job_id, baseline.result.input_revision
        ),
        Response::Baselines(names) => {
            if names.is_empty() {
                "No saved baselines.".into()
            } else {
                names.join("\n")
            }
        }
        Response::Comparison(comparison) => {
            let mut text = format!(
                "Comparison: compatible={}, later evidence {:?}",
                comparison.compatible, comparison.after_currentness
            );
            for difference in &comparison.differences {
                use std::fmt::Write as _;
                let _ = write!(
                    text,
                    "\n{} {:?} {}: {:?}",
                    difference.target.name,
                    difference.target.kind,
                    difference.function,
                    difference.change
                );
            }
            text
        }
        _ => "Unexpected evidence response.".into(),
    }
}
