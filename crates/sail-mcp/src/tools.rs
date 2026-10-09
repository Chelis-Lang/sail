//! The tools: `verify`, `counterexample`, `cached_result` and `fragility`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sail_api::output::{FunctionReport, OutcomeName, Report};
use serde_json::{Map, Value, json};

use crate::args::{LookupArgs, RunArgs};
use crate::run::{self, Cancel};
use crate::store::{self, LookupError, ProjectRecord, RunRecord, Store};
use crate::summary;

/// The `sail` arguments `verify` starts with.
const VERIFY_COMMAND: &[&str] = &["verify", "--output", "json"];

/// The `sail` arguments `fragility` starts with (`sail fragility`, ADR
/// 0018). The tool still probes `sail --help` for it on every call, so a
/// `sail` without it gives a clear not-available result.
const FRAGILITY_COMMAND: &[&str] = &["fragility", "--output", "json"];

/// How many changed files a freshness report lists.
const MAX_CHANGES: usize = 50;

/// The server's configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// The `sail` binary that tool calls run.
    pub sail: PathBuf,
    /// Where per-project results are persisted (memory only when `None`).
    pub store_dir: Option<PathBuf>,
    /// The default project directory, and the base for relative ones.
    pub cwd: PathBuf,
}

/// Shared state for tool calls.
#[derive(Debug)]
pub struct Context {
    /// The configuration.
    pub config: Config,
    /// The result store.
    pub store: Store,
    /// One lock per project: runs in one project are serialised.
    locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl Context {
    /// A context for `config`.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            store: Store::new(config.store_dir.clone()),
            config,
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self, project: &Path) -> Arc<Mutex<()>> {
        self.locks.lock().map_or_else(
            |_| Arc::new(Mutex::new(())),
            |mut locks| Arc::clone(locks.entry(project.to_path_buf()).or_default()),
        )
    }
}

/// A tool's result.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    /// A short human summary.
    pub text: String,
    /// The structured result.
    pub structured: Value,
    /// Whether the call failed (an MCP tool execution error).
    pub is_error: bool,
}

impl ToolResult {
    pub(super) fn ok(text: String, structured: Value) -> Self {
        Self {
            text,
            structured,
            is_error: false,
        }
    }

    pub(super) fn error(text: String, structured: Value) -> Self {
        Self {
            text,
            structured,
            is_error: true,
        }
    }

    pub(super) fn invalid(message: &str) -> Self {
        Self::error(
            format!("Invalid arguments: {message}"),
            json!({"error": message}),
        )
    }

    /// The MCP `CallToolResult`: the summary and the structured result as
    /// text (for clients that ignore `structuredContent`), plus
    /// `structuredContent`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "resultType": "complete",
            "content": [
                {"type": "text", "text": self.text},
                {"type": "text", "text": serde_json::to_string(&self.structured).unwrap_or_default()},
            ],
            "structuredContent": self.structured,
            "isError": self.is_error,
        })
    }
}

fn string_schema(description: &str) -> Value {
    json!({"type": "string", "description": description})
}

fn list_schema(description: &str) -> Value {
    json!({"type": "array", "items": {"type": "string"}, "description": description})
}

fn run_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "project_dir": string_schema("The Cargo project to verify (absolute, or relative to the server's working directory). Default: the server's working directory."),
            "package": string_schema("Verify only this workspace package (`-p`)."),
            "module": string_schema("Verify only this module and its descendants, relative to the crate root (Verus `--verify-module`, e.g. `arith` or `foo::bar`)."),
            "function": string_schema("Verify only this function (Verus `--verify-function`: a unique substring of the name, or a pattern with `*` at either end). It is looked up in `module`, or in the crate root if no module is given."),
            "measure_fragility": {"type": "boolean", "description": "Opt-in local solver replays to attach function fragility scores. Can take much longer than ordinary verification."},
            "portfolio": {"type": "string", "enum": ["accept-first-proof"], "description": "Explicit opt-in: run complete independent Verus attempts and accept the first complete proof. May take much longer; records every attempt."},
            "cargo_args": list_schema("Extra `cargo verus verify` arguments (before `--`), e.g. [\"--release\"]."),
            "verus_args": list_schema("Extra Verus arguments (after `--`), e.g. [\"--rlimit\", \"30\"]."),
        }
    })
}

