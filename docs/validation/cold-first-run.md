# Cold first run against stock (#232)

**Status:** the 0.1.0 bar (at most 5% slower than stock) is **not met** for the
small demo project. Branch `perf/cold-gap` cuts the demo gap from +22.5% to
+8.2% and public-profile Vest from +12.6% to +4.8%. The remaining demo gap
comes from how instant revert admits native compiles
([ADR 0050](../decisions/0050-retained-proofs-instant-revert.md)). Closing it
needs a design change that the owner must review. Warm, unchanged and edit
runs show no regression.

| Field | Evidence |
|---|---|
| Source | Base `3520f2b4` (the 0.1.0 candidate before its merge as #249). Candidate: the three #232 commits on that base, measured from the committed tree. They were then rebased onto main `4f373727`, whose code differs from the base only in shipped-record path remapping, a note's wording and tests. That rebase was not re-measured |
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

## Raw evidence

Native evidence bucket, `experiments/2026-10-20/cold-gap/cold-gap-evidence.tgz`,
SHA-256 `4b03f071c21740d8f68d35f3dd5f1701fd3e7b8ba833a26002c34d4cf1aa57c5`. It
holds every run's stdout and stderr, `results.tsv` per cohort, the shim and
driver timestamp logs, the exec traces, the harness scripts and the binary
digests.

## Attempts

- 2026-10-20, Vest cycle with a different `SAIL_JOB_EVENTS` path per step:
  invalid. The caller environment differed between steps, so retained restore
  refused, and returned A took about 24 s in both base and branch. Rerun
  without that variable.
- 2026-10-19, first demo and Vest cohort: Vest outputs were deleted by the
  harness's cleanup. The timings stand; outputs were not compared.
