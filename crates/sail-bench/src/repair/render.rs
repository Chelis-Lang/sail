//! The `docs/benchmarks.md` section for a repair result file.

use std::fmt::Write as _;

use super::judge::{self, Verdict, p95};
use super::{Arm, RepairFile, Sample};
use crate::cold::median;

fn ms(x: f64) -> String {
    if x >= 10_000.0 {
        format!("{:.1} s", x / 1000.0)
    } else {
        format!("{x:.0} ms")
    }
}

fn opt_ms(x: Option<f64>) -> String {
    x.map_or_else(|| "n/a".to_owned(), ms)
}

/// `median (min–max)`, or `n/a`.
fn spread(xs: &[f64]) -> String {
    let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    match median(xs) {
        Some(m) if xs.len() > 1 => format!("{} ({}–{})", ms(m), ms(lo), ms(hi)),
        Some(m) => ms(m),
        None => "n/a".to_owned(),
    }
}

#[allow(clippy::cast_precision_loss)] // byte counts, for display
fn mb(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1_048_576.0)
}

fn ratio(a: Option<f64>, b: Option<f64>) -> String {
    match (a, b) {
        (Some(a), Some(b)) if a > 0.0 && b > 0.0 => format!("{:.2}×", a / b),
        _ => "n/a".to_owned(),
    }
}

fn measured<'a>(
    f: &'a RepairFile,
    project: &'a str,
    cycle: &'a str,
    arm: Arm,
) -> impl Iterator<Item = &'a Sample> {
    f.samples.iter().filter(move |s| {
        s.project == project && s.cycle == cycle && s.arm == Some(arm) && s.round.is_some()
    })
}

fn walls<'a>(xs: impl Iterator<Item = &'a Sample>) -> Vec<f64> {
    xs.map(|s| s.wall_ms).collect()
}

fn actionable<'a>(xs: impl Iterator<Item = &'a Sample>) -> Vec<f64> {
    xs.filter_map(|s| s.first_actionable_ms).collect()
}

fn round_totals(totals: &[Option<f64>]) -> Vec<f64> {
    totals.iter().filter_map(|t| *t).collect()
}

/// Renders the section for one repair result file.
pub fn render(out: &mut String, name: &str, f: &RepairFile) {
    let verdicts = judge::judge(f);
    let _ = writeln!(
        out,
        "## `{name}`: M7 repair acceptance (M6 focused and full checks vs one M7 repair batch)\n"
    );
    let arm = |a: Arm| {
        f.arms.get(&a).map_or_else(
            || "unknown".to_owned(),
            |b| {
                format!(
                    "`{}` (sail sha256 `{}`)",
                    b.commit,
                    b.sail_sha256.get(..12).unwrap_or(&b.sail_sha256)
                )
            },
        )
    };
    let _ = writeln!(
        out,
        "- Environment: {}; M6 {}; M7 {}; Verus {}, Z3 {}",
        f.environment,
        arm(Arm::M6),
        arm(Arm::M7),
        f.verus_release,
        f.z3_version
    );
    let scope = match &f.cycle_filter {
        Some(filter) => format!("cycles matching `{filter}`"),
        None => "every cycle".to_owned(),
    };
    let _ = writeln!(
        out,
        "- {} round(s) requested over {} of {}; gate: at least {} complete rounds and {}× median whole-loop speedup per catalogue",
        f.rounds,
        scope,
        if f.projects.is_empty() {
            "no project".to_owned()
        } else {
            f.projects.join(", ")
        },
        f.minimum_rounds,
        f.minimum_median_speedup
    );
    let machine: Vec<String> = f.machine.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    let _ = writeln!(out, "- Machine: {}\n", machine.join("; "));
    let _ = writeln!(
        out,
        "Every sample is recorded, then judged (`docs/bench/m7-repair-acceptance.md`). A whole loop is one arm's sum of cycle wall times in one round; arm order alternates by round. Failing samples stay in the totals and fail the gate.\n"
    );
    let _ = writeln!(
        out,
        "| Catalogue | Complete rounds | M6 whole loop: median (min–max) | M7 whole loop: median (min–max) | Speedup | Failing samples | Gate |\n|---|---:|---:|---:|---:|---:|---|"
    );
    for v in &verdicts {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            v.project,
            v.complete_rounds,
            spread(&round_totals(&v.m6_round_totals)),
            spread(&round_totals(&v.m7_round_totals)),
            ratio(v.m6_median_ms, v.m7_median_ms),
            v.failing.len(),
            if v.passed { "**pass**" } else { "**fail**" }
        );
    }
    out.push('\n');
    for v in &verdicts {
        render_catalogue(out, f, v);
    }
    let failed_warmups = f
        .warmups
        .iter()
        .filter(|s| {
            s.phases
                .iter()
                .any(|p| p.error.is_some() || p.timed_out || p.exit_code.is_none())
        })
        .count();
    let _ = writeln!(
        out,
        "Warm-ups (recorded, not gated): {} invocation group(s), {} total, {} with an infrastructure failure.",
        f.warmups.len(),
        ms(f.warmups.iter().map(|s| s.wall_ms).sum()),
        failed_warmups
    );
    for note in &f.notes {
        let _ = writeln!(out, "\n- Note: {note}");
    }
}

