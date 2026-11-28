//! Per-harness, per-check comparison and the verdict rules (ADR 0181).
#![allow(
    clippy::many_single_char_names,
    reason = "each function compares a left and a right run; a/b and x/y name the two sides"
)]
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use super::record::{Claim, KaniRunRecord};

/// Every field allowed to differ between two runs, each named individually,
/// with where the variation comes from. A named field may differ only in the
/// way its rule states (a finite nonnegative number, a timestamp, or the same
/// path below each run's own directory), and only once two direct runs of the
/// same project have been seen to differ on it ([`witness`]). The first eleven
/// are ADR 0156's export fields.
pub const VARIANCE: &[(&str, &str)] = &[
    ("export:metadata.timestamp", "wall-clock time of the run"),
    (
        "export:verification_results.summary.duration_ms",
        "wall time",
    ),
    (
        "export:verification_results.results[].duration_ms",
        "wall time",
    ),
    (
        "export:cbmc[].cbmc_stats.runtime_convert_ssa_s",
        "CBMC timing",
    ),
    (
        "export:cbmc[].cbmc_stats.runtime_decision_procedure_s",
        "CBMC timing",
    ),
    (
        "export:cbmc[].cbmc_stats.runtime_post_process_s",
        "CBMC timing",
    ),
    (
        "export:cbmc[].cbmc_stats.runtime_postprocess_equation_s",
        "CBMC timing",
    ),
    ("export:cbmc[].cbmc_stats.runtime_solver_s", "CBMC timing"),
    ("export:cbmc[].cbmc_stats.runtime_symex_s", "CBMC timing"),
    (
        "export:project.output_dir",
        "below each run's own target directory",
    ),
    (
        "export:harness_metadata[].goto_file",
        "below each run's own target directory",
    ),
    ("stream:Runtime Symex", "CBMC timing line"),
    ("stream:Runtime Postprocess Equation", "CBMC timing line"),
    ("stream:Runtime Convert SSA", "CBMC timing line"),
    ("stream:Runtime Post-process", "CBMC timing line"),
    ("stream:Runtime Solver", "CBMC timing line"),
    ("stream:Runtime decision procedure", "CBMC timing line"),
    ("stream:Verification Time", "Kani timing line"),
    ("stream:Cargo Finished", "Cargo's build-time line"),
    (
        "stream:Cargo Compiling order",
        "order and interleaving of Cargo's `Compiling` lines (parallel build scheduling); compared as a multiset",
    ),
    (
        "stream:Reading GOTO program",
        "path below each run's own target directory",
    ),
    (
        "check:vtable-size description",
        "rustc table indices (`id: N`) printed in the vtable-size sanity check (S3 cause 2)",
    ),
];

const RUNTIME_LABELS: &[&str] = &[
    "Symex",
    "Postprocess Equation",
    "Convert SSA",
    "Post-process",
    "Solver",
    "decision procedure",
];

const CHECK_FIELDS: &[&str] = &[
    "id",
    "function",
    "category",
    "description",
    "location",
    "status",
];
const MAX_LISTED: usize = 20;

/// What differed between two runs of one harness.
#[derive(Debug, Clone, Default, Serialize)]
pub struct KaniComparison {
    /// Differences outside [`VARIANCE`], one line each (the first
    /// [`MAX_LISTED`] per section, then a count).
    pub differences: Vec<String>,
    /// Named fields that differed, with how many values.
    pub variances: BTreeMap<String, usize>,
    /// Checks compared.
    pub checks: usize,
}

impl KaniComparison {
    /// No difference outside the named fields.
    #[must_use]
    pub fn clean(&self) -> bool {
        self.differences.is_empty()
    }
    fn vary(&mut self, name: &str) {
        *self.variances.entry(name.to_owned()).or_insert(0) += 1;
    }
}

/// The outcome for one harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KaniVerdict {
    /// Sail's claim and every compared field match a direct run.
    Same,
    /// The direct runs differ outside the named fields and Sail matches one
    /// of them. Reported, not counted as a Sail difference.
    Unstable,
    /// At least one difference.
    Different,
    /// Sail made no claim where direct Kani made one: a downgrade, never a
    /// wrong answer, counted on its own and never as `same`.
    NoClaim,
    /// A direct run could not run (not a conformance result).
    InfraError,
}

