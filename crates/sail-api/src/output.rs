//! The `sail verify --output json` document, schema version 1.
//!
//! The schema is documented in `docs/output-schema.md`; the JSON Schema is
//! [`JSON_SCHEMA`] (`crates/sail-api/schema/verify-output-v1.json`). The
//! types ignore unknown fields when read back, as consumers must; the JSON
//! Schema is strict, and tests check that it requires every field the types
//! write and allows no other.
//!
//! Compatibility: within schema version 1, fields are only ever added
//! (consumers must ignore fields they do not know); a field is never
//! removed, renamed or given a different meaning. Any such change bumps
//! [`SCHEMA_VERSION`].

use serde::{Deserialize, Serialize};

use sail_ledger::{
    CheckRecord, FunctionRecord, ObligationRecord, Outcome, Source, SourceSummary, Tier, Verdict,
};

/// The schema version this module writes.
pub const SCHEMA_VERSION: u32 = 1;

/// The JSON Schema (draft 2020-12) for schema version 1.
pub const JSON_SCHEMA: &str = include_str!("../schema/verify-output-v1.json");

/// The whole document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Complete-run seed portfolio evidence, only when explicitly requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portfolio: Option<PortfolioReport>,
    /// Always [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The `sail` version that wrote it.
    pub sail_version: String,
    /// The verification mode: `default`, or a named opt-in mode.
    pub mode: String,
    /// The arguments passed through to `cargo verus verify`.
    pub command: Vec<String>,
    /// `cargo verus verify`'s exit code (`None` if it was killed by a signal
    /// or never ran).
    pub exit_code: Option<i32>,
    /// The solver every answer came from.
    pub solver: Option<SolverInfo>,
    /// Cache use.
    pub cache: CacheInfo,
    /// How completely solver queries were attributed to functions.
    pub attribution: AttributionInfo,
    /// One entry per crate Verus verified, in the order Verus finished them.
    pub crates: Vec<CrateReport>,
    /// Verus's diagnostics, in order, as data and verbatim.
    pub diagnostics: Vec<Diagnostic>,
    /// Counts over every function in every crate.
    pub summary: Summary,
    /// Set when `sail verify` itself could not run Verus.
    pub error: Option<ErrorInfo>,
    /// Source-engine provenance, present only for an explicitly selected
    /// engine (ADR 0027). Stock reports omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineProvenance>,
}

/// How an explicitly selected source engine established a report or function
/// (charter rule 7, ADR 0027). Absent for stock verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineProvenance {
    /// The engine name (`interactive`).
    pub name: String,
    /// SHA-256 of the validated engine build manifest, as lowercase hex.
    pub manifest_sha256: String,
    /// Whether the engine or the stock fallback produced the evidence.
    pub established_by: EngineEstablishment,
}

/// Which verifier produced the evidence of an engine-requested report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineEstablishment {
    /// The engine's complete, sealed full-phase output.
    Engine,
    /// The engine faulted; the exact stock Verus invocation's output is
    /// reported instead, and it never yields a proof receipt or accepted repair.
    StockFallback,
}

/// A solver configuration establishing a complete portfolio attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolverConfiguration {
    /// Both Z3 random seeds (zero represents the unchanged default baseline).
    pub seed: u32,
}

/// One independent Verus attempt in a sequential portfolio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioAttempt {
    /// The configuration this attempt used.
    pub configuration: SolverConfiguration,
    /// Whether complete verification was established.
    pub verified: bool,
    /// Its process exit code; absent for cancellation or startup failure.
    pub exit_code: Option<i32>,
    /// Wall time including compilation and verification.
    pub wall_ms: f64,
    /// Infrastructure failure, never a solver answer.
    pub error: Option<String>,
    /// The attempt's complete verification report, when available.
    pub report: Option<Box<Report>>,
    /// Its stderr, retained verbatim.
    pub stderr: String,
}

