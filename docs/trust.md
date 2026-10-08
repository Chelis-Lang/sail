# Trust model

How far each source of a verification result is trusted, and what is checked
before it is used. Charter rules 2, 3, 7, 9 and 10 apply.

Status: skeleton. No cache tier exists yet. Every result today comes from a
fresh run of the pinned Z3, through the pass-through shim.

| Source | Trusted for | Checks before use |
|---|---|---|
| Fresh pinned-Z3 run | Confirmed results | Solver identity (version string, binary hash and platform) matches the pin |
| In-memory tier | (not built) | |
| Local disk tier | Replies written by the local daemon, which is its only writer | Key and reply hash match the entry, and the reply has the shape its command requires (ADR 0011). Any failure is a miss |
| Shared tier | Untrusted until verified (rule 9) | (not built) |

## Solver identity

A result is only as reusable as the identity it was produced under.
Identity binds:
- the solver name and the version string it reports;
- the SHA-256 of the solver binary;
- the platform (OS and architecture).

Rlimit counts have been observed to differ across machines for the same
binary (ADR 0004 update), so the shared tier may need to bind a machine class
too. That is decided by measurement before the shared tier is built.

**Owner decision (ADR 0010).** Shared-tier entries are scoped by solver
binary hash plus platform class. Hits across platform classes are allowed
only in a named opt-in mode.

## Local disk tier: what its checks do and do not prove

The integrity checks catch corruption, truncation, misfiled entries and
shape mismatches. Fault injection showed each of them becomes a miss
(`docs/fault-injection.md`).

They do **not** catch a *forged* entry: one written by someone other than
sail, with a valid hash and a plausible shape but the wrong answer (for
example `unsat` under a key whose true answer is `sat`). The local tier is
therefore trusted only because the daemon is its only writer, and the cache
directory is owned by the user.

**Requirements this places on the shared tier** (charter rule 9):
- It must never be trusted the same way.
- Its entries need provenance the reader can verify: signatures from a
  trusted writer, or certificate checking when that arrives.
- Otherwise, entries must be re-validated against the pinned solver before
  use.

## Daemon version

The daemon reports its protocol version in `HELLO`, and clients treat a
mismatch as "daemon unavailable": the shim passes everything through to the
pinned solver. The default socket name includes the version
(`daemon-v<N>.sock`), so an upgraded `sail verify` starts its own daemon
instead of reusing a stale one.
