# Trust model

How far each source of a verification result is trusted, and what is checked
before it is used. Charter rules 2, 3, 7, 9 and 10 apply.

Status: two local tiers exist, the in-memory tier and the disk tier, both
inside the daemon (ADR 0011, ADR 0015). A shared tier of signed entries
sits behind them (ADR 0017).

| Source | Trusted for | Checks before use |
|---|---|---|
| Fresh pinned-Z3 run | Confirmed results | Solver identity (version string, binary hash and platform) matches the pin |
| In-memory tier | Bytes that are on disk, or queued to be written there by the daemon | A slot read from disk, or whose write completed, is served only while its entry file has the stamp it had then (device, inode, length, modification time). Otherwise the slot is dropped and the disk is read. A pending slot exists only for a key that has no entry file and no other queued write (ADR 0015) |
| Local disk tier | Replies written by the local daemon, which is its only writer | Key and reply hash match the entry, and the reply has the shape its command requires (ADR 0011). Any failure is a miss |
| Shared tier | Nothing, until verified (rule 9): then exactly as far as its signer is trusted | An ed25519 signature by a key in `<cache root>/trust/*.pub`, over the key, reply hash, command, mode, solver identity, platform class, effort, format version and signer id. Then: key, reply hash, solver identity and platform class equal the session's, the reply has its command's shape, and an `unknown` passes the serving policy (ADR 0009). Any failure or timeout is a miss (ADR 0017) |

## Solver identity

A result is only as reusable as the identity it was produced under.
Identity binds:
- the solver name and the version string it reports;
- the SHA-256 of the solver binary;
- the platform (OS and architecture).

Rlimit counts have been observed to differ across machines for the same
binary (ADR 0004 update), so the shared tier may need to bind a machine class
finer than the platform. That is decided by measurement.

**Owner decision (ADR 0010).** Shared-tier entries are scoped by solver
binary hash plus platform class. Hits across platform classes are allowed
only in a named opt-in mode. The shared tier implements this: entries are
filed by platform and binary hash, both are signed, and a correctly signed
entry for another platform class is refused and counted
(`shared_other_platform`). The opt-in mode is not built.

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

The shared tier meets these with signatures (next section).

## Shared tier: what its checks do and do not prove

Anyone may be able to write to a shared store: it is a directory on NFS, a
synced folder, a CI artefact. So the store is never trusted. Only a
signature is.

**What a verified entry proves.** The holder of a trusted secret key signed
this exact reply as the answer to this exact v2 key, for this command,
from this solver binary on this platform class, with this effort. Since
the v2 key binds the whole process prefix and the solver identity
(ADR 0008), that is the same question the session is asking.

**What a store writer can and cannot do** without a trusted secret key:
- plant a forgery with a valid hash and a plausible shape: refused (no
  valid signature). Fault injection shows the session stays exact;
- re-sign a forgery with their own key: refused (untrusted signer);
- corrupt, truncate or delete entries, or hang the store: a miss, at most
  one timeout per back-off period;
- move a validly signed entry to another key's path, or copy one signed
  for another solver or platform: refused (key, solver or platform
  mismatch).

They cannot make a reader serve a wrong answer.

**What a trusted key can do.** Anything: a trusted signer is trusted
completely. Whoever holds the secret key can publish wrong answers that
every trusting reader serves. Hence:
- keep secret keys where releases are made, such as a CI secret or a
  designated machine, and never in the store;
- publish only from a cache filled by the pinned solver.
  `sail cache publish` exports only entries with provenance
  `pinned-solver` and a recorded solver identity, never entries that came
  from the shared tier, and never `unknown`s the serving policy would not
  serve;
- revoke a key by removing its public key from readers. Local copies of
  its entries carry provenance `shared:<key id>`, so they can be found.

**Provenance (charter rule 7).** A shared hit is copied into the local tier
with provenance `shared:<key id>`. It is never published again; that would
need a fresh solve.

**Statistics replies** served from the shared tier carry the publisher's
time, memory and `rlimit-count` figures. Plain Verus uses only
`rlimit-count`, which is exact for the publisher's run of the same
process prefix on the same binary and platform class. See "Solver
identity" above for machine classes.

## Key management

- `sail cache keygen --out FILE` creates a signing key: `FILE` (mode 0600)
  and `FILE.pub`. Keys are one line of text:
  `sail-ed25519-secret-v1 <hex>` and `sail-ed25519-public-v1 <hex>`.
  A key id is the first 16 hex digits of the SHA-256 of the public key.
- `sail cache trust add FILE.pub` copies a public key into
  `<cache root>/trust/<id>.pub`. `trust list` and `trust remove ID` manage
  the set. The trust directory sits inside the cache root, so it has the
  local tier's trust boundary: the user's own directory. A running daemon
  re-reads it within 5 seconds.
- `sail cache publish --to STORE --key FILE` signs and writes entries.
- `sail serve --shared STORE`, or `SAIL_SHARED_STORE`, turns on the read
  path. `SAIL_SHARED_TIMEOUT_MS` (default 250) bounds each lookup.

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
(`daemon-v<N>-workspace-v1.sock`), so an upgraded `sail verify` starts its own daemon
instead of reusing a stale one.

## Solver diagnostics and publication

The stdout reply cache admits a session only after a clean solver exit with
no stderr bytes (ADR 0022). Quantifier warnings are forwarded and cause all
staged entries to be discarded; future runs therefore reproduce the warnings.
Local and signed shared entry format 2 reject older entries whose silence was
not established. A trusted shared writer must follow the same publication rule.

Solver-free completion additionally requires a silence record for the exact
complete session input. Cached replies alone cannot cover new trailing
commands. The record uses a separate key domain, with the same solver and mode
identity; shared writers must certify complete silent sessions, never synthesize
records merely from cached prefixes.
