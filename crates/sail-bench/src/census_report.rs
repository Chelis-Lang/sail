//! The `docs/benchmarks.md` section for a fragility census result file.

use std::fmt::Write as _;

use sail_portfolio::fragility::{Bucket, CheckFragility, Summary};
use sail_portfolio::stats::FloatDistribution;

use crate::fragility::{CensusFile, CensusUnit};

/// What a census section measures.
const INTRO: &str = "Each unit's solver sessions are captured from plain Verus through `sail-z3` in pass-through, then every session is replayed in a fresh pinned Z3 under each configuration: the unmodified session (`baseline`, Z3's default seed) and reseeds that set `smt.random_seed` and `sat.random_seed` after Verus's option prelude (ADR 0018). Each check is measured after its exact session prefix. *Proved* means `unsat`. *Used* is a check's largest `rlimit-count` delta over the configurations, as a share of the `:rlimit` in force. A check is *unstable* when some configurations prove it and others do not. Nothing here changes a verdict.";

fn f3(x: Option<f64>) -> String {
    x.map_or_else(|| "n/a".to_owned(), |v| format!("{v:.3}"))
}

fn pct(x: Option<f64>) -> String {
    match x {
        None => "n/a".to_owned(),
        Some(v) if v > 0.0 && v < 0.0001 => format!("{:.5}%", v * 100.0),
        Some(v) => format!("{:.3}%", v * 100.0),
    }
}

fn dist_row(out: &mut String, label: &str, d: &FloatDistribution, fmt: fn(Option<f64>) -> String) {
    let _ = writeln!(
        out,
        "| {label} | {} | {} | {} | {} | {} | {} |",
        d.count,
        d.zeros,
        fmt(d.p50),
        fmt(d.p90),
        fmt(d.p99),
        fmt(d.max)
    );
}

fn bucket_label(b: &Bucket) -> String {
    let p = |v: f64| {
        if v == 0.0 {
            "0".to_owned()
        } else if v < 0.01 {
            format!("{}%", v * 100.0)
        } else {
            format!("{:.0}%", v * 100.0)
        }
    };
    match b.hi {
        Some(hi) => format!("[{}, {})", p(b.lo), p(hi)),
        None => format!("≥ {}", p(b.lo)),
    }
}

fn unit_row(out: &mut String, u: &CensusUnit) {
    let _ = writeln!(
        out,
        "| `{}` | {} | {} | {} | {} | {} | {:.3} |",
        u.id,
        u.checks,
        u.stability.as_deref().unwrap_or("n/a"),
        pct(u.max_used_fraction),
        f3(u.max_cv),
        if u.failed_functions.is_empty() {
            "-".to_owned()
        } else {
            u.failed_functions.len().to_string()
        },
        u.score
    );
}

fn check_line(out: &mut String, c: &CheckFragility) {
    let effort = c.effort.as_ref().map_or_else(
        || "n/a".to_owned(),
        |e| format!("{}–{} (CV {})", e.min, e.max, f3(e.cv)),
    );
    let answers: Vec<String> = c
        .answer_counts
        .iter()
        .map(|(k, v)| format!("{k} ×{v}"))
        .collect();
    let _ = writeln!(
        out,
        "| `{}` #{} | {} | {} | {} | {}/{} | {} | {} | {} |",
        c.session,
        c.seq,
        c.function.as_deref().unwrap_or("-"),
        c.stability.name(),
        c.answers
            .first()
            .and_then(Option::as_deref)
            .unwrap_or("n/a"),
        c.proved,
        c.answers.len(),
        effort,
        pct(c.max_used_fraction),
        answers.join(", ")
    );
}

