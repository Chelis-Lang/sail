//! Exact proof journals and repair evidence tools; never generate proof edits.
use crate::run::{self, Cancel};
use crate::tools::{Context, ToolResult, project_dir};
use serde_json::{Map, Value, json};

pub(super) fn definitions() -> Vec<Value> {
    let string = || json!({"type":"string","minLength":1});
    let target = json!({"type":"object","additionalProperties":false,"required":["job_id","package_id","target","kind","function"],"properties":{"job_id":string(),"package_id":string(),"target":string(),"kind":{"type":"string","enum":["lib","bin"]},"function":string(),"obligation":string()}});
    vec![
        json!({"name":"proof_health","title":"Read observed proof health","description":"Read original per-check resource effort and missing coverage from a completed verification job without running a solver. Preserves verdicts, provisional establishment, provenance and currentness. Shows an optional earlier job or immutable baseline separately. A single run does not establish stability or unmeasured fragility.","inputSchema":{"type":"object","additionalProperties":false,"required":["job_id"],"properties":{"job_id":string(),"before_job":string(),"baseline":baseline_schema(),"limit":{"type":"integer","minimum":1,"maximum":1000}},"not":{"required":["before_job","baseline"]}},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"requirement_assurance","title":"Read requirement assurance","description":"Experimental: integration tool. Read versioned submitter-attributed Verus evidence grouped by requirement identity for an ordinary job started with requirements. The attribution is an unchecked submitter claim; currentness, actual tiers and mutation limitations remain explicit.","inputSchema":{"type":"object","additionalProperties":false,"required":["job_id"],"properties":{"job_id":string(),"attributed":{"type":"boolean","description":"Accepted for compatibility and ignored: this tool reads only submitter-attributed evidence."}}},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"verification_events","title":"Read proof observations","description":"Read one bounded page from proof stream v1. Resume using next.stream_id/after. Gap is explicit; early observations remain provisional. Completed target reports independently seal final proof evidence.","inputSchema":{"type":"object","additionalProperties":false,"required":["job_id"],"properties":{"job_id":string(),"stream_id":string(),"after":{"type":"integer","minimum":0}}},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"explain_failure","title":"Explain exact failure evidence","description":"Return a function/obligation report, raw models, diagnostics and establishing provenance for an exact package/target/kind identity. Unattributed diagnostics remain explicitly unattributed. No proof edits are generated.","inputSchema":target,"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"compare_verification","title":"Compare verification evidence","description":"Compare a completed job with an explicit earlier job or immutable named baseline. Scope/configuration mismatches are descriptive, never labelled proof regressions; partial scopes cannot prove removal.","inputSchema":{"type":"object","additionalProperties":false,"required":["after_job"],"properties":{"after_job":string(),"before_job":string(),"baseline":baseline_schema()}},"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"save_baseline","title":"Save immutable baseline evidence","description":"Save a completed job under a new immutable name, independently of job retention. Negative, incomplete or provisional evidence remains as reported; saving never proves current inputs or accepts a repair. Existing names cannot be overwritten.","inputSchema":{"type":"object","additionalProperties":false,"required":["job_id","name"],"properties":{"job_id":string(),"name":baseline_schema()}},"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
        json!({"name":"list_baselines","title":"List immutable baselines","description":"List saved baseline names for this server's local job service. Names are sorted; listing establishes no proof or currentness.","inputSchema":{"type":"object","additionalProperties":false,"properties":{}},"annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false}}),
        json!({"name":"get_baseline","title":"Read immutable baseline evidence","description":"Read version-one baseline evidence for its recorded job, revision, context and exact targets. Currentness is historical, independently of today's workspace and job pruning. Provisional verdicts remain provisional.","inputSchema":{"type":"object","additionalProperties":false,"required":["name"],"properties":{"name":baseline_schema()}},"annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false}}),
        json!({"name":"start_repair_validation","title":"Start focused and full repair validation","description":"Submit an explicit optional interactive-engine batch for a Cargo target or standalone source_file: exact selected-function phase followed by independent full-target validation. Returns a durable job immediately. Focus success is provisional; accepted requires complete successful Current full evidence and late compiler seal. The engine must be installed and validated; no automated editing.","inputSchema":{"type":"object","additionalProperties":false,"required":["function","engine"],"oneOf":[{"required":["source_file"]},{"required":["package","kind"]}],"properties":{"project_dir":string(),"source_file":string(),"package":string(),"kind":{"type":"string","enum":["lib","bin"]},"target":string(),"function":string(),"engine":{"type":"string","enum":["interactive"]},"cargo_args":{"type":"array","items":{"type":"string"}},"verus_args":{"type":"array","items":{"type":"string"}}}},"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
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
                Ok(document)
                    if valid(name, &document)
                        && requested_assurance_matches(name, args, &document)
                        && requested_health_matches(name, args, &document)
                        && requested_baseline_matches(name, args, &document) => ToolResult::ok(
                    if name == "proof_health" {
                        "Recorded proof effort and coverage; verdicts, provenance and currentness are preserved. No new verification."
                    } else {
                        "Scoped proof evidence; provisional observations do not authorize a repair."
                    }.into(),
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
fn baseline_schema() -> Value {
    json!({"type":"string","minLength":1,"maxLength":80,"pattern":"^[A-Za-z0-9_.-]+$","not":{"enum":[".",".."]}})
}
fn valid_baseline_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
fn baseline_name(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    let name = args.get(key).and_then(Value::as_str).unwrap_or("");
    if valid_baseline_name(name) {
        Ok(name.to_owned())
    } else {
        Err(format!(
            "{key} must use 1-80 ASCII letters, digits, dots, underscores or hyphens, excluding . and .."
        ))
    }
}
fn requested_baseline_matches(name: &str, args: &Map<String, Value>, document: &Value) -> bool {
    match name {
        "save_baseline" | "get_baseline" => {
            document["value"]["name"].as_str() == args.get("name").and_then(Value::as_str)
                && (name != "save_baseline"
                    || document["value"]["job_id"].as_str()
                        == args.get("job_id").and_then(Value::as_str))
        }
        _ => true,
    }
}
/// `assurance JOB`: submitter-attributed ordinary evidence. `attributed` is
/// accepted for compatibility and changes nothing.
fn assurance(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    match args.get("attributed") {
        None | Some(Value::Bool(_)) => Ok(vec!["assurance".into(), value(args, "job_id")?]),
        Some(_) => Err("attributed must be a boolean".into()),
    }
}
fn allowed_arguments(name: &str) -> Result<&'static [&'static str], String> {
    Ok(match name {
        "proof_health" => &["job_id", "before_job", "baseline", "limit"],
        "requirement_assurance" => &["job_id", "attributed"],
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
        "save_baseline" => &["job_id", "name"],
        "list_baselines" => &[],
        "get_baseline" => &["name"],
        "start_repair_validation" => &[
            "project_dir",
            "source_file",
            "package",
            "kind",
            "target",
            "function",
            "engine",
            "cargo_args",
            "verus_args",
        ],
        _ => return Err("unknown proof tool".into()),
    })
}

fn arguments(
    name: &str,
    args: &Map<String, Value>,
) -> Result<(Vec<String>, Option<String>), String> {
    let allowed = allowed_arguments(name)?;
    if let Some(key) = args.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown argument {key}"));
    }
    let out = match name {
        "proof_health" => health_arguments(args)?,
        "requirement_assurance" => assurance(args)?,
        "save_baseline" => vec![
            "baseline".into(),
            "save".into(),
            baseline_name(args, "name")?,
            "--job".into(),
            value(args, "job_id")?,
        ],
        "list_baselines" => vec!["baseline".into(), "list".into()],
        "get_baseline" => vec![
            "baseline".into(),
            "get".into(),
            baseline_name(args, "name")?,
        ],
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
                    baseline_name(args, "baseline")?,
                ],
                _ => return Err("provide exactly one before_job or baseline".into()),
            }
        }
        _ => return repair(args),
    };
    Ok((json_arguments(name, args, out), None))
}
fn json_arguments(name: &str, args: &Map<String, Value>, mut out: Vec<String>) -> Vec<String> {
    if matches!(name, "save_baseline" | "get_baseline") {
        // Output options precede the literal separator so a valid baseline name
        // such as --json can never be interpreted as a CLI option.
        out.splice(2..2, ["--output".into(), "json".into(), "--".into()]);
    } else if name == "compare_verification" && args.contains_key("baseline") {
        out.splice(1..1, ["--output".into(), "json".into(), "--".into()]);
    } else {
        out.extend(["--output".into(), "json".into()]);
    }
    out
}
fn repair(args: &Map<String, Value>) -> Result<(Vec<String>, Option<String>), String> {
    if value(args, "engine")? != "interactive" {
        return Err("repair engine must be interactive".into());
    }
    let mut out = if args.contains_key("source_file") {
        if ["package", "kind", "target", "cargo_args"]
            .iter()
            .any(|field| args.contains_key(*field))
        {
            return Err(
                "source_file repair cannot combine with Cargo selectors or arguments".into(),
            );
        }
        let source = value(args, "source_file")?;
        if std::path::Path::new(&source)
            .extension()
            .is_none_or(|extension| extension != "rs")
        {
            return Err("source_file must name a .rs file".into());
        }
        vec![
            "repair-check".into(),
            source,
            "--engine".into(),
            "interactive".into(),
            "--background".into(),
            "--output".into(),
            "json".into(),
            "--function".into(),
            value(args, "function")?,
        ]
    } else {
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
        out
    };
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
fn health_arguments(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    if args.contains_key("before_job") && args.contains_key("baseline") {
        return Err("choose before_job or baseline".into());
    }
    let mut out = vec!["health".into(), value(args, "job_id")?];
    if args.contains_key("before_job") {
        out.extend(["--before".into(), value(args, "before_job")?]);
    }
    if args.contains_key("baseline") {
        out.extend(["--baseline".into(), baseline_name(args, "baseline")?]);
    }
    if let Some(limit) = args.get("limit") {
        let limit = limit
            .as_u64()
            .filter(|limit| (1..=1000).contains(limit))
            .ok_or("limit must be an integer from 1 to 1000")?;
        out.extend(["--limit".into(), limit.to_string()]);
    }
    Ok(out)
}

fn requested_health_matches(name: &str, args: &Map<String, Value>, document: &Value) -> bool {
    if name != "proof_health" {
        return true;
    }
    let Ok(view) = serde_json::from_value::<sail_api::health::View>(document.clone()) else {
        return false;
    };
    if view.job_id.as_deref() != args["job_id"].as_str()
        || view.baseline_name.as_deref() != args.get("baseline").and_then(Value::as_str)
        || view.baseline.is_some()
            != (args.contains_key("before_job") || args.contains_key("baseline"))
        || u64::try_from(view.entries.len())
            .ok()
            .and_then(|count| count.checked_add(view.omitted))
            != Some(view.total_functions)
    {
        return false;
    }
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20);
    if u64::try_from(view.entries.len())
        .ok()
        .is_none_or(|count| count > limit)
    {
        return false;
    }
    if let Some(baseline) = &view.baseline
        && (baseline.kind != "sail-proof-health"
            || baseline.version != 1
            || baseline.baseline.is_some()
            || baseline.baseline_name.is_some()
            || (args.contains_key("baseline")
                && (baseline.currentness != sail_api::project::Currentness::Historical
                    || baseline.job_id.as_ref().is_none_or(String::is_empty)))
            || u64::try_from(baseline.entries.len())
                .ok()
                .and_then(|count| count.checked_add(baseline.omitted))
                != Some(baseline.total_functions)
            || args
                .get("before_job")
                .is_some_and(|expected| baseline.job_id.as_deref() != expected.as_str())
            || u64::try_from(baseline.entries.len())
                .ok()
                .is_none_or(|count| count > limit))
    {
        return false;
    }
    true
}

fn valid(name: &str, document: &Value) -> bool {
    match name {
        "proof_health" => serde_json::from_value::<sail_api::health::View>(document.clone())
            .is_ok_and(|view| view.kind == "sail-proof-health" && view.version == 1),
        "requirement_assurance" => {
            matches!(serde_json::from_value::<sail_api::jobs::Response>(document.clone()),
            Ok(sail_api::jobs::Response::AttributedAssurance(view))
            if view.version == sail_api::attribution::VERSION && !view.job_id.is_empty())
        }
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
        "save_baseline" | "get_baseline" => {
            matches!(serde_json::from_value::<sail_api::jobs::Response>(document.clone()),
                Ok(sail_api::jobs::Response::Baseline(baseline))
                if baseline.version == sail_api::evidence::VERSION
                    && valid_baseline_name(&baseline.name)
                    && !baseline.job_id.is_empty()
                    && baseline.result.kind == "sail-project-result"
                    && baseline.result.schema_version == sail_api::project::SCHEMA_VERSION
                    && baseline.result.currentness == sail_api::project::Currentness::Historical)
        }
        "list_baselines" => {
            matches!(serde_json::from_value::<sail_api::jobs::Response>(document.clone()),
                Ok(sail_api::jobs::Response::Baselines(names))
                if names.iter().all(|name| valid_baseline_name(name))
                    && names.windows(2).all(|pair| pair[0] < pair[1]))
        }
        _ => false,
    }
}

/// The response names the requested job and is the requested kind of view.
fn requested_assurance_matches(name: &str, args: &Map<String, Value>, document: &Value) -> bool {
    name != "requirement_assurance"
        || (document.pointer("/value/job_id").and_then(Value::as_str)
            == args.get("job_id").and_then(Value::as_str)
            && document.pointer("/response").and_then(Value::as_str)
                == Some("attributed_assurance"))
}

#[cfg(test)]
#[path = "baseline_tools_tests.rs"]
mod baseline_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_tools_bind_job_baseline_bounds_and_refuse_forged_view_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = json!({"job_id":"job", "before_job":"older", "limit":1});
        let args = args.as_object().ok_or("arguments")?;
        assert_eq!(
            arguments("proof_health", args)?.0,
            [
                "health", "job", "--before", "older", "--limit", "1", "--output", "json"
            ]
        );
        let empty = json!({"kind":"sail-proof-health","version":1,"job_id":"job","run_id":"run",
            "workspace":"workspace","context_sha256":"context","input_revision":"revision",
            "currentness":"historical","total_functions":0,"omitted":0,"entries":[],"error":null});
        let mut view = empty.clone();
        let mut earlier = empty.clone();
        earlier["job_id"] = json!("older");
        view["baseline"] = earlier;
        assert!(valid("proof_health", &view));
        assert!(requested_health_matches("proof_health", args, &view));
        for (pointer, value) in [
            ("/job_id", json!("foreign")),
            ("/baseline/job_id", json!("foreign")),
            ("/total_functions", json!(1)),
            ("/baseline/total_functions", json!(1)),
            ("/baseline/version", json!(2)),
        ] {
            let mut forged = view.clone();
            *forged.pointer_mut(pointer).ok_or("field")? = value;
            assert!(!requested_health_matches("proof_health", args, &forged));
        }
        for input in [
            json!({"job_id":"job","limit":0}),
            json!({"job_id":"job","limit":1001}),
            json!({"job_id":"job","limit":1.5}),
            json!({"job_id":"job","baseline":"base","before_job":"old"}),
            json!({"job_id":"job","file":"foreign.json"}),
        ] {
            assert!(arguments("proof_health", input.as_object().ok_or("args")?).is_err());
        }
        Ok(())
    }

    #[test]
    fn assurance_tool_binds_the_requested_job_and_reads_only_attributed_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = json!({"job_id":"job-123"});
        assert_eq!(
            arguments("requirement_assurance", args.as_object().ok_or("args")?)?.0,
            vec!["assurance", "job-123", "--output", "json"]
        );
        assert!(
            arguments(
                "requirement_assurance",
                json!({"job_id":"--help"}).as_object().ok_or("args")?
            )
            .is_err()
        );
        // The compatibility flag is accepted and changes nothing.
        assert_eq!(
            arguments(
                "requirement_assurance",
                json!({"job_id":"job-123","attributed":true})
                    .as_object()
                    .ok_or("args")?
            )?
            .0,
            vec!["assurance", "job-123", "--output", "json"]
        );
        assert!(
            arguments(
                "requirement_assurance",
                json!({"job_id":"job-123","attributed":"yes"})
                    .as_object()
                    .ok_or("args")?
            )
            .is_err()
        );
        let attributed = serde_json::to_value(sail_api::jobs::Response::AttributedAssurance(
            Box::new(sail_api::attribution::View {
                version: sail_api::attribution::VERSION,
                job_id: "job-123".into(),
                arguments: Vec::new(),
                currentness: sail_api::jobs::Currentness::Unknown,
                requirements: Vec::new(),
            }),
        ))?;
        assert!(valid("requirement_assurance", &attributed));
        assert!(requested_assurance_matches(
            "requirement_assurance",
            args.as_object().ok_or("args")?,
            &attributed
        ));
        assert!(!requested_assurance_matches(
            "requirement_assurance",
            json!({"job_id":"other"}).as_object().ok_or("args")?,
            &attributed
        ));
        // A reply in the removed neutral shape is not a valid answer.
        let retired = json!({"response":"assurance","value":{"version":2,"job_id":"job-123","currentness":"unknown","requirements":[]}});
        assert!(!valid("requirement_assurance", &retired));
        assert!(!requested_assurance_matches(
            "requirement_assurance",
            args.as_object().ok_or("args")?,
            &retired
        ));
        Ok(())
    }
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
                "--output",
                "json",
                "--",
                "after",
                "--baseline",
                "release"
            ]
        );
        assert!(!valid(
            "verification_events",
            &json!({"version":2,"next":{"stream_id":"job","after":0},"events":[],"terminal":false,"gap":null})
        ));
    }
}

#[cfg(test)]
mod standalone_repair_tests {
    use super::*;
    #[test]
    fn standalone_repair_uses_existing_tool_without_cargo_selectors() {
        let input =
            json!({"source_file":"proof.rs", "function":"proof::lemma", "engine":"interactive"});
        let parsed = arguments(
            "start_repair_validation",
            input.as_object().unwrap_or_else(|| unreachable!()),
        );
        assert!(
            matches!(&parsed, Ok((args, _)) if args.first().map(String::as_str) == Some("repair-check") && args.get(1).map(String::as_str) == Some("proof.rs") && !args.iter().any(|arg| arg == "--package"))
        );
        for field in ["package", "kind", "target", "cargo_args"] {
            let mut mixed = input.clone();
            mixed[field] = json!("ambiguous");
            assert!(
                arguments(
                    "start_repair_validation",
                    mixed.as_object().unwrap_or_else(|| unreachable!())
                )
                .is_err()
            );
        }
    }
}