fn lookup_schema(target: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "project_dir": string_schema("The project, as given to `verify`. Default: the server's working directory."),
            "function": string_schema(&format!("The function: its full path as reported (`demo::arith::mul_pos`), or a unique `::` suffix (`mul_pos`). {target}")),
            "obligation": string_schema("An obligation id from a report, e.g. `demo::sum_to#Function-Def/0`."),
        }
    })
}

/// The tool definitions (`tools/list`), in a fixed order.
#[must_use]
pub fn definitions() -> Value {
    let mut definitions = json!([
        {
            "name": "verify",
            "title": "Verify with sail",
            "description": "Verify a Verus crate (or one module or function) with `sail verify --output json`, which gives the same verdicts as `cargo verus verify` but serves unchanged solver work from sail's cache. Returns a short summary and the schema-v1 report (docs/output-schema.md): per-function verdicts (verified, failed, unknown, provisional) with tier and provenance, obligation ids, counterexample models, Verus diagnostics as data, timings and cache sources. `provisional` is never `verified`. Default verification uses a persistent forced-fresh workspace job, while retaining dependency build artifacts. Results are recorded for `cached_result` and `counterexample`; those tools remain responsive during verification.",
            "inputSchema": run_schema(),
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "counterexample",
            "title": "Counterexample models",
            "description": "From the latest recorded `verify` result for a function or obligation, return the counterexample models Z3 gave for its failed obligations (assignments as data, plus the raw `get-model` reply) and the Verus diagnostics located in the function, without re-running anything. Says whether the sources changed since. Verus names parameter `x` as `x!` and versions of local `i` as `i@0`, `i@1`.",
            "inputSchema": lookup_schema("Give this or `obligation`."),
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        },
        {
            "name": "cached_result",
            "title": "Latest known verdicts",
            "description": "Look up the latest known verdict for a function or obligation (or every function, with neither) without re-running verification. The answer says which run established it, when, with which arguments, and whether the project's sources changed since (by content hash): a `stale` result applies to the earlier sources only.",
            "inputSchema": lookup_schema("Optional."),
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        },
        {
            "name": "fragility",
            "title": "Proof fragility",
            "description": "Measure how fragile proofs are (how often they still verify under different solver seeds), via `sail fragility` (an opt-in measurement that re-runs the solver sessions under several seeds; it changes no verdict and can take much longer than verify). Returns a clear not-available result if this sail build has no `sail fragility`.",
            "inputSchema": run_schema(),
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "openWorldHint": false}
        }
    ]);
    if let Some(list) = definitions.as_array_mut() {
        list.extend(crate::job_tools::definitions(&run_schema()));
    }
    definitions
}

/// Runs tool `name`. `None` means there is no such tool.
#[must_use]
pub fn call(
    ctx: &Context,
    name: &str,
    args: &Map<String, Value>,
    cancel: &Cancel,
) -> Option<ToolResult> {
    Some(match name {
        "start_verification"
        | "verification_status"
        | "verification_result"
        | "cancel_verification" => crate::job_tools::call(ctx, name, args, cancel),
        "verify" => verify(ctx, args, cancel),
        "counterexample" => counterexample(ctx, args),
        "cached_result" => cached_result(ctx, args),
        "fragility" => fragility(ctx, args, cancel),
        _ => return None,
    })
}

pub(super) fn project_dir(ctx: &Context, given: Option<&str>) -> Result<PathBuf, ToolResult> {
    let path = given.map_or_else(|| ctx.config.cwd.clone(), |p| ctx.config.cwd.join(p));
    match path.canonicalize() {
        Ok(p) if p.is_dir() => Ok(p),
        Ok(p) => Err(ToolResult::invalid(&format!(
            "`project_dir` {} is not a directory",
            p.display()
        ))),
        Err(e) => Err(ToolResult::invalid(&format!(
            "`project_dir` {}: {e}",
            path.display()
        ))),
    }
}

