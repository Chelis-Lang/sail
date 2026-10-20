# Compatibility matrix

The Verus releases sail is tested against (`corpus/toolchain.toml`), and how
the corpus fares under each one. The primary pin is the release the corpus
is validated against. Secondary pins are compatibility targets (ADR 0010),
run locally with `SAIL_VERUS_RELEASE=<release>` (ADR 0013).

**Sail's Verus.** Sail runs its own Verus, built from its fork
(`engine/verus/fork.toml`, commit `64baccbb`, rebased onto the primary pin
`7d829946`; its Linux and macOS runtimes are published and pinned, see [engine](../engine/verus/README.md) and
[macOS engine](validation/macos-engine.md)), for every verification
([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)). Stock releases
are the conformance oracle: `sail-conformance run` compares Sail on its fork
with plain stock Verus at the primary pin. Under a secondary pin the fork has
no build, so the Sail side runs that stock release with `sail-z3`, as before;
the secondary rows below check the shim, not a supported product
configuration.

| Release | Role | Z3 | Rust | sail conformance (fast tier) | Notes |
|---|---|---|---|---|---|
| `0.2026.10.18.7d82994` | primary | 4.16.0 (bundled, same bytes as 426d8b0's) | 1.99.1 | Not yet run | The corpus is validated against this release; Sail's Verus is built on it. See [the upgrade](#upgrade-to-0202610187d82994) |
| `0.2026.10.04.426d8b0` | secondary | 4.16.0 (bundled) | 1.98.1 | As primary: 147 of 147 the same (macOS arm64 and Linux x86-64); standard tier 159 of 159 (Linux), stock pass-through before ADR 0043; Sail on fork `73dea021`: see [the fork-default record](validation/fork-default.md). Not yet run as a secondary | Previous primary, kept as an A/B baseline; needs `rustup toolchain install 1.98.1` |
| `0.2026.09.06.8dea4a2` | secondary | 4.16.0 (primary's build; VI-0006) | 1.98.0 | 147 of 147 the same (macOS arm64, 2026-10-09), against the 426d8b0 tree | See below |

## Upgrade to 0.2026.10.18.7d82994

The primary moved from 0.2026.10.04.426d8b0 to 0.2026.10.18.7d82994 (81
upstream commits; Rust 1.98.1 to 1.99.1; Z3 unchanged). The published crates
moved with it: `vstd`, `verus_builtin_macros` and
`verus_state_machines_macros` to `0.0.0-2026-10-18-0147`, `verus_builtin` to
`0.0.0-2026-10-11-0230` (the pinned `vstd` is in `corpus/toolchain.toml`,
[ADR 0059](decisions/0059-pinned-vstd-and-generated-source-policies.md)).
Upstream changes that can show in corpus results:

- `--verify-only-module` is now reported as partial verification (upstream
  `0a5e114f`): the summary line gains "(partial verification with
  `--verify-*`)" and the JSON `is-verifying-entire-crate` is `false`. Sail
  already treated the flag as partial, and no Sail golden or fixture records
  the old output; both conformance arms run the same release.
- vstd removed `pervasive::assume` (#3265) and added `no_unwind` and atomic
  `Ordering` preconditions (#3111, #3226), which can make previously verifying
  code fail. Corpus projects use `patch_verus_deps`, so they build against the
  pinned source.
- Vest at its corpus pin no longer verifies with the default rlimit:
  `parse_cbor_with_child` (`vest_lib/src/cbor/format.rs`) exceeds it under
  stock 7d82994 and Sail alike. Upstream verita runs Vest with
  `--rlimit 100`. The corpus entry now expects that one error
  ([VI-0032](verus-issues/VI-0032-vest-rlimit-on-7d82994.md)).
- Cargo 1.99.1 no longer passes a registry package's `[lints]` to the compiler
  under `--cap-lints allow`; the shipped native shapes were re-observed.

The corpus tiers, determinism check and benchmarks on the new runtime are
still to run.

## Plain Verus under the secondary release (fast tier)

The fast tier's in-tree sources (vstd, examples) are those of the **primary**
release's commit. The results below were taken when 426d8b0 was the primary;
the fast tier now uses 7d82994's sources. Under the older secondary release, some of them fail
simply because they are newer than that release. sail matches plain Verus on
every unit either way (147 of 147 the same).

- **Exit non-zero under the secondary:** 21 of 147 units.
- **Of those, 5 are expected-failure examples** (`assertions.rs`,
  `basic_failure.rs`, `debug.rs`, `debug_expand.rs`,
  `test_expand_errors.rs`). They fail under the primary too.
- **The rest fail to build or verify** because their sources need the newer
  release: vstd itself, human-eval-verus (it depends on the newer vstd), and
  14 further examples, including `guide/*`, `iterators/*` and `mergesort.rs`.

The secondary pin is meant for external projects written against that
release, such as vest's recent history (`sail-bench history-replay`). It is
not meant for the in-tree tier.

## Optional Singular validation

The binary releases omit Singular support. Use `ci/local-singular.sh` to build
the pinned commit with `--features=singular` and compare the tagged integer-ring
examples and memory allocator in pass-through, fill and warm modes. The separate
image pins the Singular package and uses separate build/cache volumes. No CI
workflow runs this step. Source selection is explicit through
`SAIL_VERUS_SOURCE_BUILD`; artifact identity failures are infrastructure errors.
Post-diagnostic-fix local validation on 2026-10-09 completed all three modes:
3 units, 606 functions, all the same, zero differences, zero infrastructure
errors and zero skips in each mode. Verus reports 9 and 27 verified for the
integer-ring examples and 736 for the allocator, all with zero errors. The
final cache held 8,080 entries. One unit had resource-count drift without
verdict or diagnostic drift. The source build ran natively on macOS arm64;
its Singular subprocess used the pinned local Linux image under amd64
emulation. These are correctness results, not Linux benchmark numbers.
