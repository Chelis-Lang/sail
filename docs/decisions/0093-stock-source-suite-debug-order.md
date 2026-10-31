# 0093: Named order exceptions in stock source-suite debug snapshots

- **Status:** proposed; owner approval required before implementation or adoption
- **Date:** 2026-10-31

## Context

Issue [#627](https://github.com/Chelis-Lang/sail/issues/627) blocks the original
stock-with/without-shim source-suite control. The unchanged stock verifier
serializes `func-details` object entries and the two proof-note sets in hash
order (VI-0008 and VI-0021). Its source test helper at
`source/rust_verify_test/tests/output_json.rs:277` prints the parsed result
with `dbg!(json)` before checking individual function details. These incidental
snapshots reach the test's stderr and the strict comparison.

Two complete native controls fail on different members of this same group.
The first has 4,862 Same, four Unstable and one Different; the next has 4,864
Same, two Unstable and one Different. All exact fork arms pass. The retained
original bytes and source/runtime scope are in
[the deep-tier record](../validation/deep-tier.md) and
[the performance qualification](../validation/v020-performance.md).
This proposal does not change either failed result.

The operating guide says, "Never weaken a conformance check, golden, test or
benchmark baseline to make a change pass. If a check looks wrong, raise it
with evidence." Changing this order requirement needs an explicit owner
decision. No exception is implemented or authorized by this proposal.

## Decision

If approved, permit exactly these two order differences in the active pinned
**stock source-suite debug snapshots**, without modifying stock sources:

1. The order of entries in the `func-details` object.
2. The order of strings in a function detail's `obligation_proof_notes` and
   `failed_proof_notes` arrays, which the pinned producer stores as sets.

Bind recognition to the known stock source pin, the `output_json` tests and
the exact debug-helper location/header. Parse the complete debug value with
strict bounds and reject duplicate keys, duplicate notes, malformed or unknown
syntax. Compare every key, string, value, element count and all other array
orders exactly. Keep all surrounding stderr and every actual diagnostic exact.
An unrecognized fragment retains the original comparison and fails closed.

Retain the original raw stdout/stderr and original strict differences in the
report. Record each accepted order exception separately, never silently
convert it to an ordinary exact match. All libtest outcomes, nonignored
baseline-success requirements and missing/extra test checks remain unchanged.
The fork source suite keeps its existing byte-exact comparison everywhere.
Corpus verdict/diagnostic comparison and all solver input/cache keys remain
unchanged. No runtime query normalization or solver option is introduced.

Before adoption, require original real failing snapshots, negative controls
for changed verdicts/messages/notes/counts/ordinary array orders, unknown and
malformed debug fragments, exact fork refusals, and a complete native source
suite. Document the accepted scope in the charter appendix and divergences.
Until owner approval and qualification, #627 remains a release blocker.

## Consequences

The stock oracle would continue to check complete values while acknowledging
two reproduced serialization-order defects in an incidental test dump. This
explicitly relaxes those two order requirements; it is not a product fix or
evidence that stock output is deterministic. Other serialization differences,
all real diagnostic changes and the Mac stock/fork verdict disagreement #634
remain failures. Changes to the stock pin/helper require renewed qualification.
