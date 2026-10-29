# Cold first run against stock (#232)

**Status:** the 0.1.0 bar (at most 5% slower than stock) is **not met** for the
small demo project on macOS: +12.3% in a layout without the shipped bundle,
where every arm verifies vstd. Public-profile Vest meets it on both platforms with the
closure-scoped source check ([follow-up](#follow-up-closure-scoped-retained-source-check-278)):
macOS +3.9%, Linux −0.7%. Branch `perf/cold-gap` earlier cut the Linux demo
gap from +22.5% to +8.2% on a host build without the bundle; the staged Linux
release, which serves vstd from its bundle, runs the demo's first run 2.3×
faster than stock. The remaining demo gap comes from how instant revert admits
native compiles ([ADR 0050](../decisions/0050-retained-proofs-instant-revert.md)).
Closing it needs a design change that the owner must review. Warm, unchanged
and edit runs show no regression.

| Field | Evidence |
|---|---|
| Source | Base `3520f2b4` (the 0.1.0 candidate before its merge as #249). Candidate: the three #232 commits on that base, measured from the committed tree. They were then rebased onto main `c4cb6076`, whose code differs from the base only in shipped-record path remapping, a note's wording and tests. That rebase was not re-measured |
| Runtime | Fork `9c0ca7e7`, variant `7d829946…-4ed2dee08bc877ae`, pinned Z3 4.16.0; stock oracle 0.2026.10.18.7d82994 |
| Environment | Native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, host build (no CI image), Rust 1.99.1 Cargo for both arms |
| Scope | Demo (`sail new --lib` / `cargo verus new --lib`) first run with an empty cache; public-profile Vest cycle (A cold, B edit, A returned, A unchanged, comment no-op) |
| Limits | One host and one lane; three interleaved rounds per scenario; Linux only; no macOS measurement |

## Results

Three interleaved rounds per scenario. Each round has a fresh copy and an
empty Sail cache for each arm. Every sample was recorded and none is
dropped. Values are seconds: median (min–max).

| Scenario | Stock | Base | Branch | Branch vs stock |
|---|---:|---:|---:|---:|
| Demo, first run | 31.31 (30.90–31.70) | 38.35 (38.18–38.52) | 33.89 (33.37–34.02) | +2.58 s, +8.2% |
| Demo, unchanged rerun | — | 0.216 (0.211–0.216) | 0.221 (0.216–0.222) | |
| Vest A, cold | 70.73 (68.42–70.97) | 79.64 (76.60–79.64) | 74.11 (72.67–74.20) | +3.39 s, +4.8% |
| Vest B, edit | 39.03 (38.02–39.27) | 26.58 (26.03–26.72) | 25.35 (25.18–25.68) | 1.54× faster |
| Vest A, returned | 38.12 (37.95–39.46) | 2.75 (2.67–2.79) | 1.62 (1.57–1.63) | |
| Vest A, unchanged | 0.199 (0.189–0.221) | 0.511 (0.485–0.526) | 0.517 (0.500–0.517) | |
| Vest, comment no-op | 38.42 (38.42–39.67) | 25.74 (25.07–25.97) | 24.26 (24.06–24.46) | |

The per-round Vest cold gaps are +4.9%, +4.4% and +6.2%, so Vest sits at
the bar rather than safely inside it. Vest follows the
[instant-revert setup](instant-revert.md#verus-7d82994-runtime-requalification):
its vstd requirement and lock are moved to crates.io
`vstd 0.0.0-2026-10-18-0147`, it runs `-p vest_lib -- --rlimit 100` in every
arm, and the public alloc/std retained profile is active. All Vest calls exit
0. After removing arm roots and Cargo's `Finished … in` time, every Sail run's
stdout and stderr equal stock's as a set of lines.

Earlier cohorts on the same host agree. Demo: stock 29.79 / 29.89, base
37.02 / 37.01, intermediate branch builds 32.38 / 32.53. The source-patched
Vest copy keeps the retained profile off and is 1.4–2.3% faster than stock on
the branch. That copy exits 101 in every arm because `parse_cbor_with_child`
exceeds the default rlimit under 7d82994, so its edit and no-op numbers are
not used here.

## Where the base gap went

Timestamps came from an uncommitted experiment build: per-session phase marks
in `sail-z3`, per-phase marks in the driver proxy and the runner, and a
system-wide exec/exit trace. Demo base was 7–8 s slower than stock:

- **Shim and daemon, about 3 s.** Every solver session's `HELLO` read the
  whole 35 MB z3 into a new allocation and compared it with the retained body,
  taking 29 ms median and 13.1 s summed over 366 sessions. Each finished
  session also sent about 55 stores as separate round trips, and shim exit
  waited out the stderr monitor's 10 ms sleep (3.7 ms median). On the branch,
  `HELLO` compares the file in place through a 256 KiB buffer while the version
  command runs, giving an 8 ms median. It still reads every byte of every
  handshake, and the #213 controls are unchanged. Stores go in windows of 64,
  and the monitor is unparked at drop. Fork-plus-shim fell from +3.0 s to
  +0.8 s over the plain fork, and vstd's verification now runs in stock's time.
- **Retained admission in the driver proxy, about 3.5 s.** Each native unit
  and each verified target copied and SHA-256-hashed the whole `deps/`
  directory several times, one file at a time. That included a first complete
  copy that is discarded immediately. vstd's admission took 0.57 s before
  compiling and 0.48 s after, and the project's 0.88 s and 0.47 s. On the
  branch, the producer wait uses a path listing instead of the discarded copy,
  and copies, rereads, graph checks, certificate checks, closure-source checks
  and runtime hashing run on up to eight workers. The independent
  `unchanged()` checks run concurrently, and the source snapshot overlaps the
  graph capture and the publication checks. All of these are still complete
  reads.

## What remains on the demo (+2.6 s)

- **About 0.3 s of Sail setup before Cargo starts.** This covers metadata,
  the runtime context, the input snapshot, retained plan preparation and
  daemon start.
- **About 0.5–0.9 s on the native critical path.** A native unit's admission
  waits until every artifact in `deps/` has a successful producer certificate,
  and until no compiler scratch directory remains there
  (`producers::ready_for`, `capture_after_directory_work`).
  `verus_prettyplease` therefore starts after `verus_syn` has fully finished.
  Stock pipelines it from `verus_syn`'s rmeta. `synstructure` waits 0.8 s,
  though off the critical path.
- **About 0.1–0.3 s per verified target before and after Verus.** Most of it
  is the source snapshot (about 18 ms on demo, 175 ms on Vest) and three or
  four full passes over the 44 MB `vstd.vir`.
- **About 0.17 s after Cargo exits** (0.3 s on Vest): target records and
  retained publication.

Keeping pipelining means certifying a native unit's inputs after its compile
rather than before, or changing the scratch-directory wait. Both change
ADR 0050's admission design and need owner review and their own qualification.

## Follow-up: namespace checks and a repeated graph reread

Two follow-up changes. Workspace source admission now reads and lexes its
sources on up to eight workers. The namespace digest gathers path facts
concurrently and hashes them in the original order, so the digest is
unchanged; a test compares it with the sequential digest. `publish_graph`
also no longer rereads a graph that `publish` reread in full immediately
before, with nothing in between; a test shows that `publish`'s own reread
refuses a changed copy.

These changes were measured on the same host, with three interleaved rounds,
on the public-profile Vest cycle. Values are seconds: median (min–max).

| Step | Stock | Before (#256) | After |
|---|---:|---:|---:|
| A, cold | 66.59 (66.46–67.44) | 70.32 (70.18–70.37), +5.6% | 69.84 (69.69–69.95), +4.9% |
| B, edit | 37.36 | 23.95 | 23.93 |
| A, returned | 37.53 | 1.532 | 1.433 |
| A, unchanged | 0.187 | 0.487 | 0.482 |
| Comment no-op | 37.30 | 22.80 | 22.68 |

Every output equals stock's. Sixteen workers instead of eight changed
nothing measurable. Evidence: `experiments/2026-10-20/vest-cold/vc-evidence.tgz`,
SHA-256 `a3cbf7e04825d44e669cfe68a03f797effa6d31d2965d279945b9f625dbfc412`.

## Follow-up: closure-scoped retained source check (#278)

**Status:** public-profile Vest's cold run on macOS goes from 6.6% to 3.9%
slower than stock, inside the 5% bar. On Linux it is 0.7% faster than stock.
The demo's first run on macOS stays 12.3% slower: its remaining cost is native
admission and the observed compile, which this change does not touch.
Instant revert stays intact and gets faster. The change is an accepted
[ADR 0050 amendment](../decisions/0050-retained-proofs-instant-revert.md#amendment-2026-10-20-the-source-snapshot-reads-the-selected-closure-278)
(accepted 2026-10-20).

| Field | Evidence |
|---|---|
| Source | Main `908abe1b`; branch `perf/cold-retention` `ea5f4d66` (main plus the scoping commit). The branch was then rebased onto main `539dbb7b` as `0514c265` with the same diff. Main by then serves the shipped vstd proof under a raised `--rlimit` (#268) and ships vstd solver replies (#287), which change Vest's absolute cold times; that rebase was not re-measured |
| Runtime | macOS `7d829946…-e9b69a75ea390499`, Linux `7d829946…-81186034d2e137a2` (fork `f8589ed4`); stock oracle 0.2026.10.18.7d82994, Rust 1.99.1 |
| Environment | macOS: EC2 mac-m4.metal, us-east-1d, macOS 27.0, idle apart from the cohort. Linux: native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, Linux release trees staged by `ci/run.sh release`, run inside the CI image (`core`) with the verifier toolchain reduced to the README profile |
| Scope | Demo first run; public-profile Vest cycle (A cold, B edit, A returned, A unchanged), `-p vest_lib -- --rlimit 100` |
| Limits | Three rounds per scenario, arms rotated by round; one host per platform. The macOS no-retention arm is a probe build (branch plus timing probes; one environment variable skips the plan), used only to attribute cost |

Each run has a fresh home, Cargo home (registry downloads included in every
arm) and Sail cache, plain human output, and a throwaway run that launches
every tool once first. Values are seconds: median (min–max). Every one of the
75 Sail steps exits 0 and matches stock's stdout and stderr after removing the
arm root and Cargo's `Finished … in` time, with progress lines compared as a
set.

macOS:

| Step | Stock | Main | Branch | No retention (probe) |
|---|---:|---:|---:|---:|
| Demo, first run | 16.70 (16.65–16.82) | 18.81 (18.66–18.85) | 18.75 (18.73–18.98) | 17.13 (17.08–17.33) |
| Vest A, cold | 40.34 (40.31–40.49) | 42.99 (42.70–43.43) | 41.90 (41.66–42.60) | 40.10 (39.90–40.41) |
| Vest B, edit | 22.31 (22.24–22.38) | 13.57 (13.52–13.61) | 12.92 (12.89–13.05) | 12.51 (12.50–12.57) |
| Vest A, returned | 22.36 (22.32–22.39) | 1.78 (1.77–1.78) | 1.16 (1.16–1.18) | 12.00 (11.89–12.01) |
| Vest A, unchanged | 0.23 (0.22–0.23) | 0.69 (0.69–0.70) | 0.69 (0.69–0.70) | 0.69 (0.69–0.70) |
| Vest cycle | 85.24 (85.17–85.41) | 59.04 (58.69–59.51) | 56.71 (56.56–57.35) | 65.36 (65.11–65.49) |

Linux:

| Step | Stock | Main | Branch |
|---|---:|---:|---:|
| Demo, first run | 31.94 (31.75–32.23) | 13.65 (13.63–13.71) | 13.63 (13.63–13.68) |
| Vest A, cold | 73.39 (72.90–73.79) | 73.27 (72.75–73.37) | 72.90 (72.50–73.18) |
| Vest B, edit | 40.03 (39.72–40.36) | 24.43 (24.23–24.73) | 24.34 (24.29–24.63) |
| Vest A, returned | 39.66 (39.54–40.33) | 1.62 (1.61–1.63) | 1.16 (1.14–1.18) |
| Vest A, unchanged | 0.21 (0.20–0.21) | 0.58 (0.58–0.58) | 0.58 (0.58–0.58) |
| Vest cycle | 153.16 (152.49–154.68) | 99.70 (99.39–100.29) | 99.26 (98.51–99.28) |

One further round per platform used the instant-revert harness with JSON
output (`bench.py` `b612732f…`, `analyze.py` `c1578803…`, as in the
[instant-revert record](instant-revert.md#closure-scoped-source-check-278)).
On both, returned and unchanged A run no verifier, reuse both targets and keep
cold A's establishing run; B reruns only `vest_lib`. Every step's diagnostics
equal stock's, and no `SAIL3136` appears.

The Linux demo serves vstd from the shipped bundle; on macOS that bundle is
not in the measured layout, so every arm verifies vstd.

### Where macOS retention time went

Timing probes (one cold run per arm, `SAIL_RETAINED_TRACE`, compared with the
no-retention arm of the same build):

- **Source snapshot, 200–265 ms each, five times per Vest cold run.**
  `Plan::snapshot` read every package in Cargo's metadata, about 11,600 files
  on Vest. It ran before and after vstd's compile, before and after
  `vest_lib`'s, and at publication: about 1.25 s, nearly all on the critical
  path. The branch reads the selected closure: 25–50 ms each.
- **Native admission, about 0.75 s on both projects.** Each native unit waits
  for its dependencies' certificates and for other units' scratch directories,
  so `verus_prettyplease` and `verus_builtin_macros` no longer overlap
  `verus_syn`. This is the remainder recorded above.
- **The demo's own compile, about 0.33 s** (0.60 s instead of 0.27 s). It
  stays at 0.58 s without the compile observer and without the SHA-256
  checksum request, so neither causes it; the cause is open (#288). It is not
  separable from noise on `vest_lib`'s 22 s.
- **The rest, about 0.6 s on Vest:** plan preparation (35 ms), the targets'
  graph copies and certificate checks (vstd about 0.1 s, `vest_lib` about
  0.2 s before and 0.08 s after its compile) and publication (0.15 s).

Branch against no retention: demo +1.62 s, Vest +1.80 s. Branch against main:
Vest cold −1.09 s, B −0.65 s, returned A −0.62 s. On Linux, a separate
three-round cohort of the branch against the same no-retention probe gives
demo 13.84 against 12.15 s and Vest cold 72.52 against 70.63 s, so retention
still costs about 1.7–1.9 s there, offset by Sail's other first-run savings.

A pipelining experiment, later accepted as ADR 0061
([below](#pipelined-native-admission-adr-0061)), ran demo 18.34 s against
18.61 s and Vest cold 41.29 s against 42.06 s in a second macOS cohort
without the shipped bundle.

A first run that prepares no plan (for example `SAIL_MANAGED_FORCE=1`)
compiles the native units and vstd without certificates. Every later run then
waits 2 s for certificates that cannot come and never retains (#285). So
skipping retention on a first run would lose instant revert for that target
directory until its dependencies are rebuilt, and is not a way to remove
this cost.

## Pipelined native admission (ADR 0061)

**Status:** accepted and selected. Native units compile as soon as Cargo
starts them and are certified after their own compile
([ADR 0061](../decisions/0061-pipelined-native-admission.md)). Public-profile
Vest's cold run is now faster than stock on Linux (−2.2%); on macOS it is
+4.3% (within the 5% bar). Instant revert is unchanged.

| Field | Evidence |
|---|---|
| Source | Main `539dbb7b`; #289 (closure-scoped source check) `7ea42f2b`; branch `perf/pipelined-native` `780fc392` (#289 plus ADR 0061). Rebased afterwards onto main `9ccbe879` (#276 adds a closure-difference note) with the same diff; the rebase was not re-measured |
| Runtime | macOS `7d829946…-e9b69a75ea390499`, Linux `7d829946…-81186034d2e137a2` (fork `f8589ed4`); stock 0.2026.10.18.7d82994, Rust 1.99.1 |
| Environment | Full release trees from `scripts/release.sh stage` (shipped vstd bundle and replies). macOS: EC2 mac-m4.metal, us-east-1d, macOS 27.0. Linux: native EC2 c7i.8xlarge, us-east-1, inside the CI image (`core`), README verifier toolchain |
| Scope | Demo first run; public-profile Vest cycle, `-p vest_lib -- --rlimit 100`; three rounds per platform, arms rotated; one JSON instant-revert round per platform |
| Limits | The macOS host was terminated before its raw files were copied; only the analyzer summary survives in the evidence archive |

Medians of three, seconds (Linux):

| Step | Stock | Main | #289 | Branch |
|---|---:|---:|---:|---:|
| Demo, first run | 31.05 | 13.43 | 13.53 | 13.41 |
| Vest A, cold | 71.45 | 71.31 | 70.45 | 69.91 |
| Vest B, edit | 39.15 | 24.02 | 23.76 | 23.64 |
| Vest A, returned | 38.99 | 1.57 | 1.13 | 1.13 |
| Vest A, unchanged | 0.20 | 0.56 | 0.56 | 0.56 |
| Vest cycle | 149.78 | 97.45 | 95.93 | 95.23 |

macOS:

| Step | Stock | Main | #289 | Branch |
|---|---:|---:|---:|---:|
| Demo, first run | 16.71 | 10.54 | 10.54 | 10.54 |
| Vest A, cold | 40.76 | 43.96 | 42.79 | 42.53 |
| Vest B, edit | 22.27 | 13.71 | 13.05 | 13.01 |
| Vest A, returned | 22.28 | 1.78 | 1.17 | 1.17 |
| Vest A, unchanged | 0.23 | 0.69 | 0.70 | 0.69 |
| Vest cycle | 85.49 | 60.14 | 57.63 | 57.36 |

Every Sail step exits 0. Stdout and stderr equal stock's after normalization,
except that every Sail arm, main included, prints main's intended
`note[SAIL3138]` on Vest's cold run (vstd's dependency versions differ from
the shipped build's, so vstd is verified). The JSON round passes on both
platforms: returned and unchanged A run no verifier, reuse both targets and
keep cold A's establishing run; B reruns only `vest_lib`; no `SAIL3136`.
With the shipped bundle, the demo serves vstd, so its native admission is no
longer on the critical path and every Sail arm runs it 1.6× (macOS) to 2.3×
(Linux) faster than stock.

Gates on `780fc392` (Linux lane): solver, every e2e phase plus extra, PR
conformance (33/33 in every arm and engine), fast conformance (147/147 in
every arm and engine) and standard conformance (fill 159/159, engine 159/159,
pass-through and warm 158 same plus anvil unstable on module progress-note
order, 0 different) pass. The workspace gate failed only on a missing anchor
to this section and passes after it.

Evidence: `experiments/2026-10-20/pipelined-native/pipelined-evidence.tgz`,
SHA-256 `9a49618177c2181a58f44548ecb4f006b1a66ff322c7584873136559cbb72188`.

## Raw evidence

Native evidence bucket, `experiments/2026-10-20/cold-gap/cold-gap-evidence.tgz`,
SHA-256 `4b03f071c21740d8f68d35f3dd5f1701fd3e7b8ba833a26002c34d4cf1aa57c5`. It
holds every run's stdout and stderr, `results.tsv` per cohort, the shim and
driver timestamp logs, the exec traces, the harness scripts and the binary
digests.

Closure-scoped follow-up (#278):
`experiments/2026-10-20/cold-retention/cold-retention-evidence.tgz`, SHA-256
`be1ac53a3a03aa5c6fb41e013daa21aada9f7a693c846b36139aa8002410fabb`. It holds
`results.tsv` and every step's stdout and stderr for both platforms, the
probe traces, the forced-first-run case, the JSON revert rounds, the probe and
experiment diffs, the drivers (`cr.py`, `lxb.py`) and the analyzer
(`cr-analyze.py`).

## Attempts

- 2026-10-20, Vest cycle with a different `SAIL_JOB_EVENTS` path per step:
  invalid. The caller environment differed between steps, so retained restore
  refused, and returned A took about 24 s in both base and branch. Rerun
  without that variable.
- 2026-10-19, first demo and Vest cohort: Vest outputs were deleted by the
  harness's cleanup. The timings stand; outputs were not compared.
- 2026-10-20, #278 macOS probe cycle with a different trace path per step:
  invalid for the same reason; returned A took about 29 s. Rerun with one
  path per run.
- 2026-10-20, #278 first macOS probe build: Cargo reused main's binaries from a
  shared target directory, so no probe ran. Rebuilt after touching the sources.
- 2026-10-20, #278 Linux comparison, first analysis: six cold Vest steps were
  flagged because Cargo printed `1m 11s`, which the time normalization missed.
  With the normalization fixed, all 75 steps match stock.
