# 0012: Warm and edit workloads

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter's workloads include *warm, no change*, *single-function body
edit* and *specification edit*. ADR 0007 defines the cold workload and the
result format, and expects later workloads to reuse that format with a
different `workload` value. ADR 0011 added a disk cache served by
`sail serve`. These workloads measure that cache.

Two constraints shape the harness:
- The layering table forbids `sail-bench` from linking `sail-daemon` or
  `sail-cli`.
- The charter's metrics include hit rates per cache tier. The daemon already
  counts `GET`s, hits and `PUT`s (`sail_daemon::Stats`), but nothing could
  read those counts from outside the process.

## Decision

- **Daemon per unit.** `sail-bench` starts `sail serve` as a separate
  process: the `sail` binary next to `sail-bench`, found the way `sail-z3`
  is.
  - Each unit (warm), and each round of each edit (edit), gets a fresh
    daemon and an empty cache root under `<cache>/bench/<run>/`. The socket
    is `<cache>/bench/<run>/d.sock`; paths of 100 bytes or more are
    refused, because of the Unix socket path limit.
  - The sail side runs with `SAIL_DAEMON_SOCKET` set, through a new
    `daemon_socket` field on `sail_verus::run::Solver::Shim`. `run_unit` now
    clears `SAIL_DAEMON_SOCKET` for every run, then sets it only when that
    field is given. `sail-conformance` passes its own environment's value,
    so conformance runs with the cache on work as before.
- **Warm workload** (`sail-bench warm`). Round 0 is the *fill*: sail with an
  empty cache. Rounds 1 to N are *warm*: sail again, with nothing changed.
  Plain Verus runs in every round, and which side goes first alternates, as
  in the cold workload.
- **Edit workload** (`sail-bench edit`). The edits live in a reviewable
  catalogue, `corpus/edits.toml`:
  - Each edit is one exact textual replacement, which must match exactly
    once.
  - Each edit has a kind: `body` changes only a function body or proof;
    `spec` changes a `requires` or `ensures`.
  - Each edit records the outcome plain Verus gives on the edited file.

  For each edit, the bench copies the project to a scratch directory. The
  fetched corpus tree is never modified. Each round then:
  1. starts a fresh daemon;
  2. runs plain Verus and sail with the cache on the original copy (the
     fill);
  3. applies the edit;
  4. runs plain Verus and sail with the cache on the edited copy.

  Both sides always run on the same copy, so file paths in diagnostics
  match. Only `verus` units can be edited for now.
- **Conformance on every sail run.** Every sail run, fills included, is
  compared with that round's plain run. If they differ, ADR 0009 applies
  against the unit's other plain runs: when plain runs disagree among
  themselves and sail matches one of them, the run counts as `unstable`.
  Otherwise it counts as `different`, which is a bug and never a result.
- **The `STATS` operation.** A fourth operation joins the daemon protocol
  (`sail-api`): op 4, with an empty payload. The response is status `0`
  and a JSON body with `hellos`, `gets`, `hits` and `puts`, the counters
  since the daemon started. `HELLO`s are now counted as well, one per cached
  session.

  The bench reads the counters before and after each sail run. Plain Verus
  never talks to the daemon, so the difference covers exactly that sail
  run. A session stops asking at its first miss, so:
  - `gets - hits` counts sessions that started the solver;
  - `hits` counts replies served from the cache;
  - `puts` counts replies recorded from a solver;
  - the report's *share served* is `hits / (hits + puts)`.

  `STATS` is additive, so `PROTOCOL_VERSION` stays at 1. Clients from
  before it never send it, and are served by a newer daemon exactly as
  before. A newer client talking to an older daemon loses only `STATS`: the
  old daemon rejects the unknown operation and closes that connection,
  `HELLO`, `GET` and `PUT` are unchanged, and the bench reads no counters.
  The bench always starts the `sail` binary from its own build, so this
  case does not arise there.
- **Result format.** The `BenchFile` schema stays at version 1, with new
  fields that all have serde defaults, so existing `cold` files read and
  write back unchanged:
  - `fill`, `cache`, `cache_bytes` and `unstable` on each unit;
  - `edits` at the top level, holding one `EditResult` per edit.

  `sail-bench report` renders `warm` and `edit` files in sections of their
  own. `cold` output is unchanged.
- **Reference numbers.** `.github/workflows/bench.yml` takes a `workload`
  input, so that Linux reference numbers can be produced by dispatch
  (ADR 0010).

## Consequences

- Warm and edit results sit next to the cold baseline in
  `docs/benchmarks.md`, with conformance counts and cache figures.
- The edit catalogue is small and hand-checked. Growing it, adding
  `refactor` edits and supporting Cargo units are later work, as are the
  history-replay and agent-loop workloads.
- `STATS` gives counts per daemon, not per cache tier. When more tiers
  exist, per-tier counters extend the same JSON object.
- The scratch copy for an edit is a full copy of the project. That is
  39 MB for the Verus repository.
