# VI-0017: `--output-json --time` lists modules and functions in a run-dependent order

- **Status:** open in stock Verus (tracked here; not filed upstream);
  fixed in Sail's Verus fork by `54640eed` (fix: list output-json timing
  modules and functions by name), on branch `l42/determinism`, not yet on
  `sail` or pinned.
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-15 (rescue lane L42)
- **Reproduction:** `corpus/repros/verus-timing-json-order/`
- **Impact on sail:** the fork's stdout is not byte-stable between identical
  runs, even with every wall-clock number blanked. In L42's measurement of the
  pinned fork runtime this was the only stdout difference, and it hit 136 of
  147 fast-tier units and every measured standard-tier unit
  ([determinism record](../validation/determinism.md)). Sail parses the
  breakdown into per-function records and sorts them, so verdicts and
  conformance are unaffected; a cache or comparison keyed on stdout bytes
  is not.
- **Workaround:** none in stock mode; Sail compares parsed results.

The description below is written so it could be filed upstream unchanged.

## Description

**Title:** `--output-json --time` lists `function-breakdown` in `HashMap` order
and modules by elapsed time

**Body:**

With release `0.2026.10.04.426d8b0`, the `times-ms` object of
`--output-json --time` changes order between identical runs:

- `smt.smt-run-module-times[*].function-breakdown` is built from
  `verifier.func_times`, a `HashMap<BucketId, HashMap<Fun, _>>`
  (`rust_verify/src/verifier.rs`), and never sorted
  (`rust_verify/src/main.rs`, `smt_function_breakdown`);
- `total-verify-module-times`, `air.module-times` and the `smt-*-module-times`
  lists are sorted by elapsed time, which varies, and ties fall back to
  `HashMap` order.

`repro.sh` gave 10 different name orders in 10 runs.

**Impact:** byte comparisons of `--output-json` output see spurious
differences even when the times are masked.

**Suggested fix:** sort the JSON lists by module name and the breakdown by
function name (the text `--time` report can keep its slowest-first order).

## Sail fork

Fixed by `54640eed`, one commit with its regression test
`source/rust_verify_test/tests/sail_vi0017_timing_order.rs`, which fails on
the pinned runtime commit (`13a1176c`; modules came out `["", "m1", "m3",
"m2"]`) and passes with the fix. Under `--output-json`, the module lists are
sorted by module name (a module's several buckets keep stock's time order)
and each breakdown by friendly function name, then function identity.
Entries and values are unchanged, and the text `--time` report is untouched.
This is a structured-output correction (ADR 0039's second fix rule): the
lists are timing data, and no verdict or diagnostic changes.
