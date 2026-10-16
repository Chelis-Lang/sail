//! Agent-sized tool replies.
//!
//! By default a tool reply carries what an agent acts on: a failures-first
//! summary, the requested targets' failures with source-level counterexample
//! variables, and counts. Dependency targets (vstd and the like) are a count
//! and a pointer. The optional `detail` argument asks for more:
//! - `"targets"`: the requested targets' complete reports;
//! - `"full"`: the complete document, dependencies and raw models included.
//!
//! Shaping only selects and abbreviates recorded evidence. It never changes
//! a verdict, and charter rule 7 holds: a provisional or unknown function is
//! never counted or described as verified.

use std::fmt::Write as _;

use sail_api::output::{Counterexample, FunctionReport, OutcomeName, VerdictName};
use sail_api::project::{ProjectResult, TargetResult};
use serde_json::{Map, Value, json};

use crate::summary::{effective_verdict, verdict_name};
use crate::tools::ToolResult;

/// How many failed functions, errors and unresolved names a reply lists.
const MAX_LISTED: usize = 20;

/// How much of a reply to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Detail {
    /// The agent view (default).
    Summary,
    /// The requested targets' complete reports, without dependencies.
    Targets,
    /// The complete document.
    Full,
}

/// The `detail` values each tool accepts; `None` for tools without one.
fn levels(tool: &str) -> Option<&'static [&'static str]> {
    match tool {
        "verify" | "verification_result" | "save_baseline" | "get_baseline" => {
            Some(&["summary", "targets", "full"])
        }
        "counterexample" | "cached_result" | "explain_failure" | "compare_verification" => {
            Some(&["summary", "full"])
        }
        _ => None,
    }
}

/// Adds the `detail` property to the input schemas of the tools that take it.
pub(crate) fn add_schema(definitions: &mut [Value]) {
    for definition in definitions {
        let Some(levels) = definition["name"].as_str().and_then(levels) else {
            continue;
        };
        if let Some(properties) = definition["inputSchema"]["properties"].as_object_mut() {
            properties.insert(
                "detail".to_owned(),
                json!({
                    "type": "string",
                    "enum": levels,
                    "description": "How much to return. `summary` (default): failures first, source-level counterexample variables, counts; dependency targets such as vstd are only counted. `targets`: also the requested targets' complete reports. `full`: the complete document, including dependencies and raw solver models."
                }),
            );
        }
    }
}

/// Removes and validates `detail` from a call's arguments.
///
/// # Errors
///
/// A message for the model when the tool takes no `detail` or the value is
/// not one it accepts.
pub(crate) fn take_detail(tool: &str, args: &mut Map<String, Value>) -> Result<Detail, String> {
    let Some(given) = args.remove("detail") else {
        return Ok(Detail::Summary);
    };
    let Some(levels) = levels(tool) else {
        return Err("unknown argument `detail`".to_owned());
    };
    match given.as_str() {
        Some("summary") => Ok(Detail::Summary),
        Some("targets") if levels.contains(&"targets") => Ok(Detail::Targets),
        Some("full") => Ok(Detail::Full),
        _ => Err(format!("`detail` must be one of: {}", levels.join(", "))),
    }
}

/// Shapes a successful tool result for `detail`, and gives it a summary that
/// states the result. Errors pass through unchanged.
pub(crate) fn shape(tool: &str, detail: Detail, mut result: ToolResult) -> ToolResult {
    if result.is_error {
        return result;
    }
    match tool {
        "verify" if result.structured.get("project_result").is_some() => {
            verify(detail, &mut result);
        }
        "verification_result" => job_result(detail, &mut result),
        "save_baseline" | "get_baseline" => baseline(tool, detail, &mut result),
        "counterexample" | "cached_result" | "explain_failure" | "verify" => {
            if detail != Detail::Full {
                if tool == "counterexample" {
                    drop_duplicate_reports(&mut result.structured);
                }
                compact_models(&mut result.structured);
            }
            if tool == "explain_failure" {
                result.text = explanation_summary(&result.structured);
            } else if tool != "verify" {
                let models = tool == "counterexample";
                result.text = lookup_summary(&result.structured, &result.text, models);
            }
        }
        "compare_verification" => {
            result.text = comparison_summary(&result.structured);
            if detail != Detail::Full {
                drop_unchanged(&mut result.structured["value"]);
            }
        }
        "list_baselines" => result.text = baselines_summary(&result.structured),
        "verification_events" => result.text = events_summary(&result.structured),
        "start_verification"
        | "verification_status"
        | "cancel_verification"
        | "start_repair_validation" => result.text = status_summary(tool, &result.structured),
        _ => {}
    }
    result
}

