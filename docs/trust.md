# Trust model

How far each source of a verification result is trusted, and what is checked
before it is used. Charter rules 2, 3, 7, 9 and 10 apply.

Status: two local tiers exist, the in-memory tier and the disk tier, both
inside the daemon (ADR 0011, ADR 0015). There is no shared tier yet.

| Source | Trusted for | Checks before use |
|---|---|---|
| Fresh pinned-Z3 run | Confirmed results | Solver identity (version string, binary hash and platform) matches the pin |
| In-memory tier | Bytes that are on disk, or queued to be written there by the daemon | A slot read from disk, or whose write completed, is served only while its entry file has the stamp it had then (device, inode, length, modification time). Otherwise the slot is dropped and the disk is read. A pending slot exists only for a key that has no entry file and no other queued write (ADR 0015) |
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

## In-memory tier: what it adds to the trust base

The memory tier adds no new source of answers. Every slot holds bytes that
came from one of two places:
- a disk read that passed the disk tier's integrity checks;
- a `PUT` from a shim session, the same bytes the disk tier would store.

A slot of the first kind is checked against the disk on every hit. It is
served only while its entry file has the same device, inode, length and
modification time as when the slot was filled. So:
- an entry removed by `gc`, `sail cache verify --remove` or `clear` is a
  miss at once, from any process;
- an entry corrupted, replaced or forged on disk is a miss, or is served
  exactly as the disk tier would serve it. A forgery that passes the
  disk-tier checks is served from disk too: the forged-entry limitation
  below is unchanged, not widened.

A slot of the second kind is *pending*: its write-behind write has not
finished (ADR 0015). It is trusted exactly as the disk entry it will
become, since the daemon is the only writer of both.
- It never shadows different bytes. A `PUT` is not taken when an entry
  file already exists for the key, or when a write for the key is queued.
- When the write finishes, the slot is checked against the new file's
  stamp. If the write fails, or finds another entry already there, the slot
  is dropped.

**What the stamp check does not catch.** Suppose something other than sail
rewrites an entry file in place, keeping its inode and length and restoring
its modification time to the nanosecond. The memory tier would keep serving
the old bytes. Those bytes passed the integrity checks when they were read,
so they are still the solver's answer. Anything able to do this is already
outside the trust boundary: the user's cache directory, written only by the
daemon.

**Killing the daemon** with writes still queued loses those entries. Each
loss is a future miss. An interrupted write leaves at most a temporary
file, which is never read as an entry (`crates/sail-cli/tests/daemon_ops.rs`).

## Daemon version

The daemon reports its protocol version in `HELLO`, and clients treat a
mismatch as "daemon unavailable": the shim passes everything through to the
pinned solver. The default socket name includes the version
(`daemon-v<N>.sock`), so an upgraded `sail verify` starts its own daemon
instead of reusing a stale one.