/// Evidence for the explicitly selected complete-run portfolio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioReport {
    /// Ordered configurations, including the unchanged baseline.
    pub seeds: Vec<u32>,
    /// The first configuration establishing a complete verification.
    pub winner_seed: Option<u32>,
    /// Every completed or failed attempt, in order.
    pub attempts: Vec<PortfolioAttempt>,
}

/// The solver's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolverInfo {
    /// Solver family (`z3`).
    pub name: String,
    /// The version it reported.
    pub version: Option<String>,
    /// Its path.
    pub path: Option<String>,
    /// SHA-256 of the binary, as lowercase hex.
    pub sha256: Option<String>,
}

/// Cache use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheInfo {
    /// Whether solver sessions could be served from the cache daemon.
    pub daemon: bool,
}

/// Attribution coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionStatus {
    /// Every solver process was attributed.
    Complete,
    /// Some were not; see `notes`.
    Partial,
    /// None were; see `notes`. Function verdicts are still reported.
    Unavailable,
}

/// How completely solver queries were attributed to functions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionInfo {
    /// Coverage.
    pub status: AttributionStatus,
    /// Why coverage is not complete, in plain words.
    pub notes: Vec<String>,
    /// Solver sessions that matched no Verus log.
    pub unmatched_sessions: u64,
}

/// One crate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrateReport {
    /// The crate name, if Verus reported any function for it.
    pub name: Option<String>,
    /// Verus's own crate-level counts.
    pub verus: VerusResults,
    /// Its functions, sorted by name.
    pub functions: Vec<FunctionReport>,
}

/// Verus's `verification-results` for a crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerusResults {
    /// `verified`.
    pub verified: Option<u64>,
    /// `errors`.
    pub errors: Option<u64>,
    /// `success` (only for whole-crate runs).
    pub success: Option<bool>,
    /// `encountered-vir-error`.
    pub encountered_vir_error: Option<bool>,
}

/// The reported verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerdictName {
    /// Confirmed and proved.
    Verified,
    /// Confirmed and failed.
    Failed,
    /// Confirmed, not proved, and not a settled failure.
    Unknown,
    /// Not yet established by the pinned solver; never verified.
    Provisional,
}

impl From<Verdict> for VerdictName {
    fn from(v: Verdict) -> Self {
        match v {
            Verdict::Verified => Self::Verified,
            Verdict::Failed => Self::Failed,
            Verdict::Unknown => Self::Unknown,
            Verdict::Provisional => Self::Provisional,
        }
    }
}

/// The tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TierName {
    /// Not yet established by the pinned solver.
    Provisional,
    /// Established by the pinned solver: fresh, or a cached pinned result.
    Confirmed,
}

impl From<Tier> for TierName {
    fn from(t: Tier) -> Self {
        match t {
            Tier::Provisional => Self::Provisional,
            Tier::Confirmed => Self::Confirmed,
        }
    }
}

/// One function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionReport {
    /// The function, as Verus names it.
    pub function: String,
    /// Its module, as Verus reports it (`""` for the crate root).
    pub module: String,
    /// Verus's mode for it (`exec`, `proof`, `spec`).
    pub fn_mode: Option<String>,
    /// The verdict. Never `verified` unless `tier` is `confirmed`.
    pub verdict: VerdictName,
    /// The tier.
    pub tier: TierName,
    /// Whether any answer finished near its limit or gave up from resource
    /// exhaustion (ADR 0009).
    pub unstable: bool,
    /// Verus's own `success` for it (incomplete for bit-vector queries).
    pub verus_success: Option<bool>,
    /// How its answers were established.
    pub provenance: ProvenanceReport,
    /// Time and effort, as Verus reports them.
    pub timing: Timing,
    /// Explicit replay measurement; `null` without complete attribution and replay.
    pub fragility: Option<Fragility>,
    /// Its obligations; `null` when attribution was unavailable for it.
    pub obligations: Option<Vec<ObligationReport>>,
}

