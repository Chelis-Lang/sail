//! Strict selectors: no inferred semantic dependencies or silently widened scope.
use sail_api::proof::Cursor;
use sail_api::repair::Selection;

pub(super) fn validate(selection: &Selection) -> Result<(), String> {
    if selection.package.is_empty()
        || selection.package.starts_with('-')
        || !selection.function.contains("::")
        || selection.function.starts_with('-')
    {
        return Err("repair requires --package P and exact crate-qualified --function F".into());
    }
    match (selection.kind.as_str(), selection.target.as_deref()) {
        ("lib", None) => Ok(()),
        ("bin", Some(name)) if !name.is_empty() && !name.starts_with('-') => Ok(()),
        _ => Err("repair requires exactly one explicit --lib or --bin NAME selection".into()),
    }
}

pub(super) fn repair(args: &[String]) -> Result<(Selection, Vec<String>, bool, bool), String> {
    let mut selection = Selection {
        package: String::new(),
        target: None,
        kind: String::new(),
        function: String::new(),
    };
    let mut cargo = Vec::new();
    let (mut background, mut json, mut engine) = (false, false, false);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = |iter: &mut std::slice::Iter<'_, String>| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{arg} requires a value"))
        };
        match arg.as_str() {
            "--package" | "-p" if selection.package.is_empty() => {
                selection.package = value(&mut iter)?;
            }
            "--function" if selection.function.is_empty() => selection.function = value(&mut iter)?,
            "--lib" if selection.kind.is_empty() => selection.kind = "lib".into(),
            "--bin" if selection.kind.is_empty() => {
                selection.kind = "bin".into();
                selection.target = Some(value(&mut iter)?);
            }
            "--engine" if !engine => {
                if value(&mut iter)? != "interactive" {
                    return Err("repair requires --engine interactive".into());
                }
                engine = true;
            }
            "--background" => background = true,
            "--output" => {
                if value(&mut iter)? != "json" {
                    return Err("repair output must be json".into());
                }
                json = true;
            }
            "--output=json" | "--json" => json = true,
            "--" => {
                cargo.push(arg.clone());
                cargo.extend(iter.cloned());
                break;
            }
            "--package" | "-p" | "--function" | "--lib" | "--bin" | "--engine" => {
                return Err(format!("duplicate or ambiguous selection {arg}"));
            }
            _ => cargo.push(arg.clone()),
        }
    }
    validate(&selection)?;
    if !engine {
        return Err("repair requires explicit --engine interactive".into());
    }
    if cargo.iter().any(|arg| {
        arg.starts_with("--verify-")
            || arg.starts_with("--portfolio")
            || arg.starts_with("--measure-fragility")
            || arg.starts_with("--package=")
            || arg.starts_with("--bin=")
            || arg == "--workspace"
            || arg == "--all-targets"
            || arg == "--bins"
            || arg == "--tests"
            || arg == "--examples"
            || arg == "--benches"
            || arg == "--all"
            || arg.starts_with("--exclude")
            || arg.starts_with("--manifest-path")
            || arg.starts_with("--target-dir")
            || (arg.starts_with("-p") && arg != "--")
    }) {
        return Err(
            "repair full validation cannot accept additional scope or solver experiment selectors"
                .into(),
        );
    }
    let mut selected = vec!["--package".into(), selection.package.clone()];
    if selection.kind == "lib" {
        selected.push("--lib".into());
    } else {
        selected.extend(["--bin".into(), selection.target.clone().unwrap_or_default()]);
    }
    selected.extend(cargo);
    Ok((selection, selected, background, json))
}

pub(super) fn events(args: &[String]) -> Result<(String, Cursor, bool, bool), String> {
    let id = args
        .first()
        .filter(|id| !id.starts_with('-'))
        .ok_or("events requires JOB")?
        .clone();
    let (mut cursor, mut follow, mut jsonl) = (Cursor::default(), false, true);
    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--stream" => cursor
                .stream_id
                .clone_from(iter.next().ok_or("--stream requires ID")?),
            "--after" => {
                cursor.after = iter
                    .next()
                    .ok_or("--after requires N")?
                    .parse()
                    .map_err(|_| "invalid event cursor")?;
            }
            "--follow" => follow = true,
            "--output" => {
                jsonl = match iter.next().map(String::as_str) {
                    Some("jsonl") => true,
                    Some("json") => false,
                    _ => return Err("events output must be jsonl or json".into()),
                }
            }
            _ => return Err(format!("unknown event option {arg}")),
        }
    }
    if follow && !jsonl {
        return Err("follow requires --output jsonl".into());
    }
    Ok((id, cursor, follow, jsonl))
}

pub(super) fn worker_args(selection: &Selection, args: &[String]) -> Result<Vec<String>, String> {
    let mut input = vec![
        "--function".into(),
        selection.function.clone(),
        "--engine".into(),
        "interactive".into(),
    ];
    input.extend_from_slice(args);
    let (parsed, arguments, background, json) = repair(&input)?;
    if parsed != *selection || background || json {
        return Err("SAIL0906: worker selection and verification arguments disagree".into());
    }
    Ok(arguments)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    #[test]
    fn exact_selection_preserves_full_scope_and_never_broadens_legacy_filters() {
        let (selection, arguments, background, json) = repair(&args(&[
            "--package",
            "demo",
            "--lib",
            "--function",
            "demo::arith::lemma",
            "--engine",
            "interactive",
            "--background",
            "--output",
            "json",
            "--features",
            "proofs",
        ]))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(selection.function, "demo::arith::lemma");
        assert_eq!(selection.kind, "lib");
        assert_eq!(
            arguments,
            args(&["--package", "demo", "--lib", "--features", "proofs"])
        );
        assert!(background && json);
        assert!(
            repair(&args(&[
                "--package",
                "demo",
                "--lib",
                "--function",
                "lemma",
                "--engine",
                "interactive"
            ]))
            .is_err()
        );
        assert!(
            repair(&args(&[
                "--package",
                "demo",
                "--lib",
                "--bin",
                "demo",
                "--function",
                "demo::lemma",
                "--engine",
                "interactive"
            ]))
            .is_err()
        );
        assert!(
            repair(&args(&[
                "--package",
                "demo",
                "--lib",
                "--function",
                "demo::lemma",
                "--engine",
                "interactive",
                "--",
                "--verify-root"
            ]))
            .is_err()
        );
    }
    #[test]
    fn cursors_are_bounded_unsigned_and_follow_is_explicit_jsonl() {
        let (_, cursor, follow, jsonl) = events(&args(&[
            "job", "--stream", "job", "--after", "9", "--follow", "--output", "jsonl",
        ]))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(cursor.after, 9);
        assert!(follow && jsonl);
        assert!(events(&args(&["job", "--after", "-1"])).is_err());
        assert!(events(&args(&["job", "--follow", "--output", "json"])).is_err());
    }
}
