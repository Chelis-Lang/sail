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

## Optional source engine and preparation

The interactive engine is an explicitly selected pinned Verus source variant.
Its manifest binds the source commit, Rust toolchain, patch and lockfile digests,
solver and every runtime artifact. Installation and use reject incompatible or
changed runtime bytes. A prebuilt CI archive additionally binds its archive
digest to a committed pin; access credentials authorize download, not proof.
Source builds and artifact validation happen locally before publishing that pin.

A source terminal event is insufficient to finish a verification. The adapter
requires exactly one complete bounded structured Verus document, normal process
exit 0 or 1, and an explicit typed `payload.normal_termination: true` marker.
The optional source distinguishes expected compiler-error termination from an
internal unwind before emitting that marker. Complete failed-proof and late EXEC
documents retain their original engine output and focused provisional evidence.

A panic, signal, missing or malformed document, or absent/false/malformed normal
termination marker retains the engine's failed output and falls back to the
stock verifier with the same compiler arguments and effective options. Older
producers could catch internal panics into an ordinary-shaped compiler-error
document, so a legacy terminal seal without the new marker cannot authorize any
engine result. Fallback records its actual child invocation and discards focused
evidence from the interrupted engine. A terminal event's success flag alone
cannot distinguish a compiler crash from an ordinary failure.
Preparation counters use the same normal terminal and complete retained full
result, bound to the matching invocation and run. A retained failed-engine
marker excludes all summaries from that driver, even if its old event journal
survives stock fallback. Missing or corrupt evidence reports unavailable counters.

Prepared lowering checkpoints have the local tier's trust boundary: the owned
engine writes a private user cache. Version, input key, body digest and exact
current-callback span bindings detect corruption and stale preparation. They do
not authenticate a forged checkpoint written by someone controlling that cache.
Preparation is never imported from the shared SMT store. A hit reuses lowering;
the full proof and late compiler checks still run. Focus and progress evidence
remain provisional until a complete successful full result is Current.

## Retained freshness and kernel lease ownership

On Linux, retaining expensive runtime digests requires continuously held kernel
read leases, native event coverage, and synchronous barriers. Input bytes are
hashed only after those leases are acquired. Their digests can be retained while
that exact owner continuously protects every content inode and namespace binding.
A writable mapping, conflicting writer, helper
loss, timeout or uncertain path coverage makes current evidence unavailable;
historical proof output remains available and is never relabelled Current.
The exact sorted input-file inventory can be retained only within that same
continuous lease and namespace-observer generation. Every read validates its
owner before and after selecting the captured exact-byte revision. New files,
removals, replacements, symlink changes, absent configurations and changed
declaration records invalidate the inventory and require a full recollection
and content hashing under a new owner. Digests hashed before lease acquisition
or belonging to a lost owner are never revived; elapsed time grants no validity.

Linux requires the lease holder to own the file or hold the kernel lease
capability. The helper also leases its resolved Python interpreter, loaded
standard-library files and native modules. An ordinary user may be unable to
lease a system-owned Python installation. In that case Sail cannot retain the
runtime projection and reports unavailable current evidence; it does not infer
immutability from permissions or substitute metadata checks. Container tests
whose runtime files are owned by the service account establish that environment's
coverage, not general unprivileged host coverage.

Encoded job-result frames are an allocation optimization, not freshness evidence.
Every read verifies the exact retained stdout bytes against the validated seal
and checks current input evidence before selecting a frame. Cached frames are
bounded by 128 MiB and 16 jobs, carry no time-based validity, and are discarded
on a seal mismatch. Restart establishes freshness again.

The proof worker can reuse sysroot library hashes within its own invocation.
It installs native namespace observation before enumerating installed targets,
acquires kernel read leases before the first eligible content hash, and retains
both owners through the receipt's final commit marker. Digests computed before
that acquisition remain ineligible, and all other runtime files are hashed
normally. Target installation, alias replacement, writable mappings, helper
loss or a failed barrier discard this baseline: the final check reads runtime
bytes again and cannot publish a current receipt. Enumeration errors and
unrepresentable runtime paths also fail closed. This optimization transfers no
owner or trust to the daemon; completed-result readers establish their own
freshness evidence. macOS continues to use the full runtime byte check.

Lease barriers read bounded kernel-owned `/proc/locks` state rather than waiting
for a scheduled Python acknowledgement. At cold acquisition, the helper reports
its complete descriptor inventory; Sail binds each descriptor to the requested
device/inode and its kernel lease entry. Read barriers require the exact lease
multiset for that PID, all `LEASE ACTIVE READ`, plus the same live, non-stopped
process start time before and after the sweep. Missing, breaking, unexpected,
malformed, unavailable or over-budget state permanently invalidates that owner.
The sweep retains a 50 ms work budget and a 2 MiB byte cap. These are fail-closed
limits, not a promise that a blocked kernel read can be interrupted on that clock.
Public job RPC and reader acceptance deadlines are unchanged.

The helper acquires every lease once before readiness and never adds or renews a
lease afterward. Each lease can therefore only cease being active. Although a
proc seq-file read may span multiple kernel iterations, observing every expected
lease active proves they were all active at sweep entry: a lease active later
could not have been broken earlier and restored by this generation. Sail still
brackets retained bytes with independent namespace, interpreter, mount and inode
barriers. SIGIO immediately closes the helper's descriptors so writers proceed;
owner loss never authorizes reuse. This argument follows the kernel's
[`LEASE ACTIVE`/`BREAKING` status implementation and lease transitions](https://github.com/torvalds/linux/blob/f155f3b3ed1af23884ffaffe8a669722b87ac9d6/fs/locks.c)
and the documented [`/proc/locks` interface](https://man7.org/linux/man-pages/man5/proc_locks.5.html).