/// Summary of answer sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceSummaryName {
    /// Every check solved fresh by the pinned solver.
    Fresh,
    /// Every check served from the cache.
    Cache,
    /// Some of each.
    Mixed,
    /// Not known for at least one check.
    Unrecorded,
}

impl From<SourceSummary> for SourceSummaryName {
    fn from(s: SourceSummary) -> Self {
        match s {
            SourceSummary::Fresh => Self::Fresh,
            SourceSummary::Cache => Self::Cache,
            SourceSummary::Mixed => Self::Mixed,
            SourceSummary::Unrecorded => Self::Unrecorded,
        }
    }
}

/// How a function's answers were established (charter rule 7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceReport {
    /// The proving configuration in a portfolio attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<SolverConfiguration>,
    /// The mode that produced them (`default`, or an opt-in mode's name).
    pub mode: String,
    /// Summary of `checks_*`.
    pub source: SourceSummaryName,
    /// Checks solved fresh by the pinned solver.
    pub checks_solved: u64,
    /// Checks served from the cache (pinned-solver results).
    pub checks_served: u64,
    /// Checks with no provenance record.
    pub checks_unrecorded: u64,
    /// Source-engine provenance; absent for stock verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineProvenance>,
}

/// Time and effort, as Verus reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timing {
    /// Solver time for the function's queries, in microseconds.
    pub smt_micros: Option<u64>,
    /// rlimit units.
    pub rlimit: Option<u64>,
}

/// Function fragility from complete replay configurations and primary obligations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fragility {
    /// Fraction of runs that did not prove it, from 0 to 1.
    pub score: f64,
    /// Replay configurations measured.
    pub runs: u32,
    /// Configurations proving every primary obligation together.
    pub proved: u32,
}

/// An obligation's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutcomeName {
    /// Proved.
    Proved,
    /// Settled failure.
    Failed,
    /// Gave up at or near the limit, or no answer recorded.
    Unknown,
}

impl From<Outcome> for OutcomeName {
    fn from(o: Outcome) -> Self {
        match o {
            Outcome::Proved => Self::Proved,
            Outcome::Failed => Self::Failed,
            Outcome::Unknown => Self::Unknown,
        }
    }
}

/// One obligation: one solver query op.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationReport {
    /// The stable identity: `<function>#<kind>/<ordinal>` or
    /// `<function>#<kind>/spinoff-<n>`.
    pub id: String,
    /// The op kind (`Function-Def`, `Function-Recommends`, …).
    pub kind: String,
    /// Position among the function's in-module ops of this kind (0 for
    /// spinoffs).
    pub ordinal: u32,
    /// Spinoff details, for ops Verus ran in their own process.
    pub spinoff: Option<SpinoffInfo>,
    /// Whether its failure is advisory (recommends, expand-errors, profile
    /// reruns) rather than an error that fails the function.
    pub advisory: bool,
    /// Verus's span for the op. Informational; not part of the identity.
    pub span: Option<SourceSpan>,
    /// Its outcome.
    pub outcome: OutcomeName,
    /// Whether any of its checks is unstable (ADR 0009).
    pub unstable: bool,
    /// Its `check-sat` commands, in order.
    pub checks: Vec<CheckReport>,
    /// The first model the solver gave for it, if any.
    pub counterexample: Option<Counterexample>,
}

/// A spun-off op.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpinoffInfo {
    /// Verus's spinoff counter for the op (1, 2, …).
    pub index: u32,
    /// Why it was spun off (`nonlinear`, `bitvector`, …).
    pub reason: Option<String>,
}

/// A span as Verus writes it in its logs: `file:line:col: line:col (#n)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    /// The text, verbatim.
    pub text: String,
    /// The file.
    pub file: Option<String>,
    /// Start line (1-based).
    pub line: Option<u64>,
    /// Start column (1-based).
    pub column: Option<u64>,
    /// End line.
    pub end_line: Option<u64>,
    /// End column.
    pub end_column: Option<u64>,
}