/// How a recorded run compares with the sources now.
fn freshness(run: &RunRecord, now: &store::Snapshot) -> Value {
    let changed = store::changes(&run.sources, now);
    let stale = !changed.is_empty() || run.sources_changed_during_run;
    let note = if run.sources_changed_during_run {
        "The sources changed while this run was in progress, so its results may reflect either version. Re-run verify."
    } else if stale {
        "The sources changed since this run: its results apply to the earlier sources only. Re-run verify to confirm."
    } else {
        "Tracked sources are unchanged, but this historical record has no complete workspace/runtime freshness evidence. Use verification_result for managed currentness."
    };
    json!({
        "status": if stale { "stale" } else { "unknown" },
        "run": run.run,
        "established_at": run.finished_at,
        "established_by": format!("sail {}", run.sail_args.join(" ")),
        "exit_code": run.exit_code,
        "mode": run.report.mode,
        "solver": run.report.solver,
        "winner_seed": run.report.portfolio.as_ref().and_then(|p| p.winner_seed),
        "scoped": run.scoped,
        "sources_changed_since": !changed.is_empty(),
        "sources_changed_during_run": run.sources_changed_during_run,
        "changed_files": changed.iter().take(MAX_CHANGES).collect::<Vec<_>>(),
        "changed_files_total": changed.len(),
        "note": note,
    })
}

fn freshness_line(f: &Value) -> String {
    format!(
        "Established by run {} ({}) at {}; {}",
        f["run"],
        f["established_by"].as_str().unwrap_or_default(),
        f["established_at"].as_str().unwrap_or_default(),
        f["note"].as_str().unwrap_or_default()
    )
}

