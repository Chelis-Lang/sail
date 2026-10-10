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
--active-job RUNNING_JOB --environment LABEL --out RESULT.json` exercises the
actual framed job protocol with four concurrent clients and 200 reads. It
includes transfer and JSON decoding time and keeps the five-second deadline.
It waits for initial Current establishment outside the read census and records
that duration. Every measured read must retain the completed revision, and the
other job must remain Running before and after every read. Unknown, Historical,
unavailable and incomplete samples cannot satisfy the latency goal.

Correctness includes exact solver-input and full diagnostic comparisons,
full local fast/standard corpus coverage, and affected source tests described
in ADR 0027. Long runs stay outside CI.