/// Appends the section for census `c`, named `name`.
pub fn render(out: &mut String, name: &str, c: &CensusFile) {
    let s = &c.summary;
    let _ = writeln!(out, "## `{name}`: fragility census, {} tier\n", c.tier);
    let _ = writeln!(out, "{INTRO}\n");
    let _ = writeln!(
        out,
        "- Verus {}, Z3 {}, sail `{}`; {} configurations ({}){}",
        c.verus_release,
        c.z3_version,
        c.sail_commit,
        c.configs.len(),
        c.configs.join(", "),
        if c.extra_verus_args.is_empty() {
            String::new()
        } else {
            format!("; extra Verus arguments `{}`", c.extra_verus_args.join(" "))
        }
    );
    let machine: Vec<String> = c.machine.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    let _ = writeln!(out, "- Machine: {}", machine.join("; "));
    let sessions: usize = c.units.iter().map(|u| u.sessions).sum();
    let _ = writeln!(
        out,
        "- {} units, {} solver sessions, {} checks; capture {:.0} s, replays {:.0} s of solver time",
        c.units.len(),
        sessions,
        s.checks,
        crate::stats::as_f64(c.capture_ms) / 1000.0,
        crate::stats::as_f64(c.replay_ms) / 1000.0
    );
    let b = &s.baseline_vs_recorded;
    let _ = writeln!(
        out,
        "- Baseline replay versus the capture: {} of {} answers and {} of {} `rlimit` deltas differ",
        b.answer_mismatches, b.answers_compared, b.rlimit_mismatches, b.rlimit_compared
    );
    let _ = writeln!(
        out,
        "- Infrastructure: {} incomplete runs, {} runs with unpaired batches (never counted as answers)\n",
        c.infra.len(),
        c.unpaired_runs.len()
    );

    distributions(out, s);
    let u = &s.unknown;
    let reasons: Vec<String> = u
        .reasons
        .iter()
        .map(|(k, v)| format!("`{k}` ×{v}"))
        .collect();
    let _ = writeln!(
        out,
        "`unknown` answers: {} checks answered `unknown` under some configuration, {} of them proved under another. {} `unknown` answers reached the limit; {} have no reason on record (a replay sends only the captured follow-ups). Share of the limit used by an `unknown`: p50 {}, p99 {}, max {}; largest with a recorded reason other than resource exhaustion: {}. Reasons: {}.\n",
        u.checks,
        u.proved_elsewhere,
        u.at_limit,
        u.without_reason,
        pct(u.used_fraction.p50),
        pct(u.used_fraction.p99),
        pct(u.used_fraction.max),
        pct(u.max_used_fraction_not_exhausted),
        if reasons.is_empty() {
            "none".to_owned()
        } else {
            reasons.join(", ")
        }
    );

    groups(out, c);
}

/// The stability, distribution and histogram tables.
fn distributions(out: &mut String, s: &Summary) {
    let _ = writeln!(out, "| Stability | Checks |\n|---|---:|");
    for (k, v) in &s.by_class {
        let _ = writeln!(out, "| {k} | {v} |");
    }
    let _ = writeln!(
        out,
        "| answer text changed (any) | {} |\n",
        s.answer_unstable
    );

    let _ = writeln!(
        out,
        "| Per-check distribution | Checks | Zero | p50 | p90 | p99 | Max |\n|---|---:|---:|---:|---:|---:|---:|"
    );
    dist_row(out, "effort CV across configurations", &s.effort_cv, f3);
    dist_row(out, "effort max/min", &s.effort_ratio, f3);
    dist_row(
        out,
        "largest share of the limit used",
        &s.used_fraction,
        pct,
    );
    dist_row(out, "fragility score", &s.score, f3);
    out.push('\n');

    let _ = writeln!(
        out,
        "| Largest share of the limit used | Checks | With an `unknown` | Unstable |\n|---|---:|---:|---:|"
    );
    for bucket in &s.used_fraction_histogram {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            bucket_label(bucket),
            bucket.checks,
            bucket.with_unknown,
            bucket.unstable
        );
    }
    out.push('\n');
}

