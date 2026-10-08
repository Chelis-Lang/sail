# 0004: Session-state model and query keys

- **Status:** accepted (the key's scope is provisional, see below)
- **Date:** 2026-10-08

## Context

Charter rule 2 requires a cache key to bind the complete logical state at
`check-sat`, every option in effect, and the exact solver identity. Rule 5
forbids normalization by default.

The survey (`docs/survey/verus-z3.md` §9) found that Verus runs every query of
a bucket in one long-lived Z3 process, with `(push)`/`(pop)` around each
query. Z3 is incremental. Its answer under a resource limit, and the model it
returns, may depend on the process's **history** (term ids, lemmas that
survive `pop`, random state), not only on the stack in force at the check.
`rlimit-count` is cumulative per process. Nobody has measured this yet for
Z3 4.16.0 on Verus queries.

## Decision

- **`sail-session` models a session conservatively.** It tracks:
  - the stack of push levels, each holding its assertions, declarations and
    definitions as exact bytes;
  - the latest value of every option (`push`/`pop` do not scope options);
  - `set-logic`/`set-info`;
  - its position in the history (commands and prior checks since start or
    `reset`).

  Unknown commands, malformed commands and `pop` underflow mark the session
  *inexact*. That never reverts, not even after `reset`.
- **`Snapshot::encode_state` gives an unambiguous, length-prefixed
  encoding** of the levels (with their boundaries), the options, the globals
  and the check command. History position is deliberately not part of it.
- **`sail-key::query_key` (v1) is SHA-256** over: a domain tag; the mode
  (default, or a named opt-in mode); the solver identity (name, the version
  string Z3 reports, and the binary's SHA-256); and the encoded state. Inexact
  snapshots have no key.
- **Keys bind the stack only for now; history is measured first.** Key v1
  binds the stack, not the history, and it is used only for measurement in
  traces and the M3 profile. No cache may answer from it until M3's
  history-dependence experiment shows that a check's answer and follow-up
  replies are a function of the v1 state. The experiment replays each
  captured query in a fresh process with only its stack, and compares the
  verdict, the model and the `rlimit-count` delta with the in-session run.
  - If they are, the key stays.
  - If not, the default-mode key must also bind the full command prefix of
    the process (as the survey recommends), recorded in a follow-up ADR.
- **No normalization.** Byte differences in commands produce different keys,
  even where they are only whitespace.

## Consequences

- The charter's fast-tier properties are tested in
  `crates/sail-key/tests/properties.rs`:
  - equal states give equal keys;
  - an extra assertion, a changed option, an extra level boundary, a
    different solver version or binary, or a different mode each give a
    different key;
  - length prefixing prevents concatenation collisions.
- **Hit rates depend on the keys.** Binding history would cut cross-run hits
  sharply. Only exact replay of a whole bucket would hit. The measurement
  decides this; preference does not.
- Run-to-run byte differences in what Verus sends (`HashMap` ordering,
  `--log` whitespace) cost hit rate, never correctness.
