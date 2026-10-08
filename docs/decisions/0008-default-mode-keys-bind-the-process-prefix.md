# 0008: Default-mode keys bind the whole process prefix

- **Status:** accepted
- **Date:** 2026-10-08
- **Supersedes:** the provisional key scope in ADR 0004 (key v1 stays, for
  measurement only)

## Context

ADR 0004 deferred the key's scope to an experiment: is a check's reply a
function of its assertion stack alone (key v1), or of the whole history of
its Z3 process? `sail-bench history` (`docs/survey/solver-behavior.md`) ran
every one of the 3,645 checks in the fast-tier example and vstd sessions in
a fresh process, with only that check's stack and options, on the pinned Z3
4.16.0:
- **Answers** matched the in-session answer for 3,645 of 3,645 checks.
- **Model bytes** differed for 64 of 97 models. The error labels Verus reads
  from them agreed in all 97.
- **`rlimit` deltas** differed for 2,617 of 3,644 checks, by up to 17×.
- **Control:** the first check of each session, which has no prior history,
  matched in 648 of 650 sessions. The 2 exceptions are nonlinear spinoffs
  with the known rlimit jitter
  (`corpus/repros/z3-nla-rlimit-nondeterminism/`).

Charter rule 3 requires the exact solver reply bytes for every follow-up,
models included. Verus also reports per-function rlimit derived from the
`rlimit-count` replies. A reply produced in a different history is therefore
not the reply plain Verus would have seen.

## Decision

- **Key v2 binds the whole command prefix.** The default-mode key (v2) is a
  hash over:
  - the mode;
  - the solver identity: name, version, binary SHA-256 and platform;
  - **every command byte the process received since it started**, in order,
    up to and including the check.

  It is computed as a running hash in `sail-key`, so extending it per
  command costs a constant.
- **Cached entries store every reply.** An entry stores the reply bytes for
  the check and for every follow-up up to the next state-changing command
  (models, `reason-unknown`, statistics). Because statistics replies are
  part of the same history, stored `rlimit-count`s stay consistent with
  plain Verus.
- **Key v1 (stack only) stays** for measurement, and as the candidate key
  for a future **opt-in, labelled** mode. That mode would accept verdicts
  across histories and recompute models on demand. It needs its own ADR and
  an entry in `docs/divergences.md` (open question 8).
- **Layering.** `sail-bench → sail-key` is confirmed as a permitted edge.
  Key derivation is pure computation, and the analyses need the same keys
  the shim records.

## Consequences

- **Hits come from exact prefix replay.** That is exactly what unchanged
  buckets produce:
  - re-running with no change hits every check, since Verus's pipe bytes were
    identical across runs (ADR 0003);
  - after an edit, the edited bucket hits up to the first changed byte and
    misses from there on;
  - other buckets hit entirely.

  The achievable speedup therefore depends on bucket size and on where edits
  fall. The warm and edit workloads must measure this.
- **Misses after the divergence point re-solve from the changed state.** A
  warm pool helps here only by replaying the cached-equal prefix into a
  solver process once, not by skipping it. The daemon wave designs prefix
  sharing (a trie of process prefixes) on this basis.
- **The nonlinear jitter needs a policy** (open questions 7 and 8). Plain Z3
  varies by a few rlimit units on some nonlinear spinoffs, so cached and
  fresh replies can differ in `rlimit-count` alone. The plan is to treat
  this as plain Verus's own variability, not as a sail divergence, but the
  owner decides.
- **v1 remains in the traces.** The shim's traces keep recording key v1
  until v2 lands with the cache wave. They will then record both, so hit
  rates can be compared.
