# 0146: Describe original complete-session check positions for profiling

- **Status:** implemented; bounded native Linux component qualification passes, unselected
- **Date:** 2026-11-14
- **Tracking:** #712 B; extends ADR0120/0138/0144

## Context

A profiler must replay the original process prefix, including earlier checks.
An obligation-local check position or a reply index is not that process check
position. Repeated friendly method labels also cannot identify an owner. The
existing AIR/timing join already has the appropriate conservative identity rules.

## Decision

Add a read-only, unselected library collector under
`sail_verus::attribution::profile_inputs`. Reuse the existing named-producer
identity/timing corroboration and exact legacy labels with one timing owner.
Describe each original obligation/check with its existing kind/ordinal/spinoff,
reported full-input SHA256 and zero-based complete-process check index. Do not
reset the process index at function/obligation boundaries or use reply indices.
Duplicate session checks, duplicate obligation identities, missing/ambiguous
owners, unknown kinds, malformed hashes and inconsistent logs refuse the whole
collection. Sort descriptions without rewriting input or inventing identities.

Bound document/log counts, visited timing nodes/text/labels, output checks/text.
Before the existing helpers, also bound the projected timing-row count to
100,000 and all row-cloned text to 8 MiB, counting the containing module once
per accepted row. Input JSON size alone does not bound that expansion.
Preflight timing metadata before invoking the existing join, so its allocations
remain bounded. No dependency edge, solver execution, file access, worker, report
mutation, cache key, currentness or proof authority is introduced.

Descriptions authenticate nothing. A future caller must bind the actual original
invocation/target, solver/mode, transcript and checksum-validated retained bytes,
then select the exact prefix and compare actual plain/traced executions. Public
Rust callers can supply fabricated descriptions; this API cannot make them
trusted. Missing retained input or a failed join means profiling unavailable.

## Consequences

This supplies a profiler join component, not a profile command, automatic job
lookup, session authentication, trace-to-source theorem or default-mode change.
Bounded native fixtures, ambiguous/duplicate/version/bounds controls, complete
workspace gate and fresh independent review pass; the validation record states
actual scope and original failures. Full profiler/product acceptance and caller
authentication remain separate. Consumer commands remain unchanged.