/// The blocking `verify` reply: `project_result` holds every target.
fn verify(detail: Detail, result: &mut ToolResult) {
    let Some(project) = result.structured.get("project_result").and_then(project_of) else {
        return;
    };
    let job = result.structured["job"].clone();
    let mut view = agent_view(&project, &job, &crate::job_tools::reuse_lines(&project));
    result.text = summary_of(&view);
    match detail {
        Detail::Full => {}
        Detail::Targets => {
            keep_requested(&mut result.structured["project_result"]);
            result.structured["dependencies"] = view["dependencies"].take();
        }
        Detail::Summary => {
            for key in ["project_dir", "sail_args", "duration_ms", "sources"] {
                view[key] = result.structured[key].take();
            }
            result.structured = view;
        }
    }
}

/// `verification_result`: `{"response":"result","value":{job,result}}`.
fn job_result(detail: Detail, result: &mut ToolResult) {
    let value = &mut result.structured["value"];
    let job = value["job"].clone();
    let Some(project) = value.get("result").and_then(project_of) else {
        result.text = format!(
            "Job {}: {}; no completed result yet.",
            job["id"].as_str().unwrap_or("?"),
            job["state"].as_str().unwrap_or("?")
        );
        return;
    };
    let reuse = crate::job_tools::reuse_lines(&project);
    let view = agent_view(&project, &job, &reuse);
    result.text = summary_of(&view);
    match detail {
        Detail::Full => {}
        Detail::Targets => {
            keep_requested(&mut value["result"]);
            value["dependencies"] = view["dependencies"].clone();
        }
        Detail::Summary => value["result"] = view,
    }
}

/// `save_baseline` and `get_baseline`: `{"response":"baseline","value":{name,job_id,result}}`.
fn baseline(tool: &str, detail: Detail, result: &mut ToolResult) {
    let value = &mut result.structured["value"];
    let name = value["name"].as_str().unwrap_or("?").to_owned();
    let job_id = value["job_id"].as_str().unwrap_or("?").to_owned();
    let Some(project) = value.get("result").and_then(project_of) else {
        return;
    };
    let job = json!({"id": job_id, "currentness": "historical"});
    let view = agent_view(&project, &job, &[]);
    let first = view["summary"]
        .as_str()
        .and_then(|s| s.lines().next())
        .unwrap_or_default()
        .to_owned();
    result.text = format!(
        "{} baseline `{name}` from job {job_id} (historical evidence): {first}",
        if tool == "save_baseline" {
            "Saved"
        } else {
            "Read"
        }
    );
    match detail {
        Detail::Full => {}
        Detail::Targets => {
            keep_requested(&mut value["result"]);
            value["dependencies"] = view["dependencies"].clone();
        }
        Detail::Summary => value["result"] = view,
    }
}

fn summary_of(view: &Value) -> String {
    view["summary"].as_str().unwrap_or_default().to_owned()
}

fn project_of(value: &Value) -> Option<ProjectResult> {
    if value.is_null() {
        return None;
    }
    // `&Value` is a deserializer: no copy of a possibly large result.
    serde::Deserialize::deserialize(value).ok()
}

/// Replaces dependency targets with a count and a pointer.
fn keep_requested(project: &mut Value) {
    if let Some(targets) = project["targets"].as_array_mut() {
        targets.retain(|target| target["requested"] == true);
    }
}

fn functions(target: &TargetResult) -> impl Iterator<Item = &FunctionReport> {
    target.report.crates.iter().flat_map(|c| &c.functions)
}

#[derive(Default)]
struct Counts {
    functions: u64,
    verified: u64,
    failed: u64,
    unknown: u64,
    /// The `unknown` functions that Verus's own totals clear
    /// ([`sail_api::output::CrateReport::unknown_passed_by_verus_totals`]).
    passed_by_verus_totals: u64,
    provisional: u64,
}
impl Counts {
    fn add(&mut self, f: &FunctionReport) {
        self.functions += 1;
        match effective_verdict(f) {
            VerdictName::Verified => self.verified += 1,
            VerdictName::Failed => self.failed += 1,
            VerdictName::Unknown => self.unknown += 1,
            VerdictName::Provisional => self.provisional += 1,
        }
    }
    fn json(&self) -> Value {
        json!({
            "functions": self.functions,
            "verified": self.verified,
            "failed": self.failed,
            "unknown": self.unknown,
            "unknown_passed_by_verus_totals": self.passed_by_verus_totals,
            "provisional": self.provisional,
        })
    }
}