fn verify(ctx: &Context, args: &Map<String, Value>, cancel: &Cancel) -> ToolResult {
    let request = match RunArgs::parse(args) {
        Ok(r) => r,
        Err(e) => return ToolResult::invalid(&e),
    };
    let project = match project_dir(ctx, request.project_dir.as_deref()) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if !request.measure_fragility && request.portfolio.is_none() {
        match crate::job_tools::blocking(ctx, &request, &project, cancel) {
            Ok(Some((job, result))) => {
                let Some(report) = crate::job_tools::merged_report(&result) else {
                    return ToolResult::ok("Cargo completed before any verification target produced a report; see the job diagnostics.".to_owned(), json!({"job":job,"project_result":result}));
                };
                let now = store::snapshot(&project);
                let raw = serde_json::to_value(&report).unwrap_or(Value::Null);
                let base = json!({"project_dir":project,"sail_args":request.sail_args(VERIFY_COMMAND),"exit_code":result.exit_code,"job":job,"project_result":result});
                return finish_verify(ctx, &project, &request, base, raw, report, &now, &now);
            }
            Ok(None) => {}
            Err(error) => return error,
        }
    }
    let lock = ctx.lock(&project);
    let _guard = lock.lock();
    let sail_args = request.sail_args(VERIFY_COMMAND);
    let before = store::snapshot(&project);
    let outcome = match run::run(&ctx.config.sail, &sail_args, &project, cancel) {
        Ok(o) => o,
        Err(e) => {
            return ToolResult::error(
                format!("Cannot run {}: {e}", ctx.config.sail.display()),
                json!({"project_dir": project, "sail_args": sail_args, "error": e.to_string()}),
            );
        }
    };
    let after = store::snapshot(&project);
    let base = json!({
        "project_dir": project,
        "sail_args": sail_args,
        "exit_code": outcome.exit_code,
        "duration_ms": u64::try_from(outcome.duration.as_millis()).unwrap_or(u64::MAX),
    });
    let raw: Option<Value> = serde_json::from_slice(&outcome.stdout).ok();
    let report = raw
        .as_ref()
        .and_then(|v| serde_json::from_value::<Report>(v.clone()).ok());
    let (Some(raw), Some(report)) = (raw, report) else {
        let mut s = base;
        s["stderr_tail"] = json!(outcome.stderr_tail);
        return ToolResult::error(
            format!(
                "sail verify produced no report (exit {:?}). Its stderr ends with:\n{}",
                outcome.exit_code, outcome.stderr_tail
            ),
            s,
        );
    };
    finish_verify(ctx, &project, &request, base, raw, report, &before, &after)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site; splits verify's tail"
)]
fn finish_verify(
    ctx: &Context,
    project: &Path,
    request: &RunArgs,
    mut s: Value,
    raw: Value,
    report: Report,
    before: &store::Snapshot,
    after: &store::Snapshot,
) -> ToolResult {
    let mut text = format!("sail verify: {}", summary::report(&report));
    s["sources"] = json!({"files": before.len(), "changed_during_run": before != after});
    if let Some(e) = &report.error {
        s["report"] = raw;
        s["summary"] = json!(text);
        s["stored"] = json!(false);
        return ToolResult::error(
            format!("sail verify could not run Verus: {} {}", e.code, e.message),
            s,
        );
    }
    if report.crates.is_empty() {
        // Nothing verified: the build failed first, or Cargo skipped Verus
        // because it found the crate up to date (docs/verus-issues/VI-0005).
        // Either way, keep the record.
        let last = ctx
            .store
            .with(project, |p| p.last().map(|r| freshness(r, after)));
        let built =
            report.exit_code == Some(0) && !report.diagnostics.iter().any(|d| d.level == "error");
        text.push_str(if built {
            "\nVerus did not run: Cargo found the crate up to date. Nothing was recorded."
        } else {
            "\nNo verification results: the build failed before Verus verified anything (see the errors above and `report.diagnostics`). Nothing was recorded."
        });
        if built && let Some(f) = &last {
            text.push('\n');
            text.push_str(&freshness_line(f));
            text.push_str(" Use cached_result for its verdicts. To force re-verification, change a source file or run `cargo clean -p <package>`.");
        }
        s["latest_recorded_run"] = last.unwrap_or(Value::Null);
        s["stored"] = json!(false);
        s["summary"] = json!(text);
        s["report"] = raw;
        return ToolResult::ok(text, s);
    }
    let finished = store::now_unix_ms();
    let record = RunRecord {
        run: 0,
        finished_unix_ms: finished,
        finished_at: store::rfc3339(finished),
        sail_args: s["sail_args"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        exit_code: report.exit_code,
        duration_ms: s["duration_ms"].as_u64().unwrap_or_default(),
        scoped: request.scoped(),
        sources: before.clone(),
        sources_changed_during_run: before != after,
        report,
    };
    let (number, store_error) = ctx.store.record(project, record);
    if before != after {
        text.push_str("\nWarning: the sources changed while verifying; re-run verify for results that match them.");
    }
    s["run"] = json!(number);
    s["finished_at"] = json!(store::rfc3339(finished));
    s["stored"] = json!(true);
    if let Some(e) = store_error {
        s["store_note"] = json!(e);
    }
    s["summary"] = json!(text);
    s["report"] = raw;
    ToolResult::ok(text, s)
}

fn lookup_error(e: &LookupError, what: &str) -> ToolResult {
    match e {
        LookupError::NotFound(known) => ToolResult::error(
            format!(
                "No recorded result for `{what}`. Known functions: {}",
                known.join(", ")
            ),
            json!({"error": "not_found", "query": what, "known_functions": known}),
        ),
        LookupError::Ambiguous(many) => ToolResult::error(
            format!(
                "`{what}` matches several functions; give the full path: {}",
                many.join(", ")
            ),
            json!({"error": "ambiguous", "query": what, "candidates": many}),
        ),
    }
}

fn no_runs(project: &Path) -> ToolResult {
    ToolResult::error(
        format!(
            "No verify results are recorded for {}; call verify first.",
            project.display()
        ),
        json!({"error": "no_results", "project_dir": project}),
    )
}

/// The function a lookup names: by `function`, else by the obligation's
/// function part.
fn target(a: &LookupArgs) -> Option<&str> {
    a.function.as_deref().or_else(|| {
        a.obligation
            .as_deref()
            .and_then(|o| o.split_once('#'))
            .map(|(f, _)| f)
    })
}

fn find_obligation<'a>(
    f: &'a FunctionReport,
    id: &str,
) -> Result<&'a sail_api::output::ObligationReport, ToolResult> {
    let Some(obligations) = &f.obligations else {
        return Err(ToolResult::error(
            format!(
                "{} has no obligation results: attribution was unavailable for its run.",
                f.function
            ),
            json!({"error": "attribution_unavailable", "function": f.function}),
        ));
    };
    obligations.iter().find(|o| o.id == id).ok_or_else(|| {
        let ids: Vec<&str> = obligations.iter().map(|o| o.id.as_str()).collect();
        ToolResult::error(
            format!(
                "{} has no obligation `{id}`. Its obligations: {}",
                f.function,
                ids.join(", ")
            ),
            json!({"error": "not_found", "obligation": id, "known_obligations": ids}),
        )
    })
}

