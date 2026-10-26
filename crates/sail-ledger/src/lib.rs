//! Verdicts, obligation identities, establishment (provisional or confirmed)
//! and result provenance (charter rules 7 and 8, ADR 0014).
//!
//! The ledger holds what sail knows about each verified function:
//! - Verus's own verdict for it (`--output-json --time`);
//! - its obligations: one per solver query op Verus ran for it, each with a
//!   stable [`ObligationId`] and the [`CheckRecord`]s of its `check-sat`
//!   commands;
//! - how each answer was established ([`Source`]) and under which [`Mode`];
//! - its [`Establishment`].
//!
//! The reported [`Verdict`] is derived, never stored, so the rule that
//! nothing provisional is ever reported as verified holds by construction:
//! [`FunctionRecord::verdict`] returns [`Verdict::Provisional`] for every
//! provisional record, whatever its evidence says.
#![forbid(unsafe_code)]

use std::fmt;

/// Whether a result has been established by the pinned solver on the exact
/// query (charter rule 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Establishment {
    /// Returned before the pinned solver established it (for example a
    /// speculative result). Never reported as verified.
    Provisional,
    /// Established by the pinned solver on the exact query: a fresh run, or
    /// a cache hit of a pinned-solver result.
    Confirmed,
}

impl Establishment {
    /// The schema name (`provisional`, `confirmed`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Provisional => "provisional",
            Self::Confirmed => "confirmed",
        }
    }
}

/// The verification mode a result was produced in (charter rule 8).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mode {
    /// Default mode: observationally identical to plain Verus.
    Default,
    /// A named, opt-in mode that can change verdicts (recorded in
    /// `docs/divergences.md`).
    OptIn(String),
}

impl Mode {
    /// The schema name: `default`, or the opt-in mode's name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Default => "default",
            Self::OptIn(name) => name,
        }
    }
}

/// How one solver answer was established (charter rule 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// A fresh run of the pinned solver.
    Solved,
    /// A cache hit of a reply the pinned solver produced for the same key.
    Cache,
}

impl Source {
    /// The schema name (`solved`, `cache`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solved => "solved",
            Self::Cache => "cache",
        }
    }
}

/// A `check-sat` answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Answer {
    /// `sat`: the negated goal has a model, so an assertion can fail.
    Sat,
    /// `unsat`: proved.
    Unsat,
    /// `unknown`: the solver gave up.
    Unknown,
}

impl Answer {
    /// Parses a solver reply (`sat`, `unsat` or `unknown`, surrounding
    /// whitespace ignored).
    #[must_use]
    pub fn parse(reply: &str) -> Option<Self> {
        match reply.trim() {
            "sat" => Some(Self::Sat),
            "unsat" => Some(Self::Unsat),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    /// The solver's spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sat => "sat",
            Self::Unsat => "unsat",
            Self::Unknown => "unknown",
        }
    }
}

/// Solver effort for one check (ADR 0009). rlimit consumption is an output,
/// recorded here and never part of a key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Effort {
    /// The `:rlimit` in force (0 means unlimited).
    pub limit: Option<u64>,
    /// `rlimit-count` units used: the statistics reply after the check
    /// minus the one before it.
    pub used: Option<u64>,
    /// The `get-info :reason-unknown` reply, for `unknown` answers.
    pub reason: Option<String>,
}

impl Effort {
    /// Whether the reason names resource exhaustion (`canceled`, or anything
    /// mentioning resources, rlimit, timeout or memory).
    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.reason.as_deref().is_some_and(|r| {
            let lower = r.to_ascii_lowercase();
            [
                "canceled",
                "cancelled",
                "resource",
                "rlimit",
                "timeout",
                "memout",
                "memory",
            ]
            .iter()
            .any(|w| lower.contains(w))
        })
    }

    /// Whether the check finished near its limit: it used more than half of
    /// a non-zero limit. This is the threshold the shim applies before
    /// serving a cached `unknown` (ADR 0009, owner decision 2026-10-08).
    #[must_use]
    pub fn near_limit(&self) -> bool {
        matches!((self.used, self.limit), (Some(used), Some(limit)) if limit > 0 && used.saturating_mul(2) > limit)
    }
}

/// One `check-sat` an obligation ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRecord {
    /// The answer, if the transcript recorded one.
    pub answer: Option<Answer>,
    /// Effort and reason.
    pub effort: Effort,
    /// How the answer was established; `None` when sail has no record.
    pub source: Option<Source>,
    /// The `get-model` reply that followed, if any.
    pub model: Option<Model>,
}

