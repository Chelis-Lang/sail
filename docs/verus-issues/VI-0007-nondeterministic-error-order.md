# VI-0007: Errors from different modules are reported in a run-dependent order

- **Status:** open (tracked here; not filed upstream). A fork fix was tried and
  dropped (see below).
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

Not fixed. The fork briefly printed diagnostics in bucket order (`f98b87c5`,
fix: print bucket diagnostics in bucket order). The engine conformance arm
then found `verus-examples/examples/debug_expand.rs` different: stock printed
module `M4`'s error before the root module's in 3/3 confirming runs, the fork
printed it last. ADR 0039 allows an ordering fix only when it picks an order
stock produces, and no deterministic order is provably within stock's set
(stock prints in the order buckets finish), so the commit was dropped from
the fork (2026-10-13, rescue L12). ADR 0042 makes the order irrelevant to
conformance instead.