fn render_catalogue(out: &mut String, f: &RepairFile, v: &Verdict) {
    let _ = writeln!(out, "### `{}`\n", v.project);
    if v.reasons.is_empty() {
        let _ = writeln!(out, "Gate passed.\n");
    } else {
        let _ = writeln!(out, "Gate failed:\n");
        for r in &v.reasons {
            let _ = writeln!(out, "- {r}");
        }
        out.push('\n');
    }
    let _ = writeln!(
        out,
        "| Cycle | Kind | M6 wall | M7 wall | Speedup | M6 first actionable | M7 first actionable | M7 drivers | M7 full evidence / accepted | Cache served / solved (M6; M7) | M7 preparation hits / misses | Peak RSS (M6; M7) |\n|---|---|---:|---:|---:|---:|---:|---:|---|---|---:|---|"
    );
    let cycles = f
        .catalogue
        .iter()
        .find(|c| c.project == v.project)
        .map(|c| c.cycles.clone())
        .unwrap_or_default();
    let mut all = [Vec::new(), Vec::new()];
    let mut first = [Vec::new(), Vec::new()];
    for cycle in &cycles {
        let [(w6, a6), (w7, a7)] = cycle_row(out, f, &v.project, cycle);
        for (i, (w, a)) in [(w6, a6), (w7, a7)].into_iter().enumerate() {
            if let (Some(all), Some(first)) = (all.get_mut(i), first.get_mut(i)) {
                all.extend(w);
                first.extend(a);
            }
        }
    }
    let [all6, all7] = all;
    let [first6, first7] = first;
    let _ = writeln!(
        out,
        "\nPer-cycle tails over all rounds: M6 wall p95 {} / max {}, first actionable p95 {}; M7 wall p95 {} / max {}, first actionable p95 {}.\n",
        opt_ms(p95(&all6)),
        opt_ms(all6.iter().copied().reduce(f64::max)),
        opt_ms(p95(&first6)),
        opt_ms(p95(&all7)),
        opt_ms(all7.iter().copied().reduce(f64::max)),
        opt_ms(p95(&first7)),
    );
    if !v.failing.is_empty() {
        let _ = writeln!(out, "Failing samples:\n");
        for s in &v.failing {
            let text: Vec<String> = s
                .violations
                .iter()
                .map(|x| format!("{} {}", x.code, x.message))
                .collect();
            let _ = writeln!(
                out,
                "- round {}, `{}`, {}: {}",
                s.round,
                s.cycle,
                s.arm.name(),
                text.join("; ")
            );
        }
        out.push('\n');
    }
}