/// A solver model: the reply verbatim, and its constants as data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Model {
    /// The `get-model` reply, verbatim.
    pub text: String,
    /// Every zero-arity `define-fun` in it, in order. Empty if the reply
    /// could not be parsed.
    pub assignments: Vec<Assignment>,
}

/// One constant in a model: `(define-fun <name> () <sort> <value>)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    /// The SMT name, as Verus encodes it (for example `x!` for parameter
    /// `x`, `i@1` for the second version of local `i`, or
    /// `%%location_label%%3` for an assertion label).
    pub name: String,
    /// The sort, verbatim.
    pub sort: String,
    /// The value, verbatim.
    pub value: String,
}

impl CheckRecord {
    /// Whether this answer could plausibly differ between plain runs of the
    /// same query (ADR 0009): an `unknown` from resource exhaustion, or any
    /// `unsat`/`unknown` that finished near its limit.
    #[must_use]
    pub fn unstable(&self) -> bool {
        match self.answer {
            Some(Answer::Unknown) => self.effort.exhausted() || self.effort.near_limit(),
            Some(Answer::Unsat) => self.effort.near_limit(),
            Some(Answer::Sat) | None => false,
        }
    }
}

/// What a query op is for. The kinds are Verus's own op comments
/// (`rust_verify/src/commands.rs`, `to_air_comment`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObligationKind(pub String);

impl ObligationKind {
    /// Whether a failure of this kind is an error that fails the function.
    /// Recommends checks and expand-errors queries report notes or warnings,
    /// and profile reruns report nothing, so they are advisory.
    #[must_use]
    pub fn verdict_bearing(&self) -> bool {
        matches!(
            self.0.as_str(),
            "Function-Def" | "Spec-Termination" | "Check-Api-Safety"
        )
    }

    /// Whether `word` is the kind of a query op (not a context op such as
    /// `Function-Specs`).
    #[must_use]
    pub fn is_query_kind(word: &str) -> bool {
        let base = word.strip_suffix("-Profile-Rerun").unwrap_or(word);
        matches!(
            base,
            "Function-Def"
                | "Spec-Termination"
                | "Check-Api-Safety"
                | "Function-Recommends"
                | "Function-Expand-Errors"
        )
    }
}

/// A stable identity for one obligation (one solver query op).
///
/// It is the function's crate-qualified path as Verus names it, the op kind,
/// and the op's position among that function's ops of the same kind:
/// - for ops run in the function's module process, `ordinal` counts them in
///   order (0, 1, …) and `spinoff` is `None`;
/// - for an op spun off into its own process (nonlinear, bit-vector,
///   profile rerun), `spinoff` is Verus's per-op spinoff counter (1, 2, …)
///   and `ordinal` is 0.
///
/// Source positions are deliberately not part of the identity, so it
/// survives edits elsewhere in the file. See `docs/output-schema.md`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObligationId {
    /// The function, as Verus names it (for example `demo::max` or
    /// `demo::Counter::incr`).
    pub function: String,
    /// The op kind.
    pub kind: ObligationKind,
    /// Position among the function's in-module ops of this kind.
    pub ordinal: u32,
    /// Verus's spinoff counter, for spun-off ops.
    pub spinoff: Option<u32>,
}

impl fmt::Display for ObligationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.spinoff {
            None => write!(f, "{}#{}/{}", self.function, self.kind.0, self.ordinal),
            Some(n) => write!(f, "{}#{}/spinoff-{n}", self.function, self.kind.0),
        }
    }
}

impl std::str::FromStr for ObligationId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("not an obligation id: {s:?}");
        let (function, rest) = s.rsplit_once('#').ok_or_else(bad)?;
        let (kind, position) = rest.split_once('/').ok_or_else(bad)?;
        if function.is_empty() || kind.is_empty() {
            return Err(bad());
        }
        let (ordinal, spinoff) = match position.strip_prefix("spinoff-") {
            Some(n) => (0, Some(n.parse().map_err(|_| bad())?)),
            None => (position.parse().map_err(|_| bad())?, None),
        };
        Ok(Self {
            function: function.to_owned(),
            kind: ObligationKind(kind.to_owned()),
            ordinal,
            spinoff,
        })
    }
}

/// What the evidence says about an obligation or function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    /// Proved.
    Proved,
    /// Not proved, and the failure is settled: the solver found a model, or
    /// gave up far from its limit for its own reasons.
    Failed,
    /// Not proved because the solver gave up at or near its limit (or no
    /// answer was recorded). Never presented as a settled failure (ADR 0009).
    Unknown,
}