/// `file:line:column` of a span, when known.
fn at(file: Option<&str>, line: Option<u64>, column: Option<u64>) -> Option<String> {
    Some(format!("{}:{}:{}", file?, line?, column.unwrap_or(1)))
}

/// Whether a model assignment names a source-level variable: Verus names a
/// parameter or return value `x!` and versions of a local `i@`, `i@0`, `i@1`.
/// Everything else (`fuel%…`, sort constants, `$…`) is solver internals.
fn source_variable(name: &str) -> Option<String> {
    let (base, version) = if let Some(base) = name.strip_suffix('!') {
        (base, None)
    } else {
        let (base, version) = name.rsplit_once('@')?;
        if !version.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        (base, Some(version))
    };
    let mut chars = base.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    Some(match version {
        Some(v) if !v.is_empty() => format!("{base}@{v}"),
        _ => base.to_owned(),
    })
}

/// A counterexample as source-level variables, without the raw model.
fn variables(model: &Counterexample) -> Value {
    let mut variables = Map::new();
    for a in &model.assignments {
        if let Some(name) = source_variable(&a.name) {
            variables.insert(name, Value::String(a.value.clone()));
        }
    }
    let omitted = model.assignments.len() - variables.len();
    json!({
        "check": model.check,
        "variables": variables,
        "internal_assignments_omitted": omitted,
    })
}

