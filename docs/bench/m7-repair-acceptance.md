# M7 repair acceptance

ADR 0027 requires M7 to beat M6 by at least 2× on the median whole repair loop
of each frozen catalogue. The workload is `sail-bench repair`. It records every
sample first. The gate is applied afterwards, when the report is generated.

## Workload

- **Catalogue.** `corpus/repair-edits.json` is frozen. Its SHA-256 is
  `6ffc129735220bf53511ac6bba818c5c3c7b61bca92d59c17292a807979b18d6`, and
  `sail-bench` refuses any other bytes. It holds two catalogues, human-eval-verus
  and Vest. Each catalogue has five cycles: a body edit, a specification
  extension, a refactor, a failing attempt and its repair. Each edit must match
  exactly once. Independent edits start from the original file. The repair
  cycle starts from the failing attempt.
- **Projects.** `sail-bench repair prepare` clones committed objects of the
  pinned corpus checkouts into fresh scratch directories. The fetched corpus
  is never modified.
  - human-eval-verus is wrapped as the library package `human_eval_verus`
    (edition 2021, `vstd` from the pinned Verus source).
  - Vest keeps its workspace. Its single `vstd` requirement is pinned and
    patched to the same source.
  - Preparation asserts that the package has one library target and no binary
    target. Cargo's default package selection is the library plus binaries, so
    M6's `--package` selects the same scope as M7's `--lib`. Vest's `vest_lib`
    also has a `test` target, which neither selection builds.
- **Arms.**
  - *M6* is built from the branch `rescue/m6-baseline`, commit
    `26f124f52dbf51d6c94178778873e96ac5e072ef`. That is the frozen M6 commit
    `0fde777fc79ccf07e1318ce21dc0ff84eea1b2ca` (the catalogue's
    `baseline_commit`) plus one backported fix and its test. Unpatched M6
    cleans `<target-dir>` before a focused check, but `cargo verus focus` builds
    in `<target-dir>/verus-partial` with checksum freshness. A cycle whose edit
    repeats content already compiled for that focus context therefore ran no
    verifier and failed with `SAIL3107` in about a second. That failure is not
    an M6 result. The M6 arm runs
    `verify --incremental --output json --package P -- --verify-only-module M
    --verify-function F` (the focused check), then `verify --incremental --output
    json --package P` (the full check).
  - *M7* runs a single `repair-check --engine interactive --function F --output
    json --package P --lib`.
  - Each arm has its own cache directory and an owned `sail serve`, kept across
    the run, so its SMT cache stays warm. `SAIL_MANAGED_FORCE=1` stops proof
    receipts from being reused, and inherited `SAIL_*` selectors are removed.
    The validated engine runtime is installed into the M7 cache with
    `sail engine install`.
- **Warm-ups** run before the measured rounds and are recorded but never judged:
  - the engine install;
  - one full `verify --incremental` per arm;
  - on the original source, one M6 focused check per distinct function;
  - for M7, up to two repair checks per function. A first repair run may
    establish build-script declarations and end non-Current.
- **Rounds.** In round *r*, each cycle's edited text is written, then both arms
  run. M6 goes first when *r* is even and M7 goes first when *r* is odd.

## Recorded per sample

Each sample records one arm of one cycle in one round. It holds:

- its phases (exit code, wall time, timeout flag, infrastructure error, M7
  job id and proof-journal pages);
- the sample's total wall time;
- *first actionable evidence*. For M6 this is the focused check's wall time.
  For M7 it is the time of the first focused proof-journal event that is a
  diagnostic, a function or obligation result, or the end of the focus phase.
  A started query does not count;
- full and focused Currentness;
- M7 `accepted`, `batched`, engine-manifest presence, focus presence and
  selection error;
- compiler driver count (M7 `batch.compiler_invocations`; M6 the sum of
  `execution.verifier_invocations`);
- the canonical full outcome: exit code, sorted function verdicts and
  establishments (a v2 report's `establishment`, the frozen M6 baseline's v1
  `tier`), summary, and ordered diagnostics;
- cache counters from the daemon's `STATS`: replies served, and replies solved
  and recorded.

Each invocation's command, stdout, stderr and journal pages are kept under
`--out`, outside the repository. The result `bench/results/NAME.json` is
rewritten atomically after every sample and holds only what the gate and
report read. Arms are compared by the outcome's SHA-256, and free text is
truncated.

A timeout, crash, refusal or unparseable output is recorded on the sample and
the run continues. At the deadline (`--timeout-secs`, default 900), the
harness interrupts the command's process group, cancels the exact M7 job,
then kills the group. `--resume` continues an interrupted result file with the
same binaries and filter: it re-runs warm-ups and skips recorded samples.

## Gate (applied at report time)

Per catalogue, a measured sample fails with each violation it has:

