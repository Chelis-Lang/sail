# VI-0007: Errors from different modules are reported in a run-dependent order

- **Status:** open (tracked here; not filed upstream; see the README)
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-09
- **Reproduction:** `corpus/repros/verus-nondeterministic-error-order/`
- **Impact on sail:** plain Verus's stderr varies between identical runs
  whenever errors come from more than one module. Byte-for-byte
  conformance sees that as plain-Verus instability (ADR 0009). In the
  first `rust_verify_test` run, plain Verus disagreed with itself on 11
  tests for reasons like this, and with only 2 confirming plain runs 7 of
  them were first reported as `different` (ADR 0019).
- **Workaround:** the `rust_verify_test` runner passes `--num-threads 1` to
  every test on both sides by default (ADR 0019); the order is then
  stable. With one thread Verus reports fewer errors on some inputs (one
  error instead of two from different modules), which makes three upstream
  tests fail on both sides. The corpus harness keeps Verus's default
  thread count.

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
