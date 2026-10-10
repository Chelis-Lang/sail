# M7 repair acceptance

The workload is frozen in `corpus/repair-edits.json` before optimization.
Its SHA-256 is
`6ffc129735220bf53511ac6bba818c5c3c7b61bca92d59c17292a807979b18d6`.
The M6 comparison commit is `0fde777fc79ccf07e1318ce21dc0ff84eea1b2ca`.

Both pinned real projects contain five cycles: a body edit, an implied
specification extension, a proof or implementation refactor, a failing
specification attempt and its repair. Each cycle's source text is recorded
exactly. Independent edits start from the original project; repair starts from
the failing cycle. Edits apply to scratch copies and must match exactly once.
The fetched corpus is never modified. An unexpected outcome is a failed
acceptance case, not grounds to silently substitute an easier function.

human-eval-verus has no Cargo manifest. The workload wraps the unchanged
`tasks/lib.rs` and all its modules in a single library package named
`human_eval_verus`, edition 2021, with `package.metadata.verus.verify = true`
and `vstd` from the pinned Verus source. The same wrapper and dependency
resolution are used for M6 and M7. Vest retains its workspace and `vest_lib`
library, with Verus dependencies patched to the same pinned source as the
standard-tier harness.

Each round alternates M6 and M7 ordering over the same cycle and path. M6
performs a managed focused invocation and an independent full invocation.
M7 runs one explicitly selected repair batch. Both use warm SMT caches and
the same selected package/library, full scope, dependencies and solver flags.
Warm-up and installation/build time are separate from measured cycles.

M6's managed command places its injected `--target-dir` after the user's
`--lib`; pinned cargo-verus rejects that argument ordering. The frozen M6
comparison uses explicit package selection after asserting Cargo metadata
contains exactly one library and no other target. M7 uses `--lib` explicitly.
This preserves the same actual library scope without modifying the M6 binary.
The M6 focus passes the source module and function separately, as required by
the pinned Verus filter; the public M7 selector remains crate-qualified.

Retain raw phase reports and diagnostics, process identity, source/catalogue
hashes, wall time, first actionable evidence, peak RSS, cache counters and
preparation statistics. Full M7 outcomes must equal full M6 outcomes. A
successful focus followed by a failing full result is a rejected repair.
First actionable evidence is a failure/progress event, not acceptance.

Report three rounds and their spread. The goal is at least 2× median whole-loop
improvement on each catalogue. Report established status/result latency with
active verification and concurrent readers against the 200 ms p95 goal.
Do not change client deadlines or count unavailable/unknown evidence as a
confirmed pass. On 2026-10-10 the owner directed running these workloads in
the local Linux x86-64 container now, labelling emulation, and rerunning native
host reference numbers later.

The local-only runner is `scripts/m7-repair-bench.py`. Its `prepare` command
creates fresh pinned scratch projects; `run` takes the frozen M6 binary, the
M7 binary, an `--engine-install` validated source variant, and an explicit
`--environment` label. Each mode has a private cache. M7 imports the same
validated runtime into its private cache rather than rebuilding it per cycle.
Installation and dependency warm-up are retained but excluded from cycle time.
JSON checkpoints are flushed to a temporary file, atomically replaced and
directory-synced; failures before replacement preserve the previous snapshot.
Filesystem interruption still makes a run incomplete. Recovered artifacts
must parse and match their recorded identities before they support a claim.
Each distinct repair function also gets an original-source context warm-up:
function selection is part of context identity, so warming stock verification
does not establish that repair context's build-script declarations. Exactly one
retry is permitted when a successful, complete first result explicitly reports
newly established declarations. The next result must be Current and accepted;
missing freshness coverage is never counted as initialization success.
M6's distinct focused contexts are warmed on the same original source as well;
both modes enter the measured edits with their dependencies and contexts set up.
Measured M7 batches must use one compiler driver, retain Current full evidence
and supply source preparation observations within the accepted budget. Missing
telemetry is unavailable, not a collection of invented zero counters.