impl SourceSpan {
    /// Parses Verus's span text; unparsed parts are `None`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut span = Self {
            text: text.to_owned(),
            file: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        };
        let body = text.rsplit_once(" (#").map_or(text, |(b, _)| b);
        let Some((start, end)) = body.rsplit_once(": ") else {
            return span;
        };
        let mut s = start.rsplitn(3, ':');
        let (Some(col), Some(line), Some(file)) = (s.next(), s.next(), s.next()) else {
            return span;
        };
        let mut e = end.splitn(2, ':');
        span.file = Some(file.to_owned());
        span.line = line.parse().ok();
        span.column = col.parse().ok();
        span.end_line = e.next().and_then(|v| v.parse().ok());
        span.end_column = e.next().and_then(|v| v.parse().ok());
        span
    }
}

/// A `check-sat` answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnswerName {
    /// `sat`.
    Sat,
    /// `unsat`.
    Unsat,
    /// `unknown`.
    Unknown,
}

impl From<sail_ledger::Answer> for AnswerName {
    fn from(a: sail_ledger::Answer) -> Self {
        match a {
            sail_ledger::Answer::Sat => Self::Sat,
            sail_ledger::Answer::Unsat => Self::Unsat,
            sail_ledger::Answer::Unknown => Self::Unknown,
        }
    }
}

/// How one answer was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceName {
    /// A fresh pinned-solver run.
    Solved,
    /// A cache hit of a pinned-solver result.
    Cache,
}

impl From<Source> for SourceName {
    fn from(s: Source) -> Self {
        match s {
            Source::Solved => Self::Solved,
            Source::Cache => Self::Cache,
        }
    }
}

/// One `check-sat`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReport {
    /// Its answer.
    pub answer: Option<AnswerName>,
    /// The `:rlimit` in force (0 means unlimited).
    pub rlimit_limit: Option<u64>,
    /// rlimit units used.
    pub rlimit_used: Option<u64>,
    /// The `reason-unknown` reply, verbatim.
    pub reason_unknown: Option<String>,
    /// How it was established; `null` if not recorded.
    pub source: Option<SourceName>,
}

/// A counterexample model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counterexample {
    /// Index into `checks` of the check that produced it.
    pub check: u32,
    /// The `get-model` reply, verbatim.
    pub model: String,
    /// Its zero-arity definitions as data.
    pub assignments: Vec<Assignment>,
}

/// One constant in a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    /// The SMT name (Verus encodes parameter `x` as `x!`, versions of local
    /// `i` as `i@0`, `i@1`, …).
    pub name: String,
    /// The sort, verbatim.
    pub sort: String,
    /// The value, verbatim.
    pub value: String,
}

/// A source location in a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSpan {
    /// The file, as printed.
    pub file: String,
    /// 1-based line.
    pub line: u64,
    /// 1-based column.
    pub column: u64,
    /// Primary (`-->`) or secondary (`:::`).
    pub primary: bool,
}

/// One diagnostic from Verus's stderr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// `error`, `warning`, `note` or `help`.
    pub level: String,
    /// The bracketed code, if any.
    pub code: Option<String>,
    /// The header's message, verbatim.
    pub message: String,
    /// Its locations.
    pub spans: Vec<DiagnosticSpan>,
    /// The whole block, verbatim.
    pub rendered: String,
}

/// Counts over every function.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// Functions.
    pub functions: u64,
    /// Verified.
    pub verified: u64,
    /// Failed.
    pub failed: u64,
    /// Unknown.
    pub unknown: u64,
    /// Provisional.
    pub provisional: u64,
    /// Unstable (any verdict).
    pub unstable: u64,
}

/// A setup failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// The stable code (`SAILnnnn`).
    pub code: String,
    /// What went wrong.
    pub message: String,
}

