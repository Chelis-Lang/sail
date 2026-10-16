# 0053: Disable unqualified native-artifact seeding

- **Status:** proposed; isolated fallback candidate, not selected
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

Propose disabling both v1 consumption and publication in managed runs,
including when `SAIL_DEPENDENCY_STORE_ENTRIES` is positive. Keep existing data
intact and run normal Cargo/Verus with the existing exact solver cache. The
change adds no warning or diagnostic and does not alter verifier arguments,
receipt validation, solver input or the comparator.

Keep all existing tests and performance requirements unchanged while assessing
this candidate. Its default-enabled unit assertion and strict store-reuse
acceptance cases are expected to fail. Changing those requirements needs an
explicit decision supported by actual reproduction and performance evidence;
this proposal does not waive them or authorize a merge.

## Consequences

Fresh projects and worktrees lose cross-project compiled-vstd seeding. Reusing
an existing managed target and exact solver-query reuse remain separate paths.
Earlier v1 timing results remain historical evidence, not candidate results.
Native reproduction, measured cost, ordinary correctness gates and the owner
decision remain pending. No runtime pin or release asset changes.
