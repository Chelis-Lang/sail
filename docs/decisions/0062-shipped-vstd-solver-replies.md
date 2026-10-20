# 0062: Ship the pinned vstd's solver replies in the release archive

- **Status:** proposed (needs an owner decision on its trust root, question 36)
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

## Decision (proposed)

1. **Release factory.** `scripts/release-shipped.py record` already verifies
   vstd with an owned daemon and an empty cache. After a successful record:
   - generate a fresh ed25519 key for this release (`sail cache keygen`);
   - sign the run's pinned-solver entries into `replies/` with
     `sail cache publish`, with the existing filters and the ADR 0009
     unknown-answer policy;
   - write `replies/release.pub`, then **destroy the secret key**. No
     long-lived signing secret exists, so there is nothing to steal or
     rotate.

   `release-files.json` indexes every reply file, as it indexes the proof
   bundle, and `package` and the smoke test check them.
2. **A second, read-only shared tier: the bundled store.** The daemon finds
   `replies/` beside the installed runtime (the `bin/sail` the run executes,
   the same discovery as the proof bundle). It consults that store after the
   local tier and before any configured team store. This tier:
   - trusts **only** `replies/release.pub`, and only for this store. The key
     is never added to `<cache root>/trust/`, so it authorizes nothing else;
   - applies every ADR 0017 check in the same order: format, signature,
     reply hash, solver name, version and binary, platform class, mode,
     reply shape and unknown policy. A reply for another solver binary is a
     counted miss;
   - copies hits into the local tier with provenance `bundled:<key id>`.
     `sail cache publish` never republishes them, as it already refuses
     `shared:*` entries;
   - keeps ADR 0017's 250 ms deadline, negative caching and back-off. Any
     failure is a miss, and the pinned solver answers.
3. **Opt-out.** `SAIL_BUNDLED_REPLIES=0` skips the tier. This is for
   diagnosis only; the tier never changes a reply's bytes.
4. **Scope.** Ship replies only for the runtime's own pinned vstd. Older
   crates.io vstd versions do not verify under a newer runtime, as Vest's
   pin showed, so "the last N versions" is N = 1.

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
- **Size.** The archive grows by about 4 MB compressed and about 20,000 small
  files once unpacked. Packing them into one indexed file would avoid the
  file count, but it needs a new store layout, so it is left out of this
  proposal.
- **Release time.** The release factory adds one publish step (seconds).
  Linux and macOS need separate replies, because the platform class and the
  Z3 binary differ.
- **Code.** The daemon gains a second store with its own trust set, and its
  counters gain `bundled_*` fields (additive, protocol 1). The shim does not
  change, and neither does the layering table.
- **Tests.** The ADR 0017 cases rerun against the bundled tier: forged,
  untrusted, foreign-solver, foreign-platform, truncated and hung entries.
  Added cases: a key from the bundled store does not authorize the team
  store, and the team store's keys do not authorize the bundled store. An
  archive whose `replies/` is altered fails `package` and the smoke test.
- **Not chosen first.** The record finds a larger and cheaper lever for the
  component miss. On a plan-less miss, the driver proxies rehash the engine
  runtime (530 MB, 4.4 CPU-s), and the developer toolchain's sysroot is
  hashed in full. Together these cost about 2.4 s and need no trust
  decision. They should be fixed first; this ADR then adds its 1.4–1.8 s.
- **Revisit** if the fork's vstd query generation gets faster: the solver's
  share of a miss, and with it this lever's value, would then grow.
