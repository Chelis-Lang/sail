# 0005: Shim pass-through, observation tap and fail-safe rules

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Charter rule 10 says failures fail safe: the shim falls back to the pinned
Z3, and no fault may produce a wrong answer. M2 must show the integration
point is faithful before any caching exists. A recording mode is also needed
to capture the query corpus (charter deliverable 3).

## Decision

- **Pass-through.** `sail-z3` runs `$SAIL_Z3_PATH` with its own argv
  unchanged. Two threads copy stdin to the solver and the solver's stdout to
  stdout, one read at a time, flushing every chunk. Stderr is inherited. The
  shim exits only after the solver has exited and all of its output has been
  delivered. The solver's exit status becomes the shim's (death by signal *n*
  becomes 128 + *n*).
- **Setup failures** are the only case where the shim speaks for itself. It
  exits 125 with a `SAIL2xxx` code on stderr:
  - SAIL2001: `SAIL_Z3_PATH` is unset;
  - SAIL2002: it cannot be resolved;
  - SAIL2003: it names the shim itself;
  - SAIL2004/2005: the solver cannot be started;
  - SAIL2006: the solver is lost.

  The shim never invents a solver reply.
- **Observation tap.** When `SAIL_TRACE_DIR` is set, the forwarding threads
  hand each chunk to a tap thread through a bounded channel using
  non-blocking sends.
  - If the channel fills, the tap is abandoned and the trace is marked
    incomplete. Forwarding never waits on it.
  - The tap parses, models, keys and writes raw transcripts plus a JSON-lines
    trace (`trace_version` 1).
  - A tap failure (unwritable directory, parse error, solver error reply)
    never touches the forwarded bytes. Integration tests check this with
    hostile solvers and an unwritable trace directory.
- **Layering.** The shim may depend on `sail-key` (pure computation) for trace
  keys and solver identity, in addition to `sail-smtlib`, `sail-session` and
  `sail-api`. It still may not reach the daemon, cache, solver pool,
  portfolio or Verus crates (`xtask/src/layering.rs`).

## Consequences

- **Fail-safe shape.** When the daemon path arrives, the fallback is this
  pass-through. The survey adds a requirement: before replying from anything
  other than the local solver, the shim must either decide on fallback first,
  or be able to replay the accepted prefix into a fresh pinned Z3.
- **Known limitation.** If the shim is killed with SIGKILL, the solver keeps
  running until it reads end-of-file, after finishing its current command.
  Verus never kills its solver, so this matters only for interrupted runs.
  Fault injection in the daemon wave must cover it.
- Real-solver equivalence is tested by `crates/sail-shim/tests/z3_transcript.rs`,
  which is ignored without `SAIL_TEST_Z3`, and by the conformance harness (M3).
