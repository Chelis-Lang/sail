//! Solver configurations for fragility measurement, and how each one is
//! applied to a captured session's exact bytes.
//!
//! A [`Config`] is one way of re-running the same session on the same
//! pinned Z3:
//! - **Seed 0, no shuffle** is the baseline: the captured bytes, unmodified.
//!   Z3's default seed is 0, so this is what plain Verus ran.
//! - **Reseeding** (`seed > 0`) inserts `(set-option :smt.random_seed s)` and
//!   `(set-option :sat.random_seed s)` immediately before the first command
//!   that is not a `set-option`, that is just after Verus's option prelude
//!   ([`inject_seed`]). Nothing else changes.
//! - **Assertion shuffling** (`shuffle = Some(k)`, Mariposa's strongest
//!   mutation) permutes each maximal run of consecutive `assert` commands
//!   ([`shuffle_asserts`]). See that function for exactly why this preserves
//!   the meaning of every check.
//!
//! These configurations exist only to *measure*. The mutated bytes are
//! handed to a fresh solver process and thrown away. They never reach the
//! shim, the cache or key derivation: this crate does not depend on
//! `sail-key` or `sail-cache`, and `xtask/src/layering.rs` tests that
//! neither can reach it (ADR 0018).

use serde::{Deserialize, Serialize};

use sail_smtlib::command::{CommandKind, classify};
use sail_smtlib::lex::{Lexer, TokenKind};

use crate::transcript::{Class, Cmd, split_commands};

/// One configuration of the pinned solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Config {
    /// The value for `smt.random_seed` and `sat.random_seed`; 0 leaves the
    /// session's own (Z3's default) seed in place.
    pub seed: u64,
    /// `Some(k)` shuffles assertion runs with permutation seed `k`.
    pub shuffle: Option<u64>,
}

impl Config {
    /// The unmodified session.
    pub const BASELINE: Self = Self {
        seed: 0,
        shuffle: None,
    };

    /// Whether this is the unmodified session.
    #[must_use]
    pub fn is_baseline(&self) -> bool {
        *self == Self::BASELINE
    }

    /// A short label: `baseline`, `seed=3`, `shuffle=2` or `seed=3,shuffle=2`.
    #[must_use]
    pub fn label(&self) -> String {
        match (self.seed, self.shuffle) {
            (0, None) => "baseline".to_owned(),
            (s, None) => format!("seed={s}"),
            (0, Some(k)) => format!("shuffle={k}"),
            (s, Some(k)) => format!("seed={s},shuffle={k}"),
        }
    }

    /// Applies this configuration to a session's exact input (`commands`
    /// split from `input`). Returns the input to run and its commands.
    #[must_use]
    pub fn apply(&self, input: &[u8], commands: Vec<Cmd>) -> (Vec<u8>, Vec<Cmd>) {
        if self.is_baseline() {
            return (input.to_vec(), commands);
        }
        let shuffled = match self.shuffle {
            Some(k) => shuffle_asserts(input, &commands, k),
            None => input.to_vec(),
        };
        let commands = if self.shuffle.is_some() {
            split_commands(&shuffled)
        } else {
            commands
        };
        let out = inject_seed(&shuffled, &commands, self.seed);
        let commands = if self.seed == 0 {
            commands
        } else {
            split_commands(&out)
        };
        (out, commands)
    }
}

/// The configurations for `seeds` reseeds (seed 0, the baseline, first) and
/// `shuffles` assertion shuffles under the default seed.
#[must_use]
pub fn configs(seeds: u64, shuffles: u64) -> Vec<Config> {
    let mut out: Vec<Config> = (0..seeds.max(1))
        .map(|seed| Config {
            seed,
            shuffle: None,
        })
        .collect();
    out.extend((1..=shuffles).map(|k| Config {
        seed: 0,
        shuffle: Some(k),
    }));
    out
}

