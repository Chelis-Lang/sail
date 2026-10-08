//! Small statistics helpers: `rlimit-count` extraction, percentiles and
//! dispersion.

use serde::Serialize;

/// The number after `:rlimit-count` in an `:all-statistics` reply.
#[must_use]
pub fn rlimit_count(reply: &[u8]) -> Option<u64> {
    const NEEDLE: &[u8] = b":rlimit-count";
    let at = reply.windows(NEEDLE.len()).position(|w| w == NEEDLE)?;
    let rest = reply.get(at + NEEDLE.len()..)?;
    let digits: Vec<u8> = rest
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .take_while(u8::is_ascii_digit)
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

/// The nearest-rank percentile `p` (0 to 100) of `sorted`, which must be in
/// ascending order. `None` for an empty slice.
#[must_use]
pub fn percentile(sorted: &[u64], p: f64) -> Option<u64> {
    sorted.get(rank_index(sorted.len(), p)?).copied()
}

/// The 0-based index of the nearest-rank percentile `p` among `n` sorted
/// values. `None` when `n` is zero.
fn rank_index(n: usize, p: f64) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let rank = (p / 100.0 * count_f64(n)).ceil();
    if rank <= 1.0 {
        return Some(0);
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rank is in 1..=n after the check above"
    )]
    let r = rank as usize;
    Some(r.min(n) - 1)
}

/// `n` as a float, for ratios. Exact for every count this crate handles.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "counts and microsecond totals here stay far below 2^52"
)]
pub fn as_f64(n: u64) -> f64 {
    n as f64
}

/// A count as a float; see [`as_f64`].
#[must_use]
pub fn count_f64(n: usize) -> f64 {
    as_f64(u64::try_from(n).unwrap_or(u64::MAX))
}

/// Summary of a distribution of non-negative integers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Distribution {
    /// Number of values.
    pub count: usize,
    /// Sum of the values.
    pub total: u64,
    /// Smallest value.
    pub min: Option<u64>,
    /// Median (nearest rank).
    pub p50: Option<u64>,
    /// 90th percentile (nearest rank).
    pub p90: Option<u64>,
    /// 99th percentile (nearest rank).
    pub p99: Option<u64>,
    /// Largest value.
    pub max: Option<u64>,
    /// Arithmetic mean.
    pub mean: Option<f64>,
    /// Share of [`Self::total`] contributed by the largest 1% of values
    /// (at least one value).
    pub top_1pct_share: Option<f64>,
    /// Share of [`Self::total`] contributed by the largest 10% of values
    /// (at least one value).
    pub top_10pct_share: Option<f64>,
}

impl Distribution {
    /// Summarizes `values` (in any order).
    #[must_use]
    pub fn of(values: &[u64]) -> Self {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        let total: u64 = sorted.iter().sum();
        let count = sorted.len();
        let share = |fraction: f64| {
            if count == 0 || total == 0 {
                return None;
            }
            let k = top_count(count, fraction);
            let top: u64 = sorted.iter().rev().take(k).sum();
            Some(as_f64(top) / as_f64(total))
        };
        Self {
            count,
            total,
            min: sorted.first().copied(),
            p50: percentile(&sorted, 50.0),
            p90: percentile(&sorted, 90.0),
            p99: percentile(&sorted, 99.0),
            max: sorted.last().copied(),
            mean: (count > 0).then(|| as_f64(total) / count_f64(count)),
            top_1pct_share: share(0.01),
            top_10pct_share: share(0.10),
        }
    }
}

/// How many of `count` values make up the top `fraction`: the ceiling, and
/// at least one.
fn top_count(count: usize, fraction: f64) -> usize {
    let k = (count_f64(count) * fraction).ceil();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "k is in 0..=count"
    )]
    let k = k as usize;
    k.clamp(1, count.max(1))
}