fn cached_result(ctx: &Context, args: &Map<String, Value>) -> ToolResult {
    let a = match LookupArgs::parse(args, false) {
        Ok(a) => a,
        Err(e) => return ToolResult::invalid(&e),
    };
    let project = match project_dir(ctx, a.project_dir.as_deref()) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if let Some(result) = crate::job_tools::lookup(ctx, &project, &a, false) {
        return result;
    }
    // Return the most recently completed result while a newer run is in flight.
    let now = store::snapshot(&project);
    ctx.store.with(&project, |p| {
        if p.last().is_none() {
            return no_runs(&project);
        }
        match target(&a) {
            None => all_results(p, &project, &now),
            Some(name) => match p.function(name) {
                Err(e) => lookup_error(&e, name),
                Ok(found) => one_result(&a, found, &project, &now),
            },
        }
    })
}

fn one_result(
    a: &LookupArgs,
    found: store::Found<'_>,
    project: &Path,
    now: &store::Snapshot,
) -> ToolResult {
    let f = found.function;
    let fresh = freshness(found.run, now);
    let mut text = summary::function_line(f);
    let mut s = json!({
        "project_dir": project,
        "function": f.function,
        "verdict": summary::verdict_name(summary::effective_verdict(f)),
        "tier": f.tier,
        "mode": f.provenance.mode,
        "configuration": f.provenance.configuration,
        "freshness": fresh,
    });
    if let Some(id) = &a.obligation {
        let id = format!(
            "{}#{}",
            f.function,
            id.split_once('#').map_or("", |(_, r)| r)
        );
        match find_obligation(f, &id) {
            Ok(o) => {
                let outcome = serde_json::to_value(o.outcome).unwrap_or_default();
                text = format!(
                    "{id}: {}; function {text}",
                    outcome.as_str().unwrap_or_default()
                );
                s["obligation"] = json!(o);
            }
            Err(e) => return e,
        }
    } else {
        s["result"] = json!(f);
    }
    text.push('\n');
    text.push_str(&freshness_line(&s["freshness"]));
    ToolResult::ok(text, s)
}

fn all_results(p: &ProjectRecord, project: &Path, now: &store::Snapshot) -> ToolResult {
    let found = p.all_functions();
    let mut runs: BTreeMap<u64, Value> = BTreeMap::new();
    for f in &found {
        runs.entry(f.run.run)
            .or_insert_with(|| freshness(f.run, now));
    }
    let functions: Vec<Value> = found
        .iter()
        .map(|f| {
            json!({
                "function": f.function.function,
                "verdict": summary::verdict_name(summary::effective_verdict(f.function)),
                "tier": f.function.tier,
                "unstable": f.function.unstable,
                "source": f.function.provenance.source,
                "mode": f.function.provenance.mode,
                "configuration": f.function.provenance.configuration,
                "run": f.run.run,
                "status": runs.get(&f.run.run).map_or(Value::Null, |r| r["status"].clone()),
            })
        })
        .collect();
    let mut text = found
        .iter()
        .map(|f| summary::function_line(f.function))
        .collect::<Vec<_>>()
        .join("\n");
    for r in runs.values() {
        text.push('\n');
        text.push_str(&freshness_line(r));
    }
    ToolResult::ok(
        text,
        json!({
            "project_dir": project,
            "functions": functions,
            "runs": runs.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<Map<_, _>>(),
        }),
    )
}