fn check_report(c: &CheckRecord) -> CheckReport {
    CheckReport {
        answer: c.answer.map(AnswerName::from),
        rlimit_limit: c.effort.limit,
        rlimit_used: c.effort.used,
        reason_unknown: c.effort.reason.clone(),
        source: c.source.map(SourceName::from),
    }
}

impl From<&ObligationRecord> for ObligationReport {
    fn from(o: &ObligationRecord) -> Self {
        let counterexample = o.checks.iter().enumerate().find_map(|(i, c)| {
            let model = c.model.as_ref()?;
            Some(Counterexample {
                check: u32::try_from(i).unwrap_or(u32::MAX),
                model: model.text.clone(),
                assignments: model
                    .assignments
                    .iter()
                    .map(|a| Assignment {
                        name: a.name.clone(),
                        sort: a.sort.clone(),
                        value: a.value.clone(),
                    })
                    .collect(),
            })
        });
        Self {
            id: o.id.to_string(),
            kind: o.id.kind.0.clone(),
            ordinal: o.id.ordinal,
            spinoff: o.id.spinoff.map(|index| SpinoffInfo {
                index,
                reason: o.spinoff_reason.clone(),
            }),
            advisory: !o.id.kind.verdict_bearing(),
            span: o.span.as_deref().map(SourceSpan::parse),
            outcome: o.outcome().into(),
            unstable: o.unstable(),
            checks: o.checks.iter().map(check_report).collect(),
            counterexample,
        }
    }
}

impl From<&FunctionRecord> for FunctionReport {
    fn from(f: &FunctionRecord) -> Self {
        let sources = f.sources();
        Self {
            function: f.function.clone(),
            module: f.module.clone(),
            fn_mode: f.fn_mode.clone(),
            verdict: f.verdict().into(),
            tier: f.tier.into(),
            unstable: f.unstable(),
            verus_success: f.verus_success,
            provenance: ProvenanceReport {
                configuration: None,
                mode: f.mode.name().to_owned(),
                source: sources.summary().into(),
                checks_solved: sources.solved,
                checks_served: sources.served,
                checks_unrecorded: sources.unrecorded,
                engine: None,
            },
            timing: Timing {
                smt_micros: f.smt_micros,
                rlimit: f.rlimit,
            },
            fragility: None,
            obligations: f
                .obligations
                .as_ref()
                .map(|os| os.iter().map(ObligationReport::from).collect()),
        }
    }
}

impl Summary {
    /// Counts the functions in `crates`.
    #[must_use]
    pub fn of(crates: &[CrateReport]) -> Self {
        let mut s = Self::default();
        for f in crates.iter().flat_map(|c| &c.functions) {
            s.functions += 1;
            match f.verdict {
                VerdictName::Verified => s.verified += 1,
                VerdictName::Failed => s.failed += 1,
                VerdictName::Unknown => s.unknown += 1,
                VerdictName::Provisional => s.provisional += 1,
            }
            s.unstable += u64::from(f.unstable);
        }
        s
    }
}

impl CrateReport {
    /// How many of this crate's `unknown` functions Verus's own totals clear.
    ///
    /// When attribution is incomplete in a failing crate, functions Sail
    /// cannot confirm one by one are `unknown` (VI-0003, VI-0010). Verus
    /// counts every failing verification unit in `verus.errors`, bit-vector
    /// failures included (VI-0003), and each function reported `failed`
    /// accounts for at least one. So when `errors` equals the number of failed
    /// functions, no other function had an error, and the `unknown` functions
    /// passed by Verus's totals. Their verdicts stay `unknown`: this is a
    /// count from Verus's aggregate, never a per-function proof (charter
    /// rule 7). Returns 0 whenever the totals cannot clear them.
    #[must_use]
    pub fn unknown_passed_by_verus_totals(&self) -> u64 {
        let count = |verdict: VerdictName| {
            let n = self
                .functions
                .iter()
                .filter(|f| f.verdict == verdict)
                .count();
            u64::try_from(n).unwrap_or(u64::MAX)
        };
        let failed = count(VerdictName::Failed);
        let cleared = failed > 0
            && self.verus.errors == Some(failed)
            && self.verus.verified.is_some()
            && self.verus.encountered_vir_error != Some(true);
        if cleared {
            count(VerdictName::Unknown)
        } else {
            0
        }
    }
}