/// Population coefficient of variation (standard deviation over mean).
/// `None` for fewer than two values or a zero mean.
#[must_use]
pub fn coefficient_of_variation(values: &[u64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let n = count_f64(values.len());
    let mean = values.iter().map(|&v| as_f64(v)).sum::<f64>() / n;
    if mean == 0.0 {
        return None;
    }
    let var = values
        .iter()
        .map(|&v| (as_f64(v) - mean).powi(2))
        .sum::<f64>()
        / n;
    Some(var.sqrt() / mean)
}

/// Summary of a distribution of non-negative floats.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FloatDistribution {
    /// Number of values.
    pub count: usize,
    /// Mean.
    pub mean: Option<f64>,
    /// Median (nearest rank).
    pub p50: Option<f64>,
    /// 90th percentile (nearest rank).
    pub p90: Option<f64>,
    /// 99th percentile (nearest rank).
    pub p99: Option<f64>,
    /// Largest value.
    pub max: Option<f64>,
    /// Number of values that are exactly zero.
    pub zeros: usize,
}

impl FloatDistribution {
    /// Summarizes `values`. NaNs are ignored.
    #[must_use]
    pub fn of(values: &[f64]) -> Self {
        let mut sorted: Vec<f64> = values.iter().copied().filter(|v| !v.is_nan()).collect();
        sorted.sort_by(f64::total_cmp);
        let count = sorted.len();
        let pick = |p: f64| sorted.get(rank_index(count, p)?).copied();
        Self {
            count,
            mean: (count > 0).then(|| sorted.iter().sum::<f64>() / count_f64(count)),
            p50: pick(50.0),
            p90: pick(90.0),
            p99: pick(99.0),
            max: sorted.last().copied(),
            zeros: sorted.iter().filter(|v| **v == 0.0).count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rlimit_count() {
        assert_eq!(
            rlimit_count(b"(:added-eqs 73\n :rlimit-count        24862)"),
            Some(24862)
        );
        assert_eq!(rlimit_count(b"(:memory 17.69)"), None);
    }

    #[test]
    fn nearest_rank_percentiles() {
        let v: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile(&v, 50.0), Some(50));
        assert_eq!(percentile(&v, 90.0), Some(90));
        assert_eq!(percentile(&v, 99.0), Some(99));
        assert_eq!(percentile(&v, 100.0), Some(100));
        assert_eq!(percentile(&v, 0.0), Some(1));
        assert_eq!(percentile(&[], 50.0), None);
        assert_eq!(percentile(&[7], 99.0), Some(7));
    }

    #[test]
    fn distribution_shares() {
        let mut v = vec![1u64; 99];
        v.push(901);
        let d = Distribution::of(&v);
        assert_eq!(d.count, 100);
        assert_eq!(d.total, 1000);
        assert_eq!(d.max, Some(901));
        assert_eq!(d.top_1pct_share, Some(0.901));
        // Top 10 values: 901 + 9.
        assert_eq!(d.top_10pct_share, Some(0.91));
        let empty = Distribution::of(&[]);
        assert_eq!(empty.p50, None);
        assert_eq!(empty.top_1pct_share, None);
    }

    #[test]
    fn coefficient_of_variation_basics() {
        assert_eq!(coefficient_of_variation(&[5, 5, 5]), Some(0.0));
        assert_eq!(coefficient_of_variation(&[5]), None);
        assert_eq!(coefficient_of_variation(&[0, 0]), None);
        let cv = coefficient_of_variation(&[2, 4, 4, 4, 5, 5, 7, 9]).unwrap_or_default();
        assert!((cv - 0.4).abs() < 1e-12, "{cv}");
    }

    #[test]
    fn float_distribution() {
        let d = FloatDistribution::of(&[0.0, 0.5, 0.25, f64::NAN]);
        assert_eq!(d.count, 3);
        assert_eq!(d.zeros, 1);
        assert_eq!(d.p50, Some(0.25));
        assert_eq!(d.max, Some(0.5));
    }
}