/// One harness's records and verdict.
#[derive(Debug, Clone, Serialize)]
pub struct HarnessResult {
    /// Corpus project.
    pub project: String,
    /// Harness.
    pub harness: String,
    /// Verdict.
    pub verdict: KaniVerdict,
    /// The direct claim (first direct run).
    pub direct: Claim,
    /// Sail's claim.
    pub sail: Claim,
    /// Direct run 1 against direct run 2.
    pub direct_pair: KaniComparison,
    /// Sail against each direct run, in order, as far as needed.
    pub sail_against: Vec<KaniComparison>,
    /// Why the verdict is not `same`, in words.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Compares run `b` with run `a` of the same harness.
#[must_use]
pub fn compare(a: &KaniRunRecord, b: &KaniRunRecord) -> KaniComparison {
    let mut c = KaniComparison::default();
    if a.harness != b.harness || a.unwind != b.unwind {
        c.differences.push(format!(
            "request: {}@{} / {}@{}",
            a.harness, a.unwind, b.harness, b.unwind
        ));
    }
    invocation(a, b, &mut c);
    if (a.exit, a.signal) != (b.exit, b.signal) {
        c.differences.push(format!(
            "driver exit: {:?}/{:?} / {:?}/{:?}",
            a.exit, a.signal, b.exit, b.signal
        ));
    }
    match (&a.export, &b.export) {
        (Some(x), Some(y)) => {
            let mut walk = Walk {
                a,
                b,
                c: &mut c,
                listed: 0,
            };
            walk.value(x, y, "");
            checks(a, x, y, &mut c);
        }
        (None, None) => {}
        (x, y) => c
            .differences
            .push(format!("export present: {} / {}", x.is_some(), y.is_some())),
    }
    streams("stdout", a, b, &a.stdout, &b.stdout, &mut c);
    streams("stderr", a, b, &a.stderr, &b.stderr, &mut c);
    c
}

/// Classifies one harness from its direct runs (at least two) and Sail's run.
#[must_use]
pub fn classify(direct: &[KaniRunRecord], sail: &KaniRunRecord) -> HarnessResult {
    let mut result = HarnessResult {
        project: sail.project.clone(),
        harness: sail.harness.clone(),
        verdict: KaniVerdict::InfraError,
        direct: direct
            .first()
            .map_or(Claim::NoClaim(super::NoClaim::Refused), |d| d.claim),
        sail: sail.claim,
        direct_pair: KaniComparison::default(),
        sail_against: Vec::new(),
        notes: Vec::new(),
    };
    let [first, second, ..] = direct else {
        result.notes.push("fewer than two direct runs".into());
        return result;
    };
    if first.exit.is_none() && first.signal.is_none() {
        result.notes.push("direct run did not complete".into());
        return result;
    }
    result.direct_pair = compare(first, second);
    if first.claim != second.claim {
        result.direct_pair.differences.push(format!(
            "direct claim: {:?} / {:?}",
            first.claim, second.claim
        ));
    }
    if !sail.claim.is_claim() && first.claim.is_claim() {
        result.verdict = KaniVerdict::NoClaim;
        result.notes.push(format!(
            "Sail made no claim ({:?}): {}",
            sail.claim,
            sail.refusal.as_deref().unwrap_or("no message")
        ));
        // When the driver ran to completion under Sail, its raw output must
        // still match: an unadmitted harness is no excuse for different bytes.
        if sail.exit.is_some() {
            let raw = compare(first, sail);
            if !raw.clean() {
                result.verdict = KaniVerdict::Different;
                result.notes.push("raw driver output differs".into());
            }
            result.sail_against.push(raw);
        }
        return result;
    }
    for (index, run) in direct.iter().enumerate() {
        let mut against = compare(run, sail);
        if run.claim != sail.claim {
            against.differences.push(format!(
                "claim: direct {:?} / sail {:?}",
                run.claim, sail.claim
            ));
        }
        let clean = against.clean();
        result.sail_against.push(against);
        if clean {
            result.verdict = if index == 0 && result.direct_pair.clean() {
                KaniVerdict::Same
            } else {
                KaniVerdict::Unstable
            };
            return result;
        }
    }
    result.verdict = KaniVerdict::Different;
    result
}

/// Applies the witness rule over one project's results (every round): a named
/// field may differ against Sail only if it differed between two direct runs
/// of that project. Returns the named fields seen only against Sail; each
/// affected harness becomes `different`.
pub fn witness(results: &mut [HarnessResult]) -> BTreeSet<String> {
    let mut witnessed: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for r in results.iter() {
        witnessed
            .entry(r.project.as_str())
            .or_default()
            .extend(r.direct_pair.variances.keys().cloned());
    }
    let witnessed: BTreeMap<String, BTreeSet<String>> = witnessed
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
    let mut unwitnessed = BTreeSet::new();
    for r in results.iter_mut() {
        let seen = witnessed.get(&r.project).cloned().unwrap_or_default();
        let Some(last) = r.sail_against.last() else {
            continue;
        };
        let missing: Vec<String> = last
            .variances
            .keys()
            .filter(|name| !seen.contains(*name))
            .cloned()
            .collect();
        if !missing.is_empty() && matches!(r.verdict, KaniVerdict::Same | KaniVerdict::Unstable) {
            r.verdict = KaniVerdict::Different;
            r.notes.push(format!(
                "named fields differ only against Sail: {}",
                missing.join(", ")
            ));
        }
        unwitnessed.extend(missing);
    }
    unwitnessed
}

fn relative<'p>(path: &'p str, capture: &Path) -> Option<&'p str> {
    let rest = path.strip_prefix(capture.to_str()?)?.strip_prefix('/')?;
    rest.split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..")
        .then_some(rest)
}

