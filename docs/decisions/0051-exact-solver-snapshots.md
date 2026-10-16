# 0051: Solver continuation for changed-check misses

- **Status:** proposed, blocked and unselected; research implementation archived
- **Date:** 2026-10-16
- **Relates to:** [ADR 0008](0008-default-mode-keys-bind-the-process-prefix.md),
  [ADR 0011](0011-local-cache-and-lazy-solver-sessions.md),
  [ADR 0040](0040-warm-solver-sessions.md),
  [#158](https://github.com/Chelis-Lang/sail/issues/158)

## Context

On the first exact-prefix cache miss, the current lazy solver replays the
complete process prefix. Earlier unchanged checks are solved again. The
research evaluated two ways to avoid that foreground work without changing
the solver input: fork a parked pinned-Z3 image, or prepare an independent
fresh process on the exact prefix and consume that live process once.

Forking a process does not establish an identical continuation. Pinned Z3
has timer workers and atfork handlers, historical native comparisons differ
in `num-allocs`, and thread/kernel state needs explicit coverage. The
[minimal reproduction](../../corpus/repros/z3-snapshot-timer-state/README.md)
and [report](../reports/2026-10-16-solver-snapshots.md) preserve those findings.

The live alternative avoids copying process state, but preparation is real
CPU work. Both approaches still require an exact runtime/input identity,
complete response and diagnostic equivalence, resource bounds and measured
usefulness. A solver verdict match alone does not satisfy those obligations.

## Proposed decision and current disposition

Do not select either implementation. The full research branch is retained
in the immutable bundle identified by the qualification record; none of its
shim, API, benchmark or CI integration lands with these findings. Defaults,
the pinned runtime and the existing oracle remain unchanged. Issue #158 is
open, not completed by a negative experiment.

The strict raw gate currently fails even on the three independent fresh
runs of each real trace: clock bytes vary. Both live samples were therefore
skipped. No timing field is masked and no gate is relaxed. ADR 0011 permits
replaying actual stored time/memory figures, but does not itself authorize
normalizing a new continuation oracle. The precise decision needed is in
[open questions](../open-questions.md#solver-continuation-research).

## Consequences

Any next implementation must establish its actual input/output boundary,
fail-safe behavior and resource policy before product integration. It must
account for preparation and total solver CPU rather than only foreground
latency, and record every refusal or failed sample. New acceptance claims
need native whole-corpus verdict/diagnostic qualification, both-platform
behavior and a useful measured gain. Historical masked results, tiny
controls and the present negative report do not qualify that scope.