fn variables_text(v: &Value) -> String {
    v["variables"]
        .as_object()
        .map(|vars| {
            vars.iter()
                .map(|(k, v)| format!("{k} = {}", v.as_str().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// Replaces every recorded counterexample in `value` with its source-level
/// variables. The raw `get-model` reply stays available with `detail: "full"`.
fn compact_models(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.contains_key("model") && map.contains_key("assignments") {
                if let Ok(model) =
                    serde_json::from_value::<Counterexample>(Value::Object(std::mem::take(map)))
                {
                    *value = variables(&model);
                }
                return;
            }
            for v in map.values_mut() {
                compact_models(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(compact_models),
        _ => {}
    }
}

/// The job-backed `counterexample` reply repeats each model three times:
/// in `function_report`, `obligations` and `counterexamples`.
fn drop_duplicate_reports(value: &mut Value) {
    if let Some(functions) = value["functions"].as_array_mut() {
        for f in functions {
            if let Some(entry) = f.as_object_mut() {
                entry.remove("function_report");
            }
        }
    }
}

fn is_summary_diagnostic(message: &str) -> bool {
    // rustc's "aborting due to N previous errors" and Cargo's "could not
    // compile" summarise the errors; they are not errors of their own.
    message.starts_with("aborting due to") || message.starts_with("could not compile")
}

/// The agent view of a project result: requested targets only.
#[allow(
    clippy::too_many_lines,
    reason = "one linear assembly of the reply; splitting it hides the order"
)]
fn agent_view(project: &ProjectResult, job: &Value, reuse: &[String]) -> Value {
    let requested: Vec<&TargetResult> = project.targets.iter().filter(|t| t.requested).collect();
    let dependencies: Vec<&TargetResult> =
        project.targets.iter().filter(|t| !t.requested).collect();
    let mut counts = Counts::default();
    let mut failures = Vec::new();
    let mut unresolved = Vec::new();
    let mut errors = Vec::new();
    let mut verus = (0_u64, 0_u64);
    let mut targets = Vec::new();
    for target in &requested {
        let mut target_counts = Counts::default();
        for c in &target.report.crates {
            let cleared = c.unknown_passed_by_verus_totals();
            counts.passed_by_verus_totals += cleared;
            target_counts.passed_by_verus_totals += cleared;
            for f in &c.functions {
                counts.add(f);
                target_counts.add(f);
                match effective_verdict(f) {
                    VerdictName::Verified => {}
                    VerdictName::Failed => failures.push(failure(target, f)),
                    // Cleared by Verus's totals: not an open question.
                    VerdictName::Unknown if cleared > 0 => {}
                    v @ (VerdictName::Unknown | VerdictName::Provisional) => {
                        unresolved
                            .push(json!({"function": f.function, "verdict": verdict_name(v)}));
                    }
                }
            }
        }
        let (mut tv, mut te) = (0, 0);
        for c in &target.report.crates {
            tv += c.verus.verified.unwrap_or(0);
            te += c.verus.errors.unwrap_or(0);
        }
        verus.0 += tv;
        verus.1 += te;
        for d in &target.report.diagnostics {
            if d.level == "error" && !is_summary_diagnostic(&d.message) {
                let primary = d.spans.iter().find(|s| s.primary).or(d.spans.first());
                errors.push(json!({
                    "message": d.message,
                    "at": primary.map(|s| format!("{}:{}:{}", s.file, s.line, s.column)),
                    "rendered": d.rendered,
                }));
            }
        }
        targets.push(json!({
            "package_id": target.package_id,
            "target": target.target,
            "kind": target.kind,
            "reused": target.reused,
            "establishing_run": target.establishing_run,
            "complete": target.complete,
            "attribution": target.report.attribution.status,
            "counts": target_counts.json(),
            "verus_totals": {"verified": tv, "errors": te},
        }));
    }
    let mut dependency_counts = Counts::default();
    for target in &dependencies {
        functions(target).for_each(|f| dependency_counts.add(f));
    }
    let dependency_names: Vec<&str> = dependencies.iter().map(|t| t.target.as_str()).collect();

    // The summary: outcome first, then each failure, then context.
    let names = |list: &[Value]| {
        list.iter()
            .take(MAX_LISTED)
            .filter_map(|v| v["function"].as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let scope = format!(
        "{} function(s) in {}",
        counts.functions,
        requested
            .iter()
            .map(|t| t.target.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let exit = project
        .exit_code
        .map_or_else(|| "no exit code".to_owned(), |c| format!("exit {c}"));
    let passed = counts.passed_by_verus_totals;
    let mut text = if counts.failed > 0 {
        let mut line = format!(
            "FAILED: {} of {scope} failed: {}. {} verified, ",
            counts.failed,
            names(&failures),
            counts.verified,
        );
        if passed > 0 {
            let _ = write!(line, "{passed} passed by Verus's totals, ");
        }
        let _ = write!(
            line,
            "{} unknown, {} provisional ({exit}).",
            counts.unknown - passed,
            counts.provisional
        );
        if passed > 0 {
            let _ = write!(
                line,
                "\nVerus reports no error beyond the failed function(s), so the other {passed} function(s) passed by its totals. Sail could not attribute results to them one by one (attribution incomplete), so their reports say `unknown`."
            );
        }
        line
    } else if counts.unknown + counts.provisional > 0 {
        format!(
            "NOT VERIFIED: {} of {scope} unresolved (unknown or provisional, never verified): {}. {} verified ({exit}).",
            counts.unknown + counts.provisional,
            names(&unresolved),
            counts.verified
        )
    } else if project.no_reuse.is_some() && counts.functions == 0 && errors.is_empty() {
        // Without proof reuse, Cargo-Verus skips crates it finds up to date
        // and reports nothing for them: no function result is not a proof.
        format!(
            "NO RESULT: no function was verified in this run; Cargo found every package up to date, or verified nothing ({exit})."
        )
    } else if project.exit_code == Some(0) && !requested.is_empty() {
        format!("VERIFIED: all {scope} verified ({exit}).")
    } else if errors.is_empty() {
        format!("NO RESULT: {scope}; no function failed, but the run did not succeed ({exit}).")
    } else {
        format!(
            "ERROR: {} error(s) and no failed function in {scope} ({exit}).",
            errors.len()
        )
    };
    if !requested.is_empty() && verus.0 + verus.1 != counts.functions {
        let _ = write!(
            text,
            "\nCounts are functions. Verus's totals ({} verified, {} error(s)) count verification units instead: for example, each loop is verified and counted separately from its function's body.",
            verus.0, verus.1
        );
    }
    for f in failures.iter().take(MAX_LISTED) {
        for o in f["obligations"].as_array().into_iter().flatten() {
            let _ = write!(
                text,
                "\n- {} at {}",
                o["id"].as_str().unwrap_or_default(),
                o["at"].as_str().unwrap_or("?")
            );
            if o["counterexample"].is_object() {
                let vars = variables_text(&o["counterexample"]);
                if !vars.is_empty() {
                    let _ = write!(text, ": counterexample {vars}");
                }
            }
        }
    }
    for e in errors.iter().take(MAX_LISTED) {
        let _ = write!(
            text,
            "\n- error: {} at {}",
            e["message"].as_str().unwrap_or_default(),
            e["at"].as_str().unwrap_or("?")
        );
    }
    if !dependencies.is_empty() {
        let _ = write!(
            text,
            "\nDependencies not shown: {} ({} function(s): {} verified, {} failed, {} other). Pass detail \"targets\" for the requested targets' complete reports, or \"full\" for everything.",
            dependency_names.join(", "),
            dependency_counts.functions,
            dependency_counts.verified,
            dependency_counts.failed,
            dependency_counts.unknown + dependency_counts.provisional,
        );
    }
    for line in reuse {
        text.push('\n');
        text.push_str(line);
    }
    let _ = write!(
        text,
        "\nJob {}; currentness {}.",
        job["id"].as_str().unwrap_or("?"),
        serde_json::to_value(project.currentness)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    );

    let listed = |mut list: Vec<Value>| {
        let total = list.len();
        list.truncate(MAX_LISTED);
        (list, total)
    };
    let (failures, failures_total) = listed(failures);
    let (unresolved, unresolved_total) = listed(unresolved);
    let (errors, errors_total) = listed(errors);
    json!({
        "summary": text,
        "detail": "summary",
        "job": job,
        "exit_code": project.exit_code,
        "currentness": project.currentness,
        "input_revision": project.input_revision,
        "counts": counts.json(),
        "verus_totals": {"verified": verus.0, "errors": verus.1},
        "failures": failures,
        "failures_total": failures_total,
        "unresolved": unresolved,
        "unresolved_total": unresolved_total,
        "errors": errors,
        "errors_total": errors_total,
        "targets": targets,
        "dependencies": {
            "targets": dependency_names,
            "counts": dependency_counts.json(),
            "note": "Dependency reports are not included. Pass detail \"full\" to include them.",
        },
        "notes": project.notes,
        "no_reuse": project.no_reuse,
        "error": project.error,
    })
}

/// A failed function: its failing obligations with locations and models.
fn failure(target: &TargetResult, f: &FunctionReport) -> Value {
    let obligations: Vec<Value> = f
        .obligations
        .iter()
        .flatten()
        .filter(|o| o.outcome != OutcomeName::Proved && !o.advisory)
        .map(|o| {
            let span = o.span.as_ref();
            json!({
                "id": o.id,
                "kind": o.kind,
                "outcome": o.outcome,
                "at": span.and_then(|s| at(s.file.as_deref(), s.line, s.column)),
                "counterexample": o.counterexample.as_ref().map(variables),
            })
        })
        .collect();
    json!({
        "function": f.function,
        "target": target.target,
        "verdict": verdict_name(effective_verdict(f)),
        "establishment": f.establishment,
        "evidence_tier": crate::summary::effective_evidence_tier(f),
        "obligations": obligations,
    })
}

fn explanation_summary(document: &Value) -> String {
    let value = &document["value"];
    let f = &value["function"];
    let mut text = format!(
        "{}: {} in {} (establishing run {}, currentness {}).",
        f["function"].as_str().unwrap_or("?"),
        if f["establishment"] == "provisional" {
            "provisional"
        } else {
            f["verdict"].as_str().unwrap_or("?")
        },
        value["target"]["name"].as_str().unwrap_or("?"),
        value["establishing_run"].as_str().unwrap_or("?"),
        value["currentness"].as_str().unwrap_or("?"),
    );
    for o in f["obligations"].as_array().into_iter().flatten() {
        if o["outcome"] == "proved" {
            continue;
        }
        let _ = write!(
            text,
            "\n- {} ({}): {}",
            o["id"].as_str().unwrap_or("?"),
            o["kind"].as_str().unwrap_or("?"),
            o["outcome"].as_str().unwrap_or("?")
        );
        let vars = variables_text(&o["counterexample"]);
        if !vars.is_empty() {
            let _ = write!(text, "; counterexample {vars}");
        }
    }
    let attributed = value["attributed_diagnostics"]
        .as_array()
        .map_or(0, Vec::len);
    let run = value["run_diagnostics"].as_array().map_or(0, Vec::len);
    let _ = write!(
        text,
        "\n{attributed} attributed and {run} other run diagnostic(s); attribution {}.",
        if value["diagnostic_attribution_complete"] == true {
            "complete"
        } else {
            "incomplete"
        }
    );
    text
}

/// The job-backed `counterexample` and `cached_result` replies: each
/// function's verdict and, with `models`, its failing obligations' models.
fn lookup_summary(document: &Value, fallback: &str, models: bool) -> String {
    // The job-backed reply; the older per-project store keeps its own text.
    let (Some(functions), false) = (document["functions"].as_array(), document["job"].is_null())
    else {
        return fallback.to_owned();
    };
    let mut text = String::new();
    for f in functions {
        let _ = write!(
            text,
            "{}{}: {}",
            if text.is_empty() { "" } else { "\n" },
            f["function"].as_str().unwrap_or("?"),
            f["verdict"].as_str().unwrap_or("?")
        );
        if !models {
            continue;
        }
        let mut any = false;
        for o in f["obligations"].as_array().into_iter().flatten() {
            let vars = variables_text(&o["counterexample"]);
            if o["outcome"] == "proved" && vars.is_empty() {
                continue;
            }
            any = true;
            let span = &o["span"];
            let _ = write!(
                text,
                "\n- {} at {}: {}",
                o["id"].as_str().unwrap_or("?"),
                at(
                    span["file"].as_str(),
                    span["line"].as_u64(),
                    span["column"].as_u64()
                )
                .unwrap_or_else(|| "?".to_owned()),
                if vars.is_empty() {
                    format!("{}, no model", o["outcome"].as_str().unwrap_or("?"))
                } else {
                    vars
                }
            );
        }
        if !any {
            text.push_str("; no counterexample model");
        }
    }
    let _ = write!(
        text,
        "\nJob {}; currentness {}.",
        document["job"]["id"].as_str().unwrap_or("?"),
        document["job"]["currentness"].as_str().unwrap_or("?")
    );
    text
}

fn comparison_summary(document: &Value) -> String {
    let value = &document["value"];
    let mut counts: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    for d in value["differences"].as_array().into_iter().flatten() {
        counts
            .entry(d["change"].as_str().unwrap_or("?"))
            .or_default()
            .push(d["function"].as_str().unwrap_or("?"));
    }
    let mut text = format!(
        "{} comparison: ",
        if value["compatible"] == true {
            "Compatible"
        } else {
            "Incompatible (descriptive only, not proof regressions)"
        }
    );
    let parts: Vec<String> = counts
        .iter()
        .map(|(change, names)| format!("{} {change}", names.len()))
        .collect();
    text.push_str(if parts.is_empty() {
        "no functions compared"
    } else {
        ""
    });
    text.push_str(&parts.join(", "));
    text.push('.');
    for (change, names) in &counts {
        if *change == "unchanged" {
            continue;
        }
        let _ = write!(
            text,
            "\n{change}: {}",
            names
                .iter()
                .take(MAX_LISTED)
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        );
        if names.len() > MAX_LISTED {
            let _ = write!(text, " and {} more", names.len() - MAX_LISTED);
        }
    }
    // Why functions that passed by Verus's totals count as passing, and why
    // repeated friendly names are grouped or incomparable (VI-0010).
    for note in value["notes"].as_array().into_iter().flatten() {
        if let Some(note) = note.as_str() {
            let _ = write!(text, "\n{note}");
        }
    }
    text
}

/// Keeps only the functions whose evidence changed, and counts the rest.
fn drop_unchanged(comparison: &mut Value) {
    let Some(differences) = comparison["differences"].as_array_mut() else {
        return;
    };
    let before = differences.len();
    differences.retain(|d| d["change"] != "unchanged");
    comparison["unchanged_total"] = json!(before - differences.len());
}

fn baselines_summary(document: &Value) -> String {
    let names: Vec<&str> = document["value"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if names.is_empty() {
        "No saved baselines.".to_owned()
    } else {
        format!("{} baseline(s): {}.", names.len(), names.join(", "))
    }
}

fn events_summary(document: &Value) -> String {
    let events = document["events"].as_array().map_or(0, Vec::len);
    let mut text = format!(
        "{events} proof event(s); stream {}; resume with stream_id {} after {}.",
        if document["terminal"] == true {
            "complete"
        } else {
            "still open (early observations are provisional)"
        },
        document["next"]["stream_id"].as_str().unwrap_or("?"),
        document["next"]["after"]
    );
    if let Some(gap) = document["gap"].as_str() {
        let _ = write!(text, "\nGap: {gap}");
    }
    text
}

fn status_summary(tool: &str, document: &Value) -> String {
    // `verification_status`/`cancel_verification` wrap the status in
    // `{"response":"status","value":…}`; submissions return it bare.
    let status = if document["value"].is_object() {
        &document["value"]
    } else {
        document
    };
    let state = status["state"].as_str().unwrap_or("?");
    let mut text = format!("Job {}: {state}", status["id"].as_str().unwrap_or("?"));
    if state == "completed" {
        let _ = write!(
            text,
            "; exit {}; currentness {}. Read verdicts with verification_result.",
            status["exit_code"],
            status["currentness"].as_str().unwrap_or("?")
        );
    } else if let Some(error) = status["error"].as_str() {
        let _ = write!(text, ": {error}");
    } else {
        text.push('.');
    }
    if tool == "start_verification" || tool == "start_repair_validation" {
        text.push_str(" Pass its id as job_id to verification_status or verification_result.");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_variables_are_kept_and_internals_dropped() {
        assert_eq!(source_variable("a!").as_deref(), Some("a"));
        assert_eq!(source_variable("i@0").as_deref(), Some("i@0"));
        assert_eq!(source_variable("i@").as_deref(), Some("i"));
        assert_eq!(source_variable("fuel%vstd!view.impl&%4.view."), None);
        assert_eq!(source_variable("$"), None);
        assert_eq!(source_variable("BOOL"), None);
        assert_eq!(source_variable("tmp%1@0"), None);
    }

    #[test]
    fn detail_is_validated_per_tool() {
        let mut args = Map::new();
        assert_eq!(take_detail("verify", &mut args), Ok(Detail::Summary));
        args.insert("detail".into(), json!("targets"));
        assert_eq!(take_detail("verify", &mut args), Ok(Detail::Targets));
        assert!(args.is_empty());
        args.insert("detail".into(), json!("targets"));
        assert!(take_detail("counterexample", &mut args).is_err());
        args.insert("detail".into(), json!("full"));
        assert!(take_detail("list_baselines", &mut args).is_err());
    }

    /// A project result whose requested target and dependency are both the
    /// recorded attribution fixture (two failed functions).
    fn project() -> Value {
        let report: Value = serde_json::from_str(include_str!(
            "../../sail-cli/tests/fixtures/attr/report.golden.json"
        ))
        .unwrap_or_default();
        let target = |name: &str, requested: bool| {
            json!({"package_id": format!("{name}#0.1.0"), "target": name, "kind": ["lib"],
                   "establishing_run": "run-1", "reused": false, "requested": requested,
                   "complete": true, "report": report})
        };
        json!({"kind": "sail-project-result", "schema_version": 2, "run_id": "run-1",
               "workspace": "/w", "context_sha256": "c", "input_revision": "rev", "currentness": "current",
               "exit_code": 1, "targets": [target("input", true), target("vstd", false)],
               "execution": {"verifier_invocations": 1, "reused_targets": 0, "wall_ms": 1.0},
               "notes": [], "error": null})
    }

    #[test]
    fn the_agent_view_puts_failures_first_and_only_counts_dependencies() {
        let document = json!({"response": "result",
            "value": {"job": {"id": "job-1", "state": "completed"}, "result": project()}});
        let full = ToolResult::ok(String::new(), document);
        let shaped = shape("verification_result", Detail::Summary, full.clone());
        let view = &shaped.structured["value"]["result"];
        assert!(
            shaped
                .text
                .starts_with("FAILED: 2 of 8 function(s) in input failed: "),
            "{}",
            shaped.text
        );
        assert!(
            shaped
                .text
                .contains("Dependencies not shown: vstd (8 function(s)"),
            "{}",
            shaped.text
        );
        assert_eq!(view["counts"]["verified"], 6);
        assert_eq!(view["failures_total"], 2);
        assert_eq!(view["targets"].as_array().map(Vec::len), Some(1));
        assert_eq!(view["dependencies"]["counts"]["functions"], 8);
        let text = view.to_string();
        assert!(
            !text.contains("\"model\""),
            "raw models are only sent on request"
        );
        assert!(text.len() < 16_000, "{}", text.len());
        // `targets` keeps the requested reports; `full` changes nothing.
        let targets = shape("verification_result", Detail::Targets, full.clone());
        let kept = &targets.structured["value"]["result"]["targets"];
        assert_eq!(kept.as_array().map(Vec::len), Some(1));
        assert_eq!(kept[0]["report"]["summary"]["failed"], 2);
        let unchanged = shape("verification_result", Detail::Full, full.clone());
        assert_eq!(unchanged.structured, full.structured);
    }

    #[test]
    fn verus_totals_clear_unattributed_functions_without_verifying_them() {
        let mut project = project();
        // Incomplete attribution in the failing crate: successes are unknown.
        for f in project["targets"][0]["report"]["crates"][0]["functions"]
            .as_array_mut()
            .into_iter()
            .flatten()
        {
            if f["verdict"] == "verified" {
                f["verdict"] = json!("unknown");
            }
        }
        let document = json!({"response": "result",
            "value": {"job": {"id": "job-1"}, "result": project}});
        let shaped = shape(
            "verification_result",
            Detail::Summary,
            ToolResult::ok(String::new(), document),
        );
        let view = &shaped.structured["value"]["result"];
        assert!(
            shaped.text.contains(
                "0 verified, 6 passed by Verus's totals, 0 unknown, 0 provisional (exit 1)."
            ),
            "{}",
            shaped.text
        );
        // The verdicts stay unknown; only the count says Verus's totals clear them.
        assert_eq!(view["counts"]["verified"], 0);
        assert_eq!(view["counts"]["unknown"], 6);
        assert_eq!(view["counts"]["unknown_passed_by_verus_totals"], 6);
        assert_eq!(view["unresolved_total"], 0);
        // Verus counted 11 units for 8 functions; the summary says why.
        assert!(
            shaped
                .text
                .contains("Counts are functions. Verus's totals (9 verified, 2 error(s))"),
            "{}",
            shaped.text
        );
    }

    #[test]
    fn comparisons_list_only_changes_by_default() {
        let document = json!({"response": "comparison", "value": {"compatible": true,
            "differences": [{"function": "a", "change": "unchanged"},
                            {"function": "b", "change": "regressed"}]}});
        let shaped = shape(
            "compare_verification",
            Detail::Summary,
            ToolResult::ok(String::new(), document),
        );
        assert_eq!(shaped.structured["value"]["unchanged_total"], 1);
        assert_eq!(
            shaped.structured["value"]["differences"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert!(shaped.text.contains("1 regressed"), "{}", shaped.text);
        assert!(shaped.text.contains("regressed: b"), "{}", shaped.text);
    }

    /// The release-candidate walkthrough on vest: a baseline with one failing
    /// function, whose other functions Sail could not attribute one by one
    /// (`unknown`, passed by Verus's totals), compared with the fixed run.
    #[test]
    fn a_fix_against_an_unattributed_baseline_reports_one_recovered_function() {
        let Ok(mut after) = serde_json::from_value::<ProjectResult>(project()) else {
            unreachable!("fixture")
        };
        after.targets.truncate(1);
        after.exit_code = Some(0);
        let krate = &mut after.targets[0].report.crates[0];
        let template = krate.functions[0].clone();
        let function = |name: String| {
            let mut f = template.clone();
            f.function = name;
            f.verdict = VerdictName::Verified;
            f
        };
        // 3,127 functions, 95 friendly names held by two implementations each (VI-0010).
        krate.functions = (0..2937)
            .map(|i| function(format!("vest_lib::f{i}")))
            .chain((0..190).map(|i| function(format!("vest_lib::Impl::dup{}", i / 2))))
            .collect();
        krate.verus.success = Some(true);
        krate.verus.errors = Some(0);
        krate.verus.verified = Some(3236);
        let mut before = after.clone();
        before.exit_code = Some(1);
        let krate = &mut before.targets[0].report.crates[0];
        for f in &mut krate.functions {
            f.verdict = VerdictName::Unknown;
        }
        krate.functions[0].verdict = VerdictName::Failed;
        krate.verus.success = Some(false);
        krate.verus.errors = Some(1);
        assert_eq!(krate.unknown_passed_by_verus_totals(), 3126);

        let comparison = sail_api::evidence::compare(&before, &after);
        let document = json!({"response": "comparison", "value": comparison});
        let full = shape(
            "compare_verification",
            Detail::Full,
            ToolResult::ok(String::new(), document.clone()),
        );
        assert_eq!(
            full.structured["value"]["differences"]
                .as_array()
                .map(Vec::len),
            Some(3032)
        );
        let shaped = shape(
            "compare_verification",
            Detail::Summary,
            ToolResult::ok(String::new(), document),
        );
        assert!(
            shaped.text.starts_with(
                "Compatible comparison: 1 recovered, 3031 unchanged.\nrecovered: vest_lib::f0\n"
            ),
            "{}",
            shaped.text
        );
        assert!(
            shaped
                .text
                .contains("3126 function(s) reported `unknown` passed by Verus's totals")
        );
        assert!(
            shaped
                .text
                .contains("95 repeated name(s), shared by 190 functions")
        );
        assert_eq!(shaped.structured["value"]["unchanged_total"], 3031);
        let size = serde_json::to_string(&shaped.structured).map_or(usize::MAX, |s| s.len())
            + shaped.text.len();
        assert!(size < 4 * 1024, "{size} bytes");
    }

    #[test]
    fn models_become_source_variables() {
        let mut value = json!({"obligations": [{"counterexample": {
            "check": 0,
            "model": "(model …)",
            "assignments": [
                {"name": "a!", "sort": "Int", "value": "1", "itf": {"#bigint": "1"}},
                {"name": "fuel%vstd!x.", "sort": "Fuel", "value": "Fuel!val!0", "itf": {"#unserializable": "Fuel!val!0"}},
                {"name": "r!", "sort": "Int", "value": "0", "itf": {"#bigint": "0"}}
            ]
        }}]});
        compact_models(&mut value);
        let c = &value["obligations"][0]["counterexample"];
        assert_eq!(c["variables"], json!({"a": "1", "r": "0"}));
        assert_eq!(c["internal_assignments_omitted"], 1);
        assert!(c.get("model").is_none());
        assert_eq!(variables_text(c), "a = 1, r = 0");
    }
}