fn same_relative(x: &str, a: &KaniRunRecord, y: &str, b: &KaniRunRecord) -> bool {
    matches!((relative(x, &a.capture), relative(y, &b.capture)), (Some(p), Some(q)) if p == q)
}

fn invocation(a: &KaniRunRecord, b: &KaniRunRecord, c: &mut KaniComparison) {
    let map = |value: &str, r: &KaniRunRecord| -> String {
        relative(value, &r.capture).map_or_else(|| value.to_owned(), |rest| format!("<run>/{rest}"))
    };
    let argv = |r: &KaniRunRecord| r.argv.iter().map(|v| map(v, r)).collect::<Vec<_>>();
    if argv(a) != argv(b) {
        c.differences
            .push(format!("driver argv: {:?} / {:?}", argv(a), argv(b)));
    }
    let env = |r: &KaniRunRecord| {
        r.environment
            .iter()
            .map(|(k, v)| (k.clone(), map(v, r)))
            .collect::<BTreeMap<_, _>>()
    };
    if env(a) != env(b) {
        c.differences
            .push(format!("driver environment: {:?} / {:?}", env(a), env(b)));
    }
}

struct Walk<'a, 'c> {
    a: &'a KaniRunRecord,
    b: &'a KaniRunRecord,
    c: &'c mut KaniComparison,
    listed: usize,
}

impl Walk<'_, '_> {
    fn differ(&mut self, path: &str, x: &Value, y: &Value) {
        let name = format!("export:{path}");
        if self.allowed(&name, x, y) {
            self.c.vary(&name);
            return;
        }
        self.listed += 1;
        if self.listed <= MAX_LISTED {
            let show = |v: &Value| {
                let text = v.to_string();
                if text.len() > 160 {
                    format!("{}…", &text[..text.floor_char_boundary(160)])
                } else {
                    text
                }
            };
            self.c
                .differences
                .push(format!("export {path}: {} / {}", show(x), show(y)));
        } else if self.listed == MAX_LISTED + 1 {
            self.c.differences.push("export: more differences".into());
        }
    }

    fn allowed(&self, name: &str, x: &Value, y: &Value) -> bool {
        if !VARIANCE.iter().any(|(n, _)| *n == name) {
            return false;
        }
        let number = |v: &Value| v.as_f64().is_some_and(|f| f.is_finite() && f >= 0.0);
        match name {
            "export:metadata.timestamp" => [x, y].iter().all(|v| {
                v.as_str().is_some_and(|s| {
                    s.len() >= 20 && s.ends_with('Z') && s.as_bytes().get(4) == Some(&b'-')
                })
            }),
            "export:project.output_dir" | "export:harness_metadata[].goto_file" => {
                matches!((x.as_str(), y.as_str()), (Some(p), Some(q)) if same_relative(p, self.a, q, self.b))
            }
            n if n.ends_with("duration_ms") => x.as_u64().is_some() && y.as_u64().is_some(),
            _ => number(x) && number(y),
        }
    }

