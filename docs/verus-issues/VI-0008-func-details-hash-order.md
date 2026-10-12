# VI-0008: `--output-json` lists `func-details` in hash order

- **Status:** open in stock Verus (tracked here; not filed upstream);
  fixed in Sail's Verus fork by `3b388cf9` (fix: list output-json func-details sorted by name).
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-09
- **Reproduction:** `rust_verify_test`'s
  `output_json::test_json_proof_note_on_assume_and_req_with_no_cheating`
  (Verus repository at the pinned commit), run from the source build
  (ADR 0019): 40 plain runs gave 4 distinct outputs (12, 10, 9 and 9
  runs), differing only in the order of the `func-details` entries.
- **Impact on sail:** the corpus harness compares `func-details` as parsed
  JSON, which ignores key order, so corpus units are unaffected. The
  `rust_verify_test` tests in `output_json` print the parsed document
  (`dbg!`) in Verus's order, so their stderr varies between identical plain
  runs. With up to 4 variants, 5 confirming plain runs missed sail's
  variant once in a cache-on run (`different`, until 40 plain runs showed
  sail's output 9 times); the runner now makes up to 10.
- **Workaround:** none needed for correctness; the runner classifies the
  variation as `unstable` (ADR 0009, ADR 0019).

The description below is written so it could be filed upstream unchanged.

## Description

**Title:** `--output-json` emits `func-details` in `HashMap` order

**Body:**

With release `0.2026.10.04.426d8b0`, the order of the entries in
`--output-json`'s `func-details` object changes between identical runs.
`rust_verify/src/main.rs` (around line 522) builds the object by iterating
over `verifier.func_details`, a `HashMap<Fun, FuncDetails>`
(`rust_verify/src/verifier.rs`, line 324), and the workspace enables
`serde_json`'s `preserve_order`, so the hash order reaches the output.
`rust_verify_test`'s `output_json` tests print the parsed document, and 40
runs of one of them printed 4 different orders.

**Impact:** byte-level comparisons of `--output-json` output (CI diffs,
expected-output tests) see spurious differences.

**Suggested fix:** collect `func_details` into a `BTreeMap`, or sort the
entries by name before inserting them.

## Sail fork

Fixed in [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus) by
`3b388cf9` (fix: list output-json func-details sorted by name), one commit
with its regression test
`source/rust_verify_test/tests/sail_vi0008_func_details_order.rs`, which fails
on the pinned stock source and passes on the fork. `func-details` entries are
inserted sorted by friendly name, then by function identity. The test checks
eight functions; stock passes by chance once in 40,320 runs.

The fork builds only the optional interactive engine. Stock-mode workaround:
None was needed; stock runs are unchanged (the corpus harness compares parsed
JSON).
