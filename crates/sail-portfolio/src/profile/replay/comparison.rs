//! Compare diagnostic runs without rewriting their original captures.

use std::collections::{BTreeMap, BTreeSet};

use sail_smtlib::command::Output;
use sail_smtlib::sexpr;
use sail_smtlib::split::{Item, ResponseSplitter};
use serde::Serialize;

use crate::PortfolioError;
use crate::transcript::{Class, Transcript};

use super::refuse;

const MAX_CHANGES: usize = 4096;
const OBSERVATIONS: [&[u8]; 4] = [b":num-allocs", b":memory", b":max-memory", b":time"];

/// An original diagnostic allocation, memory or elapsed-time observation.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StatisticChange {
    /// Index of the original statistics command in the complete prefix.
    pub command_index: usize,
    /// Exact observational field name, never a resource-limit counter.
    pub field: String,
    /// Exact original plain value; only `:time` may be absent on one side.
    pub plain: Option<String>,
    /// Exact original traced value; only `:time` may be absent on one side.
    pub traced: Option<String>,
}

fn replies(output: &[u8]) -> Vec<Item> {
    let mut splitter = ResponseSplitter::new();
    let mut items = Vec::new();
    splitter.push(output, &mut items);
    items.extend(splitter.finish());
    items
}

struct Statistics<'a> {
    exact: Vec<(&'a [u8], &'a [u8])>,
    observed: BTreeMap<&'a [u8], &'a [u8]>,
}

fn statistics(bytes: &[u8]) -> Result<Statistics<'_>, PortfolioError> {
    let parsed = sexpr::parse_one(bytes).map_err(|_| refuse("malformed statistics reply"))?;
    let entries = parsed
        .as_list()
        .filter(|entries| entries.len() <= 8192 && entries.len().is_multiple_of(2))
        .ok_or_else(|| refuse("unsupported statistics inventory"))?;
    let mut seen = BTreeSet::new();
    let mut out = Statistics {
        exact: Vec::new(),
        observed: BTreeMap::new(),
    };
    for pair in entries.as_chunks::<2>().0 {
        let key = pair[0].text(bytes);
        let value = pair[1].text(bytes);
        if key.first() != Some(&b':')
            || key.len() < 2
            || key.len() > 128
            || !key[1..]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            || !seen.insert(key)
        {
            return Err(refuse("duplicate or unsupported statistics key"));
        }
        if OBSERVATIONS.contains(&key) {
            if value.len() > 128
                || !std::str::from_utf8(value)
                    .ok()
                    .and_then(|value| value.parse::<f64>().ok())
                    .is_some_and(|value| value.is_finite() && value >= 0.0)
            {
                return Err(refuse("invalid observational statistics value"));
            }
            out.observed.insert(key, value);
        } else {
            out.exact.push((key, value));
        }
    }
    Ok(out)
}

fn compare_statistics(
    command_index: usize,
    plain: &[u8],
    traced: &[u8],
    changes: &mut Vec<StatisticChange>,
) -> Result<(), PortfolioError> {
    let plain = statistics(plain)?;
    let traced = statistics(traced)?;
    if plain.exact != traced.exact {
        return Err(refuse("non-observational statistics differ"));
    }
    for field in OBSERVATIONS {
        let before = plain.observed.get(field).copied();
        let after = traced.observed.get(field).copied();
        if field != b":time" && before.is_some() != after.is_some() {
            return Err(refuse("observational statistics inventory differs"));
        }
        if before != after {
            if changes.len() >= MAX_CHANGES {
                return Err(refuse("observational statistics change budget exceeded"));
            }
            // Names are fixed ASCII above, and values have passed UTF-8 parsing.
            let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
            changes.push(StatisticChange {
                command_index,
                field: text(field),
                plain: before.map(text),
                traced: after.map(text),
            });
        }
    }
    Ok(())
}

fn span(output: &[u8], item: &Item) -> Result<std::ops::Range<usize>, PortfolioError> {
    let start = usize::try_from(item.offset).map_err(|_| refuse("reply offset overflow"))?;
    let end = start
        .checked_add(item.bytes.len())
        .ok_or_else(|| refuse("reply end overflow"))?;
    if output.get(start..end) != Some(item.bytes.as_slice()) {
        return Err(refuse("reply span differs from original capture"));
    }
    Ok(start..end)
}

