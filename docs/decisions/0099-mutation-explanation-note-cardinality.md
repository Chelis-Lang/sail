# 0099: Count mutation-harness explanation notes by code

- **Status:** accepted; correction of existing explanation-note classification
- **Date:** 2026-11-03

## Context

The unchanged native Linux CapybaraKV catalogue records matching stock/Sail
verdicts, complete diagnostics and exits, but marks its cold samples with
`more than one bundle-miss notice in one invocation`. Each contains one
`SAIL3138` explanation for the shipped-vstd miss and one `SAIL3146` explanation
for its unqualified retained dependency profile. The existing separator
explicitly recognizes both notes; the runner incorrectly counts their combined
length as repeated bundle-miss output. No Verus diagnostic disagreement or
false verified result is observed in these samples. The original failed rows
and complete raw output remain evidence.

Tracked in [#655](https://github.com/Chelis-Lang/sail/issues/655). The first
cold witness contains 675 compared functions and zero disagreements. Its
header, original failed sample and both complete stdout/stderr pairs are in
`capy-notice-witness.tgz`, SHA-256
`4d6c027da793fd4decc73530ca6ee5788f490a6df5c9457422e6d4a6d92d8a40`.

## Decision

Require at most one of each of the two already recognized explanation
codes per invocation. Keep the exact existing prefix/suffix recognition.
Repeated instances of either code, including different text under the same
code, remain failures. Malformed or unknown notes and every Verus diagnostic
remain in the exact comparison. Both the runner and retained-log review use
the same cardinality check.

Do not rewrite original samples or rerun timings to replace them. Any later
classification of retained samples must preserve their original hashes and
failure, bind the exact raw stderr to its two recorded notes, and report the
new interpretation separately. Verdict, function, exit, provenance, daemon,
return and timing/memory checks remain unchanged. Qualification requires the
real retained witness and negative controls before adoption.

## Consequences

Independent existing explanations can coexist without a spurious duplication
failure. This changes harness classification only; Sail, Verus, solver input
bytes, proof authority and cache keys are unchanged. The original strict
performance attempt stays failed. A separately qualified retained-output recheck
may apply the corrected per-code check without rewriting its original records.
No new diagnostic exception is introduced; this corrects the cardinality of
explanations the comparator already recognizes. Final release publication
requires separate owner approval.
