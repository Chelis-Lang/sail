//! The M7 repair acceptance workload (ADR 0027, `docs/bench/m7-repair-acceptance.md`).
//!
//! The frozen catalogue `corpus/repair-edits.json` lists edit-and-repair
//! cycles in pinned real projects. Each measured round runs every cycle once
//! per arm, with the arm order alternating between rounds:
//! - **M6** (the frozen baseline binary): a focused `verify --incremental`,
//!   then an independent full `verify --incremental`;
//! - **M7**: one `repair-check --engine interactive` batch.
//!
//! The workload *records every sample, then judges*. A failed, refused,
//! timed-out or protocol-violating invocation is kept as a sample with its
//! raw facts, and the run continues. [`judge`] applies the acceptance gate
//! at report time; nothing is dropped and no failing sample counts as a
//! pass.
//!
//! - [`run`]: preparation and the measured run (Unix only);
//! - [`judge`]: the per-catalogue gate;
//! - [`render`]: the `docs/benchmarks.md` section.

pub mod judge;
#[cfg(unix)]
pub mod process;
pub mod render;
#[cfg(unix)]
pub mod run;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::daemon::CacheRun;

/// `workload` value of a repair result file.
pub const WORKLOAD: &str = "repair";

/// SHA-256 of the frozen `corpus/repair-edits.json`.
pub const FROZEN_SHA256: &str = "6ffc129735220bf53511ac6bba818c5c3c7b61bca92d59c17292a807979b18d6";

/// Largest accepted retained-preparation memory or disk budget (512 MiB).
pub const PREPARATION_BUDGET_BYTES: u64 = 512 * 1024 * 1024;

/// Lowercase hex of `bytes`.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// SHA-256 of `bytes`, as lowercase hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&sail_key::sha256(bytes))
}

/// The frozen repair catalogue.
#[derive(Debug, Clone, Deserialize)]
pub struct Catalogue {
    /// Format version (1).
    pub schema_version: u32,
    /// The M6 comparison commit.
    pub baseline_commit: String,
    /// The pinned Verus source commit.
    pub verus_commit: String,
    /// Rounds required for acceptance.
    pub minimum_rounds: usize,
    /// Required ratio of median whole-loop totals, M6 over M7.
    pub minimum_median_speedup: f64,
    /// The catalogues (one per project).
    pub projects: Vec<Project>,
}

/// One project's catalogue.
#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    /// Corpus project name.
    pub id: String,
    /// Pinned project commit.
    pub commit: String,
    /// Cargo package.
    pub package: String,
    /// Cargo target name.
    pub target: String,
    /// Cargo target kind.
    pub target_kind: String,
    /// Crate root or workspace member.
    pub root: String,
    /// Whether the workload wraps the project in a generated Cargo package.
    pub cargo_wrapper: bool,
    /// The cycles, in catalogue order.
    pub cycles: Vec<Cycle>,
}

/// One edit-and-check cycle.
#[derive(Debug, Clone, Deserialize)]
pub struct Cycle {
    /// Unique id within the project.
    pub id: String,
    /// `body`, `spec`, `refactor`, `failure` or `repair`.
    pub kind: String,
    /// Crate-qualified function the repair focuses on.
    pub function: String,
    /// Edited file, relative to the project root.
    pub file: String,
    /// `verified` or `errors`.
    pub expected: String,
    /// `original`, or the id of the cycle whose text this edit starts from.
    pub from: String,
    /// Text that occurs exactly once.
    pub find: String,
    /// Its replacement.
    pub replace: String,
}

impl Catalogue {
    /// Reads `path`, refusing anything but the frozen bytes.
    ///
    /// # Errors
    ///
    /// A message if it cannot be read, differs from the frozen catalogue or
    /// does not parse.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let digest = sha256_hex(&bytes);
        if digest != FROZEN_SHA256 {
            return Err(format!(
                "error[SAIL7001]: {} is not the frozen repair catalogue (sha256 {digest})",
                path.display()
            ));
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// The two compared arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arm {
    /// The frozen M6 baseline: focused, then full verification.
    M6,
    /// One M7 interactive repair batch.
    M7,
}

impl Arm {
    /// Both arms, in round-zero order.
    pub const BOTH: [Self; 2] = [Self::M6, Self::M7];