pub(super) fn compare(
    input: &[u8],
    plain: &[u8],
    traced: &[u8],
    checks: usize,
) -> Result<Vec<StatisticChange>, PortfolioError> {
    let before = Transcript::new(input, plain);
    let after = Transcript::new(input, traced);
    let observed = before.checks();
    if before.unpaired_batches != 0
        || after.unpaired_batches != 0
        || !before.errors.is_empty()
        || !after.errors.is_empty()
        || observed.len() != checks
        || observed != after.checks()
        || observed.iter().any(|check| {
            !matches!(
                check.answer.as_deref(),
                Some(b"sat" | b"unsat" | b"unknown")
            )
        })
    {
        return Err(refuse(
            "incomplete, differing or erroneous replay responses",
        ));
    }
    let plain_items = replies(plain);
    let traced_items = replies(traced);
    let commands: Vec<_> = before
        .commands
        .iter()
        .enumerate()
        .filter(|(_, command)| command.output() == Output::One)
        .collect();
    if commands.len() != plain_items.len() || commands.len() != traced_items.len() {
        return Err(refuse("reply inventory differs from original commands"));
    }
    let mut changes = Vec::new();
    let (mut plain_end, mut traced_end) = (0, 0);
    for ((index, command), (p, t)) in commands
        .into_iter()
        .zip(plain_items.iter().zip(&traced_items))
    {
        if command.class != Class::Statistics {
            continue;
        }
        let p_span = span(plain, p)?;
        let t_span = span(traced, t)?;
        if plain.get(plain_end..p_span.start) != traced.get(traced_end..t_span.start) {
            return Err(refuse("stdout outside statistics differs"));
        }
        compare_statistics(index, &p.bytes, &t.bytes, &mut changes)?;
        plain_end = p_span.end;
        traced_end = t_span.end;
    }
    if plain.get(plain_end..) != traced.get(traced_end..) {
        return Err(refuse("stdout outside statistics differs"));
    }
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &[u8] = b"(get-info :all-statistics)\n(check-sat)";

    #[test]
    fn only_named_observations_vary_and_raw_values_remain_visible() -> Result<(), PortfolioError> {
        let plain = b"(:num-allocs 10 :memory 17.90 :rlimit-count 2)\nunsat\n";
        let traced = b"(:num-allocs 12 :memory 17.91 :rlimit-count 2 :time 0.00)\nunsat\n";
        let changes = compare(INPUT, plain, traced, 1)?;
        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].field, ":num-allocs");
        assert_eq!(changes[0].plain.as_deref(), Some("10"));
        assert_eq!(changes[0].traced.as_deref(), Some("12"));
        assert_eq!(changes[2].field, ":time");
        assert_eq!(changes[2].plain, None);
        assert_eq!(changes[2].traced.as_deref(), Some("0.00"));
        assert_eq!(
            compare(INPUT, plain, plain, 1)?,
            Vec::<StatisticChange>::new()
        );
        Ok(())
    }

    #[test]
    fn resource_counters_protocol_inventory_and_other_bytes_stay_strict() {
        let original = b"(:num-allocs 10 :rlimit-count 2)\nunsat\n";
        for changed in [
            b"(:num-allocs 12 :rlimit-count 3)\nunsat\n".as_slice(),
            b"(:num-allocs 12 :rlimit-count 2)\nsat\n",
            b"(:num-allocs 12 :rlimit-count 2)\nunsat\nextra\n",
            b"(:rlimit-count 2)\nunsat\n",
            b"(:num-allocs 12 :rlimit-count 2 :foreign 0)\nunsat\n",
            b"(:num-allocs NaN :rlimit-count 2)\nunsat\n",
            b"(:num-allocs 12 :num-allocs 13 :rlimit-count 2)\nunsat\n",
            b"(:num-allocs 12 :rlimit-count 2)\n unsat\n",
        ] {
            assert!(compare(INPUT, original, changed, 1).is_err());
        }
    }
}