The runner starts and owns one daemon per mode and reuses it across projects.
It refuses an existing socket rather than adopting someone else's service.
`actor-lifecycle.json` records owned process IDs, binary hashes and shutdown
outcomes; separate daemon streams are retained. A workload timeout, observation
error or interruption first sends SIGINT to the owned foreground CLI so its
handler can cancel its daemon job, then uses a bounded group kill if necessary.
If a repair job ID was observed, cleanup also cancels that exact ID through the
unchanged job protocol. Finally, owned daemons receive graceful shutdown and a
bounded fallback kill. Cleanup never targets discovered or unrelated actors,
and never replaces the original workload failure. Process ownership tests are
in `scripts/test-m7-repair-bench-lifecycle.py`.

First actionable M7 evidence is read through the public resumable proof journal,
using the foreground command's observed job identity. Retained pages include
their cursors and gaps. A started query alone is not counted as an actionable
result. Reports include nearest-rank p95 and maximum cycle wall time, actionable
evidence latency and GNU-time peak RSS alongside medians and round spreads.

Run with scratch projects, runtime, caches and output on the container's native
Linux filesystem (for example under `/tmp`), then copy the retained output to
the repository. A Docker Desktop host bind mount cannot establish the required
kernel freshness coverage. This affects evidence classification, not the
verification verdict or the permission to run the workload under emulation.

`scripts/m7-job-read-bench.py --socket SOCKET --job COMPLETED_JOB
--active-job RUNNING_JOB --active-worker LINUX_PID --active-crate human_eval_verus
--environment LABEL --out RESULT.json` exercises the
actual framed job protocol with four concurrent clients and 200 reads. It
includes transfer and JSON decoding time and keeps the five-second deadline.
It waits for initial Current establishment outside the read census and records
that duration. Every measured read must retain the completed revision, and the
other job must remain Running before and after every read. Unknown, Historical,
unavailable and incomplete samples cannot satisfy the latency goal.
The supplied worker PID must be the active job's `_repair-worker`. Every sample
also records a live `rust_verify` descendant before and after the timed RPC,
bound to the worker's kernel start time and the exact selected Cargo crate;
metadata setup, ordinary builtin/macros dependency compilation, stopped and zombie processes
cannot qualify as active verification. Process inspection is outside the RPC
timing, and the result includes these observations for review.
The same compiler PID and kernel start time must occur on both sides of each
RPC; distinct callbacks around an otherwise idle interval do not qualify.
Every parent in the sampled lineage must predate its child, rejecting recycled
ancestor PIDs. State and CPU accounting may change during legitimate work.
The process census retains only the crate selector from a bounded command-line
read, with kernel identity checked on both sides. It does not retain arbitrary
compiler arguments or process environments. A process named `rust_verify` alone
is insufficient: Cargo-Verus also invokes that executable for non-verifying
dependencies. Both the Python and typed Rust probes require `--active-crate`.

Correctness includes cached-versus-fresh lowering from the same compiler input,
comparing actual ordered solver writes, framing and follow-ups with independent
owners. Stock verdict and diagnostic comparisons remain mandatory; independent
stock transcript instability is recorded separately. Missing or incomplete
captures cannot pass. This is the owner-approved ADR 0027 amendment, and the
original failed independent-stock byte comparison remains retained. Full local
fast/standard corpus coverage and affected source tests are also required.
Long runs stay outside CI.

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
connections. A mismatched cursor must report a gap; replaying a completed cursor
must preserve its exact events. It then submits one explicitly forced owned
repair, observes its actual worker and descendants through kernel identities,
cancels that job and requires those processes to stop. The externally supplied
completed job and its original evidence must survive unchanged. Cleanup cancels
only the new owned job and closes only this scenario's MCP sessions.

The scenario keeps the 120-second workflow, 30-second command and five-second
framed RPC limits. All public responses, diagnostics, operation durations and
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

On failure the controller retains public job output and cancels only jobs and
the private service it created. On success `--keep-daemon` leaves that exact
service available for the retained CLI/MCP scenario above, with its actual PID,
baseline job ID and input revision in `summary.json`. These are setup and test
tools, not new proof modes or evidence that the performance gates have passed.