/// Inserts the two seed options before the first command of `commands`
/// (split from `input`) that is not a `set-option`. Seed 0 returns the input
/// unchanged.
#[must_use]
pub fn inject_seed(input: &[u8], commands: &[Cmd], seed: u64) -> Vec<u8> {
    if seed == 0 {
        return input.to_vec();
    }
    let at = commands
        .iter()
        .find(|c| c.class != Class::SetOption)
        .and_then(|c| usize::try_from(c.offset).ok())
        .unwrap_or(input.len())
        .min(input.len());
    let (head, tail) = input.split_at(at);
    let mut out = Vec::with_capacity(input.len() + 80);
    out.extend_from_slice(head);
    if !head.is_empty() && !head.ends_with(b"\n") {
        out.push(b'\n');
    }
    out.extend_from_slice(
        format!("(set-option :smt.random_seed {seed})\n(set-option :sat.random_seed {seed})\n")
            .as_bytes(),
    );
    out.extend_from_slice(tail);
    out
}

/// Whether `bytes` is an `assert` that introduces no name: shuffling may
/// move it.
fn movable_assert(bytes: &[u8]) -> bool {
    let is_assert = classify(bytes).is_ok_and(|c| c.kind == CommandKind::Assert);
    is_assert
        && !Lexer::new(bytes)
            .filter_map(Result::ok)
            .any(|t| t.kind == TokenKind::Keyword && t.text(bytes) == b":named")
}

/// `SplitMix64`: a small, fixed, portable generator, so a shuffle seed means
/// the same permutation on every machine and release.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..bound` (`bound > 0`). The modulo bias is irrelevant
    /// here: the permutation only has to be fixed by the seed.
    fn below(&mut self, bound: usize) -> usize {
        let b = u64::try_from(bound).unwrap_or(u64::MAX).max(1);
        usize::try_from(self.next() % b).unwrap_or(0)
    }
}

