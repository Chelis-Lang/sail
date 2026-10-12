# VI-0013: Stock Verismo tspec waits until the unit deadline

- **Status:** open; cause unknown. Observed locally, not filed upstream.
- **Verus:** pinned `0.2026.10.04.426d8b0`, source
  `426d8b01e7ffb910a36c15e1f870ff985bb61eef`, pinned Z3 4.16.0.
- **Environment:** Linux x86-64 container under emulation on arm64 macOS.
  This identifies the environment; it does not establish emulation as the cause.
- **Found:** 2026-10-11.
- **Input:** corpus project `verismo`, commit
  `6bc6dd326d8b9ca30ffec57a544b13b5fadf3cb3`,
  `source/verismo_tspec/src`. This is the retained project reproduction;
  a minimal source reproduction is not yet available.
- **Impact:** the latest complete default standard attempt has 158 matching
  units and one infrastructure failure at the original 3,600-second plain-arm
  deadline. The Sail arm of this unit and cache fill/warm were not reached.
- **Workaround:** none adopted. Do not change solver settings, thread counts,
  deadlines or the conformance classifier to turn this failure into a pass.

## Observed phase

The stock compiler had started and launched three pinned Z3 children. At the
retained instant all sixteen observed verifier threads were sleeping, its CPU
count had not advanced for about 504 seconds, and the solver children were
waiting for input. Later non-consuming pipe occupancy checks found no queued
bytes in the nine solver stdio descriptors. There was no complete result or
final verdict before timeout.

These observations do not expose AIR's userspace channels or buffers, identify
an owning lock, or establish a compiler, solver, harness or emulation defect.
The sampled futex program counter was absent from the retained memory maps.
The harness was draining output on independent threads and polling its original
deadline; no capture-file write wait was sampled. A full userspace stack or
source-level wait dependency is still needed before choosing a correction.

The later [transcript diagnostic](../validation/m7.md)
was interrupted by filesystem/container failure. Its 3,880 logged batches all
have responses; the newest bit-vector transcript is empty. Pinned source
allows that log to exist before its lazy solver launch. The query context is
a fresh iteration-local value, not an assignment replacing the prior context.
This narrows the next observation seam, not the historical
cause. The [phase diagnostic follow-up](../validation/m7.md)
specifies the missing context, channel and pipe ownership evidence; no new
execution is authorized by that plan.

The earlier standard attempt also timed out in `bitflags_verus`; one separate
ordinary bitflags diagnostic later completed both sides, 48 verified and zero
errors. That scoped pass neither explains nor reclassifies either earlier
standard failure. Historical managed/cache issue #68 has a different runtime
and workflow; similar sleeping symptoms do not establish a shared cause.

## Reproduction and evidence

The corpus harness prepares the pinned Cargo project and Verus dependency
patches. Use the normal local conformance command with a new output directory:

```sh
ci/local.sh conformance standard --project verismo --filter verismo_tspec
```

This selects ordinary default verification; it is local-only. A new execution
is a separate diagnostic, not recovery of an old result. Preserve its complete
inputs, actual compiler and solver bytes, original flags and 3,600-second bound,
partial streams, terminal outcome and owned process cleanup. If investigating
waits, collect supplementary stacks without consuming actor streams or changing
proof authority. Static reasoning alone cannot qualify the unit.

The [complete terminal record](../validation/m7.md),
[retained phase audit](../validation/m7.md) and
[sleeping/pipe observation](../validation/m7.md)
bind the original frozen harness, project/runtime identities and observations.
No retry, deadline extension or normalization is part of those records.
