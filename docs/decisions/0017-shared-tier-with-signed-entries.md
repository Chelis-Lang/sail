# 0017: The shared tier: signed entries, verified before use

- **Status:** accepted
- **Date:** 2026-10-09

## Context

The charter asks for a shared tier for teams and CI (rule 9): "entries from
a shared cache are checked for integrity and key binding before use."
Three earlier findings constrain it:

- **The local tier's checks do not catch forgeries.** `docs/trust.md`
  records that a forged entry with a valid hash and a plausible shape (an
  `unsat` under a key whose answer is `sat`) passes every disk-tier check.
  The local tier is trusted only because the daemon is its sole writer. A
  shared store has many writers, so its entries need provenance the reader
  can verify: signatures from a trusted writer, certificate checking (not
  yet available), or re-solving (which would make the tier pointless).
- **Scope (ADR 0010, item 6).** Shared entries are scoped by solver binary
  hash plus platform class. Hits across platform classes are allowed only
  in a named opt-in mode.
- **Unknown answers (ADR 0009).** An `unknown` carries its effort, and only
  far-from-limit, non-exhaustion `unknown`s may be served.

## Decision

### Trust: ed25519 signatures from configured keys

An entry in the shared tier is used only if it carries a valid ed25519
signature by a key the reader trusts. Nothing else about the entry, its
location or its store makes it trusted.

The signature covers a length-prefixed encoding of every header field
(`sail_cache::shared::Header::signed_bytes`), never the JSON text, so no
JSON canonicalization is involved:
- a domain tag and the format name and version (`sail-shared-entry`, 1);
- the v2 key (ADR 0008);
- the SHA-256 of the exact reply bytes;
- the command the reply answers;
- the mode (`default`; opt-in modes need their own decision);
- the solver identity: name, version, binary SHA-256, platform class;
- the effort (ADR 0009): limit, used, `reason-unknown`;
- the publication time;
- the signer's key id.

**Keys.** A key file is one line of text:
`sail-ed25519-secret-v1 <hex seed>` or `sail-ed25519-public-v1 <hex key>`.
A key id is the first 16 hex digits of the SHA-256 of the public key. An
entry names its signer by id. The reader verifies with the trusted key of
that id, so a colliding id never verifies anything.
- `sail cache keygen --out FILE` writes `FILE` (mode 0600, never
  overwritten) and `FILE.pub`. Randomness comes from `/dev/urandom`.
- Readers trust the keys in `<cache root>/trust/*.pub`, the same directory
  tree as the local tier and so the same trust boundary.
  `sail cache trust add FILE.pub | list | remove ID` manages it. A running
  daemon re-reads the directory at most every 5 seconds. Weak (small-order)
  public keys are refused.

**Verification** uses `verify_strict`, which rejects non-canonical and
small-order signatures. Checks run in this order, and the first failure is a
miss: header parses; format and version; key matches; signer trusted;
signature verifies; reply hash; solver name, version and binary equal the
session's; platform class equals the session's; mode is default; reply
shape fits the command (`sail_cache::shape_ok`); and, for `unknown`
answers, the serving policy. The solver and platform checks run after the
signature, so the counters count real, correctly signed entries for another
solver or platform.

### Store: a directory

A shared store is a directory reachable as a path: local, NFS or a synced
folder. Entries live at
`<store>/v1/<platform>/<solver binary sha256>/<k0k1>/<key>`: one JSON
header line, then the reply bytes, written atomically. The path is only a
lookup hint, scoped by ADR 0010's solver hash and platform class; every
field in it is also signed and checked.

**HTTP is designed, not built.** A read-only base URL would serve the same
layout: `GET <base>/v1/<platform>/<sha>/<k0k1>/<key>`, where a 404 is a
miss. Signatures make transport integrity unnecessary, so plain HTTP would
be safe for correctness; TLS would only add confidentiality. Building it
now would mean a hand-written HTTP/1.1 client (redirects, chunked encoding,
proxies) or a TLS stack such as `rustls` with its root store. Neither is
cheap, and a mounted directory covers today's teams and CI. When HTTP comes,
it plugs in behind the same lookup, with the same timeout and back-off.

### Read path: the daemon

