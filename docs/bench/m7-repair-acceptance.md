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

- its phases (arguments, exit code, wall time, timeout flag, infrastructure
  error, GNU `time` RSS of the command, M7 job id and proof-journal pages);
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
- the canonical full outcome: exit code, sorted function verdicts and tiers,
  summary, and ordered diagnostics;
- cache counters from the daemon's `STATS`: replies served, and replies solved
  and recorded;
- M7 preparation hits and misses, from results recorded before the engine stopped preparing lowering;
- peak RSS of the command's and the owned daemon's process trees, sampled
  every 100 ms from `/proc`.

Raw stdout, stderr, `time` output and journal pages are kept under `--out`.
Two files are rewritten atomically after every sample:

- the raw sidecar (`--raw`, default `OUT/NAME.raw.json`, outside the
  repository), holding each phase's arguments, full error text and the
  canonical full outcome;
- the compact result `bench/results/NAME.json`, holding only what the gate
  and report read. Arms are compared by the outcome's SHA-256. Free text is
  truncated, and the raw file's name and SHA-256 are recorded.

A timeout, crash, refusal or unparseable output is recorded on the sample and
the run continues. At the deadline (`--timeout-secs`, default 900), the
harness interrupts the command's process group, cancels the exact M7 job,
then kills the group. `--resume` continues an interrupted result file with the
same binaries and filter from the compact result, appending to its raw
sidecar: it re-runs warm-ups and skips recorded samples.

## Gate (applied at report time)

Per catalogue, a measured sample fails with each violation it has:

| Code | Violation |
|---|---|
| `SAIL7101` | An invocation timed out, could not start, has no exit code or printed no JSON document |
| `SAIL7102` | The full evidence is not Current |
| `SAIL7103` | M7 used other than exactly one compiler driver |
| `SAIL7104` | M7 is not a real source-engine batch: not batched, no engine manifest, no focused evidence, or a selection error |
| `SAIL7105` | Retired. It required M7 preparation telemetry; the engine no longer prepares lowering (rescue L7a) |
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
counts and Current/accepted status, cache served/solved counts, preparation
hits/misses and peak RSS.

## Running

Reference numbers come from native x86-64 Linux: an EC2 host from
`scripts/perf-host.sh`, running the CI image. Build M6 `sail` and `sail-z3`
from `rescue/m6-baseline` (`26f124f`) in a separate source tree, for example
from `git archive 26f124f`. Then run:

```sh
sail-bench repair prepare --scratch SCRATCH
sail-bench repair run --m6 M6/sail --m6-commit 26f124f52dbf51d6c94178778873e96ac5e072ef \
  --m7 target/release/sail --m7-commit SHA --engine /opt/sail-engine \
  --scratch SCRATCH --out OUT --environment "native x86-64 Linux, EC2 c7i.4xlarge" \
  --name repair-acceptance-LABEL [--raw ~/sail-raw-evidence/LABEL.raw.json] \
  [--rounds 3] [--project ID]... [--filter CYCLE] [--resume]
sail-bench report --out docs/benchmarks.md bench/results/*.json
```

The run is local-only and refuses `CI=true`. `docs/benchmarks.md` is generated,
and the workspace gate checks that it is up to date.

## Other M7 gates

Read latency during active verification is measured separately by
`scripts/m7-job-read-bench.py` (four clients, 200 reads, five-second deadline,
p95 below 200 ms, every read bound to a live selected `rust_verify`). Unknown,
Historical, unavailable and incomplete samples cannot satisfy it.

Correctness includes comparing cached and fresh lowering from the same compiler
input: the actual ordered solver writes, framing and follow-ups. Missing or
incomplete captures cannot pass. Stock verdict and diagnostic comparisons, and
the local fast and standard corpus tiers, remain mandatory.

## Real retained-job CLI/MCP integration

`scripts/m7-retained-jobs-acceptance.py` consumes a completed genuine repair job
from the separate human-eval or Vest workload. It requires accepted Current
full evidence for exactly one requested package/target/kind and an exact unique
function. A bounded Cargo metadata query resolves the exact expected package ID;
job/full revisions must agree and all dependency reports must be complete and
error-free. It neither establishes that proof nor edits catalogue sources.
Pass `--sail`, `--project`, `--cache`, `--socket`, `--job`, `--package`, `--target`,
`--kind`, `--function`, a fresh `--out` directory and `--environment-label`.

