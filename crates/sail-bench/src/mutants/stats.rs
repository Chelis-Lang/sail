//! The mutation suite's report: soundness counts and edit-loop timing with
//! medians and bootstrap confidence intervals, from a run's
//! `samples.jsonl` (ADR 0063).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;

use crate::cold::median;

/// Resamples for the bootstrap intervals.
const RESAMPLES: usize = 2000;

/// A deterministic generator (`SplitMix64`) so a report regenerates
/// identically.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        let n64 = u64::try_from(n.max(1)).unwrap_or(1);
        usize::try_from(self.next() % n64).unwrap_or(0)
    }
}

/// The 95% percentile bootstrap interval of `stat` over `n` items,
/// resampled with replacement.
#[must_use]
pub fn bootstrap(n: usize, stat: &dyn Fn(&[usize]) -> f64) -> Option<(f64, f64)> {
    if n < 2 {
        return None;
    }
    let mut rng = Rng(0x5a11_0062);
    let mut values: Vec<f64> = (0..RESAMPLES)
        .map(|_| {
            let idx: Vec<usize> = (0..n).map(|_| rng.below(n)).collect();
            stat(&idx)
        })
        .filter(|v| v.is_finite())
        .collect();
    values.sort_by(f64::total_cmp);
    let at = |q: f64| {
        let len = values.len();
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "an index into at most RESAMPLES values"
        )]
        let i = ((len.saturating_sub(1)) as f64 * q).round() as usize;
        values.get(i).copied()
    };
    Some((at(0.025)?, at(0.975)?))
}

#[allow(clippy::cast_precision_loss, reason = "counts far below 2^52")]
fn f(n: usize) -> f64 {
    n as f64
}

/// One mutant's samples.
#[derive(Default)]
struct Mutant {
    class: String,
    stock: Vec<f64>,
    sail: Vec<f64>,
    hits: u64,
    replies: u64,
    restored: usize,
    runs: usize,
}

/// Timing summary of a set of mutants.
struct Row {
    n: usize,
    stock_med: f64,
    sail_med: f64,
    ratio: f64,
    ratio_ci: Option<(f64, f64)>,
    geo: f64,
    geo_ci: Option<(f64, f64)>,
    served: Option<f64>,
    restored: f64,
}

fn row(ms: &[&Mutant]) -> Option<Row> {
    let stock: Vec<f64> = ms.iter().filter_map(|m| median(&m.stock)).collect();
    let sail: Vec<f64> = ms.iter().filter_map(|m| median(&m.sail)).collect();
    if stock.is_empty() || stock.len() != sail.len() {
        return None;
    }
    let ratio_of = |idx: &[usize]| {
        let s: f64 = idx.iter().filter_map(|&i| stock.get(i)).sum();
        let t: f64 = idx.iter().filter_map(|&i| sail.get(i)).sum();
        s / t
    };
    let logs: Vec<f64> = stock.iter().zip(&sail).map(|(s, t)| (s / t).ln()).collect();
    let geo_of =
        |idx: &[usize]| (idx.iter().filter_map(|&i| logs.get(i)).sum::<f64>() / f(idx.len())).exp();
    let all: Vec<usize> = (0..stock.len()).collect();
    let hits: u64 = ms.iter().map(|m| m.hits).sum();
    let replies: u64 = ms.iter().map(|m| m.replies).sum();
    let runs: usize = ms.iter().map(|m| m.runs).sum();
    let restored: usize = ms.iter().map(|m| m.restored).sum();
    #[allow(clippy::cast_precision_loss, reason = "reply counts far below 2^52")]
    let served = (replies > 0).then(|| hits as f64 / replies as f64);
    Some(Row {
        n: stock.len(),
        stock_med: median(&stock).unwrap_or(0.0),
        sail_med: median(&sail).unwrap_or(0.0),
        ratio: ratio_of(&all),
        ratio_ci: bootstrap(all.len(), &ratio_of),
        geo: geo_of(&all),
        geo_ci: bootstrap(all.len(), &geo_of),
        served,
        restored: f(restored) / f(runs.max(1)),
    })
}

fn ci(c: Option<(f64, f64)>) -> String {
    c.map_or_else(|| "n/a".to_owned(), |(a, b)| format!("{a:.2}–{b:.2}"))
}

fn pct(x: Option<f64>) -> String {
    x.map_or_else(|| "n/a".to_owned(), |v| format!("{:.0}%", v * 100.0))
}

fn get_f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}

