# 0011: Local cache, daemon protocol and lazy solver sessions

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Measurements on the pinned corpus show:
- A check's answer depends only on its assertion stack.
- Model bytes and rlimit effort depend on process history.

ADR 0008 therefore binds default-mode keys to the whole process prefix (key
v2). Charter rule 3 requires replaying exactly what the solver said, for
check-sat and for every follow-up. Rule 10 requires failing safe to the
pinned Z3. The charter's dependency structure keeps the shim small: it uses
the protocol, the session model and a thin client to the daemon.

## Decision

- **Cache unit: every reply.** The cache maps a v2 key to the solver's exact
  reply bytes, for *every* output-producing command, not only `check-sat`.
  The key covers the whole process prefix up to and including that command.
  The value is the raw output segment Z3 emitted for it, from the start of
  that reply to the start of the next, including the trailing newline. This
  makes echoes, version replies, statistics, models and `reason-unknown`
  replay exactly through one rule.
- **Lazy solver sessions.** With `SAIL_DAEMON_SOCKET` set and argv
  `-smt2 -in`, `sail-z3` starts in *serving* mode:
  - It reads commands and extends the prefix hash over the raw bytes. For
    each output-producing command, it asks the daemon for that key's reply
    and writes the reply to stdout. No solver is started.
  - On the **first miss** (or any daemon error, an unknown command, or a
    cached `check-sat` reply of `unknown`), it starts the pinned Z3 and
    writes the whole buffered input to it. It discards the solver's first
    *N* replies, the ones it already served from cache, and from then on
    forwards bytes as in pass-through.
  - The solver therefore always sees exactly the history plain Verus would
    have given it, so no reply is ever produced from a different history.
  - If every reply hits, the solver is never started.
- **Recording.** While a solver runs, replies are paired with commands, as in
  the trace tap, and stored under their keys. Recording stops for the session
  at the first solver error, or at any command whose output cannot be
  predicted. `check-sat` replies of `unknown` are not stored: ADR 0009 treats
  every cached `unknown` as unstable until the near-limit threshold is
  calibrated.
- **Daemon protocol.** `sail-api` defines it: length-prefixed binary frames
  over a Unix socket, with three operations:
  - `HELLO`: solver path in, solver identity out. The daemon hashes the
    binary once and caches the result by path, size and modification time;
  - `GET`: key in, reply or miss out;
  - `PUT`: key, reply and metadata in.
- **Daemon.** `sail-daemon` serves the socket and owns the cache. `sail serve`
  runs it. For now it is started explicitly; on-demand start comes with
  `sail verify`.