/// Where a function starts: the file and first line of its earliest
/// obligation span.
fn start_of(f: &FunctionReport) -> Option<(&str, u64)> {
    f.obligations
        .iter()
        .flatten()
        .filter_map(|o| {
            let s = o.span.as_ref()?;
            Some((s.file.as_deref()?, s.line?))
        })
        .min_by_key(|(_, line)| *line)
}

/// Diagnostics located in the function: a span in its file, from its first
/// line up to the line before the next function (in the same report) starts.
/// Verus's spans give where a function starts but not where it ends, so this
/// is the closest attribution the report allows.
fn diagnostics_for(report: &Report, f: &FunctionReport) -> Vec<Value> {
    let Some((file, lo)) = start_of(f) else {
        return Vec::new();
    };
    let hi = report
        .crates
        .iter()
        .flat_map(|c| &c.functions)
        .filter_map(start_of)
        .filter(|(other, line)| *other == file && *line > lo)
        .map(|(_, line)| line - 1)
        .min()
        .unwrap_or(u64::MAX);
    report
        .diagnostics
        .iter()
        .filter(|d| {
            d.spans
                .iter()
                .any(|s| s.file == file && (lo..=hi).contains(&s.line))
        })
        .map(|d| json!(d))
        .collect()
}

fn counterexample(ctx: &Context, args: &Map<String, Value>) -> ToolResult {
    let a = match LookupArgs::parse(args, true) {
        Ok(a) => a,
        Err(e) => return ToolResult::invalid(&e),
    };
    let project = match project_dir(ctx, a.project_dir.as_deref()) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if let Some(result) = crate::job_tools::lookup(ctx, &project, &a, true) {
        return result;
    }
    // Return the most recently completed result while a newer run is in flight.
    let now = store::snapshot(&project);
    ctx.store.with(&project, |p| {
        if p.last().is_none() {
            return no_runs(&project);
        }
        let name = target(&a).unwrap_or_default();
        match p.function(name) {
            Err(e) => lookup_error(&e, name),
            Ok(found) => models(&a, found, &project, &now),
        }
    })
}

fn models(
    a: &LookupArgs,
    found: store::Found<'_>,
    project: &Path,
    now: &store::Snapshot,
) -> ToolResult {
    let f = found.function;
    let report = &found.run.report;
    let fresh = freshness(found.run, now);
    let selected: Vec<&sail_api::output::ObligationReport> = if let Some(id) = &a.obligation {
        let id = format!(
            "{}#{}",
            f.function,
            id.split_once('#').map_or("", |(_, r)| r)
        );
        match find_obligation(f, &id) {
            Ok(o) => vec![o],
            Err(e) => return e,
        }
    } else {
        f.obligations
            .iter()
            .flatten()
            .filter(|o| o.outcome != OutcomeName::Proved || o.counterexample.is_some())
            .collect()
    };
    let with_model = selected
        .iter()
        .filter(|o| o.counterexample.is_some())
        .count();
    let mut text = summary::function_line(f);
    if f.obligations.is_none() {
        text.push_str("\nNo obligation results: attribution was unavailable for this run");
        if let Some(n) = report.attribution.notes.first() {
            let _ = write!(text, " ({n})");
        }
        text.push_str(". Diagnostics below come from Verus's output.");
    } else if with_model == 0 {
        text.push_str("\nNo counterexample model: ");
        text.push_str(if selected.is_empty() {
            "every obligation was proved."
        } else {
            "Z3 gave no model for the selected obligations."
        });
    } else {
        let _ = write!(
            text,
            "
{with_model} obligation(s) with a counterexample model:"
        );
        for o in selected.iter().filter(|o| o.counterexample.is_some()) {
            let assigns: Vec<String> = o
                .counterexample
                .iter()
                .flat_map(|c| &c.assignments)
                // Source variables only (`x!`, `i@0`); the full model is
                // in the structured result.
                .filter(|x| {
                    !x.name.contains('%') && (x.name.ends_with('!') || x.name.contains('@'))
                })
                .map(|x| format!("{} = {}", x.name, x.value))
                .collect();
            let _ = write!(text, "\n  {}: {}", o.id, assigns.join(", "));
        }
    }
    let diagnostics = if f.obligations.is_some() {
        diagnostics_for(report, f)
    } else {
        report.diagnostics.iter().map(|d| json!(d)).collect()
    };
    text.push('\n');
    text.push_str(&freshness_line(&fresh));
    ToolResult::ok(
        text,
        json!({
            "project_dir": project,
            "function": f.function,
            "verdict": summary::verdict_name(summary::effective_verdict(f)),
            "tier": f.tier,
            "mode": f.provenance.mode,
            "configuration": f.provenance.configuration,
            "obligations": selected,
            "diagnostics": diagnostics,
            "diagnostics_matched_by": if f.obligations.is_some() { "source location: from the function's first line to the next function's" } else { "none: every diagnostic of the run" },
            "attribution": report.attribution,
            "freshness": fresh,
        }),
    )
}