On a local miss (memory and disk), for a session whose `HELLO` gave a
solver identity, the daemon looks the key up in the shared store.
- **Bounded.** The read and all checks run on a helper thread, and the
  daemon waits at most the timeout: 250 ms by default
  (`--shared-timeout-ms`, `SAIL_SHARED_TIMEOUT_MS`).
  - A timeout makes the daemon skip the store for 30 seconds, so a hung
    mount costs one timeout per back-off, not one per lookup.
  - At most 64 reads run at once. A read stuck in the kernel holds a
    thread, never a connection.
  - A key that missed or was rejected is not looked up again for 60
    seconds (negative caching), so concurrent sessions that share a
    prefix ask once.
  - The local path is untouched: memory and disk hits never wait for the
    shared store.
- **Served and copied.** A verified entry is served and queued for the local
  disk tier with provenance `shared:<key id>` (charter rule 7) and the
  session's solver identity. Later lookups are local.
- **Counted.** `STATS` gains `shared_lookups`, `shared_hits`,
  `shared_rejected`, `shared_other_solver`, `shared_other_platform`,
  `shared_timeouts` and `shared_skipped`. `INFO` gains `shared` (store,
  trust directory, trusted key ids, timeout). Both are additive with serde
  defaults, so `PROTOCOL_VERSION` stays 1. The daemon logs the first 20
  rejections and timeouts with their reasons.
- **Configured** with `sail serve --shared DIR` or `SAIL_SHARED_STORE`.
  The daemon `sail verify` starts inherits the environment.

**Cross-class hits are refused and counted.** A correctly signed entry for
this solver on another platform class is a miss, counted in
`shared_other_platform`. The named opt-in mode that would allow it is not
built (ADR 0010).

### Write path: `sail cache publish`

`sail cache publish --to STORE --key FILE [--filter command=NAME]...
[--filter solver=SHA256-PREFIX] [--dry-run]` signs local entries into a
store. It exports only entries that:
- pass the disk tier's full check (integrity and shape);
- have provenance `pinned-solver`, recorded by the local daemon from the
  pinned solver. Entries with provenance `shared:*` are never published
  again; re-publishing them would need a fresh solve first;