The local Linux scenario checks the actual CLI and stdio MCP result, exact
explanation, immutable named baseline and comparison, explicit missing-job and
missing-function errors, and a proof cursor resumed across independent MCP
connections. A cursor from another stream must return the existing explicit
`SAIL0902` error through its CLI/MCP projection; an actual journal gap remains
incomplete evidence. Replaying a completed cursor must preserve its exact
events. The [original conflicting assertion and corrected real run](../validation/m7.md)
are retained separately. The scenario then submits one explicitly forced owned
repair, observes its actual worker and descendants through kernel identities,
cancels that job and requires those processes to stop. The externally supplied
completed job and its original evidence must survive unchanged. Cleanup cancels
only the new owned job and closes only this scenario's MCP sessions.

The scenario keeps the 120-second workflow, 30-second CLI command and five-second
RPC limits. Each MCP response, including decoding, uses one absolute five-second
deadline; a partial frame cannot restart it. A collector audit corrected MCP's
previous use of the 30-second command allowance before real-project integration
was qualified. Admission and deadline regressions exercise the actual collector;
they do not manufacture proof evidence. All public responses, diagnostics, operation durations and
cleanup outcomes are retained; private environments are not copied. Passing
this integration does not replace the three-round performance workload, source
oracle, full corpus conformance, or concurrent-read latency census.

## Reproducible warm-reader preparation

`scripts/m7-reader-acceptance.py` prepares the reader workload from an explicitly
supplied source-only Cargo workspace. Supply the immutable product and collector
directories, their `freeze.json` files, the qualified source runtime, and the
exact package, target kind and function:

```sh
python3 scripts/m7-reader-acceptance.py \
  --bin-dir acceptance/product --source-runtime acceptance/qualified-engine \
  --source-workspace acceptance/pinned-source-only \
  --python-probe acceptance/probes/python-reader-probe.py \
  --probe-bin-dir acceptance/probes --out acceptance/reader-evidence \
  --package human-eval-verus --target human_eval_verus --kind lib \
  --function human_eval_verus::human_eval_013::compute_gcd \
  --environment-label linux-x86_64-local --setup-timeout 0 --keep-daemon
```

The workspace must already have the corpus's supported Cargo manifest and pinned
dependency paths. The controller makes fresh baseline and active source copies,
excluding actual Cargo output; it never copies target artifacts or receipts. In
each copy it first runs ordinary independent full verification, then establishes
the exact interactive repair-function context. It records every preparation
invocation, result, elapsed time and failure. Only the catalogue's explicit
first-run input-declaration transition permits one establishment rerun. Other
Unknown, Historical, incomplete, wrong-scope or failed original reports reject
admission. When the exact owned worker output records an independently complete,
successful Current full proof but the public daemon projection is temporarily
Unknown, preparation waits on that same completed job. It records every public
status and result read and requires eventual public Current; it neither rewrites
Unknown evidence nor starts another proof for projection establishment.

`--setup-timeout 0` explicitly permits this unmeasured dependency and context
preparation to finish, with progress output and owned-process cancellation.
The default preparation budget is 600 seconds per job, covering submission,
terminal completion and any projection establishment together. Preparation is
separate from read-latency acceptance: both workspaces must have independently
complete, successful Current full reports before the measured actor starts.
The original Python and Rust collectors each retain four clients, 200 samples,
five-second RPC deadlines, exact selected-crate ancestry and p95 below 200 ms.
Their own process bound remains 600 seconds. A passing collector cannot replace
missing or stale full evidence, and the earlier cold 600-second failure remains
in the acceptance record.

For a new acceptance attempt requiring cold dependency/context preparation,
use `--prepare-contexts --setup-timeout 600`. It records a distinct unmeasured
preparation phase in those same owned workspaces, then restores the original
600-second ordinary and exact-context checks before either frozen collector.
Ordinary incremental checks may use receipts legitimately established in that
workspace; the repair submission still completes its independent full proof.
Retain every preparation job and its total cost. This option cannot change the
collector bounds or turn the entire acceptance into a zero-timeout run. See
the [reader guide](../reader-acceptance.md).

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