    /// `m6` or `m7`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::M6 => "m6",
            Self::M7 => "m7",
        }
    }

    /// The arm order for `round`: M6 first in even rounds, M7 first in odd.
    #[must_use]
    pub fn order(round: usize) -> [Self; 2] {
        if round.is_multiple_of(2) {
            [Self::M6, Self::M7]
        } else {
            [Self::M7, Self::M6]
        }
    }
}

/// The binary one arm ran.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmBinary {
    /// Source commit the binary was built from, as supplied by the operator.
    pub commit: String,
    /// SHA-256 of the `sail` binary.
    pub sail_sha256: String,
    /// SHA-256 of the `sail-z3` shim next to it, if present.
    pub shim_sha256: Option<String>,
}

/// Longest free-text field kept in the compact result; the raw file keeps
/// the full text.
pub const COMPACT_TEXT: usize = 240;

#[allow(
    clippy::ref_option,
    reason = "serde's serialize_with passes a reference to the field"
)]
fn truncated<S: serde::Serializer>(v: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(t) if t.chars().count() > COMPACT_TEXT => {
            let cut: String = t.chars().take(COMPACT_TEXT).collect();
            s.serialize_some(&format!("{cut}…"))
        }
        Some(t) => s.serialize_some(t),
        None => s.serialize_none(),
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_false(b: &bool) -> bool {
    !*b
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// One process invocation inside a sample.
///
/// Arguments and full error text are kept in memory and in the raw file;
/// the compact result keeps only what the gate and report read.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Phase {
    /// `focus`, `full`, `repair`, `verify` or `engine-install`.
    pub name: String,
    /// Arguments after the binary (raw file only).
    #[serde(skip)]
    pub args: Vec<String>,
    /// Exit code (`None` if killed by a signal or never started).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Wall time from spawn to exit, in milliseconds.
    pub wall_ms: f64,
    /// Whether the harness deadline expired and the process group was stopped.
    #[serde(skip_serializing_if = "is_false")]
    pub timed_out: bool,
    /// Infrastructure failure: the process could not be started, waited for
    /// or its output could not be parsed (truncated in the compact result).
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "truncated")]
    pub error: Option<String>,
    /// GNU `time` peak RSS of the foreground command's own process tree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_max_rss_bytes: Option<u64>,
    /// Durable repair job id reported by the foreground command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    /// Proof-journal pages read for first-actionable timing.
    #[serde(skip_serializing_if = "is_zero")]
    pub proof_pages: u64,
    /// First proof-journal read error, if any; timing is then unavailable.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "truncated")]
    pub observation_error: Option<String>,
}

/// Source-reported preparation counters summed over the reported drivers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preparation {
    /// Reported driver records.
    pub drivers: u64,
    /// Whether any driver had preparation reuse enabled.
    pub enabled: bool,
    /// In-batch memory hits.
    pub batch_hits: u64,
    /// Validated persistent hits.
    pub persistent_hits: u64,
    /// Misses, including rejected admission.
    pub misses: u64,
    /// Largest per-driver retained peak.
    pub peak_retained_bytes: u64,
    /// Every record is version 1, within [`PREPARATION_BUDGET_BYTES`] and
    /// its peak within its memory budget.
    pub within_budget: bool,
}

/// Facts read from a sample's final report. No judgement is applied here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Facts {
    /// Full-scope currentness (`current`, `historical`, `unknown`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_currentness: Option<String>,
    /// Focused-scope currentness, where reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_currentness: Option<String>,
    /// M7: the repair document's `accepted`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted: Option<bool>,
    /// M7: `batch.batched`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batched: Option<bool>,
    /// M7: a validated engine manifest digest was reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_manifest: Option<bool>,
    /// M7: focused evidence was produced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_present: Option<bool>,
    /// M7: focus selection error (truncated in the compact result).
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "truncated")]
    pub selection_error: Option<String>,
    /// Verifier driver processes started (M7 `batch.compiler_invocations`;
    /// M6 the sum of `execution.verifier_invocations`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_invocations: Option<u64>,
    /// Canonical full outcome: exit code, sorted function verdicts and
    /// tiers, summary and ordered diagnostics (raw file only).
    #[serde(skip)]
    pub outcome: Option<Value>,
    /// SHA-256 of the canonical outcome's JSON; arms are compared by it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome_sha256: Option<String>,
    /// Why no canonical outcome could be read.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "truncated")]
    pub outcome_error: Option<String>,
    /// Full exit code was zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

