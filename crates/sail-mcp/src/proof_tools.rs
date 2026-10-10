//! Exact proof journals and repair evidence tools; never generate proof edits.
use crate::run::{self, Cancel};
use crate::tools::{Context, ToolResult, project_dir};
use serde_json::{Map, Value, json};

pub(super) fn definitions() -> Vec<Value> {
    let string = || json!({"type":"string","minLength":1});
    let target = json!({"type":"object","additionalProperties":false,"required":["job_id","package_id","target","kind","function"],"properties":{"job_id":string(),"package_id":string(),"target":string(),"kind":{"type":"string","enum":["lib","bin"]},"function":string(),"obligation":string()}});
    vec![
        json!({"name":"verification_events","title":"Read proof observations","description":"Read one bounded page from proof stream v1. Resume using next.stream_id/after. Gap is explicit; early observations remain provisional. Completed target reports independently seal final proof evidence.","inputSchema":{"type":"object","additionalProperties":false,"required":["job_id"],"properties":{"job_id":string(),"stream_id":string(),"after":{"type":"integer","minimum":0}}},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"explain_failure","title":"Explain exact failure evidence","description":"Return a function/obligation report, raw models, diagnostics and establishing provenance for an exact package/target/kind identity. Unattributed diagnostics remain explicitly unattributed. No proof edits are generated.","inputSchema":target,"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"compare_verification","title":"Compare verification evidence","description":"Compare a completed job with an explicit earlier job or immutable named baseline. Scope/configuration mismatches are descriptive, never labelled proof regressions; partial scopes cannot prove removal.","inputSchema":{"type":"object","additionalProperties":false,"required":["after_job"],"properties":{"after_job":string(),"before_job":string(),"baseline":string()}},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"start_repair_validation","title":"Start focused and full repair validation","description":"Submit an explicit optional interactive-engine batch: exact selected-function phase followed by independent full-target validation. Returns a durable job immediately. Focus success is provisional; accepted requires complete successful Current full evidence and late compiler seal. The engine must be installed and validated; no automated editing.","inputSchema":{"type":"object","additionalProperties":false,"required":["package","kind","function","engine"],"properties":{"project_dir":string(),"package":string(),"kind":{"type":"string","enum":["lib","bin"]},"target":string(),"function":string(),"engine":{"type":"string","enum":["interactive"]},"cargo_args":{"type":"array","items":{"type":"string"}},"verus_args":{"type":"array","items":{"type":"string"}}}},"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
    ]
}

pub(super) fn call(
    ctx: &Context,
    name: &str,
    args: &Map<String, Value>,
    cancel: &Cancel,
) -> ToolResult {
    let parsed = arguments(name, args);
    let (arguments, given) = match parsed {
        Ok(parsed) => parsed,
        Err(message) => return ToolResult::invalid(&message),
    };
    let project = match project_dir(ctx, given.as_deref()) {
        Ok(project) => project,
        Err(error) => return error,
    };
    match run::run(&ctx.config.sail, &arguments, &project, cancel) {
        Ok(outcome) if outcome.exit_code == Some(0) => {
            match serde_json::from_slice::<Value>(&outcome.stdout) {
                Ok(document) if valid(name, &document) => ToolResult::ok(
                    "Scoped proof evidence; provisional observations do not authorize a repair."
                        .into(),
                    document,
                ),
                _ => ToolResult::error(
                    "SAIL0907: invalid proof service response".into(),
                    json!({"error":"invalid_response"}),
                ),
            }
        }
        Ok(outcome) => ToolResult::error(
            format!("SAIL0907: proof operation failed: {}", outcome.stderr_tail),
            json!({"exit_code":outcome.exit_code}),
        ),
        Err(error) => ToolResult::error(
            format!("SAIL0907: cannot contact proof service: {error}"),
            json!({"error":"service_unavailable"}),
        ),
    }
}
fn value(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && !value.starts_with('-') && !value.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} must be a nonempty string, not a flag"))
}
fn arguments(
    name: &str,
    args: &Map<String, Value>,
) -> Result<(Vec<String>, Option<String>), String> {
    let allowed: &[&str] = match name {
        "verification_events" => &["job_id", "stream_id", "after"],
        "explain_failure" => &[
            "job_id",
            "package_id",
            "target",
            "kind",
            "function",
            "obligation",
        ],
        "compare_verification" => &["after_job", "before_job", "baseline"],
        "start_repair_validation" => &[
            "project_dir",
            "package",
            "kind",
            "target",
            "function",
            "engine",
            "cargo_args",
            "verus_args",
        ],
        _ => return Err("unknown proof tool".into()),
    };
    if let Some(key) = args.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown argument {key}"));
    }
    let mut out = match name {
        "verification_events" => {
            let mut out = vec!["jobs".into(), "events".into(), value(args, "job_id")?];
            if args.contains_key("stream_id") {
                out.extend(["--stream".into(), value(args, "stream_id")?]);
            }
            if let Some(after) = args.get("after") {
                out.extend([
                    "--after".into(),
                    after
                        .as_u64()
                        .ok_or("after must be a nonnegative integer")?
                        .to_string(),
                ]);
            }
            out
        }
        "explain_failure" => {
            let kind = value(args, "kind")?;
            if kind != "lib" && kind != "bin" {
                return Err("kind must be lib or bin".into());
            }
            let mut out = vec!["explain".into(), value(args, "job_id")?];
            for (key, flag) in [
                ("package_id", "--package-id"),
                ("target", "--target"),
                ("kind", "--kind"),
                ("function", "--function"),
            ] {
                out.extend([flag.into(), value(args, key)?]);
            }
            if args.contains_key("obligation") {
                out.extend(["--obligation".into(), value(args, "obligation")?]);
            }
            out
        }
        "compare_verification" => {
            let after = value(args, "after_job")?;
            match (
                args.contains_key("before_job"),
                args.contains_key("baseline"),
            ) {
                (true, false) => vec!["compare".into(), value(args, "before_job")?, after],
                (false, true) => vec![
                    "compare".into(),
                    after,
                    "--baseline".into(),
                    value(args, "baseline")?,
                ],
                _ => return Err("provide exactly one before_job or baseline".into()),
            }
        }
        _ => return repair(args),
    };
    out.extend(["--output".into(), "json".into()]);
    Ok((out, None))
}
fn repair(args: &Map<String, Value>) -> Result<(Vec<String>, Option<String>), String> {
    if value(args, "engine")? != "interactive" {
        return Err("repair engine must be interactive".into());
    }
    let kind = value(args, "kind")?;
    let mut out = vec![
        "repair-check".into(),
        "--engine".into(),
        "interactive".into(),
        "--background".into(),
        "--output".into(),
        "json".into(),
        "--package".into(),
        value(args, "package")?,
        "--function".into(),
        value(args, "function")?,
    ];
    match kind.as_str() {
        "lib" if !args.contains_key("target") => out.push("--lib".into()),
        "bin" => out.extend(["--bin".into(), value(args, "target")?]),
        _ => return Err("select lib without target, or bin with target".into()),
    }
    for key in ["cargo_args", "verus_args"] {
        if let Some(values) = args.get(key) {
            let list = values
                .as_array()
                .ok_or_else(|| format!("{key} must be an array"))?;
            if key == "verus_args" && !list.is_empty() {
                out.push("--".into());
            }
            for value in list {
                let text = value
                    .as_str()
                    .filter(|value| !value.contains('\0'))
                    .ok_or_else(|| format!("{key} requires strings"))?;
                if key == "cargo_args"
                    && (text == "--" || text == "--output" || text.starts_with("--output="))
                {
                    return Err("cargo_args cannot override output or Verus separator".into());
                }
                out.push(text.into());
            }
        }
    }
    Ok((
        out,
        if args.contains_key("project_dir") {
            Some(value(args, "project_dir")?)
        } else {
            None
        },
    ))
}
fn valid(name: &str, document: &Value) -> bool {
    match name {
        "verification_events" => serde_json::from_value::<sail_api::proof::Page>(document.clone())
            .is_ok_and(|page| page.version == 1),
        "start_repair_validation" => {
            serde_json::from_value::<sail_api::jobs::Status>(document.clone())
                .is_ok_and(|status| status.version == 1)
        }
        "explain_failure" => {
            matches!(serde_json::from_value::<sail_api::jobs::Response>(document.clone()),Ok(sail_api::jobs::Response::Explanation(explanation)) if explanation.version==1)
        }
        "compare_verification" => {
            matches!(serde_json::from_value::<sail_api::jobs::Response>(document.clone()),Ok(sail_api::jobs::Response::Comparison(comparison)) if comparison.version==1)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(name: &str, value: &Value) -> Result<(Vec<String>, Option<String>), String> {
        arguments(name, value.as_object().ok_or("expected object")?)
    }
    #[test]
    fn repair_tool_requires_explicit_exact_target_and_unavailable_engine_is_not_hidden() {
        let parsed=parse("start_repair_validation", &json!({"package":"demo","kind":"bin","target":"demo","function":"demo::proof","engine":"interactive"})).unwrap_or_else(|error|unreachable!("{error}"));
        assert!(parsed.0.windows(2).any(|pair| pair == ["--bin", "demo"]));
        assert!(
            parsed
                .0
                .windows(2)
                .any(|pair| pair == ["--function", "demo::proof"])
        );
        assert!(parsed.0.contains(&"--background".to_owned()));
        assert!(parse("start_repair_validation", &json!({"package":"demo","kind":"lib","target":"demo","function":"demo::proof","engine":"interactive"})).is_err());
        assert!(
            parse(
                "start_repair_validation",
                &json!({"package":"demo","kind":"lib","function":"demo::proof","engine":"stock"})
            )
            .is_err()
        );
    }
    #[test]
    fn cursors_explanations_and_baselines_do_not_accept_ambiguous_inputs() {
        assert!(parse("verification_events", &json!({"job_id":"job","after":-1})).is_err());
        assert!(
            parse(
                "explain_failure",
                &json!({"job_id":"job","package_id":"pkg","target":"same","function":"demo::f"})
            )
            .is_err()
        );
        assert!(
            parse(
                "compare_verification",
                &json!({"after_job":"after","before_job":"before","baseline":"name"})
            )
            .is_err()
        );
        let (arguments, _) = parse(
            "compare_verification",
            &json!({"after_job":"after","baseline":"release"}),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            arguments,
            vec![
                "compare",
                "after",
                "--baseline",
                "release",
                "--output",
                "json"
            ]
        );
        assert!(!valid(
            "verification_events",
            &json!({"version":2,"next":{"stream_id":"job","after":0},"events":[],"terminal":false,"gap":null})
        ));
    }
}