/// The unit, function and check tables.
fn groups(out: &mut String, c: &CensusFile) {
    let mut units: Vec<&CensusUnit> = c.units.iter().filter(|u| u.checks > 0).collect();
    units.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    let _ = writeln!(
        out,
        "Most fragile units (top 15 by score):\n\n| Unit | Checks | Worst class | Max used | Max CV | Failing functions | Score |\n|---|---:|---|---:|---:|---:|---:|"
    );
    for unit in units.iter().take(15) {
        unit_row(out, unit);
    }
    out.push('\n');

    if !c.functions.is_empty() {
        let _ = writeln!(
            out,
            "Attributed functions:\n\n| Function | Checks | Worst class | Max used | Max CV | Score |\n|---|---:|---|---:|---:|---:|"
        );
        for g in &c.functions {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {:.3} |",
                g.name,
                g.checks,
                g.stability.name(),
                pct(g.max_used_fraction),
                f3(g.max_cv),
                g.score
            );
        }
        out.push('\n');
    }

    let mut shown: Vec<&CheckFragility> = c.notable.iter().collect();
    shown.sort_by(|a, b| b.score.total_cmp(&a.score));
    let _ = writeln!(
        out,
        "Most fragile checks (top 10 by score):\n\n| Check | Function | Class | Baseline | Proved | Effort (rlimit units) | Max used | Answers |\n|---|---|---|---|---:|---:|---:|---|"
    );
    for check in shown.iter().take(10) {
        check_line(out, check);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragility::CensusUnit;
    use sail_portfolio::Identity;
    use sail_portfolio::fragility::{BaselineCheck, analyse_check, group, summarise};
    use sail_portfolio::transcript::CheckObservation;

    fn obs(answer: &str, delta: u64) -> CheckObservation {
        CheckObservation {
            command_index: 0,
            answer: Some(answer.as_bytes().to_vec()),
            rlimit_before: Some(0),
            rlimit_delta: Some(delta),
            followups: vec![],
        }
    }

    #[test]
    fn renders_a_census_section() {
        let (p, q, u) = (obs("unsat", 10), obs("unsat", 700), obs("unknown", 900));
        let mut checks = vec![
            analyse_check("p/u.rs/1-1", 0, None, Some(1000), &[Some(&p), Some(&q)]),
            analyse_check("p/u.rs/1-1", 1, None, Some(1000), &[Some(&p), Some(&u)]),
        ];
        checks[1].function = Some("u::f".to_owned());
        let file = CensusFile {
            schema_version: 1,
            workload: "fragility".to_owned(),
            tier: "fast".to_owned(),
            verus_release: "v".to_owned(),
            z3_version: "4.16.0".to_owned(),
            sail_commit: "abc".to_owned(),
            started_unix: 0,
            machine: std::collections::BTreeMap::new(),
            solver: Identity {
                path: String::new(),
                version: None,
                sha256: None,
            },
            configs: vec!["baseline".to_owned(), "seed=1".to_owned()],
            near_limit: 0.5,
            extra_verus_args: vec![],
            units: vec![CensusUnit {
                id: "p/u.rs".to_owned(),
                exit_code: Some(0),
                functions: 2,
                failed_functions: vec![],
                sessions: 1,
                checks: 2,
                stability: Some("unstable".to_owned()),
                by_class: std::collections::BTreeMap::new(),
                score: 1.0,
                max_used_fraction: Some(0.9),
                max_cv: Some(0.5),
                error: None,
            }],
            summary: summarise(&checks, 0.5, BaselineCheck::default()),
            functions: group(&checks, |c| c.function.clone()),
            notable: checks,
            infra: vec![],
            unpaired_runs: vec![],
            capture_ms: 1000,
            replay_ms: 2000,
        };
        let mut md = String::new();
        render(&mut md, "fragility-x", &file);
        assert!(
            md.contains("## `fragility-x`: fragility census, fast tier"),
            "{md}"
        );
        assert!(md.contains("| unstable | 1 |"), "{md}");
        assert!(md.contains("| [50%, 75%) | 1 | 0 | 0 |"), "{md}");
        assert!(md.contains("| [75%, 100%) | 1 | 1 | 1 |"), "{md}");
        assert!(md.contains("| `u::f` | 1 | unstable |"), "{md}");
        assert!(md.contains("| `p/u.rs` | 2 | unstable | 90.000% |"), "{md}");
        assert!(md.contains("1 of them proved under another"), "{md}");
        // The file round-trips through JSON.
        let json = serde_json::to_string(&file).unwrap_or_default();
        let back: Result<CensusFile, _> = serde_json::from_str(&json);
        assert_eq!(back.ok(), Some(file));
    }
}