/// One obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationRecord {
    /// Its stable identity.
    pub id: ObligationId,
    /// Verus's span for the op, as written in its log (`file:line:col:
    /// line:col (#n)`). Informational: it is not part of the identity.
    pub span: Option<String>,
    /// Why Verus spun the op off (`nonlinear`, `bitvector`, …).
    pub spinoff_reason: Option<String>,
    /// Its `check-sat` commands, in order. The first decides validity; later
    /// ones locate further failing assertions.
    pub checks: Vec<CheckRecord>,
}

impl ObligationRecord {
    /// The obligation's outcome, from its first check.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        let Some(first) = self.checks.first() else {
            return Outcome::Unknown;
        };
        match first.answer {
            Some(Answer::Unsat) => Outcome::Proved,
            Some(Answer::Sat) => Outcome::Failed,
            Some(Answer::Unknown) if !first.unstable() => Outcome::Failed,
            Some(Answer::Unknown) | None => Outcome::Unknown,
        }
    }

    /// Whether any of its checks is unstable (ADR 0009).
    #[must_use]
    pub fn unstable(&self) -> bool {
        self.checks.iter().any(CheckRecord::unstable)
    }
}

/// How a function's answers were established, counted per check.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceCounts {
    /// Checks answered by a fresh pinned-solver run.
    pub solved: u64,
    /// Checks answered from the cache.
    pub served: u64,
    /// Checks with no provenance record.
    pub unrecorded: u64,
}

/// A summary of [`SourceCounts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceSummary {
    /// Every check was solved fresh.
    Fresh,
    /// Every check was served from the cache.
    Cache,
    /// Some of each (and none unrecorded).
    Mixed,
    /// At least one check has no record, or no check was attributed.
    Unrecorded,
}

impl SourceSummary {
    /// The schema name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Cache => "cache",
            Self::Mixed => "mixed",
            Self::Unrecorded => "unrecorded",
        }
    }
}

impl SourceCounts {
    /// Summarises the counts.
    #[must_use]
    pub fn summary(self) -> SourceSummary {
        match (self.solved, self.served, self.unrecorded) {
            (_, _, u) if u > 0 => SourceSummary::Unrecorded,
            (0, 0, _) => SourceSummary::Unrecorded,
            (_, 0, _) => SourceSummary::Fresh,
            (0, _, _) => SourceSummary::Cache,
            _ => SourceSummary::Mixed,
        }
    }
}

/// The verdict sail reports for a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// Confirmed and proved.
    Verified,
    /// Confirmed and failed.
    Failed,
    /// Confirmed, not proved, and not a settled failure (ADR 0009).
    Unknown,
    /// Not yet established by the pinned solver. Whatever the evidence says,
    /// a provisional result is never reported as verified.
    Provisional,
}

impl Verdict {
    /// The schema name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Provisional => "provisional",
        }
    }
}

/// Everything the ledger knows about one function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRecord {
    /// The function, as Verus names it.
    pub function: String,
    /// Its module, as Verus reports it (`""` for the crate root).
    pub module: String,
    /// Verus's mode for it (`exec`, `proof`, `spec`), if reported.
    pub fn_mode: Option<String>,
    /// Verus's own `success` from `--output-json --time`. It is incomplete:
    /// it stays `true` when the only failure is a bit-vector query
    /// (`docs/verus-issues/VI-0003`).
    pub verus_success: Option<bool>,
    /// Solver time Verus reports for it, in microseconds.
    pub smt_micros: Option<u64>,
    /// rlimit units Verus reports for it.
    pub rlimit: Option<u64>,
    /// Its obligations, or `None` when attribution was unavailable.
    pub obligations: Option<Vec<ObligationRecord>>,
    /// How it was established (charter rule 7).
    pub establishment: Establishment,
    /// The mode that produced it.
    pub mode: Mode,
}

