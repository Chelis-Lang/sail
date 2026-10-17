# 0053: Disable unqualified native-artifact seeding

- **Status:** accepted containment policy; revised CI layout qualification pending
- **Date:** 2026-10-16
- **Amends:** [ADR 0045](0045-dependency-store.md)

## Context

[#187](https://github.com/Chelis-Lang/sail/issues/187) demonstrates that v1
dependency-store seeding can suppress a fresh native compilation failure.
Replacing a private `cc` executable at the same `PATH` made fresh stock
Cargo-Verus fail while a seeded Sail run reported success. The retained macOS
reproduction has SHA-256
`7bea96ebf3bd10a292ec6508bb7322a8a799fbc25ef024a89cf256d90783f51c`.

The proposed replacement also has an unresolved APFS input-guard failure.
Neither identity model is qualified. Checking a few familiar tool names would
not establish every native input. The correctness contract requires either a
correct result or normal pinned-tool execution.

## Decision

Disable both v1 consumption and publication in managed runs,
including when `SAIL_DEPENDENCY_STORE_ENTRIES` is positive. Keep existing data
intact and run normal Cargo/Verus with the existing exact solver cache. The
change adds no warning or diagnostic and does not alter verifier arguments,
receipt validation, solver input or the comparator.

After the three-round native assessment, the owner authorized sequencing the
correctness repair while retaining real performance gains as a project
requirement. Replace the old default-enabled/store-seeding assertions with
strict refusal checks, fresh native compilation and the changed-compiler
regression. Preserve same-project receipt reuse, per-function comparisons,
diagnostics, conformance and solver-cache checks. Record the original red
assessment gate; it is not a passed gate for the integrated implementation.

This is temporary containment. A qualified fast replacement and recovery of
first-run performance remain required. No correctness comparator or benchmark
baseline is relaxed, and the lost store speedup is reported explicitly.

## Consequences

Fresh projects and worktrees lose cross-project compiled-vstd seeding. Reusing
an existing managed target and exact solver-query reuse remain separate paths.
Earlier v1 timing results remain historical evidence. On a c7i.4xlarge in
us-east-1, three fresh-project rounds had medians of 1.28 s with v1 seeding,
32.57 s with containment and 34.36 s for fresh stock. In every changed-compiler
round, v1 incorrectly succeeded while containment and fresh stock failed with
the replacement compiler's diagnostic. The
[validation record](../validation/dependency-store-fail-closed.md) gives source,
raw evidence and limits. Native ordinary gates, full FAST and STANDARD pass
at the stated source identities; the revised parallel CI layout and final CI
remain pending. No runtime
pin or release asset changes.