/// One recorded sample: one arm of one cycle in one round, or a warm-up.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sample {
    /// Catalogue project id.
    pub project: String,
    /// Cycle id (for warm-ups, the function or `project`).
    pub cycle: String,
    /// Cycle kind (for warm-ups, the warm-up kind).
    pub kind: String,
    /// Focused function, if any.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub function: String,
    /// Catalogue expectation (`verified` or `errors`; empty for warm-ups).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub expected: String,
    /// Arm.
    pub arm: Option<Arm>,
    /// Measured round; `None` for a warm-up.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<usize>,
    /// Position within its round's pair (0 runs first).
    pub position: usize,
    /// SHA-256 of the edited file as measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_sha256: Option<String>,
    /// Invocations, in order.
    pub phases: Vec<Phase>,
    /// Sum of the phases' wall times.
    pub wall_ms: f64,
    /// Time to first actionable evidence: M6 the focused result, M7 the
    /// first focused proof-journal diagnostic, function, obligation or
    /// phase-finished event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_actionable_ms: Option<f64>,
    /// Report facts.
    pub facts: Facts,
    /// Cache daemon counters over the sample (`phase` is `sample`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheRun>,
    /// Preparation counters (M7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preparation: Option<Preparation>,
    /// Peak sampled RSS of the owned daemon's and command's process trees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_rss_bytes: Option<u64>,
    /// Raw evidence directory, relative to the run's output directory.
    pub raw_dir: String,
}

/// A compact repair workload result file in `bench/results/`: what the
/// gate and report need. Full detail is in the raw file it names.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RepairFile {
    /// Format version (2: compact samples with a raw sidecar).
    pub schema_version: u32,
    /// Always [`WORKLOAD`].
    pub workload: String,
    /// Operator label of the execution environment (native or emulated).
    pub environment: String,
    /// SHA-256 of the catalogue used.
    pub catalogue_sha256: String,
    /// Catalogue minimum rounds.
    pub minimum_rounds: usize,
    /// Catalogue minimum median speedup.
    pub minimum_median_speedup: f64,
    /// Requested measured rounds.
    pub rounds: usize,
    /// Every catalogue project and its cycle ids, in catalogue order. The
    /// gate judges against these, so a subset run is incomplete.
    pub catalogue: Vec<CatalogueEntry>,
    /// Projects run (all catalogue projects when complete).
    pub projects: Vec<String>,
    /// Cycle-id filter, if a subset was run.
    pub cycle_filter: Option<String>,
    /// Binaries per arm.
    pub arms: BTreeMap<Arm, ArmBinary>,
    /// Pinned Verus release.
    pub verus_release: String,
    /// Pinned Z3 version.
    pub z3_version: String,
    /// Run start, Unix seconds.
    pub started_unix: u64,
    /// Machine description.
    pub machine: BTreeMap<String, String>,
    /// File name of the raw sidecar (kept outside the repository).
    pub raw_file: Option<String>,
    /// SHA-256 of the raw sidecar as last written.
    pub raw_sha256: Option<String>,
    /// Warm-ups: recorded, never gated.
    pub warmups: Vec<Sample>,
    /// Measured samples.
    pub samples: Vec<Sample>,
    /// Harness events (daemon restarts, resumptions).
    pub notes: Vec<String>,
}

/// `workload` value of a raw sidecar file.
pub const RAW_WORKLOAD: &str = "repair-raw";

/// What the compact result omits for one sample.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RawRecord {
    /// `warmup` or `sample`.
    pub list: String,
    /// Index in that list of the compact result.
    pub index: usize,
    /// Project, cycle, arm and round, for reading the file on its own.
    pub key: String,
    /// Each phase's arguments.
    pub args: Vec<Vec<String>>,
    /// Each phase's full error text.
    pub errors: Vec<Option<String>>,
    /// Full selection, observation and outcome errors.
    pub notes: Vec<String>,
    /// The canonical full outcome whose digest the compact result holds.
    pub outcome: Option<Value>,
}

