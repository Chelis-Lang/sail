# 0035: Guarded retained output digests

- **Status:** rejected (2026-10-12, rescue L14): removed, because the owner almost never engaged and saved at most the 1.6 ms output hash per read
- **Date:** 2026-10-11

## Context

Every Status and Result read of a completed job re-reads the job's persisted
stdout and compares its complete SHA-256 with the seal recorded at completion,
before selecting an encoded frame. Under x86 emulation that output-seal stage
measured 15.5 ms at p95, and the reader gate (four clients, p95 below 200 ms)
was failing.

## Proposed decision

An output-only `managed::GuardedOutput` owner would hash the output once under
the same Linux namespace observer, mount-generation coverage and kernel read
leases used for input freshness, and later reads would reuse that digest while
the owner stayed continuously valid. Any doubt fell back to the complete read
and hash. A background constructor in the daemon built owners from the result
frame cache's memory budget.

## Outcome

On a native host (L5 rescue lane report, tag `archive/rescue-m7-98f6292`) the complete
read and hash costs about 1.6 ms of a 25-45 ms read, and 86 of 88 sampled
guarded checks fell back to it anyway, because owners rarely fit the shared
budget. The reader gate passes natively without the owner. Rescue L14 removed
the daemon integration, its tests and its profiler; every read again re-reads
and hashes the output, which was already the path that ran. The unused
`managed::GuardedOutput` library primitive goes with the managed-freshness
rework (rescue L7c). The validation
history is preserved in the `archive/m7-contracts-f7103bf` tag.