/// Writes one cycle's table row; returns each arm's walls and first-actionable times.
fn cycle_row(
    out: &mut String,
    f: &RepairFile,
    project: &str,
    cycle: &str,
) -> [(Vec<f64>, Vec<f64>); 2] {
    let m6 = || measured(f, project, cycle, Arm::M6);
    let m7 = || measured(f, project, cycle, Arm::M7);
    let kind = m6().chain(m7()).next().map_or("", |s| s.kind.as_str());
    let (w6, w7) = (walls(m6()), walls(m7()));
    let (a6, a7) = (actionable(m6()), actionable(m7()));
    let drivers: Vec<String> = m7()
        .map(|s| {
            s.facts
                .compiler_invocations
                .map_or_else(|| "?".to_owned(), |n| n.to_string())
        })
        .collect();
    let status: Vec<String> = m7()
        .map(|s| {
            format!(
                "{}/{}",
                s.facts.full_currentness.as_deref().unwrap_or("?"),
                s.facts
                    .accepted
                    .map_or("?", |a| if a { "yes" } else { "no" })
            )
        })
        .collect();
    let cache = |xs: Vec<&Sample>| {
        let served: u64 = xs
            .iter()
            .filter_map(|s| s.cache.as_ref())
            .map(|c| c.hits)
            .sum();
        let solved: u64 = xs
            .iter()
            .filter_map(|s| s.cache.as_ref())
            .map(|c| c.puts)
            .sum();
        if xs.iter().any(|s| s.cache.is_some()) {
            format!("{served} / {solved}")
        } else {
            "n/a".to_owned()
        }
    };
    let prep: Vec<String> = m7()
        .map(|s| {
            s.preparation.as_ref().map_or_else(
                || "n/a".to_owned(),
                |p| format!("{} / {}", p.batch_hits + p.persistent_hits, p.misses),
            )
        })
        .collect();
    let rss = |xs: Vec<&Sample>| {
        xs.iter()
            .filter_map(|s| s.peak_rss_bytes)
            .max()
            .map_or_else(|| "n/a".to_owned(), mb)
    };
    let _ = writeln!(
        out,
        "| `{cycle}` | {kind} | {} | {} | {} | {} | {} | {} | {} | {}; {} | {} | {}; {} |",
        spread(&w6),
        spread(&w7),
        ratio(median(&w6), median(&w7)),
        spread(&a6),
        spread(&a7),
        if drivers.is_empty() {
            "n/a".to_owned()
        } else {
            drivers.join(", ")
        },
        if status.is_empty() {
            "n/a".to_owned()
        } else {
            status.join(", ")
        },
        cache(m6().collect()),
        cache(m7().collect()),
        if prep.is_empty() {
            "n/a".to_owned()
        } else {
            prep.join(", ")
        },
        rss(m6().collect()),
        rss(m7().collect()),
    );
    [(w6, a6), (w7, a7)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repair::{CatalogueEntry, Facts, Phase};

    #[test]
    fn renders_verdicts_and_failing_samples() {
        let mut f = RepairFile {
            workload: "repair".to_owned(),
            environment: "native".to_owned(),
            rounds: 1,
            minimum_rounds: 3,
            minimum_median_speedup: 2.0,
            projects: vec!["p".to_owned()],
            catalogue: vec![CatalogueEntry {
                project: "p".to_owned(),
                cycles: vec!["c".to_owned()],
            }],
            ..RepairFile::default()
        };
        for (arm, wall) in [(Arm::M6, 4000.0), (Arm::M7, 1000.0)] {
            f.samples.push(Sample {
                project: "p".to_owned(),
                cycle: "c".to_owned(),
                kind: "body".to_owned(),
                expected: "verified".to_owned(),
                arm: Some(arm),
                round: Some(0),
                wall_ms: wall,
                phases: vec![Phase {
                    exit_code: Some(0),
                    ..Phase::default()
                }],
                facts: Facts {
                    full_currentness: Some("current".to_owned()),
                    compiler_invocations: Some(2),
                    ..Facts::default()
                },
                ..Sample::default()
            });
        }
        let mut md = String::new();
        render(&mut md, "repair-x", &f);
        assert!(
            md.contains("| `p` | 1 | 4000 ms | 1000 ms | 4.00× | 2 | **fail** |"),
            "{md}"
        );
        assert!(md.contains("SAIL7108: 1 complete round(s)"), "{md}");
        assert!(
            md.contains("round 0, `c`, m7: SAIL7103 2 compiler drivers, not one"),
            "{md}"
        );
        assert!(
            md.contains("| `c` | body | 4000 ms | 1000 ms | 4.00× |"),
            "{md}"
        );
    }
}
