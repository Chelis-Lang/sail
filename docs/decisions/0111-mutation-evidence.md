# 0111: A Verus rejection counts as a mutation detection

- **Status:** accepted by owner, 2026-11-10
- **Date:** 2026-11-10
- **Amends:** the evidence rule of
  [ADR 0100](0100-mutation-first-roadmap.md) ("never convert Unknown into a
  kill") for `sail mutate` campaigns, and settles the evidence decision that
  [ADR 0109](0109-oyster-retired-v030-scope.md) left to the owner
- **Supersedes:** the interim rule for Verus campaigns that kept rejections
  and solver-confirmed kills as separate counts with no combined total
- **Unchanged:** solver answers, Verus verdicts and diagnostics, plain
  `sail verify` output, the benchmark mutation catalogues, neutral mutation
  and scalar literal witnesses, which keep their own classification

## Context

ADR 0100 says a mutant whose rejection rests on a solver `unknown` is never a
kill. The [brief](../design/verus-mutation-release.md) carried that rule into
`sail mutate`. The #664 spike
([record](../validation/mutation-evidence.md)) ran it against real projects
with stock Verus and the pinned Z3:

- 39 of 40 exec-token mutants on IronKV and Vest were rejected, each with a
  Verus error attributed to the mutated function.
- All 68 mutants gave the same outcome in each of five fresh runs.
- Every one of the 39 rejections rests only on Z3's
  `unknown "(incomplete quantifiers)"`. Native `sat` appeared only in
  bit-vector queries. This is how Verus reports a failed obligation that
  involves quantifiers. It is not a sign of a weak result.
- The Vest pin's baseline always fails one function on its rlimit, so
  survival has to be judged against the baseline, not against exit status.
- Sail's report had no attributed solver answers for 3 of the 39, because
  several functions share one friendly name.

Under the old rule almost every real mutant would be "inconclusive", and the
campaign would say nothing about specification strength.

## Decision

The owner decided on 2026-11-10 that a Verus rejection counts as a detection,
under the rules below. They apply to `sail mutate` campaigns only.

### Outcomes

Every mutant gets exactly one outcome.

- **`rejected`** (counted as detected). All of these must hold:
  1. Verus reports a diagnostic attributed to the mutated function. At least
     one of the function's failed obligations must end in something other
     than a resource limit, a timeout or a cancellation. A resource-limit
     failure in another function is listed next to the outcome but does not
     block it.
  2. The unmutated baseline twin proves with headroom. Its function is
     verified and attributed in the campaign's baseline, under the same
     source identity, runtime and limits. None of its checks is near the
     limit by the ledger's existing rule (ADR 0009): no check used more than
     half of its rlimit, and none gave up from resource exhaustion. The twin
     is not marked `unstable`.
  3. Every repeated run of the mutant and the baseline gives the same
     outcome: the same verdict per function, the same attributed diagnostic
     messages and the same evidence sub-class. Exact solver input bytes and
     stderr line order need not match, because stock Verus varied in both
     between runs. The implementation design sets the repeat count, which is
     at least two. The spike used five.

  Each `rejected` mutant also gets an evidence sub-class. Campaign views
  always show the breakdown:
  - **`counterexample-confirmed`:** a check of the mutated function returned
    native `sat`. The model is kept as it was returned. It is not
    independently validated (ADR 0106).
  - **`verifier-rejected`:** the failed checks returned `unknown` (for
    example `"(incomplete quantifiers)"`), or Verus reported the failure
    without a `sat`.
- **`survived`:** the mutated function verified, and no function failed
  beyond the baseline's own failures. An unscoped run of the whole unit must
  confirm it. A survivor seen only in a run scoped to the mutated function or
  module is **`survived-scoped`** until that unscoped run passes. Both are
  observations of specification strength. Neither is a claim of adequacy or
  vacuity.
- **`inconclusive`**, never detected. Each carries a named reason:
  - the mutated function's only failures are rlimit, timeout or
    cancellation;
  - the baseline twin is near its limit, unstable, failed or missing;
  - the diagnostic cannot be attributed to the mutated function. This covers
    a span with no function, a rejection only in other functions, and a
    friendly name shared by several functions. A mutant stays inconclusive
    until its attribution is unambiguous;
  - the repeated runs differ.
- **`unviable`:** the mutant does not compile as Rust, or Verus rejects it
  before verification (a mode error, for example).
- **`unsupported`:** the mutator or runner cannot handle the shape, so the
  mutant was not run.

### Score

The score is `rejected / (rejected + survived)`, with `survived` including
`survived-scoped`. Every view that shows a score also shows those two counts,
the evidence breakdown, and the counts of inconclusive (by reason), unviable,
unsupported, omitted and cancelled mutants. Those other counts are excluded
from the score. With no rejected or survived mutants there is no score.

### What detection means

Documentation and agent-facing output say it plainly: **detected means that
the specification together with its proof rejects the change.** It does not
mean the specification provably forbids it. A `verifier-rejected` mutant
might be correct code that the existing proof cannot carry.

### Where the classification appears

The outcomes, sub-classes and score appear only in campaign views: the
`sail mutate` CLI, its JSON and the MCP tools. Plain `sail verify` output,
its JSON, verdicts, diagnostics and the ledger never carry them. Solver
answers stay verbatim everywhere. An `unknown` is still reported as
`unknown` with its reason, including inside the campaign evidence of a
`rejected` mutant.

## Consequences

- Real campaigns give a usable signal: in the spike, 36 of the 40 exec-token
  mutants would have scored as rejected, 3 would have been inconclusive
  because of duplicate names, and one would have survived. This is a
  retrospective reading; the spike did not check baseline headroom.
- The campaign has to run a baseline, keep each twin's per-check rlimit use,
  and repeat every mutant. That cost belongs in the campaign cost
  measurements the brief requires.
- Ambiguous friendly names now cost detections. Attribution that tells
  same-named functions apart turns those mutants back into rejected or
  survived ones without changing the rule.
- A `verifier-rejected` detection can hide a correct but unprovable change.
  The evidence breakdown keeps that visible, and the wording rule keeps it
  from being read as a proof.
- The benchmark catalogues, neutral mutation and scalar literal witnesses are
  unchanged. Any change to them needs its own decision.