- have a recorded solver identity. **The daemon now records the session's
  solver identity with every `PUT`** (`name version sha256 platform` in the
  entry header's `solver` field, which used to be empty). Older entries are
  counted as unattributed and skipped;
- for `unknown` answers, would be served by the policy of ADR 0009.
  Near-limit and exhausted `unknown`s are never published.

An entry the store already holds, signed by the same key, is left alone.

A trusted writer is, for example, a CI release job holding the secret key
as a secret, or one designated machine. Readers only need the `.pub` file.

### The serving policy moves to `sail-api`

The ADR 0009 policy (`unknown_servable`) moves from the shim into
`sail_api::policy`, so the shim (serving), the daemon (shared reads) and
`sail cache publish` apply one definition. The shim already depended on
`sail-api`; its behaviour is unchanged.

### Dependency: `ed25519-dalek`

`ed25519-dalek` 2.2 (BSD-3-Clause), a new dependency of `sail-cache` only.
- It is the standard pure-Rust Ed25519, from the dalek-cryptography
  project, widely used and reviewed across the Rust ecosystem. Writing our
  own signature code is out of the question.
- It brings `curve25519-dalek`, `ed25519`, `signature`, `subtle` and
  `zeroize`, and reuses the `sha2` 0.10 already in the workspace.
  Version 3.0 exists, but moves to `sha2` 0.11, which would duplicate it;
  revisit when the workspace moves.
- `cargo deny check` passes (licences, bans, sources), with every
  feature enabled. `Cargo.lock` also lists the crate's optional
  dependencies (`pkcs8`, `rand_core`, `getrandom` and others), which sail
  does not build.
- Secret keys are wiped on drop (`zeroize`), and `Debug` prints only the
  key id.
- `sail-shim` does not link it: the shim never reaches `sail-cache`, so its
  fallback path is unchanged. The layering table is unchanged.

## Trust model and its limits

- **A trusted key is trusted completely.** Whoever holds a trusted secret
  key can publish wrong answers that every trusting reader will serve.
  Keep secret keys in CI secrets or on a designated machine. Revoke a key
  by removing its `.pub` from readers (`sail cache trust remove ID`).
  Already-copied local entries keep their `shared:<id>` provenance, so
  they can be found and removed.
- **The store itself is untrusted.** Anyone who can write to the store can
  delete or corrupt entries (a miss), replay an older entry signed by a
  trusted key under its own key (the same answer, since the key binds the
  whole process prefix and solver identity), or plant unsigned or
  foreign-signed entries (a miss). They cannot change an answer.
- **Platform class is `<os>-<arch>`.** ADR 0004's update notes that rlimit
  counts can differ between machines for one binary. Statistics replies
  served from the shared tier carry the publisher's counts. A check's
  answer is a function of its key's history (ADR 0008). Whether a finer
  machine class is needed is decided by measurement, as `docs/trust.md`
  says. The `unknown` policy already refuses anything near its limit.
- **Certificate checking** (charter roadmap), when it arrives, becomes a
  distinct and stronger tier: it would not depend on trusting a signer.

## Consequences

**Tests, all in the per-PR gates:**
- `sail-cache` unit tests: signing binds every field (editing any field
  after signing fails); forged, untrusted, corrupt, truncated, misfiled,
  foreign-solver and foreign-platform entries are rejected; key files
  round-trip; `publish` exports only local pinned-solver entries.
- `crates/sail-daemon/tests/shared.rs` (fast gate, no solver, about 2 s):
  the read path against a real daemon. It covers a verified hit, its local
  copy and its provenance; every rejection; negative caching; no lookups
  without `HELLO`; a hung store (a FIFO at the entry's path) costing one
  timeout then being skipped; recovery after the back-off.
- `crates/sail-cli/tests/shared_ops.rs` (fast gate): `keygen`, `trust`,
  `publish` and `sail serve --shared` as real processes.
- `crates/sail-shim/tests/faults.rs` (solver tier, a few seconds): real
  shim sessions against a store published from another cache. The cases
  are a verified store, forged wrong answers with valid hashes, wrong
  answers signed by an untrusted key, wrong answers correctly signed for
  another solver or platform, truncated entries, and a hung store. Every
  session gives the pinned solver's exact replies, and no wrong answer
  reaches the local tier. Disabling the signature check makes the forgery
  case fail with a wrong `check-sat` reply.

**Conformance** (local only, macOS arm64, 2026-10-09). The fast-tier guide
subset (`--tier fast --project verus-examples --filter /guide/`, 33 units)
ran in three steps:
1. A writer daemon on cache A filled it: 33 same, 0 different.
2. `sail cache publish` signed all 3,390 entries into a store. None were
   held back: the subset's `unknown`s all pass the serving policy.
3. A reader daemon on an empty cache B, trusting the key and reading the
   store, ran the subset again: 33 same, 0 unstable, 0 different, over
   311 functions.

The reader made 3,203 shared lookups: 3,200 hits, 3 misses, 0 rejected,
0 timeouts. The misses were re-solved, and those 190 replies were stored
with provenance `pinned-solver`. The 3,200 copies carry
`shared:<key id>`. A dry-run publish from B offered only its 190 own
entries.

**Revisit:**
- HTTP(S) stores, when a team needs one;
- a machine class finer than `<os>-<arch>`, if measurement shows rlimit
  differences that matter;
- certificate-backed entries as their own tier;
- the opt-in cross-class mode, if the owner wants it.

### Foreground filesystem boundary correction

The first implementation read signing trust synchronously before starting the
lookup helper and its receive timeout. HTTP discovery/status also performed
foreground configuration reads. A stalled trust/configuration mount could exceed
the documented bound without reaching the helper. This is the concrete defect
tracked as [issue 76](https://github.com/Chelis-Lang/sail/issues/76); it does not
establish the cause of retained HTTP wall-time failures.

Trust and configuration reads now belong to existing bounded lookup/setup owners.
The deadline begins at lookup entry, includes discovery, and is checked after
copying a coalesced reply. Pending publication stores shared owners so a full
reply copy cannot hold the result mutex. Foreground publication/cleanup uses
try-locks; a subsequent bounded table access retires completed owners outside
the lock. The original 64 helper limit, 250 ms default, five-second trust reload,
entry checks, backoff and pinned-solver fallback remain intact.
Retirement removes only the exact finished owner; it cannot remove a newer
same-key lookup installed by another caller. Admission uses short bounded parks
on contention, under that same request-entry deadline.

Status uses one optional metadata owner, at most 100 ms and never beyond the
configured timeout. Old presence answers do not authorize new calls. Busy or
expired trust snapshots are omitted; status is descriptive, never proof or
signing authority. No additional dependency or internal layer edge is introduced.
The paused-I/O controls, retained draft failures and unchanged integration scope
are recorded in [the focused validation record](../validation/m7-shared-trust-foreground.md).