## Retained acceptance progress (2026-10-10)

M7 remains in progress. Passing integration checks do not substitute for the
real-project performance and exact-input gates.

| Check | Observed result | Evidence |
|---|---|---|
| Native Linux CI repair CLI/MCP integration | Passed 215 operations with unchanged deadlines, including cancellation and baseline comparison; uses the small no-vstd fixture | `bench/validation/m7-repair-cli-mcp-native-linux-2026-10-10.json` |
| Final d19/b346 local CLI/MCP integration | Failed unchanged 120-second bound on the first cold no-vstd job; Cargo printed Finished in 82 seconds, no managed result before cleanup; final phase unresolved | `docs/validation/m7-d19-b346-repair-cli-mcp-local-failure.json` |
| Current 2cce/b346 native Linux CI repair CLI/MCP integration | Passed the actual small no-vstd scenario in 46.790 seconds with unchanged bounds: Current one-driver batch, provisional events, missing-selector full preservation, rejected regressions/late EXEC, raw model and CLI/MCP baseline tools; 169 CLI operations | `docs/validation/m7-2cce-b346-repair-cli-mcp-native-success.json` |
| Corrected 1dc/b346 local CLI/MCP integration | Failed unchanged 120-second bound on first cold no-vstd job; a live `rust_verify` descendant remained at 107 seconds, no full result before cleanup; callback phase unresolved | `docs/validation/m7-1dc-b346-repair-cli-mcp-local-failure.json` |
| Corrected source variant, host differential tests | Eleven exact fixtures, eight prepared revisions, cancellation/restart, three provenance regressions, three driver-termination regressions and the preserved worker-TLS reproduction pass | `engine/verus/qualification-host.json` |
| Corrected source variant, Linux qualification | Eleven exact fixtures, eight prepared revisions, cancellation/restart, driver and provenance regressions, and preparation encoding pass; vstd verifies 2,060 obligations | `engine/verus/qualification-linux.json` |
| Corrected source variant, real human-eval | Two full callbacks each verify 441 obligations without the worker TLS panic; exact differential and selected persistent-restore assertions fail | `engine/verus/qualifications/cd40e16e-real-he-failure.json` |
| Established concurrent reads, prior inventory variant | Failed: 82/200 Current reads, status p95 372.334 ms and result p95 454.569 ms; all samples have an actual live verifier | `docs/validation/m7-f111-reader-failure.json` |
| Frozen whole repair loops | No qualified measured samples yet; the earlier run failed during context warm-up | Whole-loop acceptance remains outstanding |
| Full default fast tier, earlier frozen variant | Pass-through and fill each match 147/147; warm has 146 Same and one vstd infrastructure timeout | Original timeout remains a failure; the later traced diagnostic does not replace it |
| Frozen 1dc default fast tier, later complete run | All 147 units / 3,450 functions match in pass-through, fill and warm; standard phase lost its container before completion | [Immutable completed and interrupted phases](../../bench/validation/m7-default-corpus-1dc-interrupted-emulated-2026-10-10.json) |
| Integrated 62ad Linux workspace gate | Full gate passed, including 436/436 nextest tests with two case workers; internal concurrency fixtures unchanged | [Gate record](../../bench/validation/m7-linux-gate-62ad-emulated-2026-10-10.json) |
| Integrated 62ad real solver suite | 36/37 passed; shared-entry forgery test timed out at its original 180-second limit; diagnosis pending | [Failure record](../../bench/validation/m7-linux-solver-62ad-failure-emulated-2026-10-10.json) |
| Integrated d12 workspace gate | 431/436 passed; five HTTP-cache concurrency tests failed, with original limits and assertions retained | [Failure record](../../bench/validation/m7-linux-gate-d12-failure-emulated-2026-10-10.json) |
| Integrated a9 workspace gate | 435/436 passed after separate actual transport/trust establishment; one verified paused-initiator check still fails under its original serving assertions | [Failure record](../../bench/validation/m7-linux-gate-a9-failure-emulated-2026-10-10.json) |
| Integrated 5fd workspace gate | All 439 workspace tests and every gate step passed; caller-entry coalescing and strict development arithmetic optimization included, original HTTP assertions/deadlines unchanged | [Gate record](../../bench/validation/m7-linux-gate-5fd-emulated-2026-10-10.json) |
| Integrated d12 real solver suite | 37/37 passed in 114.446 seconds, including the previously timed-out case in 9.652 seconds; original timeout cause remains unresolved | [Record](../../bench/validation/m7-linux-solver-d12-emulated-2026-10-10.json) |
| Candidate 2459 ordinary source build | Passed build and source controls; independent-stock human-eval byte qualification failed and candidate remains unselected | [Build](../../engine/verus/candidates/qualifications/2459-linux-build.json), [failed qualification](../../engine/verus/candidates/qualifications/2459-linux.json) |
| Candidate 2459 other-module edit | Stock and engine outcomes and 114 transcripts match, but zero demonstrated reuse across edits; one full hit reused this invocation's focus preparation | [Control](../../engine/verus/candidates/qualifications/2459-other-module-edit.json) |
| Candidate 2421 source build and hit controls | Ordinary build/vstd passed; corrected observable declaration-order, body and effects controls all exercised actual restored hits; complete source/product qualification remains pending | [Retained first failure](../../engine/verus/candidates/qualifications/2421-linux-first-attempt.json), [completed controls](../../engine/verus/candidates/qualifications/2421-linux-hit-controls.json) |
| Candidate 2421 complete source attempt | All 106 restored/fresh bucket comparisons complete, but authoritative unchanged-revision full validation times out at 600 seconds without a report or normal seal; later gates not reached | [Failure](../../engine/verus/candidates/qualifications/2421-linux-resumed-failure.json) |
| a6/b346 real reader diagnostic | Genuine public Current baseline and independent active preparation passed; Python census failed: 125/200 complete admission checks, result p95 402.821 ms; all 200 observations retained an actual selected verifier. Rust and retained CLI/MCP not attempted | [Complete failure record](../validation/m7-a6-b346-reader-census-failure.json) |
| Separate retained-job contention profile | No active verifier or new proof; 187/200 Current, 13 Unknown with recorded validation-owner waits. Full traced timings fail acceptance, original job bytes unchanged; original live failure cause remains unknown | [Diagnostic](../validation/m7-retained-proof-freshness-profile.json) |
| Caller-entry coalescing diagnostic | No active verifier or new proof; 200/200 Current with unchanged guards. Traced status p95 209.120 ms and result p95 407.420 ms still fail the 200 ms target; original retained job files unchanged | [Diagnostic](../validation/m7-reader-entry-coalescing-diagnostic.json) |
| Strict shared-signature CPU controls | Fixed 16-check valid/forged controls show development arithmetic cost falling from approximately 310/320 ms to 4–7 ms. Original HTTP deadlines, strict forgery refusal and all binding checks remain; subsequent integrated 5fd gate passed | [Diagnostic](../validation/m7-shared-signature-cpu-diagnostic.json) |

The human-eval input difference is confined to the already recorded stock
declaration-order variation in `human_eval_160`. All other 113 full SMT inputs,
diagnostics, counters and function details match. The second fresh callback
restores only one preparation and misses 105; this does not establish useful
reuse across edits. Both failures remain visible, without normalization or an
acceptance waiver.

Input digest retention under continuous kernel guards and the worker's guarded
sysroot baseline have additional invalidation tests. Their real-project gains
still need measuring. The additive Rust probe described in `docs/freshness.md`
uses the actual product client; the original Python census and its failures
remain independent evidence.

The current source variant explicitly marks normal driver termination. Internal
compiler panics escape without a terminal seal; ordinary failed proofs and
frontend or late EXEC errors retain their exact full result. The adapter also
requires a complete structured result and normal exit before retaining focused
evidence. An interrupted or malformed engine run keeps its failure streams and
restarts the exact stock invocation, which is counted as a second compiler
invocation. The new Linux runtime has independent source qualification. Neither
source qualification nor the host results qualify the whole-loop performance gates.