impl Report {
    /// Rules the JSON Schema cannot express. Returns every violation.
    #[must_use]
    pub fn invariant_violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.schema_version != SCHEMA_VERSION {
            out.push(format!("schema_version is {}", self.schema_version));
        }
        let mut ids = std::collections::BTreeSet::new();
        for f in self.crates.iter().flat_map(|c| &c.functions) {
            if f.verdict == VerdictName::Verified && f.tier != TierName::Confirmed {
                out.push(format!("{}: verified but not confirmed", f.function));
            }
            if (f.tier == TierName::Provisional) != (f.verdict == VerdictName::Provisional) {
                out.push(format!("{}: tier and verdict disagree", f.function));
            }
            let p = &f.provenance;
            let counted = (p.checks_solved, p.checks_served, p.checks_unrecorded);
            let expected = match p.source {
                SourceSummaryName::Fresh => counted.0 > 0 && counted.1 == 0 && counted.2 == 0,
                SourceSummaryName::Cache => counted.0 == 0 && counted.1 > 0 && counted.2 == 0,
                SourceSummaryName::Mixed => counted.0 > 0 && counted.1 > 0 && counted.2 == 0,
                SourceSummaryName::Unrecorded => counted.2 > 0 || counted == (0, 0, 0),
            };
            if !expected {
                out.push(format!(
                    "{}: provenance counts disagree with source",
                    f.function
                ));
            }
            for o in f.obligations.iter().flatten() {
                if !ids.insert(o.id.clone()) {
                    out.push(format!("duplicate obligation id {}", o.id));
                }
                if !o.id.starts_with(&format!("{}#", f.function)) {
                    out.push(format!("obligation {} is not {}'s", o.id, f.function));
                }
            }
        }
        if self.summary != Summary::of(&self.crates) {
            out.push("summary does not match the functions".to_owned());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_parse() {
        let s = SourceSpan::parse("src/lib.rs:46:1: 46:26 (#0)");
        assert_eq!(s.file.as_deref(), Some("src/lib.rs"));
        assert_eq!(
            (s.line, s.column, s.end_line, s.end_column),
            (Some(46), Some(1), Some(46), Some(26))
        );
        let s = SourceSpan::parse("garbage");
        assert_eq!((s.file, s.line), (None, None));
        assert_eq!(s.text, "garbage");
    }
    #[test]
    fn engine_provenance_is_additive_and_matches_the_schemas() {
        let stock: Report = serde_json::from_str(include_str!(
            "../../sail-cli/tests/fixtures/attr/report.golden.json"
        ))
        .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        // Stock documents serialize exactly as before: no engine fields.
        let stock_text = serde_json::to_string(&stock).unwrap_or_default();
        assert!(!stock_text.contains("\"engine\""));
        let engine = EngineProvenance {
            name: "interactive".to_owned(),
            manifest_sha256: "ab".repeat(32),
            established_by: EngineEstablishment::StockFallback,
        };
        let mut r = stock.clone();
        r.engine = Some(engine.clone());
        for f in r.crates.iter_mut().flat_map(|c| &mut c.functions) {
            f.provenance.engine = Some(engine.clone());
        }
        let schema = serde_json::from_str(JSON_SCHEMA).unwrap_or_default();
        let value = serde_json::to_value(&r).unwrap_or_default();
        assert_eq!(
            crate::schema::validate(&schema, &value),
            Vec::<String>::new()
        );
        assert_eq!(value["engine"]["established_by"], "stock-fallback");
        let mut wrong = value.clone();
        wrong["engine"]["established_by"] = serde_json::json!("default");
        assert_ne!(
            crate::schema::validate(&schema, &wrong),
            Vec::<String>::new()
        );
        let mut wrong = value;
        if let Some(engine) = wrong["engine"].as_object_mut() {
            engine.remove("manifest_sha256");
        }
        assert_ne!(
            crate::schema::validate(&schema, &wrong),
            Vec::<String>::new()
        );
        let project: serde_json::Value =
            serde_json::from_str(include_str!("../schema/project-result-v1.json"))
                .unwrap_or_default();
        let identity = crate::project::EngineIdentity {
            name: "interactive".to_owned(),
            manifest_sha256: "cd".repeat(32),
        };
        let identity = serde_json::to_value(identity).unwrap_or_default();
        assert_eq!(
            crate::schema::validate(&project["properties"]["engine"], &identity),
            Vec::<String>::new()
        );
        let mut extra = identity;
        extra["established_by"] = serde_json::json!("engine");
        assert_ne!(
            crate::schema::validate(&project["properties"]["engine"], &extra),
            Vec::<String>::new()
        );
    }
    #[test]
    fn verus_totals_clear_unknown_functions_only_when_every_error_is_accounted_for() {
        let golden: Report = serde_json::from_str(include_str!(
            "../../sail-cli/tests/fixtures/attr/report.golden.json"
        ))
        .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        // The fixture: 8 functions, 2 failed, and Verus reports 2 errors.
        let mut krate = golden.crates[0].clone();
        assert_eq!(krate.verus.errors, Some(2));
        assert_eq!(krate.unknown_passed_by_verus_totals(), 0);
        // Incomplete attribution leaves the 6 successes unknown (VI-0003).
        for f in &mut krate.functions {
            if f.verdict == VerdictName::Verified {
                f.verdict = VerdictName::Unknown;
            }
        }
        assert_eq!(krate.unknown_passed_by_verus_totals(), 6);
        // A further Verus error may belong to any unknown function.
        krate.verus.errors = Some(3);
        assert_eq!(krate.unknown_passed_by_verus_totals(), 0);
        krate.verus.errors = Some(2);
        krate.verus.encountered_vir_error = Some(true);
        assert_eq!(krate.unknown_passed_by_verus_totals(), 0);
        krate.verus.encountered_vir_error = Some(false);
        krate.verus.verified = None;
        assert_eq!(krate.unknown_passed_by_verus_totals(), 0);
        // Without a failed function there is nothing to account errors to.
        krate.verus.verified = Some(9);
        for f in &mut krate.functions {
            f.verdict = VerdictName::Unknown;
        }
        krate.verus.errors = Some(0);
        assert_eq!(krate.unknown_passed_by_verus_totals(), 0);
    }
    #[test]
    fn portfolio_evidence_matches_the_schema() {
        let mut r: Report = serde_json::from_str(include_str!(
            "../../sail-cli/tests/fixtures/attr/report.golden.json"
        ))
        .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let attempt = PortfolioAttempt {
            configuration: SolverConfiguration { seed: 4 },
            verified: false,
            exit_code: Some(1),
            wall_ms: 1.0,
            error: None,
            report: Some(Box::new(r.clone())),
            stderr: "original diagnostics".to_owned(),
        };
        r.mode = "accept-first-proof".to_owned();
        r.portfolio = Some(PortfolioReport {
            seeds: vec![0, 4],
            winner_seed: None,
            attempts: vec![attempt],
        });
        let schema = serde_json::from_str(JSON_SCHEMA).unwrap_or_default();
        let value = serde_json::to_value(r).unwrap_or_default();
        assert!(
            crate::schema::validate(&schema, &value).is_empty(),
            "{:?}",
            crate::schema::validate(&schema, &value)
        );
    }
}
