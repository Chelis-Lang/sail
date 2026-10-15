# VI-0007: Errors from different modules are reported in a run-dependent order

- **Status:** open in stock Verus (tracked here; not filed upstream);
  fixed in Sail's Verus fork by `4285ae11` (fix: print bucket diagnostics in
  bucket order), on branch `l42/determinism`, not yet on `sail` or pinned.
  The same change was dropped once (`f98b87c5`); stock has since been shown
  to produce its order (see below).
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-09
- **Reproduction:** `corpus/repros/verus-nondeterministic-error-order/`
- **Impact on sail:** plain Verus's stderr varies between identical runs
  whenever errors come from more than one module. Since ADR 0042,
  conformance compares complete diagnostic messages as a multiset, so this
  order is no longer a difference. Before that, byte-for-byte conformance
  saw it as plain-Verus instability (ADR 0009). In the
  first `rust_verify_test` run, plain Verus disagreed with itself on 11
  tests for reasons like this, and with only 2 confirming plain runs 7 of
  them were first reported as `different` (ADR 0019).
- **Workaround:** none needed since ADR 0042. Until then the
  `rust_verify_test` runner passed `--num-threads 1` to every test on both
  sides (ADR 0019), which made the order stable; it now uses Verus's
  default threads (`--verus-threads 1` still selects one). With one thread
  Verus reports fewer errors on some inputs (one error instead of two from
  different modules), which makes three upstream tests fail on both sides.
  The corpus harness keeps Verus's default thread count.

The description below is written so it could be filed upstream unchanged.

## Description

**Title:** Diagnostics from different modules are printed in a
nondeterministic order when verifying with several threads

**Body:**

With release `0.2026.10.04.426d8b0`, verifying the same file twice can print
the same errors in a different order. The attached `test.rs` (the input of
`rust_verify_test`'s `traits_modules::test_verify_6`) has one error in
module `M5` and one in `M7`. In 12 runs of `verus --crate-type=lib
--num-threads 8 test.rs`, 5 printed the `M7` error first and 7 printed the
`M5` error first. With `--num-threads 1`, all 12 runs printed the same
order.

**Impact:** tools that compare Verus output between runs (CI log diffs,
expected-output tests, differential testing) see spurious differences. The
verdicts and the set of errors are unaffected.

**Suggested fix:** buffer each module's diagnostics and print them in a
deterministic order (for example module order), or document that the order
is unspecified with more than one thread.

**Reproduction:** `repro.sh <install dir> 12 8` next to `test.rs` counts
distinct stderr outputs over 12 runs.

## Sail fork

Fixed by `4285ae11` (fix: print bucket diagnostics in bucket order), one
commit with its regression test
`source/rust_verify_test/tests/sail_vi0007_error_order.rs`. With several
threads, stock makes the first bucket to send a message the active one and
prints the others' buffered messages after it, so the order follows which
bucket reports first. The fork makes the lowest-numbered unfinished bucket the
active one, so diagnostics appear in bucket (queue) order, and a bucket's VIR
error is reported in that order too. Stock prints this same sequence whenever
its buckets happen to report in queue order.

**History.** The same change (`f98b87c5`) was dropped on 2026-10-13 (rescue
L12): the engine conformance arm found `verus-examples/examples/debug_expand.rs`
different, because stock printed module `M4`'s error before the root module's
in 3/3 confirming runs, and no stock run had been seen to print the fork's
order. ADR 0039 allows an ordering fix only when stock is shown to produce the
chosen order.

**Stock produces the fork's order (rescue L42, 2026-10-15).** On a c7i.8xlarge
(Linux x86-64), with the conformance flags (`--crate-type=lib --output-json
--time --no-report-long-running -Z write-long-types-to-disk=no`):

| Input | Stock release runs | Stock outputs | Fork output |
|---|---|---|---|
| `examples/debug_expand.rs` | 40 | 39 × `M4` first (`3472446f`), 1 × root first (`36df931b`) | `36df931b`, byte-identical to stock's second variant |
| same, earlier batch | 5 stock, 5 pinned runtime (`13a1176c`) | stock 1 × `36df931b`; pinned runtime 2 × `36df931b` | — |
| `corpus/repros/verus-nondeterministic-error-order/test.rs`, `--num-threads 8` | 20 | 12 × `b82ba810`, 8 × `d24e598b` | `d24e598b` |

Digests are SHA-256 prefixes of complete stderr; the raw outputs are in the
[determinism record](../validation/determinism.md)'s tarball. ADR 0042 already
makes the order irrelevant to conformance; the fix makes it stable for caching
and agents.

The fork builds only the optional interactive engine until the default
switches to it (lane L41). Stock-mode workaround: none needed since ADR 0042.
