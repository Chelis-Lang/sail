# Seeded vstd solver replies on a shipped-bundle miss (#236)

**Status:** built and measured on Linux ([ADR 0062](../decisions/0062-shipped-vstd-solver-replies.md),
accepted by the owner on 2026-10-20). A release archive now ships
signed vstd solver replies. On a shipped-proof miss they answer every vstd
query (20,197 of the run's 20,208 lookups; the other 11 are the user crate's
own queries). The cost of a toolchain-component miss falls from 6.5% slower
than stock to stock parity (+0.4%), and lock-file and loader-variable misses
run 7–10% faster than stock. A bundle hit is unchanged. The
[implementation section](#shipped-replies-implementation) has the numbers.
macOS is not measured. The earlier experiment below, with a seeded team
store, motivated the change.

| Field | Evidence |
|---|---|
| Source | Sail main `006ee5d0`, staged by `ci/local.sh release` as `0.1.0-miss`, with the shipped vstd bundle. Fork runtime `f8589ed4` (variant `7d829946…-81186034d2e137a2`), pinned Z3 4.16.0 |
| Stock | Verus `0.2026.10.18.7d82994` x86-linux release (`cargo verus`) |
| Environment | Native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, lane `miss1020`. Host install as in the README: rustup, default 1.99.0, pinned 1.99.1 |
| Toolchains | *Minimal*: 1.99.1 with the minimal profile and `rust-src` (the bundle hits). *Developer*: the same plus `rustc-dev` and `llvm-tools`, which change the proof's compiler identity, so the bundle misses and Sail prints no note |
| Isolation | Every run gets a fresh `HOME`, `CARGO_HOME` (crates are downloaded again) and `SAIL_CACHE_DIR`, plus its own project name and root. Stock runs on the same toolchain as the Sail arm it is compared with |
| Store | One producer run (developer toolchain, empty cache, project `p0fill`) filled a local cache. `sail cache publish` signed all 20,208 entries into a directory store (17.6 MB raw; 4.3 MB with `gzip -9`, 3.0 MB with `zstd -19`). Seeded arms trust the key (`sail cache trust add`) and set `SAIL_SHARED_STORE` |
| Scope | Demo `--lib` project, first `verify` after `new`. `new` is not timed. One host, Linux only, three rotated rounds per cohort |

## Results

Wall seconds of `verify` only: median (min–max) over three rotated rounds. All
runs exit 0. Every sample is recorded and none is dropped. Cohorts were run at
different times on the same host, so compare arms within a cohort, not across
cohorts.

**Cohort A** (toolchain-component miss):

| Arm | Wall | vs stock |
|---|---:|---:|
| Stock (developer toolchain) | 34.14 (34.05–34.15) | |
| Sail, bundle miss (developer toolchain) | 35.63 (35.36–35.69) | +4.4% |
| Sail, bundle miss, seeded store | **34.02 (33.98–34.21)** | −0.4% |
| Sail, bundle hit (minimal toolchain), reference | 14.11 (14.07–14.59) | 2.42× faster |

**Cohort P** (the same arms plus a loader-variable miss). A `/proc` poller ran
every 25 ms during these runs and adds about 2 s to each one:

| Arm | Wall | vs stock |
|---|---:|---:|
| Stock (developer) | 36.50 (36.02–36.63) | |
| Sail, component miss | 37.70 (37.44–37.86) | +3.3% |
| Sail, component miss, seeded | 35.94 (35.91–36.29) | −1.5% |
| Sail, `LD_LIBRARY_PATH` miss (`SAIL3112`, minimal) | 34.33 (34.24–34.37) | −5.9% |
| Sail, `LD_LIBRARY_PATH` miss, seeded | 32.84 (32.81–32.90) | −10.0% |

**Cohort B** (lock-closure miss: `cargo update -p proc-macro2 --precise
1.0.106` after `sail new`, minimal toolchain):

| Arm | Wall | vs stock |
|---|---:|---:|
| Stock (developer, original lock) | 36.20 (36.03–36.44) | |
| Sail, component miss (developer) | 37.56 (37.46–37.59) | +3.8% |
| Sail, `LD_LIBRARY_PATH` miss (developer) | 34.71 (34.65–34.73) | −4.1% |
| Sail, lock miss (minimal) | 35.25 (34.85–35.29) | −2.6% |
| Sail, lock miss, seeded | 33.88 (33.55–33.91) | −6.4% |

**Cohort R** (toolchain × path):

| | Plain path (`LD_LIBRARY_PATH`, `SAIL3112`) | Managed path (receipts recorded) |
|---|---:|---:|
| Minimal toolchain | 33.91 (33.71–33.94) | 34.52 (34.50–34.63), lock miss |
| Developer toolchain | 34.14 (34.00–34.16) | 37.18 (36.93–37.18), component miss |

## Hit rate and identity

- **Hit rate.** Every seeded run used 20,976 gets, all of them hits: 20,208
  from the shared store and 768 from memory. None was rejected and none timed
  out. No Z3 process ran a query. Unseeded misses made 20,208 puts.
- **Machine independence.** The keys bind exact solver input bytes
  (ADR 0008), and the store was filled from another project, home and Cargo
  home. Some consumers also ran on another toolchain (the minimal one,
  against the developer-toolchain producer), with a loader variable set, or
  with another lock closure. All of them hit 100%. The fork's vstd queries
  depend on none of these, and no normalization was involved.
- **Output.** After removing the run root, the project name and Cargo's
  `Finished … in` time, every Sail run's stdout equals stock's, and its stderr
  equals stock's as a set of lines. There are three exceptions, all expected:
  - the `note[SAIL3112]` line on loader-variable runs;
  - the lock arms' `Compiling proc-macro2 v1.0.106` line, because the stock
    control keeps the original lock;
  - the order of Cargo's parallel progress lines.

  Both vstd and the user crate report the same verification results
  (`2060 verified` and `1 verified`).

## Where a miss spends its time

**Timeline of the vstd unit.** Cohort P's poller recorded when the vstd
verifier and the solver processes ran:

| Arm | vstd verifier | Before the first query | Query window |
|---|---:|---:|---:|
| Stock | 27.95 (27.64–28.17) | 16.5 | 11.4 |
| Sail, component miss | 26.04 (25.89–26.20) | 14.9 | 11.2 |
| Sail, seeded | 24.42 (24.27–24.64) | (no Z3 started) | |
| Sail, loader miss | 25.79 (25.75–25.82) | 14.7 | 11.1 |

The fork's vstd verifier is about 1.9 s faster than stock's. Served replies
remove only about 1.6 s of the 11.2 s query window. The rest of the window is
Verus building and emitting queries, which stays on the critical path whatever
the solver costs. A full miss therefore costs about:
- 9–11 s of native compilation of the macro crates;
- about 15 s of vstd frontend;
- about 11 s of vstd query window, of which about 1.6 s is the solver;
- 1.5–2 s for the user crate;
- Sail's managed-path overhead (below).

**Managed-path overhead grows with the toolchain.** In cohort R, recording
receipts costs 0.6 s with the minimal toolchain and 3.0 s with the developer
toolchain. A system-wide `perf` profile of one component miss and one lock
miss shows where the difference goes (`perf-*-sym.txt` in the evidence):
- **Driver proxies.** On the component miss, the driver proxies spend
  4.4 CPU-s in SHA-256; on the lock miss, 0.1 CPU-s. An `openat` trace shows
  them reading the engine runtime's files 34 times (530 MB) on the component
  miss and never on the lock miss.
- **`sail` itself.** Its SHA-256 time is 3.0 CPU-s on the component miss and
  1.6 CPU-s on the lock miss. The developer toolchain adds about 0.8 GB of
  target libraries (978 MB against 160 MB), which the shipped-proof identity
  hashes; `sail new`, which computes the same identity, is 0.57 s slower on
  the developer toolchain (1.12 s against 0.55 s).

So when no shipped plan applies, the driver proxies rehash the runtime, and
when a plan exists, as on the lock miss, they do not. This comes from the
traces; the code path responsible has not been located.

## What this means

- **The lever works as designed, but it is small.** Replies are
  byte-identical across machines, so the hit rate is 100% and nothing is
  normalized. The saving is 1.4–1.8 s per vstd miss (4–5%). That turns the
  component miss from +3.3–4.4% to parity. Combined with a plain or
  lock-closure miss, Sail is 6–10% faster than stock.
- **Other vstd versions.** Shipping replies for the last N crates.io vstd
  versions helps only if those versions verify under the shipped runtime.
  Vest's 2026-09-06 pin had to be moved to the runtime's vstd to run at all
  ([cold first-run record](cold-first-run.md)), so N is 1 in practice.
- **The cheaper next lever is the managed path's hashing.** Rehashing the
  runtime in each driver proxy and the extra sysroot pass account for most of
  the 2.4 s that the developer toolchain adds. Hashing the runtime once per
  run, and refusing the bundle on its file-name set before hashing contents,
  would plausibly bring the component miss to about the loader-miss time
  (34.1 s against stock's 34.1–36.2 s). That is an estimate, not a
  measurement. It needs no new trust decision (#286, roadmap lever 1.15).

## Shipped replies (implementation)

| Field | Evidence |
|---|---|
| Source | Branch `perf/vstd-miss-replies` `c44fe25b` (main `2293db1f` plus ADR 0062), staged by `ci/local.sh release` as `0.1.0-replies`. The factory's `bundle-replies` step packed 20,208 entries (18.4 MB, key `bc2f6595896e5213`, pack SHA-256 `6b9690ae…`, `release-files.json` `3cafc9bb…`) |
| Environment | Native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, lane `missr2`, with the same host install and isolation as above. `release-shipped.py verify` passes on the stage |
| Arms | Stock; developer-toolchain miss with the tier on and with `SAIL_BUNDLED_REPLIES=0`; lock-file miss (`proc-macro2` downgraded, minimal toolchain); loader-variable run (`LD_LIBRARY_PATH`, minimal toolchain); bundle hit (minimal toolchain), with the tier on and off |

Wall seconds of `verify`: median (min–max), three rotated rounds (four for
the hit pair).

| Arm | Wall | vs stock |
|---|---:|---:|
| Stock (developer toolchain) | 29.82 (29.76–29.85) | |
| Sail, component miss, replies off | 31.76 (31.48–31.81) | +6.5% |
| Sail, component miss, **replies on** | **29.93 (29.88–30.19)** | +0.4% |
| Sail, lock-file miss | 27.76 (27.71–27.81) | −6.9% |
| Sail, loader-variable run (`SAIL3112`) | 26.95 (26.95–27.06) | −9.6% |
| Sail, bundle hit, replies on | 12.63 (12.61–12.66) | 2.36× faster |
| Sail, bundle hit, replies off | 12.63 (12.51–12.64) | |

- **Hit rate.** Every miss arm made 20,197 bundled lookups, all hits, none
  rejected. Its 11 puts are the user crate's own queries. With the tier
  off, the same run made 20,208 puts. The factory's recording uses the
  boundary research path and another project and Cargo home. Its vstd
  queries are still byte-identical to a consumer's, with no normalization.
- **Identity.** Every run's stdout equals stock's, and its stderr equals
  stock's as a set of lines. The only differences are Sail's expected notes
  (`SAIL3138` naming why the proof was not used, `SAIL3112`) and the lock
  arm's `proc-macro2 v1.0.106` line.
- **Bundle hit.** On, 12.63 s; off, 12.63 s. The pack loads on a helper
  thread when the daemon starts, and it stays resident (about 18 MB).
- **Tests.**
  - `crates/sail-daemon/tests/bundled.rs`, five tests against a real
    daemon. Each of these is a counted miss: a modified entry, an entry for
    another solver, an entry signed by another key, and a misfiled entry.
    Each of these turns the tier off: a key outside the index, a pack
    outside the index, a swapped key, a modified or malformed pack, and a
    missing index. Release and team trust never cross.
  - `sail_cache::pack` unit tests.
  - `crates/sail-cli/tests/shared_ops.rs`, for `bundle-replies`.
  - `scripts/test-release-shipped.py`, for the factory and `verify`.

Evidence: `experiments/2026-10-20/miss-replies/replies-impl-evidence.tgz`,
SHA-256 `d5d6c863a8a2f075899355c6a1a1381137b08c68788ced91d5035af215dee5b4`.

**Confirming cohort after rebasing onto main `13ef5258`** (with ADR 0060's
rlimit witness in the factory). This is a second release stage on lane
`missr2g`, which also ran the gates, so its timings are noisier: the load
average was still falling from the release build, and timings drift
upward across rounds. The factory packed 20,208 entries (pack SHA-256
`3c4bf83e…`) before running the rlimit witness. Three rotated rounds,
median (min–max):

| Arm | Wall | vs stock |
|---|---:|---:|
| Stock (developer) | 33.24 (33.06–33.61) | |
| Component miss, replies off | 34.14 (33.81–34.79) | +2.7% |
| Component miss, replies on | 33.13 (32.19–33.40) | −0.3% |
| Lock-file miss | 30.18 (29.62–31.06) | −9.2% |
| Loader-variable run | 29.18 (28.79–30.10) | −12.2% |
| Bundle hit | 13.41 (13.35–13.66) | 2.48× faster |

Hit rates and output identity are as in the first cohort. Evidence:
`experiments/2026-10-20/miss-replies/replies-rebased-evidence.tgz`, SHA-256
`7810d592f4f923840013ad62353f63a3db0b7fe06506dbdbe93544e3ee16dff9`.

## Raw evidence

Native evidence bucket,
`experiments/2026-10-20/miss-replies/miss-replies-evidence.tgz`, SHA-256
`9a6f2a0c004ffcaa55266e2a2268b75d636dc8303c50f2da314c2eaeb5a735d0`. It holds the
harness scripts and `results.tsv`, each run's stdout, stderr, daemon
statistics and poller timeline, the strace process and open traces, the
`perf` summaries, the identity report and the store's per-entry SHA-256
manifest. It does not hold the store, the caches or the build trees.

## Attempts

- 2026-10-20, strace-traced arms (`t-*`): the timings are invalid. The traced
  daemon kept `strace` alive after `verify` exited, and strace inflated stock
  to 43 s. Only the process timelines are used.