/// Permutes every maximal run of two or more consecutive, unnamed `assert`
/// commands, with a Fisher–Yates shuffle driven by `seed`. Every byte
/// between commands (whitespace, comments) and every other command stays
/// where it was.
///
/// **Why the meaning of every check is preserved.** A run contains only
/// `assert`s, with no command between them, so all of them are in the same
/// scope and see the same declarations, all made before the run. An
/// `assert` adds a formula to the current scope's set of assertions, and a
/// check asks about the conjunction of that set, which does not depend on
/// order. `:named` assertions are never moved, because a name can be used
/// by a later assertion in the same run. `push`, `pop`, declarations,
/// definitions, options and checks are never moved, so scoping, the
/// declarations in force and the options in force at each check are
/// exactly as captured.
///
/// What may change is Z3's *search*: its answer under a resource limit,
/// its effort, its models. Measuring that sensitivity is the purpose.
#[must_use]
pub fn shuffle_asserts(input: &[u8], commands: &[Cmd], seed: u64) -> Vec<u8> {
    let mut order: Vec<usize> = (0..commands.len()).collect();
    let mut rng = SplitMix64(seed ^ 0x5341_494C_5348_5546); // "SAILSHUF"
    let mut i = 0;
    while i < commands.len() {
        let movable = |j: usize| commands.get(j).is_some_and(|c| movable_assert(&c.bytes));
        if !movable(i) {
            i += 1;
            continue;
        }
        let start = i;
        while movable(i) {
            i += 1;
        }
        if let Some(run) = order.get_mut(start..i) {
            for k in (1..run.len()).rev() {
                run.swap(k, rng.below(k + 1));
            }
        }
    }
    let mut out = Vec::with_capacity(input.len());
    let mut cursor = 0usize;
    for (slot, &source) in commands.iter().zip(&order) {
        let at = usize::try_from(slot.offset).unwrap_or(usize::MAX);
        if let Some(gap) = input.get(cursor..at) {
            out.extend_from_slice(gap);
        }
        if let Some(cmd) = commands.get(source) {
            out.extend_from_slice(&cmd.bytes);
        }
        cursor = at.saturating_add(slot.bytes.len());
    }
    if let Some(rest) = input.get(cursor..) {
        out.extend_from_slice(rest);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_after_the_option_prelude() {
        let input = b"(set-option :a 1)\n(set-option :b 2)\n(declare-const x Int)\n(check-sat)\n";
        let commands = split_commands(input);
        let out = inject_seed(input, &commands, 7);
        assert_eq!(
            String::from_utf8_lossy(&out),
            "(set-option :a 1)\n(set-option :b 2)\n\
(set-option :smt.random_seed 7)\n(set-option :sat.random_seed 7)\n\
(declare-const x Int)\n(check-sat)\n"
        );
        assert_eq!(inject_seed(input, &commands, 0), input);
        // No prelude: the options go first.
        let input = b"(push)(check-sat)";
        let out = inject_seed(input, &split_commands(input), 3);
        assert!(out.starts_with(b"(set-option :smt.random_seed 3)\n"));
        assert!(out.ends_with(b"(push)(check-sat)"));
        // Only options: they go last.
        let input = b"(set-option :a 1)";
        let out = inject_seed(input, &split_commands(input), 3);
        assert!(out.starts_with(b"(set-option :a 1)\n(set-option :smt"));
    }

    const RUNS: &[u8] = b"(declare-const x Int)\n(assert (> x 0))\n(assert (< x 9))\n\
(assert (! (> x 1) :named n))\n(assert (=> n (> x 2)))\n(assert (> x 3))\n(assert (> x 4))\n\
(push)\n(assert (> x 5))\n(check-sat)\n";

    fn sorted_commands(bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut v: Vec<Vec<u8>> = split_commands(bytes).into_iter().map(|c| c.bytes).collect();
        v.sort();
        v
    }

    #[test]
    fn shuffles_only_within_unnamed_assert_runs() {
        let commands = split_commands(RUNS);
        let mut seen_change = false;
        for seed in 0..20 {
            let out = shuffle_asserts(RUNS, &commands, seed);
            assert_eq!(out.len(), RUNS.len());
            assert_eq!(sorted_commands(&out), sorted_commands(RUNS));
            let cmds: Vec<Vec<u8>> = split_commands(&out).into_iter().map(|c| c.bytes).collect();
            let at = |i: usize| String::from_utf8_lossy(&cmds[i]).into_owned();
            // Fixed points: the declaration, the named assert, push, the
            // lone assert after push, and the check.
            assert_eq!(at(0), "(declare-const x Int)");
            assert_eq!(at(3), "(assert (! (> x 1) :named n))");
            assert_eq!(at(7), "(push)");
            assert_eq!(at(8), "(assert (> x 5))");
            assert_eq!(at(9), "(check-sat)");
            // Each run keeps its own members.
            let mut first: Vec<String> = vec![at(1), at(2)];
            first.sort();
            assert_eq!(first, ["(assert (< x 9))", "(assert (> x 0))"]);
            let mut second: Vec<String> = vec![at(4), at(5), at(6)];
            second.sort();
            assert_eq!(
                second,
                [
                    "(assert (=> n (> x 2)))",
                    "(assert (> x 3))",
                    "(assert (> x 4))"
                ]
            );
            seen_change |= out != RUNS;
        }
        assert!(seen_change, "some seed must permute something");
        // Deterministic per seed.
        assert_eq!(
            shuffle_asserts(RUNS, &commands, 5),
            shuffle_asserts(RUNS, &commands, 5)
        );
    }

    #[test]
    fn configurations_and_labels() {
        let c = configs(3, 2);
        let labels: Vec<String> = c.iter().map(Config::label).collect();
        assert_eq!(
            labels,
            ["baseline", "seed=1", "seed=2", "shuffle=1", "shuffle=2"]
        );
        assert!(c[0].is_baseline());
        assert_eq!(configs(0, 0), [Config::BASELINE]);
        let both = Config {
            seed: 3,
            shuffle: Some(2),
        };
        assert_eq!(both.label(), "seed=3,shuffle=2");
        let input = b"(set-option :a 1)\n(assert p)\n(assert q)\n(check-sat)\n";
        let (out, cmds) = both.apply(input, split_commands(input));
        assert!(out.starts_with(b"(set-option :a 1)\n(set-option :smt.random_seed 3)\n"));
        assert_eq!(cmds.len(), 6);
        let (same, _) = Config::BASELINE.apply(input, split_commands(input));
        assert_eq!(same, input);
    }
}
