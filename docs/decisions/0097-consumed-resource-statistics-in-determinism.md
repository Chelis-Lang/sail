# 0097: Distinguish consumed resource statistics in determinism qualification

- **Status:** accepted; owner approved the narrow classification on 2026-11-02
- **Date:** 2026-11-02
- **Related:** ADR 0009's classification of rlimit consumption as metadata

## Evidence

Native mac-m4.metal, us-east-1c, macOS 26.7, fork `a0e11cff`, manifest
`6bdbf6c1e51757212463ac0f491e07e7d9cacb20920264738d5bcf596f262173`,
pinned upstream `7d829946` and Z3 4.16.0. The unchanged strict three-run tool
finds all solver streams stable across 417 inputs. On `examples/imo_1988_6.rs`
it fails stdout comparison: the consumed total is 82,316 versus 82,312, with
the same four-unit change in one module and function consumption field.
All other stdout values, ordering and bytes after the existing wall-time
handling, complete stderr and exit codes agree. The five solver streams agree
exactly, including requested limits. The original strict result remains failed:
416/417, with zero varying solver buckets.

Raw three-run witness and full original log: `mac-rlimit-drift/evidence.tgz`,
SHA-256 `ac41746970b24c96a9b2a0de8780895202fc2e4b45a9ace92188026deaea68a7`.
This does not establish a new compiler or solver defect or a changed verdict.

## Decision

Keep the strict tool available and its original failed capture. Add an explicit
qualification mode which separately reports consumed-counter differences only
at these known Verus output locations:

- `times-ms.smt.rlimit-run`;
- `times-ms.smt.smt-run-module-times[].rlimit`;
- `times-ms.smt.smt-run-module-times[].function-breakdown[].rlimit`.

Require complete, bounded, unambiguous JSON and nonnegative integer counters.
Preserve every other stdout byte and order after the existing wall-time handling.
Unknown paths, duplicate fields, malformed data or any other difference retain
the strict failure. Report each admitted metadata difference and its actual
values separately; never silently label the original strict comparison a pass.

Exact solver input bytes and multiplicity, requested limits, per-function
verdicts, complete diagnostics, stderr and exits remain unchanged and exact.
No query normalization, runtime cache-key change, solver fork, consumption
fabrication or relaxation of an actual verifier test is authorized.

Before adoption, exercise the real witness plus controls changing query bytes,
requested limits, verdicts, diagnostics, unrelated values and ordering, and
unknown/malformed/duplicate JSON. Complete both-platform native qualification
retains the original strict captures and the explicit metadata classifications.

The operating guide says “Never weaken a conformance check, golden, test or
benchmark baseline to make a change pass.” This owner-approved classification changes
the strict determinism gate's treatment of these three consumption fields;
the owner approved that narrow change on 2026-11-02. It does not authorize
changes to any other comparison.


The candidate implements `--classify-consumed-resources` in
`scripts/verus-determinism.py`, using the bounded lossless
`scripts/verus_resource_statistics.py` helper. Native controls and the complete
retained-capture recheck are recorded in
[the runtime qualification](../validation/binary-compiler-inputs.md#adr-0097-implementation-and-native-qualification).
