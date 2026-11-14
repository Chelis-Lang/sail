# Optional retention decline reasons

Status: implemented plan-admission candidate; uncompiled and unqualified.
Tracks #712 package C and #595; ADR0136 records the boundary.

The candidate returns a typed reason directly from each failed optional
plan-admission check. It preserves the normal compiler fallback, check order,
original successful Plan fields and the existing profile/runtime notes.
There is no thread-local last-step state, publication journal, new cost policy,
source observation or reuse authority.

Required validation: focused metadata-before-storage controls, affected lint,
full local gate, solver and end-to-end retained-reuse controls, PR conformance
and independent review. The malformed-metadata test exercises the production
admission helper and requires both the correct typed failure and no storage
change. No native, corpus, Mac or performance result is claimed yet.

This is an explanation-only change, not a selected-runtime verification-policy
change under ADR0087. Admission checks, solver interaction, keys and proof reuse
conditions are preserved. Review must substantiate that classification; any
runtime-policy change would require its applicable standard corpus gate.

Documentation changed: ADR0136, this record, instant-revert guide and changelog.
The consumer skill needs no change because the existing SAIL_EXPLAIN=1 command
is unchanged. Restore/publication reasons, missing-export recovery and the
cost-gate decision remain open in #712.
