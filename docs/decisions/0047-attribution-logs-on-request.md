# 0047: Attribution logs on request, and cheaper managed checks

- **Status:** accepted (lead decision for the owner's "certainly we don't
  want a perf regression", 2026-10-15); see the
  [2026-10-16 note](#note-2026-10-16-cheaper-checks-156) for the checks'
  current cost
- **Date:** 2026-10-15
- **Amends:** [ADR 0014](0014-attribution-ledger-and-output-schema.md)
  (when Sail asks Verus for `--log air` and `--log smt-transcript`) and
  [ADR 0046](0046-managed-verification-by-default.md) (the managed driver's
  flags, item 6)

## Context

- **The regression.** With managed verification as the only `sail verify`
  (ADR 0046), a vest edit took 28.5 s against 25.4 s for the removed
  pass-through (plain stock Verus: 39.6 s). ADR 0046 attributed about 1 s to
  writing 355 MB of AIR and SMT transcript logs on every run, 0.3 s to
  reading them, and 1.2 s to input checks. The owner asked whether
  per-function attribution logs are needed on every run, or could be
  produced later.
- **What needs the logs.** Verus's own `--output-json --time`, which the
  driver asks for on every run anyway (to rebuild Verus's stdout and to bind
  receipts), already gives every function's verdict, rlimit and solver time.
  The logs add per-obligation records: op kinds, spans, per-check answers,
  rlimit use, cache provenance (the shim's session records join
  transcripts) and counterexample models. Consumers:

  | Consumer | Needs from the logs |
  |---|---|
  | `sail verify` without `--output json` (stdout, stderr, exit code) | nothing: Verus's own output |
  | Proof receipts and Currentness | nothing: a receipt needs a passing target, whose functions are all verified by Verus's JSON |
  | `sail verify --output json`, jobs, `--background`, `watch`, MCP `verify`, `verification_result`, `counterexample`, `explain_failure`, `sail explain` | obligations, failing spans, counterexamples, `unknown` for resource give-ups, per-check provenance |
  | Repairs (interactive engine) | the focused phase's obligations (`logs/focus`) |
  | `--measure-fragility`, `--portfolio` | their own logs (they never ran the managed driver) |

  Every reader of obligation detail is a structured caller: the daemon's
  job worker runs `sail verify --output json`, and MCP, `sail explain` and
  `watch` read jobs. Plain `sail verify` prints only what Verus prints; its
  report is used for receipts and is otherwise never shown.
- **Where the rest of the time went** (vest `-p vest_lib`, EC2 c7i.4xlarge,
  phase timings from `SAIL_JOB_EVENTS` and experiments on the pass-through
  build, interleaved, three rounds each):
  - the logs cost about 0.8 s per edit, writing and reading them included;
    `--output-json --time`, which every managed run needs, about 0.3 s;
  - Sail's targeted `cargo clean -p` of the edited package deletes rustc's
    incremental state, which plain Cargo keeps across edits: the
    pass-through with that clean added took 24.9 s against 23.9 s, and
    with the incremental directory kept, 24.0 s;
  - the "double hashing" was mostly not hashing: one input snapshot of
    11,600 files took about 250 ms, of which reading and hashing every file
    was 65 ms, resolving every path with `realpath` 130 ms, and three
    `lstat` passes the rest.
- **The fork alternative.** A Verus-fork option writing a compact
  attribution sidecar (function, kind, span, answer, effort, model per op)
  would make structured runs cheap too. Its gain is bounded by what the
  logs cost there: about 0.8 s per vest edit for `--output json`, jobs and
  MCP, and nothing for plain `sail verify` after this change.

## Decision

1. **Logs only when the report is read.** The managed driver asks Verus for
   `--log air --log smt-transcript --log-dir <sidecar>` only when the run's
   caller reads its report: `sail verify --output json` (and therefore
   jobs, `--background`, `watch` and MCP), repairs, and every
   interactive-engine run (`managed::Config::attribution_logs`, carried to
   the driver in the run's runtime record). Plain `sail verify` asks for
   neither log, and sets no `SAIL_PROVENANCE_DIR`. No new flag or
   environment variable selects this: the output format decides.
2. **Reports without logs say so.** Without logs, a target's report is
   built from Verus's JSON alone: `attribution.status` is `unavailable`,
   its first note says the obligation detail was not recorded by the run
   that established the report, every `obligations` is `null` and every
   provenance `source` is `unrecorded`. Verdicts follow the existing ledger
   rules, so a passing target's functions are `verified` exactly as with
   logs (a target with no Verus error has no failing verdict-bearing
   obligation), and a failing target keeps the incomplete-attribution rule
   (VI-0003: successful timing entries in a failing crate are `unknown`).
   Such a failing report is never shown and gets no receipt. A passing
   target's receipt carries the report, so a later `--output json` or MCP
   run that reuses it shows the note rather than obligations, until the
   target is verified again with logs; it never shows invented detail.
3. **No lagging re-run.** A background re-run of failing targets with logs
   is not built. Failing targets are the only ones whose detail matters,
   their readers already run with logs, and a re-run would repeat Verus's
   frontend (about 20 s on vest) and need report amendment in receipts and
   jobs for detail nobody requested.
4. **Cleans keep rustc's incremental state.** Sail's targeted cleans move
   each `<profile>/incremental` directory of its target aside around
   `cargo clean -p` and put it back (`managed/runner.rs`, `clean`). The
   state is a cache that rustc checks against the source contents and the
   compiler options before using; plain Cargo keeps it across edits. A
   directory that cannot be restored is deleted, which only loses the
   cache. What the clean forces (Cargo compiles and Verus verifies the
   package again) is unchanged.
5. **The input walk resolves only symbolic links, and stamps overlap their
   `lstat` calls.** A directory entry that is not a link is already its own
   canonical path. The snapshot's files, digests, roots, stamps and
   revisions are identical (a unit test compares the walk with the old one
   on a tree with file and directory links, a cycle and a dangling link);
   ADR 0041's rules are unchanged: every input is read in full before and
   after the run, and the stamps cover the same paths.
6. **Conformance.** `sail-conformance`'s Sail arm keeps the logging flags
   (`adapter::OBSERVATION_LOG_FLAGS`): it covers `--output json`, jobs and
   MCP, and the flags are pipe-neutral (ADR 0014). Plain `sail verify` now
   runs Verus exactly as the bare-shim arm does (`--sail-arm shim`), and the
   e2e legacy phase compares it with plain `cargo verus verify` byte for
   byte; the e2e managed phase checks that it writes no logs and that a
   later `--output json` run reuses its proof with the note.
7. **The fork sidecar is proposed, not built.** It would remove at most
   about 0.8 s from structured runs on vest. The owner's rule for fork changes
   is a measured gain first; this records the bound, and a fork change
   needs its own measurement on the fork build.

## Measurements

`scripts/perf-loops.sh` with `SAIL_PERF_BASELINES`, three rounds, medians,
on EC2 c7i.4xlarge (us-east-1, Ubuntu 24.04, no other work on the host, one
timed run at a time), Sail's Verus `73dea021`, 2026-10-15. Every mode runs in
the same rotation: plain stock `cargo verus verify`; the removed
pass-through (`main` at `5f7ece1a`); managed verification before this change
(PR #155 at `27fee790`); and this change. Seconds:

| Project, phase | plain | pass-through | before (#155) | after |
|---|---:|---:|---:|---:|
| vest, edit | 41.8 | 26.4 | 29.9 | 27.8 |
| vest, failing edit | 37.3 | 26.7 | 29.2 | 28.2 |
| vest, fix | 41.7 | 26.5 | 29.9 | 27.7 |
| vest, unchanged | 0.23 | 0.23 | 1.31 | 0.99 |
| vest, cold | 75.1 | 75.4 | 78.4 | 77.6 |
| vest, MCP `verify` unchanged | — | — | — | 1.34 |
| demo, edit | 0.80 | 0.82 | 1.17 | 1.11 |
| demo, failing edit | 0.58 | 0.62 | 0.94 | 0.87 |
| demo, fix | 0.80 | 0.82 | 1.12 | 1.06 |
| demo, unchanged | 0.12 | 0.11 | 0.41 | 0.34 |
| demo, cold | 32.7 | 33.1 | 34.0 | 33.8 |
| demo, MCP `verify` unchanged | — | — | — | 0.47 |

This host ran about 5% slower than ADR 0046's (plain vest edit 41.8 s
against 39.6 s); compare within the table. Exit codes were identical across
modes in every phase. Raw logs and tables are kept outside the repository in
`l54-linux-perf.tgz` (sha256
`142d3a133d833d8820ba0a566b0d969ac3f9b142a65589598fd346aab9f1865f`).

**What this change removed** (vest edit): 2.1 s of the 3.5 s that managed
verification cost over the pass-through; unchanged re-runs 1.31 → 0.99 s.

**What remains** (vest edit, 1.4 s over the pass-through; demo, 0.29 s):

- **Receipt and Currentness checks (ADR 0041), about 0.8 s on vest and
  0.28 s on demo**, from `SAIL_JOB_EVENTS` phases: Cargo metadata (85 ms),
  the input snapshot before the run (about 200 ms on vest: 11,600 files read
  and hashed, three stamp passes, the absent-configuration observer), the
  dirty-receipt scan and the targeted clean (100 ms, one Cargo start), the
  target records (120–150 ms: receipt checks, which hash every reused
  artifact, vstd's included), the second metadata and snapshot (230 ms) and
  publication (40 ms). On demo, hashing vstd's artifacts for its receipt is
  the largest item. Each check is one a receipt or a Current label depends
  on; none can be skipped without changing what they guarantee.
- **`--output-json --time`, about 0.3–0.5 s on vest.** Managed verification
  needs Verus's per-function breakdown, which only `--time` produces, for
  every run's report and receipt. With the same flags the managed invocation
  and the pass-through take the same time (c7i.8xlarge, four interleaved
  rounds: Cargo 21.9–22.3 s each).
- **Logs on structured runs.** `--output json`, jobs and MCP still write the
  logs; see below.

**Where the flags and logs cost** (vest edits, c7i.4xlarge, four
interleaved rounds each, medians):

| Run | Seconds |
|---|---:|
| pass-through, no extra flags | 25.89 |
| pass-through, `-- --output-json` | 25.88 |
| pass-through, `-- --time` | 25.77 |
| pass-through, `-- --output-json --time` (what managed runs add) | 26.21 |
| this change, `sail verify` (no logs) | 26.99 |
| this change, `sail verify --output json` (logs) | 27.76 |

So a fork-side compact attribution sidecar could save structured runs
(`--output json`, jobs, MCP) at most about 0.8 s per vest edit, and plain
`sail verify` nothing. That is the bound to beat with a measured fork build
before proposing the change to the owner.

On this Mac (Apple silicon, shared with other lanes, load average 47–92 at
the time) no timing was meaningful; macOS was checked for correctness only
(`ci/run.sh host`).

## Consequences

- **Plain `sail verify` writes no logs**, so it is no longer slower than the
  removed pass-through on solver-heavy edits by the logs' cost, and its
  sidecars are a few files instead of hundreds of megabytes.
- **Structured callers are unchanged**: `--output json`, jobs and MCP pay the
  logs as before and get full obligation detail for every target they
  verify. A target reused from a plain run's receipt reports Verus's
  verdicts, rlimit and time, with a note instead of obligations.
- **Rustc incremental state persists in Sail's target**, as in the user's
  own target; `sail cache projects prune` removes it with the rest.
- **What remains** is listed under Measurements: the receipt and
  Currentness checks (ADR 0041) and, for structured runs, the logs.

## Note (2026-10-16): cheaper checks (#156)

**Where the time went.** An instrumented build (timestamps at every
statement of the run, `perf` on the Sail process, `strace` for its
children) on vest `-p vest_lib`, c7i.4xlarge, unchanged re-run, about
1.0 s in all: Cargo metadata, overlapping the runtime context, 0.12 s; the
input snapshot before the run 0.19 s, a third of it ordered inserts that
compare paths component by component, the rest walking and reading 11,600
files; Cargo-Verus 0.17 s; the receipt checks 0.13 s, which read vstd's
40 MB `.vir` twice and parsed each stored receipt twice; the post-run
metadata and snapshot 0.2 s; the two stamps 0.03 s; dependency-store
publication 0.05 s, re-validating vstd's receipt on every run although the
store already held it; the result document 0.025 s; freeing the run's data
0.02 s. On the demo (0.34 s), the receipt checks (0.11 s) and the store
publication (0.05 s) were the largest items.

**What changed.** Every check reads what it read before, completely, at
the same point relative to the invocation; nothing is skipped and no
stamp replaces a content read (ADR 0041):

1. Snapshots collect paths in hashed sets and sort them once by a byte key
   that orders exactly as `Path` does. A stable sort preserves the first raw
   spelling of equivalent paths, as ordered inserts did; a unit test compares
   the original path bytes with ordered inserts. Digests reuse one read
   buffer per thread.
2. The snapshot before the run is taken while Cargo reads the metadata,
   from what the previous run of the same directory and arguments selected
   (`<cache>/snapshot-hints/`). It is used only if this run's selection
   (the metadata fields the snapshot reads and the extra inputs) is equal,
   the snapshot is complete, and every path it observed still has the
   stamp it had around its reads once the metadata has arrived, since
   Sail's own `cargo metadata` may rewrite the lockfile or extract
   registry sources. Otherwise it is taken again. Its reads still finish
   before Cargo-Verus starts, and the run's mutation stamp is still taken
   after them.
3. After the invocation, when no retry can follow (a run that shows Verus's
   output, a run without reuse, or one that failed), the post-run inputs
   overlap the receipt checks, and the snapshot overlaps the post-run
   metadata; it is kept only for the same package list and unchanged
   stamps. The stamp that brackets every content read is taken once both
   have finished. One difference: if the receipt checks fail with an I/O
   error, the build-script and source-input records have already been
   updated from this run's Cargo output, which they describe.
4. One phase's receipt checks read each artifact and dependency once, in
   one overlapping batch, and parse each stored receipt once.
5. Target revisions copy only the metadata fields the snapshot reads, not
   the whole metadata per target.
6. A dependency the store already holds is not re-validated for
   publication, which would store nothing.
7. The driver's complete read of Verus overlaps its dependency reads.
8. The run's large values are freed off the caller's path.

**Historical measurements.** These precede the rebase and final raw-path
spelling correction; final-code qualification follows below.
`scripts/perf-loops.sh` with `SAIL_PERF_BASELINES`, three
rounds, medians, EC2 c7i.4xlarge (us-east-1, Ubuntu 24.04, no other work
on the host, one timed run at a time), Sail's Verus `73dea021`,
2026-10-16. "main" is `6deb592f` (#173), the pass-through `5f7ece1a`.
This host ran about 3% slower than the one above (plain vest edit 43.2 s
against 41.8 s). Seconds:

| Project, phase | plain | pass-through | main | this change |
|---|---:|---:|---:|---:|
| vest, edit | 43.2 | 27.8 | 29.1 | 28.6 |
| vest, failing edit | 38.9 | 27.8 | 29.5 | 28.8 |
| vest, fix | 43.5 | 27.9 | 29.4 | 28.7 |
| vest, unchanged | 0.23 | 0.23 | 1.04 | 0.59 |
| vest, cold | 77.9 | 79.0 | 80.7 | 80.5 |
| vest, MCP `verify` unchanged | — | — | — | 1.02 |
| demo, edit | 0.85 | 0.85 | 1.19 | 1.08 |
| demo, failing edit | 0.61 | 0.64 | 0.92 | 0.81 |
| demo, fix | 0.84 | 0.84 | 1.11 | 1.02 |
| demo, unchanged | 0.12 | 0.12 | 0.35 | 0.22 |
| demo, cold | 35.0 | 36.0 | 36.5 | 36.6 |
| demo, MCP `verify` unchanged | — | — | — | 0.35 |

Exit codes were identical across modes in every phase. Raw logs and tables
are kept outside the repository in `m114-linux-perf.tgz` (sha256
`2b94e9bf9be67ac3410576a9eacc61b634750b73e25e3b73f43ad90e92b5f8af`).

**What remains** (vest edit 0.8 s over the pass-through, demo 0.23 s;
unchanged re-runs 0.36 s and 0.10 s over Cargo's no-op):

- Verus's `--output-json --time`, which every report and receipt needs:
  0.4–0.5 s per vest edit (Sail's Verus through plain `cargo verus verify`
  on c7i.8xlarge, four interleaved rounds: 35.6 s without, 36.1 s with).
  A cheaper per-function report in the fork is bounded by this and needs
  its own measured fork build.
- Sail's own `cargo metadata` before the run, 0.11 s on vest (it also pays
  Cargo's rustc probes, since the user's `target/` holds no cache), now
  overlapping the snapshot; the targeted clean of an edited package,
  0.05–0.08 s (one Cargo start); the receipt checks, which read vstd's
  artifacts (0.03–0.05 s); the two stamps (0.03 s); the result document
  (0.025 s on vest).
- Not done, because each would weaken or change a check: reusing digests
  across the before and after snapshots by stat (ADR 0041 reads every
  input in full on both sides); skipping the targeted clean when Cargo
  would rebuild anyway (it needs Cargo's freshness rules replicated);
  answering Cargo-Verus's own metadata call from Sail's (Cargo-Verus would
  observe metadata taken earlier).

**Final-code qualification.** Corrected `6563a170` against released rc.5
`eecaf731`, isolated c7i.8xlarge/us-east-1, same fork `73dea021` and pinned Z3,
three alternating, interleaved rounds: unchanged demo 0.388 to 0.207 s,
unchanged vest 1.013 to 0.539 s, demo body edit 1.154 to 0.957 s, vest body
edit 24.078 to 23.817 s. Vest edit ranges overlap, so that edit gain remains
inconclusive. Independent compiler and automatic SMT-daemon caches were warm
for every measured mode; all 24 measured commands exit 0. There is no final-head
cold, plain, failing/fixing, MCP or Mac performance claim. The
[validation record](../validation/managed-overhead.md) records full native gates,
standard conformance, raw evidence hashes and superseded attempts.
