//! Evidence commands make exact identities explicit.
use sail_api::evidence::{Change, Comparison, Reference};
use sail_api::jobs::{Request, Response};
use sail_api::proof::Target;
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
            "--" => {
                // Preserve literal baseline names, including names such as --json.
                rest.extend(iter.cloned());
                break;
            }
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
            Some("get") if args.len() == 3 => Ok(Request::GetBaseline {
                name: args[2].clone(),
            }),
            Some("save") if args.len() == 5 && args[3] == "--job" => Ok(Request::SaveBaseline {
                id: args[4].clone(),
                name: args[2].clone(),
            }),
            _ => Err(
                "usage: baseline save NAME --job JOB | baseline list | baseline get NAME".into(),
            ),
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
            "Immutable baseline {} from job {} at recorded revision {} (historical evidence)",
            baseline.name, baseline.job_id, baseline.result.input_revision
        ),
        Response::Baselines(names) => {
            if names.is_empty() {
                "No saved baselines.".into()
            } else {
                names.join("\n")
            }
        }
        Response::Comparison(comparison) => comparison_text(comparison),
        _ => "Unexpected evidence response.".into(),
    }
}

/// The `--output json` name of a change.
fn change_name(change: Change) -> &'static str {
    match change {
        Change::Unchanged => "unchanged",
        Change::Regressed => "regressed",
        Change::BecameUnknown => "became_unknown",
        Change::Recovered => "recovered",
        Change::Added => "added",
        Change::NotObserved => "not_observed",
        Change::Removed => "removed",
        Change::Incomparable => "incomparable",
        Change::Changed => "changed",
    }
}

/// Counts by change, then each changed function, then the notes. Unchanged
/// functions are counted only; `--output json` lists every function.
fn comparison_text(comparison: &Comparison) -> String {
    use std::fmt::Write as _;
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for difference in &comparison.differences {
        *counts.entry(change_name(difference.change)).or_default() += 1;
    }
    let mut text = format!(
        "Comparison: compatible={}, later evidence {:?}: {}.",
        comparison.compatible,
        comparison.after_currentness,
        if counts.is_empty() {
            "no functions compared".to_owned()
        } else {
            counts
                .iter()
                .map(|(change, n)| format!("{n} {change}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    for difference in &comparison.differences {
        if difference.change == Change::Unchanged {
            continue;
        }
        let _ = write!(
            text,
            "\n{} {:?} {}: {}",
            difference.target.name,
            difference.target.kind,
            difference.function,
            change_name(difference.change)
        );
    }
    for note in &comparison.notes {
        let _ = write!(text, "\n{note}");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparisons_count_unchanged_functions_and_list_the_changed_ones() {
        let difference = |function: &str, change| sail_api::evidence::Difference {
            target: Target {
                package_id: "vest_lib".into(),
                name: "vest_lib".into(),
                kind: vec!["lib".into()],
            },
            function: function.into(),
            before: None,
            after: None,
            change,
            before_passed_by_verus_totals: false,
            after_passed_by_verus_totals: false,
            obligations: Vec::new(),
        };
        let comparison = Comparison {
            version: 1,
            compatible: true,
            before_revision: "a".into(),
            after_revision: "b".into(),
            after_currentness: sail_api::project::Currentness::Current,
            differences: vec![
                difference("vest_lib::a", Change::Unchanged),
                difference("vest_lib::b", Change::Recovered),
                difference("vest_lib::c", Change::Unchanged),
            ],
            notes: vec!["2 function(s) reported `unknown` passed by Verus's totals.".into()],
        };
        assert_eq!(
            comparison_text(&comparison),
            "Comparison: compatible=true, later evidence Current: 1 recovered, 2 unchanged.\n\
             vest_lib [\"lib\"] vest_lib::b: recovered\n\
             2 function(s) reported `unknown` passed by Verus's totals."
        );
    }

    #[test]
    fn literal_baseline_names_cannot_become_output_options() {
        for name in ["release", "--json", "--output", "--output=json", "-named"] {
            let args = ["baseline", "get", "--output", "json", "--", name].map(str::to_owned);
            let (json, rest) = output(&args).unwrap_or_else(|error| unreachable!("{error}"));
            assert!(json);
            assert!(
                matches!(request(&rest), Ok(Request::GetBaseline { name: actual }) if actual == name)
            );
            let args = [
                "baseline", "save", "--output", "json", "--", name, "--job", "job",
            ]
            .map(str::to_owned);
            let (json, rest) = output(&args).unwrap_or_else(|error| unreachable!("{error}"));
            assert!(json);
            assert!(
                matches!(request(&rest), Ok(Request::SaveBaseline { name: actual, id }) if actual == name && id == "job")
            );
            let args = [
                "compare",
                "--output",
                "json",
                "--",
                "after",
                "--baseline",
                name,
            ]
            .map(str::to_owned);
            let (json, rest) = output(&args).unwrap_or_else(|error| unreachable!("{error}"));
            assert!(json);
            assert!(matches!(request(&rest), Ok(Request::Compare {
                before: Reference::Baseline(actual), after
            }) if actual == name && after == "after"));
        }
        assert!(request(&["baseline".into(), "get".into()]).is_err());
        assert!(
            request(&[
                "baseline".into(),
                "get".into(),
                "name".into(),
                "extra".into()
            ])
            .is_err()
        );
        assert!(matches!(
            request(&["baseline".into(), "list".into()]),
            Ok(Request::Baselines)
        ));
    }
}