impl FunctionRecord {
    /// What the evidence says, from Verus's verdict and the obligations:
    /// - a failed verdict-bearing obligation fails the function, even when
    ///   Verus's `success` says otherwise (the bit-vector gap);
    /// - a function Verus failed whose only non-proved verdict-bearing
    ///   obligations are [`Outcome::Unknown`] is unknown;
    /// - any other function Verus failed is failed;
    /// - otherwise it is proved if Verus says so (or, with no Verus verdict,
    ///   if every verdict-bearing obligation is proved).
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        let bearing: Vec<Outcome> = self
            .obligations
            .iter()
            .flatten()
            .filter(|o| o.id.kind.verdict_bearing())
            .map(ObligationRecord::outcome)
            .collect();
        if bearing.contains(&Outcome::Failed) {
            return Outcome::Failed;
        }
        let unknown = bearing.contains(&Outcome::Unknown);
        match self.verus_success {
            Some(false) if unknown => Outcome::Unknown,
            Some(false) => Outcome::Failed,
            None if unknown || self.obligations.is_none() => Outcome::Unknown,
            Some(true) | None => Outcome::Proved,
        }
    }

    /// The verdict to report. Provisional records are always
    /// [`Verdict::Provisional`] (charter rule 7).
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        if self.establishment == Establishment::Provisional {
            return Verdict::Provisional;
        }
        match self.outcome() {
            Outcome::Proved => Verdict::Verified,
            Outcome::Failed => Verdict::Failed,
            Outcome::Unknown => Verdict::Unknown,
        }
    }

    /// Whether any obligation is unstable (ADR 0009).
    #[must_use]
    pub fn unstable(&self) -> bool {
        self.obligations
            .iter()
            .flatten()
            .any(ObligationRecord::unstable)
    }

    /// Per-check source counts over all obligations.
    #[must_use]
    pub fn sources(&self) -> SourceCounts {
        let mut counts = SourceCounts::default();
        for check in self.obligations.iter().flatten().flat_map(|o| &o.checks) {
            match check.source {
                Some(Source::Solved) => counts.solved += 1,
                Some(Source::Cache) => counts.served += 1,
                None => counts.unrecorded += 1,
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(answer: Answer, used: u64, reason: Option<&str>) -> CheckRecord {
        CheckRecord {
            answer: Some(answer),
            effort: Effort {
                limit: Some(1000),
                used: Some(used),
                reason: reason.map(str::to_owned),
            },
            source: Some(Source::Solved),
            model: None,
        }
    }

    fn obligation(function: &str, kind: &str, checks: Vec<CheckRecord>) -> ObligationRecord {
        ObligationRecord {
            id: ObligationId {
                function: function.to_owned(),
                kind: ObligationKind(kind.to_owned()),
                ordinal: 0,
                spinoff: None,
            },
            span: None,
            spinoff_reason: None,
            checks,
        }
    }

    fn function(verus: Option<bool>, obligations: Option<Vec<ObligationRecord>>) -> FunctionRecord {
        FunctionRecord {
            function: "c::f".to_owned(),
            module: String::new(),
            fn_mode: None,
            verus_success: verus,
            smt_micros: None,
            rlimit: None,
            obligations,
            establishment: Establishment::Confirmed,
            mode: Mode::Default,
        }
    }

    #[test]
    fn obligation_ids_round_trip() {
        for text in [
            "demo::max#Function-Def/0",
            "demo::<T as Tr>::m#Function-Recommends/2",
            "demo::arith::mul_pos#Function-Def/spinoff-1",
        ] {
            let id: Result<ObligationId, _> = text.parse();
            assert_eq!(id.map(|i| i.to_string()).as_deref(), Ok(text));
        }
        for bad in ["", "f", "f#K", "#K/0", "f#/0", "f#K/x", "f#K/spinoff-"] {
            assert!(bad.parse::<ObligationId>().is_err(), "{bad}");
        }
    }

    #[test]
    fn effort_policy_matches_the_shim() {
        // The same thresholds as the shim's `unknown_servable` (ADR 0009).
        let e = |used, limit, reason: &str| Effort {
            limit: Some(limit),
            used: Some(used),
            reason: Some(reason.to_owned()),
        };
        assert!(!e(2_000, 30_000_000, "(incomplete quantifiers)").exhausted());
        assert!(!e(2_000, 30_000_000, "x").near_limit());
        assert!(e(30_000_000, 30_000_000, "canceled").exhausted());
        assert!(e(16_000_000, 30_000_000, "x").near_limit());
        assert!(!e(15_000_000, 30_000_000, "x").near_limit());
        assert!(!e(5, 0, "x").near_limit());
    }

    #[test]
    fn obligation_outcomes() {
        let o = |c| obligation("f", "Function-Def", vec![c]);
        assert_eq!(o(check(Answer::Unsat, 10, None)).outcome(), Outcome::Proved);
        assert_eq!(o(check(Answer::Sat, 10, None)).outcome(), Outcome::Failed);
        assert_eq!(
            o(check(Answer::Unknown, 10, Some("(incomplete quantifiers)"))).outcome(),
            Outcome::Failed
        );
        assert_eq!(
            o(check(Answer::Unknown, 1000, Some("canceled"))).outcome(),
            Outcome::Unknown
        );
        assert_eq!(
            obligation("f", "Function-Def", vec![]).outcome(),
            Outcome::Unknown
        );
        // A proof near its limit is proved, and unstable.
        let near = o(check(Answer::Unsat, 900, None));
        assert_eq!(near.outcome(), Outcome::Proved);
        assert!(near.unstable());
    }

    #[test]
    fn a_served_near_limit_proof_is_reported_unstable() {
        // Charter rule 4 as amended (ADR 0009): a cached `unsat` that finished
        // near its limit may be served, and is reported unstable. A served
        // answer carries the effort its original solve recorded.
        let served = |used| CheckRecord {
            source: Some(Source::Cache),
            ..check(Answer::Unsat, used, None)
        };
        let near = function(
            Some(true),
            Some(vec![obligation("c::f", "Function-Def", vec![served(501)])]),
        );
        assert_eq!(near.verdict(), Verdict::Verified);
        assert!(near.unstable());
        let far = function(
            Some(true),
            Some(vec![obligation("c::f", "Function-Def", vec![served(500)])]),
        );
        assert!(!far.unstable());
    }

    #[test]
    fn function_verdicts() {
        let proved = || obligation("f", "Function-Def", vec![check(Answer::Unsat, 1, None)]);
        let failed = || obligation("f", "Function-Def", vec![check(Answer::Sat, 1, None)]);
        let advisory = || {
            obligation(
                "f",
                "Function-Recommends",
                vec![check(Answer::Sat, 1, None)],
            )
        };
        let exhausted = || {
            obligation(
                "f",
                "Function-Def",
                vec![check(Answer::Unknown, 1000, Some("canceled"))],
            )
        };
        let v = |verus, obs| function(verus, obs).verdict();
        assert_eq!(v(Some(true), None), Verdict::Verified);
        assert_eq!(v(Some(false), None), Verdict::Failed);
        assert_eq!(v(Some(true), Some(vec![proved()])), Verdict::Verified);
        // The bit-vector gap: Verus says success, an obligation failed.
        assert_eq!(v(Some(true), Some(vec![failed()])), Verdict::Failed);
        // Advisory failures never fail a function.
        assert_eq!(
            v(Some(false), Some(vec![failed(), advisory()])),
            Verdict::Failed
        );
        assert_eq!(
            v(Some(true), Some(vec![proved(), advisory()])),
            Verdict::Verified
        );
        // Exhaustion is unknown, not a settled failure.
        assert_eq!(v(Some(false), Some(vec![exhausted()])), Verdict::Unknown);
        assert_eq!(v(None, Some(vec![proved()])), Verdict::Verified);
        assert_eq!(v(None, None), Verdict::Unknown);
    }

    #[test]
    fn provisional_is_never_verified() {
        for verus in [Some(true), Some(false), None] {
            let mut f = function(
                verus,
                Some(vec![obligation(
                    "f",
                    "Function-Def",
                    vec![check(Answer::Unsat, 1, None)],
                )]),
            );
            f.establishment = Establishment::Provisional;
            assert_eq!(f.verdict(), Verdict::Provisional);
        }
    }

    #[test]
    fn source_counts() {
        let mut c = check(Answer::Unsat, 1, None);
        let mut f = function(Some(true), None);
        assert_eq!(f.sources().summary(), SourceSummary::Unrecorded);
        f.obligations = Some(vec![obligation("f", "Function-Def", vec![c.clone()])]);
        assert_eq!(f.sources().summary(), SourceSummary::Fresh);
        c.source = Some(Source::Cache);
        f.obligations = Some(vec![obligation("f", "Function-Def", vec![c.clone()])]);
        assert_eq!(f.sources().summary(), SourceSummary::Cache);
        let solved = check(Answer::Sat, 1, None);
        f.obligations = Some(vec![obligation(
            "f",
            "Function-Def",
            vec![solved, c.clone()],
        )]);
        assert_eq!(f.sources().summary(), SourceSummary::Mixed);
        c.source = None;
        f.obligations = Some(vec![obligation("f", "Function-Def", vec![c])]);
        assert_eq!(f.sources().summary(), SourceSummary::Unrecorded);
    }
}
