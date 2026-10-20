# 0062: Ship the pinned vstd's solver replies in the release archive

- **Status:** accepted (owner decision 2026-10-20, question 36)
- **Date:** 2026-10-20

## Context

The release archive ships a verified vstd proof (ADR 0048), which makes a
fresh project's first run about 2.4× faster than stock. Anything that changes
the proof's identity or closure makes that proof miss, and Sail then verifies
vstd again. Examples are extra toolchain components (`rustc-dev`,
`llvm-tools`), another lock closure, a loader variable (`SAIL3112`), a
missing `rust-src` (`SAIL3137`) or another Cargo. On such a miss the solver
cache stays on (question 30).

The [miss-replies record](../validation/vstd-miss-replies.md) measured what
signed vstd replies do on a fresh machine:
- **They always match.** The fork's 20,208 vstd solver replies are
  byte-identical across homes, Cargo homes, project names, toolchain
  components, lock closures and loader variables. Every query hit, the
  replies passed the ADR 0017 checks, and no normalization was needed
  (ADR 0008 keys bind exact input bytes).
- **They save 1.4–1.8 s** (4–5%) on every vstd miss. That brings a
  toolchain-component miss from 3.3–4.4% slower than stock to parity, and
  lock-closure and loader-variable misses to 6–10% faster than stock. No Z3
  process runs a vstd query.
- **They do not touch most of the miss.** That is Verus's vstd frontend
  (about 15 s) and its query generation (most of an 11 s query window). The
  saving is bounded by the solver's share of that window.
- **They are small.** 17.6 MB raw, 4.3 MB with `gzip -9`.

ADR 0017 trusts a shared entry only if a key in `<cache root>/trust/` signed
it: "nothing else about the entry, its location or its store makes it
trusted." The daemon reads one shared store. A release that carries replies
therefore needs a trust root that ADR 0017 does not have.

## Decision

1. **Release factory.** `scripts/release-shipped.py record` already verifies
   vstd with an owned daemon and an empty cache. After a successful record,
   it waits for the daemon's write queue to drain, stops the daemon and
   runs `sail cache bundle-replies --cache <recording cache> --out <dir>`.
   That command:
   - generates a fresh ed25519 key, in memory only;
   - signs every entry that `sail cache publish` would export into one
     pack, `vstd.pack`. These are the pinned solver's replies with a
     recorded solver identity, and only the `unknown` answers that the
     ADR 0009 policy serves. Each entry is an ordinary ADR 0017 shared
     entry;
   - writes the public half to `release.pub`, and **never writes the
     secret**. It is gone when the command exits, so no long-lived signing
     secret exists to steal or rotate.

   The factory moves both files to `replies/` in the stage before writing
   `release-files.json`, so the index binds them like every other member.
   `verify`, `package` and the smoke test require exactly these two files,
   a well-formed and non-empty pack, and a public key.
2. **A pack, not a directory.** A directory store would add about 20,000
   files, more than the release index allows (16,384 members). The pack is
   the line `sail-reply-pack-v1`, then records in ascending key order:
   32 key bytes, a little-endian `u32` length, and the entry bytes. It has
   no trust of its own (`sail_cache::pack`).
3. **The bundled tier in the daemon.** `sail serve` finds the release root
   from its own executable (`<root>/bin/sail`, after resolving symbolic
   links). A helper thread then loads the tier once:
   - it reads `release-files.json`;
   - it requires `replies/release.pub` and `replies/vstd.pack` to match the
     SHA-256 and size the index lists for them;
   - it parses the key and the pack.

   Any failure leaves the tier off: every lookup misses, and `INFO` and
   `sail status` say why. Lookups made before loading finishes miss. On a
   local miss in default mode, for a session whose `HELLO` gave a solver
   identity, the daemon looks the key up in the pack before any team
   store. The tier:
   - trusts **only** the release key, and only for this pack. The key is
     never added to `<cache root>/trust/`, so it authorizes nothing else,
     and the trust directory's keys authorize nothing in the pack;
   - applies `verify_entry` unchanged: format, key, signature, reply hash,
     solver name, version and binary, platform class, mode, reply shape,
     then the `unknown` policy. A failure is a counted miss
     (`bundled_rejected`);
   - copies hits into the local tier with provenance `bundled:<key id>`.
     `sail cache publish` never republishes them, because it exports only
     `pinned-solver` entries;
   - reads only memory once loaded, so it needs no deadline or back-off.

   `STATS` gains `bundled_lookups`, `bundled_hits` and `bundled_rejected`,
   and `INFO` gains `bundled`. Both are additive with serde defaults, so
   protocol 1 is unchanged.
4. **Opt-out.** `SAIL_BUNDLED_REPLIES=0` skips the tier. This is for
   diagnosis only; the tier never changes a reply's bytes.
5. **Scope.** Ship replies only for the runtime's own pinned vstd. Older
   crates.io vstd versions do not verify under a newer runtime, as Vest's
   pin showed, so "the last N versions" is N = 1. The pack also holds the
   few replies of the recipe's own one-function crate, which no user
   project matches; they are harmless.
6. **Default-mode keys are unchanged.** An entry serves only a
   byte-identical process prefix on the same solver binary and platform
   class (ADR 0008). Nothing is normalized.

**Why the archive may carry a trust root.** The archive's checksum is
already the trust boundary for `bin/sail`, `bin/sail-z3`, Z3 itself and the
proof bundle (`docs/releasing.md`). A user who runs that `sail` already
trusts whoever produced the archive with every verdict. A public key in the
same archive, bound by the same index and scoped to the same archive's
store, adds no new party to trust. Signing still matters: it keeps the
store's integrity and solver binding checked by the existing code, so a
corrupted or replaced reply file is a miss, not a wrong answer.

## Consequences

- **Speed.** Every vstd miss saves 1.4–1.8 s on Linux (macOS is
  unmeasured). This alone does not make a component miss faster than stock;
  it brings it to parity.
- **Size.** The archive grows by two files: a pack of about 18 MB
  (about 4 MB compressed) and a key.
- **Release time.** The release factory adds one publish step (seconds).
  Linux and macOS need separate replies, because the platform class and the
  Z3 binary differ.
- **Code.** The daemon gains a second store with its own trust set. The
  shim does not change, and neither does the layering table.
- **Tests.**
  - `crates/sail-daemon/tests/bundled.rs` runs against a real daemon:
    - a factory-built pack is served and copied locally with its
      provenance, and is never republished;
    - each of these entries is a counted miss: a modified entry, an entry
      for another solver binary, an entry signed by another key, and a
      misfiled entry;
    - each of these turns the tier off: the key outside the index, the
      pack outside the index, a swapped key, a modified pack, a malformed
      pack, and a missing index;
    - with a swapped key and a rewritten index, the release's entries are
      untrusted;
    - release and team trust never cross.
  - `sail_cache::pack` unit tests refuse every malformed pack.
  - `crates/sail-cli/tests/shared_ops.rs` checks that `bundle-replies`
    writes exactly the pack and the public key, and that every packed
    entry verifies.
  - `scripts/test-release-shipped.py` covers the factory and the
    `verify` checks.
- **Complementary lever.** On a plan-less miss, the driver proxies rehash
  the engine runtime (530 MB, 4.4 CPU-s), and the developer toolchain's
  sysroot is hashed in full. Together these cost about 2.4 s; #286 fixes
  that separately. This ADR adds its 1.4–1.8 s on top.
- **Revisit** if the fork's vstd query generation gets faster: the solver's
  share of a miss, and with it this lever's value, would then grow.