/// Whether this `sail` lists a `fragility` command in its help.
fn has_fragility(sail: &Path, dir: &Path) -> bool {
    run::run(sail, &["--help".to_owned()], dir, &Cancel::default()).is_ok_and(|o| {
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|l| l.split_whitespace().next() == Some(FRAGILITY_COMMAND[0]))
    })
}

fn fragility(ctx: &Context, args: &Map<String, Value>, cancel: &Cancel) -> ToolResult {
    let request = match RunArgs::parse(args) {
        Ok(r) => r,
        Err(e) => return ToolResult::invalid(&e),
    };
    let project = match project_dir(ctx, request.project_dir.as_deref()) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if request.portfolio.is_some() {
        return ToolResult::error(
            "Portfolio is available only on the verify tool".to_owned(),
            json!({"error": "portfolio_requires_verify"}),
        );
    }
    if !has_fragility(&ctx.config.sail, &project) {
        return ToolResult::error(
            "Fragility measurement is not available yet: this sail has no `sail fragility` command. Use verify; each function's `unstable` flag marks answers that could differ between runs.".to_owned(),
            json!({"available": false, "error": "not_available", "sail": ctx.config.sail}),
        );
    }
    let lock = ctx.lock(&project);
    let _guard = lock.lock();
    let sail_args = request.sail_args(FRAGILITY_COMMAND);
    match run::run(&ctx.config.sail, &sail_args, &project, cancel) {
        Err(e) => ToolResult::error(
            format!("Cannot run {}: {e}", ctx.config.sail.display()),
            json!({"available": true, "error": e.to_string()}),
        ),
        Ok(o) => {
            let parsed: Option<Value> = serde_json::from_slice(&o.stdout).ok();
            let ok = parsed.is_some() || o.exit_code == Some(0);
            let s = json!({
                "available": true,
                "project_dir": project,
                "sail_args": sail_args,
                "exit_code": o.exit_code,
                "result": parsed.clone().unwrap_or_else(|| json!(String::from_utf8_lossy(&o.stdout))),
                "stderr_tail": o.stderr_tail,
            });
            let text = format!("sail fragility exited {:?}.", o.exit_code);
            if ok {
                ToolResult::ok(text, s)
            } else {
                ToolResult::error(text, s)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_api::output::{PortfolioAttempt, PortfolioReport, SolverConfiguration, VerdictName};

    #[test]
    fn lookup_returns_completed_evidence_while_a_project_run_is_locked() {
        let root = std::env::temp_dir().join(format!("sail-mcp-lookup-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let project = root.canonicalize().unwrap_or(root.clone());
        let context = Arc::new(Context::new(Config {
            sail: root.join("missing-sail"),
            store_dir: None,
            cwd: project.clone(),
        }));
        let report = store::tests::report(vec![store::tests::function(
            "demo::proved",
            VerdictName::Verified,
        )]);
        context.store.record(
            &project,
            RunRecord {
                run: 0,
                finished_unix_ms: 0,
                finished_at: store::rfc3339(0),
                sail_args: vec![],
                exit_code: Some(0),
                duration_ms: 0,
                scoped: false,
                sources: store::snapshot(&project),
                sources_changed_during_run: false,
                report,
            },
        );
        let lock = context.lock(&project);
        let guard = lock.lock();
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker_context = Arc::clone(&context);
        let worker = std::thread::spawn(move || {
            let result = cached_result(&worker_context, &Map::new());
            let _ = sender.send(result);
        });
        let result = receiver.recv_timeout(std::time::Duration::from_secs(1));
        drop(guard);
        assert!(worker.join().is_ok());
        assert!(result.is_ok(), "lookup must not wait for the run lock");
        assert!(result.is_ok_and(|result| !result.is_error));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persisted_portfolio_lookups_keep_mode_and_configuration() {
        let mut f = store::tests::function("demo::proved", VerdictName::Verified);
        f.provenance.mode = "accept-first-proof".to_owned();
        f.provenance.configuration = Some(SolverConfiguration { seed: 4 });
        let mut report = store::tests::report(vec![f]);
        report.solver = Some(sail_api::output::SolverInfo {
            name: "z3".to_owned(),
            version: Some("4.16.0".to_owned()),
            path: None,
            sha256: Some("ab".repeat(32)),
        });
        let solver = serde_json::to_value(&report.solver).unwrap_or_default();
        report.mode = "accept-first-proof".to_owned();
        report.crates[0].verus.verified = Some(1);
        report.crates[0].verus.errors = Some(0);
        report.portfolio = Some(PortfolioReport {
            seeds: vec![0, 4],
            winner_seed: Some(4),
            attempts: vec![
                PortfolioAttempt {
                    configuration: SolverConfiguration { seed: 0 },
                    verified: false,
                    exit_code: Some(1),
                    wall_ms: 1.0,
                    error: None,
                    report: None,
                    stderr: "baseline failed".to_owned(),
                },
                PortfolioAttempt {
                    configuration: SolverConfiguration { seed: 4 },
                    verified: true,
                    exit_code: Some(0),
                    wall_ms: 1.0,
                    error: None,
                    report: Some(Box::new(report.clone())),
                    stderr: String::new(),
                },
            ],
        });
        let mut record = ProjectRecord::default();
        record.record(RunRecord {
            run: 0,
            finished_unix_ms: 0,
            finished_at: store::rfc3339(0),
            sail_args: vec![
                "verify".to_owned(),
                "--portfolio=accept-first-proof".to_owned(),
            ],
            exit_code: Some(0),
            duration_ms: 1,
            scoped: false,
            sources: store::Snapshot::new(),
            sources_changed_during_run: false,
            report,
        });
        let encoded = serde_json::to_vec(&record).unwrap_or_default();
        let Ok(record) = serde_json::from_slice::<ProjectRecord>(&encoded) else {
            unreachable!("record did not round trip")
        };
        let Ok(found) = record.function("proved") else {
            unreachable!("recorded function missing")
        };
        let args = LookupArgs::default();
        let now = store::Snapshot::new();
        let project = Path::new("demo");
        for result in [
            one_result(&args, found, project, &now),
            models(&args, found, project, &now),
        ] {
            assert_eq!(result.structured["mode"], "accept-first-proof");
            assert_eq!(result.structured["configuration"]["seed"], 4);
            assert_eq!(result.structured["freshness"]["winner_seed"], 4);
            assert_eq!(result.structured["freshness"]["solver"], solver);
            assert!(result.text.contains("mode accept-first-proof, seed 4"));
        }
        let all = all_results(&record, project, &now);
        assert_eq!(all.structured["functions"][0]["mode"], "accept-first-proof");
        assert_eq!(all.structured["functions"][0]["configuration"]["seed"], 4);
        assert!(all.text.contains("mode accept-first-proof, seed 4"));
        assert!(summary::report(&found.run.report).contains("complete proof from seed 4"));
    }
}