| Code | Violation |
|---|---|
| `SAIL7101` | An invocation timed out, could not start, has no exit code or printed no JSON document |
| `SAIL7102` | The full evidence is not Current |
| `SAIL7103` | M7 used other than exactly one compiler driver |
| `SAIL7104` | M7 is not a real source-engine batch: not batched, no engine manifest, no focused evidence, or a selection error |
| `SAIL7106` | The outcome differs from the catalogue (M7 `accepted`, M6 full exit), or no comparable full outcome exists |
| `SAIL7110` | An M6 focused or full check reported a Sail error (for example `SAIL3107`) instead of a verification result |

An M6 sample with `SAIL7110` or `SAIL7101` is an invalid baseline sample: it
measured no M6 loop. Its time is never counted as M6 time, so its round is
incomplete. Counting it could only make M7 look faster. A failing M7 sample
keeps its time.

The catalogue passes only if all of the following hold. Otherwise it fails:

- every cycle has an M6 and an M7 sample in at least three rounds
  (`SAIL7108`);
- no measured sample has a violation;
- every pair's full outcomes are identical (`SAIL7107`);
- the speedup is at least 2× (`SAIL7109`).

The **whole-loop total** of an arm in a round is the sum of its cycles' sample
wall times. The **speedup** is the median over complete rounds of the M6
whole-loop total, divided by the same median for M7.

Failing samples stay in the totals and are listed, except invalid M6 baseline
samples, which are listed but excluded from M6's totals as described above.
No failing sample is dropped from the list or counts as a pass. A subset run (`--project`, `--filter`) is incomplete by
construction. The report also shows round spread (min–max), per-cycle medians,
nearest-rank p95 and maximum cycle wall time, first-actionable p95, M7 driver
counts and Current/accepted status, and cache served/solved counts.

## Running

Reference numbers come from native x86-64 Linux: an EC2 host from
`scripts/perf-host.sh`, running the CI image. Build M6 `sail` and `sail-z3`
from `rescue/m6-baseline` (`26f124f`) in a separate source tree, for example
from `git archive 26f124f`. Then run:

```sh
sail-bench repair prepare --scratch SCRATCH
sail-bench repair run --m6 M6/sail --m6-commit 26f124f52dbf51d6c94178778873e96ac5e072ef \
  --m7 "$PWD/target/release/sail" --m7-commit SHA --engine /opt/sail-engine \
  --scratch SCRATCH --out OUT --environment "native x86-64 Linux, EC2 c7i.4xlarge" \
  --name repair-acceptance-LABEL [--rounds 3] [--project ID]... [--filter CYCLE] [--resume]
sail-bench report --out docs/benchmarks.md bench/results/*.json
```

`--m6` and `--m7` must be absolute paths. Each arm runs from the project
directory, so a relative binary fails to start and every sample of that arm
records `SAIL7101`. The run is local-only and refuses `CI=true`. `docs/benchmarks.md` is generated,
and the workspace gate checks that it is up to date.

## Other M7 gates

Read latency during active verification is measured separately by
`scripts/reader-gate.sh` (four clients, 200 reads, p95 below 200 ms, every
sample Current); see [M7 validation, gate 5](../validation/m7.md#gate-5-responsive-reads).

Correctness includes comparing cached and fresh lowering from the same compiler
input: the actual ordered solver writes, framing and follow-ups. Missing or
incomplete captures cannot pass. Stock verdict and diagnostic comparisons, and
the local fast and standard corpus tiers, remain mandatory.

`ci/local.sh e2e` retains managed-phase evidence under
`target/managed-acceptance` and repair evidence under `target/repair-acceptance`;
set `SAIL_MANAGED_ACCEPTANCE_OUT` and `SAIL_REPAIR_ACCEPTANCE_OUT` to fresh
directories for separately archived attempts. All three phases remain required.
An isolated phase is useful for diagnosis and cannot replace a failed aggregate.

On failure the controller retains public job output and cancels only jobs and
the private service it created. On success `--keep-daemon` leaves that exact
service available for the retained CLI/MCP scenario above, with its actual PID,
baseline job ID and input revision in `summary.json`. These are setup and test
tools, not new proof modes or evidence that the performance gates have passed.

## Acceptance status

[M7 validation](../validation/m7.md) records each gate's current status, best
evidence and indexed failed attempts. Passing integration checks do not
substitute for the real-project performance and exact-input gates.

## Engine termination and fallback

The current source variant explicitly marks normal driver termination. Internal
compiler panics escape without a terminal seal; ordinary failed proofs and
frontend or late EXEC errors retain their exact full result. The adapter also
requires a complete structured result and normal exit before retaining focused
evidence. An interrupted or malformed engine run keeps its failure streams and
restarts the exact stock invocation, which is counted as a second compiler
invocation. The new Linux runtime has independent source qualification. Neither
source qualification nor the host results qualify the whole-loop performance gates.
