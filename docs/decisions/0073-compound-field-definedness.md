# 0073: Preserve compound-field definedness and qualify retained lowering

- **Status:** accepted
- **Date:** 2026-10-23

## Context

Issue 323 reproduced a neutral source job returning `Proved` while evaluating its
runtime guarantee overflows for an admitted input. A scalar field selection used
the correct mathematical base value but reset its definedness to true. Existing
version-one evidence cannot be silently promoted after correcting that translation.

## Decision

Field projection keeps its base definedness and its existing scalar conversion.
New lowering and newly executed source jobs use semantics version two; contract
syntax and service protocol versions stay unchanged.

Retained manifest admission uses the corrected projection under the recorded
version-one or version-two header and requires complete exact manifest equality.
This preserves byte-identical unaffected version-one evidence without retaining
a faulty translation implementation. Affected historical digests fail closed.
Unknown versions fail explicitly. The original source, stdout, stderr and backend
report remain immutable; no validation failure is relabeled a logical refutation.

## Acceptance

Compile the original and entry-guarded neutral sources against the real runtime
and exercise their exact boundary inputs. Run both sources through the actual
owned source-job service and retain their independent `NotProved` and `Proved`
outcomes, plus original solver reasons. Recheck safe historical jobs/baselines
through restart and pruning and reject affected history with intact content seals.
The ordinary Verus differential gates remain required and unchanged.
