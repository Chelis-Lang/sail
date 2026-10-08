
## Update 2026-10-09: opt-in stack-key mode deferred, with reasons

The owner approved a stack-only key (v1) as a named opt-in mode, with models
recomputed on demand (ADR 0010). Building it was deferred after this
analysis:

- **What it would gain.** It would help mainly after an edit inside a
  bucket: later checks whose stack is unchanged could hit, where the prefix
  key misses. A single-module file edit, such as `vectors.rs`, gains nothing
  today.
- **Why a served answer can't stand alone.**
  - Verus follows every check with `(set-option :rlimit 0)` and
    `(get-info :all-statistics)`, and subtracts successive `rlimit-count`s.
  - A check-sat answer served by stack key, from a different history, leaves
    those statistics without a source. Making them up is ruled out by the
    prior-art lessons and by charter rule 3. Replaying them from the other
    history gives wrong or even negative per-function rlimits.
  - Every reply after a stack-key hit therefore still needs a solver at the
    exact current history, which has to execute the check anyway. That cancels
    most of the gain.
- **VI-0001 limits it too.** Verus's nondeterministic declaration order
  changes the stack bytes as well, so the stack key would not recover those
  misses either.

**Revisit if** Verus offers a way to skip statistics, or if a warm
frontend (the charter roadmap) changes how sessions are formed.
