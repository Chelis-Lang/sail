//! Cache hit-rate estimates from traces, before any cache exists.
//!
//! Given traces from a *base* run and a *then* run, this counts the checks of
//! the then run whose key also occurs in the base run. That is how many a
//! cache filled by the base run would have answered. It does this for key
//! v2 (whole process prefix, the default-mode key, ADR 0008) and for key v1
//! (stack only, the candidate opt-in key, ADR 0010). It also sums the solver
//! time of the hits, which is the time that would have been saved.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::error::BenchError;
use crate::trace::{self, CheckRecord};

/// Hit counts for one key version.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct KeyHits {
    /// Checks in the then run that have this key.
    pub keyed: u64,
    /// Of those, checks whose key occurs in the base run.
    pub hits: u64,
    /// Solver microseconds of the hits in the then run.
    pub hit_solver_us: u64,
}

/// The estimate.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct HitReport {
    /// Checks in the then run.
    pub checks: u64,
    /// Solver microseconds of all checks in the then run.
    pub solver_us: u64,
    /// Key v2 (default mode).
    pub v2_prefix: KeyHits,
    /// Key v1 (stack only).
    pub v1_stack: KeyHits,
}

fn solve_us(c: &CheckRecord) -> u64 {
    c.answered_us.map_or(0, |a| a.saturating_sub(c.sent_us))
}

fn checks(dir: &Path) -> Result<Vec<CheckRecord>, BenchError> {
    let mut all = Vec::new();
    for files in trace::discover(dir)? {
        all.extend(trace::load(&files)?.checks);
    }
    Ok(all)
}

/// Estimates hits from the checks of a base run and a then run.
#[must_use]
pub fn estimate(base: &[CheckRecord], then: &[CheckRecord]) -> HitReport {
    let v2: BTreeSet<&str> = base
        .iter()
        .filter_map(|c| c.prefix_key.as_deref())
        .collect();
    let v1: BTreeSet<&str> = base.iter().filter_map(|c| c.key.as_deref()).collect();
    let mut r = HitReport::default();
    for c in then {
        let t = solve_us(c);
        r.checks += 1;
        r.solver_us += t;
        for (key, set, hits) in [
            (c.prefix_key.as_deref(), &v2, &mut r.v2_prefix),
            (c.key.as_deref(), &v1, &mut r.v1_stack),
        ] {
            if let Some(k) = key {
                hits.keyed += 1;
                if set.contains(k) {
                    hits.hits += 1;
                    hits.hit_solver_us += t;
                }
            }
        }
    }
    r
}

/// Loads both trace directories and estimates hits.
///
/// # Errors
///
/// Trace discovery and parsing errors.
pub fn run(base: &Path, then: &Path) -> Result<HitReport, BenchError> {
    Ok(estimate(&checks(base)?, &checks(then)?))
}

/// One line of human-readable summary.
#[must_use]
#[allow(clippy::cast_precision_loss)] // counts and microseconds, for display
pub fn summary(r: &HitReport) -> String {
    let pct = |a: u64, b: u64| {
        if b == 0 {
            0.0
        } else {
            a as f64 * 100.0 / b as f64
        }
    };
    format!(
        "{} checks ({:.1} s solver time). key v2 (prefix): {} hits ({:.1}% of checks, {:.1}% of solver time). key v1 (stack): {} hits ({:.1}%, {:.1}%).",
        r.checks,
        r.solver_us as f64 / 1e6,
        r.v2_prefix.hits,
        pct(r.v2_prefix.hits, r.checks),
        pct(r.v2_prefix.hit_solver_us, r.solver_us),
        r.v1_stack.hits,
        pct(r.v1_stack.hits, r.checks),
        pct(r.v1_stack.hit_solver_us, r.solver_us),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(v1: &str, v2: &str, solve: u64) -> CheckRecord {
        CheckRecord {
            key: Some(v1.to_owned()),
            prefix_key: Some(v2.to_owned()),
            sent_us: 10,
            answered_us: Some(10 + solve),
            ..CheckRecord::default()
        }
    }

    #[test]
    fn counts_hits_per_key_version() {
        let base = [check("a", "p1", 5), check("b", "p2", 5)];
        // Same stack as `b` but a different history; and a brand-new check.
        let then = [
            check("a", "p1", 100),
            check("b", "p9", 50),
            check("c", "p3", 10),
        ];
        let r = estimate(&base, &then);
        assert_eq!(r.checks, 3);
        assert_eq!(r.solver_us, 160);
        assert_eq!((r.v2_prefix.hits, r.v2_prefix.hit_solver_us), (1, 100));
        assert_eq!((r.v1_stack.hits, r.v1_stack.hit_solver_us), (2, 150));
    }
}
