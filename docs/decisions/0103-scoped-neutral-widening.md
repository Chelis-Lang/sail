# 0103: Scoped lifting of the neutral-widening hold

- **Status:** superseded by [ADR 0109](0109-oyster-retired-v030-scope.md): the surface this decision describes was removed in v0.3.0. The last implementation is tagged `archive/neutral-contracts-final`. Previously: accepted by owner, 2026-11-08
- **Date:** 2026-11-08
- **Amends:** the neutral-widening hold in [ADR 0100](0100-mutation-first-roadmap.md),
  for the items listed below only

## Context

ADR 0100 held Oyster integration expansion and said not to widen the neutral
subset. The neutral source path then admitted loop-free, call-free, single-file
scalar code, and it refused specification division and remainder. Ordinary
functions that read buffers, loop over data, use integer division, or split
helpers across modules could only be verified through the ordinary Verus route,
which has no requirement attribution.

On 2026-11-08 the owner lifted the hold for a named set of neutral and
attribution items, and kept it in force for everything else.

## Decision

The hold is lifted for exactly these items:

1. **Qualified truncating division and remainder.** Specification `/` and `%`
   lower with Rust's truncating, signed semantics. Each operation is defined only
   for a nonzero divisor and an in-range result, and the arithmetic qualifier
   names the rule.
2. **Multi-file source closure.** Out-of-line modules, annotated helpers and
   imports within the submitted crate are admitted as one closure. Anything
   outside that closure is refused explicitly.
3. **A wider executable body grammar.** Loops with invariants and decreases,
   calls to admitted functions, casts, indexing, fixed arrays and `match`.
4. **Requirement attribution for the ordinary Verus route.** Function results
   from ordinary `sail verify` runs can carry caller-supplied requirement
   identities.
5. **Decoded requirement-level counterexamples.** A failed neutral proof can
   report the input values of its counterexample by source name, separate from
   the raw solver model.
6. **Cache placement under an inherited Cargo configuration.** Neutral jobs
   whose cache sits under an ancestor Cargo configuration get a placement that
   avoids that configuration, or an actionable refusal. The refusal policy for
   unreviewed configuration is unchanged.

Rules for every item:

- **Explicit selection.** Widened neutral syntax and semantics (items 1 to 3)
  are admitted only under explicitly selected neutral source profile three,
  with lowering version four and contract version three. Profiles one and two,
  lowering versions one to three, and their retained manifests and emitted
  bytes stay unchanged. Profile three is assembled across the pull requests
  for items 1 to 3 and is frozen when it is first released. [ADR 0107](0107-freeze-neutral-profile-three.md)
  amends this: profile three is frozen from 2026-11-09, and forms not yet
  admitted take a new lowering identity.
- **Explicit refusals remain.** Each widening admits a stated form and keeps a
  `SAIL3140` or `SAIL0701` refusal for everything else.
- **The correctness contract is unchanged.** Default-mode verdicts and
  diagnostics stay equal to the pinned Verus and Z3. The native fork and stock
  oracle runs each new form. No check is weakened, and nothing provisional is
  reported as verified.
- **Sail never writes proofs.** Loop invariants, decreases measures and
  contracts come from the submitted source. Lowering adds no invariant,
  assertion or lemma that could make a body pass.

Everything else that ADR 0100 holds stays held. This includes other neutral
profile widenings, new backends, and SDK changes beyond the items above.

## Consequences

Each item lands in its own pull request with its own tests, documentation and
native oracle cases. A release carrying these items needs the owner's approval
of the exact candidate, as for every release. Consumers must select profile
three explicitly and must refuse it on endpoints that do not advertise it.