- **Disk tier.** `sail-cache` stores one file per entry under
  `<cache>/results/v2/<k0k1>/<key>`:
  - a JSON header line (key, reply SHA-256, solver identity, the command, the
    time it was stored, sail's version);
  - then the reply bytes.

  Writes are atomic (a temporary file, then a rename). A read checks the key
  and the reply hash. Any mismatch or parse failure counts as a miss, and the
  entry is removed.
- **Fail-safe.** Every daemon or cache failure behaves as a miss. A miss
  falls back to the replay path, which is pass-through with the pinned Z3.
  Without `SAIL_DAEMON_SOCKET`, the shim behaves exactly as the M2
  pass-through.

## Consequences

- **"Warm, no change" can skip the solver entirely** for every bucket, since
  pipe bytes are identical across runs. An edit costs the edited bucket a
  full re-solve; other buckets still hit (ADR 0008).
- **The first miss in a bucket replays the bucket's earlier checks in the
  solver.** That costs time, never correctness. Prefix sharing in a warm pool
  can recover it later.
- **Statistics replies are served with the time and memory figures recorded
  when they were stored.** Plain Verus tolerates any values there; only
  `rlimit-count` is used, and it is exact for the stored history.
- **What this mode adds is tested:** conformance runs with the daemon on and
  with it off, and a fault-injection test kills the daemon mid-session and
  expects fallback.

## Update 2026-10-08: a pairing bug, and defence in depth

The first conformance run with the cache on hung in human-eval-verus. The
cause was a bug in recording. The loop that pairs replies with commands
popped both queues before checking that both had an entry. When a command's
expectation arrived before its reply, the expectation was dropped, and every
later reply was stored under the *next* command's key. The trigger was
timing, so it only showed up under concurrency. Here it surfaced as a hang:
an `echo` was served a statistics reply, and Verus kept waiting for
`<<DONE>>`.

Fixes:
- **The bug.** Pairing is now a pure `Pairer` that pops only when both
  sides have an entry. A property test checks that pairing is independent of
  arrival order, and it fails against the old loop.
- **Shape checks (defence in depth).** Every reply is checked against its
  command, both when it is served and when it is stored:
  - an `echo` reply must be exactly its text plus a newline;
  - `check-sat` must be `sat`, `unsat` or `unknown`;
  - `:version` and `:reason-unknown` replies must have their own forms;
  - statistics must be a `(:...)` list;
  - for any other command, the reply must be non-empty and not an error.

  A mismatch is a miss when serving, and stops recording when storing.
- **Unit timeout.** The conformance harness runs each unit in its own process
  group, with a time limit (`SAIL_UNIT_TIMEOUT`, 3,600 s by default). A hang
  becomes an `infra-error` and does not block the run.

Evidence:
- **Before the fix:** a concurrent Verus fill of human-eval-verus, followed by
  replaying all 113 sessions against the cache, gave 14 sessions with wrong
  replies on one run and 2 on another.
- **After the fix:** two concurrent fills and the same replay gave 0.

## Update 2026-10-08: fault injection

The fault-injection suite (`crates/sail-shim/tests/faults.rs`, described in
`docs/fault-injection.md`) covers four kinds of fault: daemon death and hangs,
corrupt entries, solver kills, and concurrent clients. It found two shim bugs
and one cache bug.

- **Hang when the solver died after a miss.** The input side waited for the
  solver only once stdin closed. Verus keeps stdin open while it waits for a
  reply, so a solver killed mid-query left both sides waiting. The output
  side now owns the solver process and waits for it when its output ends. If
  input has not ended by then, the shim exits with the solver's status, as
  pass-through does. Only expectations already queued are paired; this can
  lose a recording, never mis-pair one.
- **A partial reply could be stored.** The last reply runs to the end of the
  output stream. A solver that died partway through a reply left its
  partial bytes on the end of the reply before them. When that reply was a
  model or a `:version` reply, the shape check passed and the entry was
  stored (for example, a model followed by `<<DO`). The last reply is now
  stored only if the solver exited with status 0. It is still forwarded,
  exactly as in pass-through.
- **Shared temporary files in the disk tier.** Two connections storing one
  key at once wrote the same temporary file. The renamed entry could mix
  their bytes. It failed the integrity check, so this cost a miss, not a
  wrong answer. Temporary names now carry a per-process sequence number.

Hardening and configuration:
- **No reuse after a daemon error.** After a daemon error the shim drops
  the connection for the rest of the session instead of reusing it for
  stores, so a late reply is never read as the answer to a later request.
  Starting the solver no longer depends on any connection.
- **Configurable client timeout.** `SAIL_DAEMON_TIMEOUT_MS` sets the timeout
  in milliseconds (default 10,000). A hung daemon costs a session at most
  one timeout: before falling back if it hangs on a lookup, or at exit if it
  hangs on a store.

## Update 2026-10-08: a `STATS` operation

The protocol has a fourth operation, `STATS` (op 4, empty payload). It
returns the daemon's counters since start as JSON: `hellos`, `gets`, `hits`
and `puts`. Benchmarks use it to report what the cache did in each run.
See ADR 0012.

## Update 2026-10-08: memory tier, write-behind and `INFO` (ADR 0015)

- **`INFO`.** A fifth operation, op 5, read-only and additive like `STATS`.
- **New counters.** `STATS` gains `memory_hits` and `dropped_puts`.
- **The memory tier.** It sits in front of the disk tier, and every hit
  from it is checked against the entry file's stamp.
- **`PUT` is written behind.** The daemon acknowledges a `PUT` once the
  entry is held in memory and queued for disk, so an acknowledgement no
  longer means the entry is on disk. The disk tier's atomic-write and
  integrity rules are unchanged.
