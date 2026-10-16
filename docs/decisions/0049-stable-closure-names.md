# 0049: The fork names closure types by definition path

- **Status:** proposed
- **Date:** 2026-10-16

## Context

Verus names each closure type after rustc's crate-wide `DefIndex` of the
closure: `anonymous_closure%<N>` (sort, `TYPE%`, `Poly%` and related
declarations) and `impl_closure&%<kind><N>` (its `Fn*` impl and the axioms
derived from it). rustc numbers definitions crate-wide in source order, so
inserting any item renumbers every closure after it in the crate. Modules the
edit did not touch then get different solver input from their first check,
and Sail's prefix-bound cache misses them (VI-0019, issue #118).

On vest, inserting one small lemma in `asn1/der_ord.rs` (the `lemma-mid`
edit of `scripts/perf-real.sh`) changes three of 298 SMT logs. Two are other
modules, `cbor::format` (25 of 25 checks) and `combinators::recursive::exec`
(6 of 6), and the only tokens that differ there are closure-type numbers.
The [real-project report](../reports/2026-10-15-real-project-performance.md)
measured this edit at 39.8 s under Sail against 27.9 s for an all-hits run.
Comments and blank lines change no query bytes; the closure numbers were the
only crate-wide counter found.

ADR 0039 lets a fork fix only choose an output stock can already produce, or
correct structured output. Renaming does neither literally: for a given
source, stock always prints the `DefIndex` numbers, and the fixed fork prints
others. This ADR records why the change is still safe to run in default mode
and how it is validated.

## Decision

1. **Closure types are named by a stable number.** The fork (VI-0019) names
   a closure type by the local half of the closure's `DefPathHash` instead
   of its `DefIndex`. That hash depends only on the closure's own definition
   path: its enclosing items' names and rustc's per-parent disambiguators.
   rustc guarantees it is unique within a crate (it aborts compilation on a
   collision), so two closures never share a name. Closure declarations keep
   stock's order by `DefIndex`, with the stable name breaking ties across
   imported crates (`3f7f6c51`). Equality and hashing of closure types use
   the new number alone.
2. **What may change, exactly.** For any input, the fork's SMT output equals
   the previous fork's output after a one-to-one renaming of closure-type
   numbers, consistent within each solver session, plus the line breaks the
   printer moves because names got longer. No declaration, assertion or
   check is added or removed. Declarations with distinct original indices
   retain their order; imported closures with equal indices now use a stable
   tie-break instead of hash iteration order. Verdicts and diagnostics do not
   change, because closure-type names never reach a diagnostic and the
   solver sees an alpha-renamed problem.
3. **Why this is acceptable under ADR 0039.** Solver input that differs only
   by consistent symbol renaming is the same logical problem. Z3 could still
   behave differently on it (symbol names can affect hashing inside the
   solver), the same exposure stock already has whenever an unrelated edit
   renumbers closures. ADR 0039's rule exists to keep the fork's verdicts
   equal to stock's; for this fix, that is enforced by evidence instead of
   by construction:
   - conformance, Sail on the candidate fork against plain stock, `fast` and
     `standard` tiers, 0 Different in every arm;
   - the fork's full `rust_verify_test` suite, with the regression test
     `sail_vi0019_closure_ids`;
   - the renaming check `corpus/repros/verus-closure-renumbering/renaming-equal.py`
     on a real project's SMT logs.
4. **No change to Sail's keys.** Cache keys still bind the exact solver
   bytes (ADR 0008). The fix only makes those bytes independent of unrelated
   items. Nothing in Sail normalizes queries (charter rule 5).
5. **Prove the gain before pinning.** The re-pin needs the measured gain on
   the vest `lemma-mid` edit, recorded in
   [VI-0019](../verus-issues/VI-0019-crate-wide-closure-numbering.md).
   Issue #118 also requires the broader frozen incidental-edit catalogue:
   comments, blank lines, helper insertion/relocation and local renames on
   three pinned projects. The [acceptance record](../validation/incidental-edits.md)
   distinguishes the prepared harness from outstanding final-runtime timing,
   solver-work and correctness gates.

## Consequences

- An inserted item no longer invalidates unrelated modules that use
  closures. The edited module itself still changes, as before.
- Closure names are longer (up to 20 digits instead of about 4), which adds a
  few bytes per mention to the SMT input.
- The number depends on the crate's `StableCrateId` (crate name and Cargo's
  `-C metadata`). It is fixed for a project's configuration, and it changes
  together with other crate-identity inputs when that configuration
  changes.
- Other per-parent numbering remains: `impl&%<N>` names follow rustc's
  disambiguator within a module, so adding an `impl` block renumbers later
  impls in the same module, which can change other modules that mention
  them. No measured edit showed it, and it is not addressed here.
- Each upstream rebase must keep the regression test passing; if upstream
  changes closure naming, the fix is revisited.