impl RawRecord {
    /// The record for `s`, at `index` of `list`.
    #[must_use]
    pub fn of(list: &str, index: usize, s: &Sample) -> Self {
        let notes = [
            s.facts.selection_error.as_ref(),
            s.facts.outcome_error.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(s.phases.iter().filter_map(|p| p.observation_error.as_ref()))
        .cloned()
        .collect();
        Self {
            list: list.to_owned(),
            index,
            key: format!(
                "{}/{}/{}/{}",
                s.project,
                s.cycle,
                s.arm.map_or("-", Arm::name),
                s.round
                    .map_or_else(|| "warm-up".to_owned(), |r| r.to_string())
            ),
            args: s.phases.iter().map(|p| p.args.clone()).collect(),
            errors: s.phases.iter().map(|p| p.error.clone()).collect(),
            notes,
            outcome: s.facts.outcome.clone(),
        }
    }
}

/// The raw sidecar of a repair result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RawFile {
    /// Format version (1).
    pub schema_version: u32,
    /// Always [`RAW_WORKLOAD`].
    pub workload: String,
    /// The compact result's file name.
    pub result: String,
    /// One record per warm-up and sample, in recording order.
    pub records: Vec<RawRecord>,
}

/// One catalogue project's cycle ids.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogueEntry {
    /// Project id.
    pub project: String,
    /// Cycle ids, in catalogue order.
    pub cycles: Vec<String>,
}

impl Catalogue {
    /// The cycle ids of every project.
    #[must_use]
    pub fn entries(&self) -> Vec<CatalogueEntry> {
        self.projects
            .iter()
            .map(|p| CatalogueEntry {
                project: p.id.clone(),
                cycles: p.cycles.iter().map(|c| c.id.clone()).collect(),
            })
            .collect()
    }
}

