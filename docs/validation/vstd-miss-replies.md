# Seeded vstd solver replies on a shipped-bundle miss (#236)

**Status:** experiment complete; nothing built. Seeding a fresh machine's
solver cache with signed vstd replies serves every vstd solver query (20,208
of 20,208) and saves 1.4–1.8 s on a cold run that misses the shipped vstd
proof. That brings a toolchain-component miss from 3.3–4.4% slower than stock
to parity, but not faster. Most of a miss is Verus's own vstd frontend and
query generation, which no reply cache touches. The rest of the gap to stock
is Sail's own hashing on the managed path. Shipping the replies needs the
owner's trust decision ([ADR 0062](../decisions/0062-shipped-vstd-solver-replies.md),
proposed; [question 36](../open-questions.md)).

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