fn get_s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// The Markdown report of one run directory.
///
/// # Errors
///
/// A message if its samples cannot be read.
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one table after another, in report order; `t` is the text, `s` a sample"
)]
pub fn report_dir(dir: &Path) -> Result<String, String> {
    let header: Value = std::fs::read_to_string(dir.join("header.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let text = std::fs::read_to_string(dir.join("samples.jsonl"))
        .map_err(|e| format!("cannot read {}: {e}", dir.join("samples.jsonl").display()))?;
    let samples: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let mut t = String::new();
    let _ = writeln!(
        t,
        "## {} ({} mutants, {} rounds)\n",
        get_s(&header, "unit"),
        header
            .get("mutants")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        header.get("rounds").and_then(Value::as_u64).unwrap_or(0)
    );
    let _ = writeln!(
        t,
        "Sail `{}`; stock Verus {}; {}\n",
        get_s(&header, "sail_version"),
        get_s(&header, "stock_verus"),
        get_s(&header, "machine")
    );
    // Soundness.
    let mut judged: BTreeMap<&str, usize> = BTreeMap::new();
    let mut fv_samples = 0;
    let mut fv_functions = 0;
    let mut mismatches = 0;
    let mut accepted = 0;
    for s in &samples {
        if s.get("accepted")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
        {
            accepted += 1;
        }
        *judged.entry(get_s(s, "judgement")).or_default() += 1;
        let fv = s
            .get("false_verified")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if fv > 0 {
            fv_samples += 1;
            fv_functions += fv;
        }
        if s.get("expected_mismatch") == Some(&Value::Bool(true)) {
            mismatches += 1;
        }
    }
    let _ = writeln!(t, "### Soundness\n");
    let _ = writeln!(
        t,
        "| Comparisons | Same | Unstable | Different | False verified (steps / functions) | Infra | Stock outcome not as catalogued | Same with an accepted divergence |"
    );
    let _ = writeln!(t, "|---:|---:|---:|---:|---:|---:|---:|---:|");
    let _ = writeln!(
        t,
        "| {} | {} | {} | {} | {fv_samples} / {fv_functions} | {} | {mismatches} | {accepted} |\n",
        samples.len(),
        judged.get("same").unwrap_or(&0),
        judged.get("unstable").unwrap_or(&0),
        judged.get("different").unwrap_or(&0),
        judged.get("infra").unwrap_or(&0),
    );
    let _ = writeln!(
        t,
        "| Round | Daemons seen | Revert: judged / own answers / served | Forced revert: judged / own answers / served |"
    );
    let _ = writeln!(t, "|---:|---:|---|---|");
    let rounds: BTreeSet<u64> = samples
        .iter()
        .filter_map(|s| s.get("round").and_then(Value::as_u64))
        .collect();
    for r in &rounds {
        let in_round: Vec<&Value> = samples
            .iter()
            .filter(|s| s.get("round").and_then(Value::as_u64) == Some(*r))
            .collect();
        let pids: BTreeSet<u64> = in_round
            .iter()
            .filter_map(|s| s.get("daemon_pid").and_then(Value::as_u64))
            .collect();
        let check = |phase: &str| {
            in_round
                .iter()
                .find(|s| get_s(s, "phase") == phase)
                .map_or_else(
                    || "not run".to_owned(),
                    |s| {
                        format!(
                            "{} / {} / {}",
                            get_s(s, "judgement"),
                            s.get("own_answers")
                                .and_then(Value::as_bool)
                                .map_or("n/a", |b| if b { "yes" } else { "NO" }),
                            get_s(s, "served")
                        )
                    },
                )
        };
        let _ = writeln!(
            t,
            "| {r} | {} | {} | {} |",
            pids.len(),
            check("revert"),
            check("revert-forced")
        );
    }
    // Edit loop.
    let mut mutants: BTreeMap<String, Mutant> = BTreeMap::new();
    let mut per_round: BTreeMap<u64, (f64, f64)> = BTreeMap::new();
    for s in samples
        .iter()
        .filter(|s| get_s(s, "phase") == "mutant" && get_s(s, "judgement") != "infra")
    {
        let (Some(a), Some(b)) = (get_f(s, "stock_s"), get_f(s, "sail_s")) else {
            continue;
        };
        let m = mutants.entry(get_s(s, "id").to_owned()).or_default();
        get_s(s, "class").clone_into(&mut m.class);
        m.stock.push(a);
        m.sail.push(b);
        let c = s.get("cache").cloned().unwrap_or(Value::Null);
        let hits = c.get("hits").and_then(Value::as_u64).unwrap_or(0);
        m.hits += hits;
        m.replies += c.get("gets").and_then(Value::as_u64).unwrap_or(0);
        m.runs += 1;
        if get_s(s, "served") == "retained" {
            m.restored += 1;
        }
        let r = per_round
            .entry(s.get("round").and_then(Value::as_u64).unwrap_or(0))
            .or_default();
        r.0 += a;
        r.1 += b;
    }
    let _ = writeln!(
        t,
        "\n### Edit loop (per mutant: stock `cargo verus verify` vs `sail verify`)\n"
    );
    let _ = writeln!(
        t,
        "Speedup is stock/Sail. *Total* is the ratio of summed per-mutant medians (the loop's wall time); *geomean* is the geometric mean of per-mutant ratios. Intervals are 95% bootstrap intervals over mutants ({RESAMPLES} resamples). *Served* is the cache hit share of solver lookup requests (including misses whose results were not cached); *restored* is the share of Sail runs with a newly published retained-proof receipt. No solver sessions alone does not establish instant revert.\n"
    );
    let _ = writeln!(
        t,
        "| Class | Mutants | Stock median (s) | Sail median (s) | Total speedup [95% CI] | Geomean speedup [95% CI] | Served | Restored |"
    );
    let _ = writeln!(t, "|---|---:|---:|---:|---|---|---:|---:|");
    let mut classes: Vec<String> = mutants.values().map(|m| m.class.clone()).collect();
    classes.sort();
    classes.dedup();
    let mut lines: Vec<(String, Vec<&Mutant>)> = classes
        .iter()
        .map(|c| {
            (
                c.clone(),
                mutants.values().filter(|m| &m.class == c).collect(),
            )
        })
        .collect();
    lines.push(("**all**".to_owned(), mutants.values().collect()));
    for (name, ms) in &lines {
        if let Some(r) = row(ms) {
            let _ = writeln!(
                t,
                "| {name} | {} | {:.1} | {:.1} | {:.2}× [{}] | {:.2}× [{}] | {} | {} |",
                r.n,
                r.stock_med,
                r.sail_med,
                r.ratio,
                ci(r.ratio_ci),
                r.geo,
                ci(r.geo_ci),
                pct(r.served),
                pct(Some(r.restored))
            );
        }
    }
    let _ = writeln!(
        t,
        "\n| Round | Stock total (s) | Sail total (s) | Speedup |"
    );
    let _ = writeln!(t, "|---:|---:|---:|---:|");
    for (r, (a, b)) in &per_round {
        let _ = writeln!(t, "| {r} | {a:.0} | {b:.0} | {:.2}× |", a / b);
    }
    let _ = writeln!(
        t,
        "\n| Step | Stock median (s) | Sail median (s) | Speedup | Served |"
    );
    let _ = writeln!(t, "|---|---:|---:|---:|---|");
    for phase in ["cold", "revert", "revert-forced"] {
        let ps: Vec<&Value> = samples
            .iter()
            .filter(|s| get_s(s, "phase") == phase && get_s(s, "judgement") != "infra")
            .collect();
        let a: Vec<f64> = ps.iter().filter_map(|s| get_f(s, "stock_s")).collect();
        let b: Vec<f64> = ps.iter().filter_map(|s| get_f(s, "sail_s")).collect();
        let served: Vec<&str> = ps.iter().map(|s| get_s(s, "served")).collect();
        if let (Some(a), Some(b)) = (median(&a), median(&b)) {
            let _ = writeln!(
                t,
                "| {phase} | {a:.1} | {b:.1} | {:.2}× | {} |",
                a / b,
                served.join(", ")
            );
        }
    }
    t.push('\n');
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_brackets_a_constant() {
        let xs = [2.0f64; 10];
        let mean =
            |idx: &[usize]| idx.iter().filter_map(|&i| xs.get(i)).sum::<f64>() / f(idx.len());
        assert_eq!(bootstrap(xs.len(), &mean), Some((2.0, 2.0)));
        assert_eq!(bootstrap(1, &mean), None);
    }

    #[test]
    fn reports_a_run() {
        let dir = std::env::temp_dir().join(format!("sail-mutants-report-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut lines = Vec::new();
        for (round, id, stock, sail) in [
            (0, "a", 10.0, 5.0),
            (0, "b", 4.0, 4.0),
            (1, "a", 12.0, 6.0),
            (1, "b", 4.0, 2.0),
        ] {
            lines.push(serde_json::json!({
                "round": round, "phase": "mutant", "id": id, "class": "arith", "judgement": "same",
                "stock_s": stock, "sail_s": sail, "false_verified": [], "served": "mixed",
                "cache": {"gets": 4, "hits": 3, "puts": 1}, "daemon_pid": 7
            }).to_string());
        }
        lines.push(serde_json::json!({"round": 0, "phase": "revert", "judgement": "same", "own_answers": true,
            "stock_s": 9.0, "sail_s": 1.0, "served": "retained", "false_verified": [], "daemon_pid": 7}).to_string());
        let _ = std::fs::write(dir.join("samples.jsonl"), lines.join("\n"));
        let r = report_dir(&dir).unwrap_or_default();
        assert!(r.contains("| 5 | 5 | 0 | 0 | 0 / 0 | 0 | 0 | 0 |"), "{r}");
        assert!(r.contains("same / yes / retained"), "{r}");
        assert!(r.contains("| **all** | 2 |"), "{r}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