/// Reads a `currentness` string from a project result.
fn currentness(project: &Value) -> Option<String> {
    project
        .get("currentness")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The canonical full outcome of a `sail-project-result` document: the
/// value compared between arms.
///
/// # Errors
///
/// Why the document carries no comparable outcome (an infrastructure
/// error, no exit code, or not exactly one requested target).
pub fn canonical_outcome(project: &Value) -> Result<Value, String> {
    if project.get("error").is_some_and(|e| !e.is_null()) {
        return Err(format!(
            "project infrastructure error: {}",
            project
                .get("error")
                .map_or_else(String::new, Value::to_string)
        ));
    }
    let exit = project.get("exit_code").and_then(Value::as_i64);
    let Some(exit) = exit else {
        return Err("no exit code".to_owned());
    };
    let targets: Vec<&Value> = project
        .get("targets")
        .and_then(Value::as_array)
        .map(|t| {
            t.iter()
                .filter(|t| t.get("requested").and_then(Value::as_bool) == Some(true))
                .collect()
        })
        .unwrap_or_default();
    let [target] = targets.as_slice() else {
        return Err(format!(
            "{} requested targets, expected exactly one",
            targets.len()
        ));
    };
    let report = target.get("report").cloned().unwrap_or(Value::Null);
    if report.get("error").is_some_and(|e| !e.is_null()) {
        return Err("selected target has an infrastructure error".to_owned());
    }
    let mut functions: Vec<Value> = report
        .get("crates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|c| {
            c.get("functions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .map(|f| {
            serde_json::json!([
                f.get("function").cloned().unwrap_or(Value::Null),
                f.get("verdict").cloned().unwrap_or(Value::Null),
                f.get("tier").cloned().unwrap_or(Value::Null),
            ])
        })
        .collect();
    functions.sort_by_key(Value::to_string);
    Ok(serde_json::json!({
        "exit_code": exit,
        "functions": functions,
        "summary": report.get("summary").cloned().unwrap_or(Value::Null),
        "diagnostics": report.get("diagnostics").cloned().unwrap_or(Value::Null),
    }))
}

/// Facts from an M6 full `verify --incremental --output json` document,
/// with the focused document when it parsed.
#[must_use]
pub fn m6_facts(focus: Option<&Value>, full: Option<&Value>) -> Facts {
    let invocations = |v: Option<&Value>| {
        v.and_then(|d| d.pointer("/execution/verifier_invocations"))
            .and_then(Value::as_u64)
    };
    let mut facts = Facts {
        focus_currentness: focus.and_then(currentness),
        full_currentness: full.and_then(currentness),
        compiler_invocations: match (invocations(focus), invocations(full)) {
            (Some(a), Some(b)) => Some(a + b),
            (a, b) => a.or(b),
        },
        ..Facts::default()
    };
    set_outcome(&mut facts, full);
    facts
}

fn set_outcome(facts: &mut Facts, full: Option<&Value>) {
    match full.map(canonical_outcome) {
        Some(Ok(outcome)) => {
            facts.verified = outcome
                .get("exit_code")
                .and_then(Value::as_i64)
                .map(|c| c == 0);
            facts.outcome_sha256 = serde_json::to_vec(&outcome).ok().map(|b| sha256_hex(&b));
            facts.outcome = Some(outcome);
        }
        Some(Err(e)) => facts.outcome_error = Some(e),
        None => facts.outcome_error = Some("no full report".to_owned()),
    }
}

/// Facts and preparation counters from an M7 repair result document.
#[must_use]
pub fn m7_facts(doc: Option<&Value>) -> (Facts, Option<Preparation>) {
    let batch = doc.and_then(|d| d.get("batch"));
    let full = batch.and_then(|b| b.get("full"));
    let focus = batch.and_then(|b| b.get("focus")).filter(|f| !f.is_null());
    let mut facts = Facts {
        full_currentness: full.and_then(currentness),
        focus_currentness: focus.and_then(currentness),
        accepted: doc.and_then(|d| d.get("accepted")).and_then(Value::as_bool),
        batched: batch
            .and_then(|b| b.get("batched"))
            .and_then(Value::as_bool),
        engine_manifest: batch.map(|b| {
            b.get("manifest_sha256")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
        }),
        focus_present: batch.map(|_| focus.is_some()),
        selection_error: batch
            .and_then(|b| b.get("selection_error"))
            .filter(|e| !e.is_null())
            .map(Value::to_string),
        compiler_invocations: batch
            .and_then(|b| b.get("compiler_invocations"))
            .and_then(Value::as_u64),
        ..Facts::default()
    };
    set_outcome(&mut facts, full);
    let preparation = batch
        .and_then(|b| b.get("preparation"))
        .and_then(Value::as_array)
        .map(|records| preparation(records));
    (facts, preparation)
}

/// Sums source-reported preparation records.
#[must_use]
pub fn preparation(records: &[Value]) -> Preparation {
    let n = |r: &Value, k: &str| r.get(k).and_then(Value::as_u64);
    let mut p = Preparation {
        drivers: records.len() as u64,
        within_budget: !records.is_empty(),
        ..Preparation::default()
    };
    for r in records {
        p.enabled |= r.get("enabled").and_then(Value::as_bool) == Some(true);
        p.batch_hits += n(r, "batch_hits").unwrap_or(0);
        p.persistent_hits += n(r, "persistent_hits").unwrap_or(0);
        p.misses += n(r, "misses").unwrap_or(0);
        let peak = n(r, "peak_retained_bytes");
        p.peak_retained_bytes = p.peak_retained_bytes.max(peak.unwrap_or(0));
        let memory = n(r, "memory_budget_bytes");
        let disk = n(r, "disk_budget_bytes");
        let ok = n(r, "version") == Some(1)
            && memory.is_some_and(|m| 0 < m && m <= PREPARATION_BUDGET_BYTES)
            && disk.is_some_and(|d| 0 < d && d <= PREPARATION_BUDGET_BYTES)
            && peak.zip(memory).is_some_and(|(p, m)| p <= m);
        p.within_budget &= ok;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project(exit: i64, currentness: &str) -> Value {
        json!({
            "kind": "sail-project-result", "currentness": currentness, "exit_code": exit,
            "error": null, "execution": {"verifier_invocations": 1},
            "targets": [
                {"requested": false, "report": {"crates": []}},
                {"requested": true, "report": {
                    "error": null, "summary": {"verified": 2},
                    "diagnostics": [{"message": "m"}],
                    "crates": [{"functions": [
                        {"function": "b", "verdict": "verified", "tier": "solver"},
                        {"function": "a", "verdict": "failed", "tier": "solver"}
                    ]}]
                }}
            ]
        })
    }

    #[test]
    fn catalogue_is_frozen() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/repair-edits.json");
        let c = Catalogue::load(&path);
        let Ok(c) = c else { unreachable!("{c:?}") };
        assert_eq!(c.projects.len(), 2);
        assert!(c.projects.iter().all(|p| p.cycles.len() == 5));
        assert_eq!(c.minimum_rounds, 3);
    }

    #[test]
    fn arm_order_alternates() {
        assert_eq!(Arm::order(0), [Arm::M6, Arm::M7]);
        assert_eq!(Arm::order(1), [Arm::M7, Arm::M6]);
        assert_eq!(Arm::order(2), [Arm::M6, Arm::M7]);
    }

    #[test]
    fn outcome_is_canonical_and_requires_one_target() {
        let a = canonical_outcome(&project(1, "current"));
        let Ok(a) = a else { unreachable!("{a:?}") };
        assert_eq!(a["functions"][0][0], "a");
        assert_eq!(a["exit_code"], 1);
        let mut two = project(0, "current");
        two["targets"][0]["requested"] = json!(true);
        assert!(canonical_outcome(&two).is_err());
        let mut broken = project(0, "current");
        broken["error"] = json!({"code": "SAIL0001"});
        assert!(canonical_outcome(&broken).is_err());
    }

    #[test]
    fn m6_and_m7_facts() {
        let full = project(0, "current");
        let focus = project(0, "historical");
        let f = m6_facts(Some(&focus), Some(&full));
        assert_eq!(f.full_currentness.as_deref(), Some("current"));
        assert_eq!(f.focus_currentness.as_deref(), Some("historical"));
        assert_eq!(f.compiler_invocations, Some(2));
        assert_eq!(f.verified, Some(true));

        let doc = json!({"version": 1, "accepted": true, "batch": {
            "focus": full, "full": full, "manifest_sha256": "ab",
            "compiler_invocations": 2, "batched": true,
            "preparation": [{"version": 1, "enabled": true, "batch_hits": 3,
                "persistent_hits": 1, "misses": 2, "peak_retained_bytes": 10,
                "memory_budget_bytes": 100, "disk_budget_bytes": 100}]
        }});
        let (f, p) = m7_facts(Some(&doc));
        assert_eq!(f.compiler_invocations, Some(2));
        assert_eq!(f.accepted, Some(true));
        assert_eq!(f.engine_manifest, Some(true));
        assert_eq!(f.focus_present, Some(true));
        let Some(p) = p else { unreachable!() };
        assert_eq!((p.batch_hits, p.persistent_hits, p.misses), (3, 1, 2));
        assert!(p.within_budget && p.enabled);

        let (f, p) = m7_facts(None);
        assert_eq!(f.outcome_error.as_deref(), Some("no full report"));
        assert!(p.is_none());
    }

    #[test]
    fn compact_samples_keep_judged_facts_and_drop_raw_detail() {
        let full = project(0, "current");
        let (facts, _) = m7_facts(Some(&json!({"accepted": true, "batch": {
            "focus": full, "full": full, "manifest_sha256": "ab",
            "compiler_invocations": 1, "batched": true,
            "selection_error": "x".repeat(1000)
        }})));
        let s = Sample {
            project: "p".to_owned(),
            cycle: "c".to_owned(),
            arm: Some(Arm::M7),
            round: Some(0),
            phases: vec![Phase {
                name: "repair".to_owned(),
                args: vec!["repair-check".to_owned()],
                error: Some("e".repeat(1000)),
                exit_code: Some(0),
                ..Phase::default()
            }],
            facts,
            ..Sample::default()
        };
        let text = serde_json::to_string(&s).unwrap_or_default();
        assert!(
            !text.contains("repair-check") && !text.contains("diagnostics"),
            "{text}"
        );
        assert!(text.len() < 1500, "{}", text.len());
        let back: Option<Sample> = serde_json::from_str(&text).ok();
        let Some(back) = back else {
            unreachable!("{text}")
        };
        assert_eq!(back.facts.outcome_sha256, s.facts.outcome_sha256);
        assert!(back.facts.outcome_sha256.is_some() && back.facts.outcome.is_none());
        let codes = |s: &Sample| -> Vec<&'static str> {
            super::judge::violations(s).iter().map(|v| v.code).collect()
        };
        assert_eq!(codes(&back), codes(&s));
        let raw = RawRecord::of("sample", 0, &s);
        assert_eq!(raw.args, [["repair-check"]]);
        assert_eq!(
            raw.errors.first().cloned().flatten().map(|e| e.len()),
            Some(1000)
        );
        assert!(raw.outcome.is_some());
        assert_eq!(raw.key, "p/c/m7/0");
    }

    #[test]
    fn preparation_budget() {
        let over = json!({"version": 1, "enabled": true, "batch_hits": 0, "persistent_hits": 0,
            "misses": 0, "peak_retained_bytes": 1, "memory_budget_bytes": PREPARATION_BUDGET_BYTES + 1,
            "disk_budget_bytes": 1});
        assert!(!preparation(&[over]).within_budget);
        assert!(!preparation(&[]).within_budget);
    }
}