    fn value(&mut self, x: &Value, y: &Value, path: &str) {
        match (x, y) {
            (Value::Object(p), Value::Object(q)) => {
                let keys: BTreeSet<&String> = p.keys().chain(q.keys()).collect();
                for key in keys {
                    let sub = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    if sub == "verification_results.results[].checks" {
                        continue;
                    }
                    match (p.get(key), q.get(key)) {
                        (Some(v), Some(w)) => self.value(v, w, &sub),
                        (v, w) => {
                            self.differ(&sub, v.unwrap_or(&Value::Null), w.unwrap_or(&Value::Null));
                        }
                    }
                }
            }
            (Value::Array(p), Value::Array(q)) if p.len() == q.len() => {
                let sub = format!("{path}[]");
                for (v, w) in p.iter().zip(q) {
                    self.value(v, w, &sub);
                }
            }
            _ if x == y => {}
            _ => self.differ(path, x, y),
        }
    }
}

/// `Check N: property` from Kani's regular output, by check number.
fn property_ids(stdout: &str) -> BTreeMap<u64, &str> {
    stdout
        .lines()
        .filter_map(|line| {
            let (number, property) = line.strip_prefix("Check ")?.split_once(": ")?;
            Some((number.parse().ok()?, property))
        })
        .collect()
}

fn vtable_masked(text: &str) -> Option<String> {
    if !text.contains("Correct CBMC vtable size") {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("id: ") {
        out.push_str(&rest[..at + 4]);
        rest = rest[at + 4..].trim_start_matches(|ch: char| ch.is_ascii_digit());
        out.push('N');
    }
    out.push_str(rest);
    Some(out)
}

fn checks(a: &KaniRunRecord, x: &Value, y: &Value, c: &mut KaniComparison) {
    let list = |v: &Value| -> Vec<Value> {
        v["verification_results"]["results"]
            .as_array()
            .map(|r| {
                r.iter()
                    .flat_map(|h| h["checks"].as_array().cloned().unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default()
    };
    let (p, q) = (list(x), list(y));
    let ids = property_ids(&a.stdout);
    c.checks = p.len();
    if p.len() != q.len() {
        c.differences
            .push(format!("check count: {} / {}", p.len(), q.len()));
    }
    let failed = |v: &[Value]| -> Vec<String> {
        v.iter()
            .filter(|ch| ch["status"] == "Failure")
            .map(|ch| format!("{} {}", ch["id"], ch["description"]))
            .collect()
    };
    if failed(&p) != failed(&q) {
        c.differences.push(format!(
            "failed checks: {:?} / {:?}",
            failed(&p),
            failed(&q)
        ));
    }
    let mut listed = 0;
    for (v, w) in p.iter().zip(&q) {
        if v == w {
            continue;
        }
        let mut fields: BTreeSet<&str> = CHECK_FIELDS.iter().copied().collect();
        if let (Some(m), Some(n)) = (v.as_object(), w.as_object()) {
            fields.extend(m.keys().chain(n.keys()).map(String::as_str));
        }
        for field in fields {
            let (s, t) = (&v[field], &w[field]);
            if s == t {
                continue;
            }
            let masked = |z: &Value| z.as_str().and_then(vtable_masked);
            if field == "description" && masked(s).is_some() && masked(s) == masked(t) {
                c.vary("check:vtable-size description");
                continue;
            }
            listed += 1;
            if listed <= MAX_LISTED {
                let id = v["id"].as_u64().unwrap_or(0);
                c.differences.push(format!(
                    "check {id} ({}) {field}: {s} / {t}",
                    ids.get(&id).copied().unwrap_or("?")
                ));
            }
        }
    }
    if listed > MAX_LISTED {
        c.differences.push(format!(
            "checks: {} more field differences",
            listed - MAX_LISTED
        ));
    }
}

fn number_s(text: &str) -> bool {
    text.strip_suffix('s')
        .and_then(|n| n.parse::<f64>().ok())
        .is_some_and(|f| f.is_finite() && f >= 0.0)
}

/// The named stream variance two differing lines share, if any.
fn stream_variance(x: &str, a: &KaniRunRecord, y: &str, b: &KaniRunRecord) -> Option<String> {
    let (x, y) = (x.trim_end_matches('\n'), y.trim_end_matches('\n'));
    for label in RUNTIME_LABELS {
        let prefix = format!("Runtime {label}: ");
        if let (Some(p), Some(q)) = (x.strip_prefix(&prefix), y.strip_prefix(&prefix)) {
            return (number_s(p) && number_s(q)).then(|| format!("stream:Runtime {label}"));
        }
    }
    if let (Some(p), Some(q)) = (
        x.strip_prefix("Verification Time: "),
        y.strip_prefix("Verification Time: "),
    ) {
        return (number_s(p) && number_s(q)).then(|| "stream:Verification Time".into());
    }
    if let (Some(p), Some(q)) = (
        x.strip_prefix("Reading GOTO program from file "),
        y.strip_prefix("Reading GOTO program from file "),
    ) {
        return same_relative(p, a, q, b).then(|| "stream:Reading GOTO program".into());
    }
    let finished = |line: &str| -> Option<(String, bool)> {
        let trimmed = line.trim_start();
        let (head, time) = trimmed.rsplit_once(" target(s) in ")?;
        let ok =
            trimmed.starts_with("Finished `") && head.contains("` profile [") && number_s(time);
        Some((format!("{}{head}", &line[..line.len() - trimmed.len()]), ok))
    };
    if let (Some((p, true)), Some((q, true))) = (finished(x), finished(y)) {
        return (p == q).then(|| "stream:Cargo Finished".into());
    }
    match (vtable_masked(x), vtable_masked(y)) {
        (Some(p), Some(q)) if p == q => Some("check:vtable-size description".into()),
        _ => None,
    }
}

fn streams(
    name: &str,
    a: &KaniRunRecord,
    b: &KaniRunRecord,
    x: &str,
    y: &str,
    c: &mut KaniComparison,
) {
    // Cargo's `Compiling` status lines follow its parallel job order, which
    // two direct runs do not repeat. They are compared as a multiset; every
    // other line is compared in order.
    let compiling = |line: &&str| line.starts_with("   Compiling ");
    let (mut pc, p): (Vec<&str>, Vec<&str>) = x.split_inclusive('\n').partition(compiling);
    let (mut qc, q): (Vec<&str>, Vec<&str>) = y.split_inclusive('\n').partition(compiling);
    if pc != qc {
        pc.sort_unstable();
        qc.sort_unstable();
        if pc == qc {
            c.vary("stream:Cargo Compiling order");
        } else {
            c.differences.push(format!(
                "{name}: Cargo `Compiling` lines differ as a multiset ({} / {})",
                pc.len(),
                qc.len()
            ));
        }
    }
    if p.len() != q.len() {
        let at = p
            .iter()
            .zip(&q)
            .position(|(v, w)| v != w)
            .unwrap_or(p.len().min(q.len()));
        c.differences.push(format!(
            "{name}: {} / {} lines; first difference at line {}: {:?} / {:?}",
            p.len(),
            q.len(),
            at + 1,
            p.get(at).unwrap_or(&"<end>"),
            q.get(at).unwrap_or(&"<end>")
        ));
        return;
    }
    let mut listed = 0;
    for (index, (v, w)) in p.iter().zip(&q).enumerate() {
        if v == w {
            continue;
        }
        if let Some(variance) = stream_variance(v, a, w, b) {
            c.vary(&variance);
            continue;
        }
        listed += 1;
        if listed <= MAX_LISTED {
            c.differences.push(format!(
                "{name} line {} (Compiling lines aside): {v:?} / {w:?}",
                index + 1
            ));
        }
    }
    if listed > MAX_LISTED {
        c.differences.push(format!(
            "{name}: {} more differing lines",
            listed - MAX_LISTED
        ));
    }
}

#[cfg(test)]
mod tests;
