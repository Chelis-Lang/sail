# Real warm-reader acceptance

`scripts/m7-reader-acceptance.py` runs the local reader workload against frozen
Sail binaries, a compatible installed source engine, and a prepared source-only
Cargo workspace. It prepares independent baseline and active copies without
copying prior receipts or Cargo output. Preparation verifies the full project
ordinarily, then establishes the exact repair-function context. Preparation
costs and failed attempts are retained separately from measured reader latency.

The [frozen `b89546c` product](validation/m7-compiler-script-bootstrap/scheduling-recovery.json)
passes both unchanged collectors and retained CLI/MCP scenarios. Each collector
retains 200 total samples, full Current graphs and the same live verifier.
Python Result/Status p95 is 91.227/56.989 ms; Rust is 83.867/44.371 ms, below
the original 200 ms bound. Current `387ba80` separately passes its
[complete gate/solver, product installations and original repair fixture](validation/m7-compiler-script-bootstrap/source-inventory.json).
Its reader execution is running separately; no collector or retained CLI/MCP
acceptance result is available yet. The historical b895 pass grants it no reader credit.

The earlier [frozen `730a321` product](validation/m7-reader-730a321-terminal.md) passes
this complete workflow: both unchanged four-client collectors retain 200 samples,
full Current graphs and p95 below 200 ms during actual verification, followed by
retained CLI/MCP scenarios. Python Result/Status p95 is 94.197/59.896 ms; Rust is
67.483/50.273 ms. Preparation costs and historical failures remain separate;
real-project repair speed and complete standard/cache gates are still outstanding.

[Optional RPC stage diagnostics](result-rpc-diagnostics.md) can attribute server
costs in a separate owned diagnostic scope. They do not change frozen collectors,
admit proofs or replace the original latency acceptance checks.

Pass the frozen product and probe directories, qualified source-runtime root,
prepared source workspace, exact package/target/kind/function, Python probe,
output directory and environment label. `--help` lists the required arguments.
The source-runtime manifest must be accepted by that exact Sail binary; an
incompatible engine is a setup failure. This workload is local-only.

Before setup, the controller records actual ownership/capability prerequisites
for both installed product binaries and refuses foreign-owned products without
effective `CAP_LEASE`. This check does not cover the complete runtime inventory.
Optional `--lease-inventory candidate-guard-bindings.json` audits every binding
in the exact candidate's original complete Guard capture and records its hash.
It cannot prove completeness of an arbitrary supplied list or cover future
created inputs; stale/changed bindings are refused. Production Guard acquisition
remains the authority. Use [native product setup](native-product-setup.md) to
create new byte-identical service-owned copies without changing old seals.
Neither this preflight nor a successful installation admits a proof, changes
preparation deadlines or qualifies a selected runtime.

`--setup-timeout 600` is the default preparation budget per invocation.
`--setup-timeout 0` explicitly permits unmeasured preparation to completion,
with progress reports and cancellation of owned actors. Neither setting changes
the 30-second command and five-second RPC bounds, or the four-reader,
200-sample census with a p95 below 200 milliseconds. A real complete, successful,
public `Current` baseline and a continuously running selected-crate verifier
are prerequisites. Both independent collectors must pass before the controller
reports success. `--keep-daemon` retains only a successful owned service for
subsequent retained-job CLI/MCP inspection.

`--prepare-contexts --setup-timeout 600` explicitly adds a separately recorded,
unmeasured preparation phase before the original bounded setup checks. In each
fresh, owned workspace it completes the full ordinary verification and exact
repair-function context with the same source, arguments, runtime and private
cache. No receipt or Cargo output is copied from another run. Every preparation
command, full proof, job ID and total wall time is retained; progress and owned
cancellation remain active. Preparation is not a latency sample or a performance
claim, and its cost must accompany any resulting acceptance record.

After preparation, the existing ordinary and repair-context setup checks still
have their original 600-second bounds. This frozen controller sets
`SAIL_MANAGED_FORCE=1`, so the ordinary `verify --incremental` check cleans the
selected target and invokes its compiler again. Preparation warms dependencies
and establishes contexts; it does not turn this forced check into receipt-only
reuse. Ordinary and repair contexts keep separate target/receipt directories.
The repair submission and public complete Current admission remain
unchanged. Neither collector, the 600-second collector process bound, five-second
RPC nor p95 threshold changes. The explicit preparation flag requires a positive
subsequent setup budget: it cannot turn these bounded checks into a whole
`--setup-timeout 0` run. The earlier cold setup timeout remains a failure.

A completed job may temporarily have an unavailable freshness projection. The
controller can wait on that same job only when its original owned output was
accepted, independently complete and Current, and its public Unknown result
matches the precise unavailable-projection transformation. Raw output never
admits a proof. Public Current must eventually be observed within the same
preparation budget. Failed, incomplete, changed or generally Unknown original
proofs remain failures; the existing single declaration-establishment transition
is the only permitted proof rerun.

## Retained-job evidence

The controller supports exactly these two private job layouts under its own
cache:

- `projects/jobs/<job-id>` for older daemons;
- `results/projects/jobs/<job-id>` for newer daemons.

Exactly one directory must exist. Missing or duplicate layouts fail explicitly;
the controller does not choose the first match. It verifies the record's job ID,
version and submission scope. Reads used for freshness-establishment waiting
also match the public job's workspace, arguments and input revision, and verify
the exact stdout seal when present. Directory and file descriptors refuse
symlinks, path traversal and non-regular files, including FIFOs. Holding the
opened directory prevents its later replacement from redirecting a read.

`worker-output/<job-id>/` contains available stdout, stderr and event streams,
plus `archive.json` with the selected layout, copied-byte hashes, missing files
and any detected changes during copying. A completed job with missing stdout is
an archival failure. Queued jobs that never started can explicitly record missing
worker files. The private `record.json`, including its environment, is not copied.
Archival errors appear in `summary.json`, prevent a passing acceptance result and
do not bypass owned-job cancellation. An earlier proof or census failure remains
recorded even if archival also fails.

The original frozen controller looked only in the newer layout. An older-daemon
run exposed that lookup mismatch: its job records remained in the legacy path,
while the controller checked the newer path. The cache itself and its
authoritative public proof remained intact. The revised resolver applies only to
future controller freezes. Running controller files must not be edited in place.

## Developer checks

The daemon bounds retained complete-result cache storage while preserving full
responses. Capacity remains charged through in-flight output after eviction;
quota pressure serves an uncached response. This is a cache-retention bound,
not a bound on cold encoding or process RSS. See the
[ownership validation](validation/m7-result-frame-ownership.md). Freshness
validation and the original reader thresholds remain unchanged.

Run `python3 scripts/test-m7-reader-acceptance.py` for the fast harness regressions.
They use actual old/new filesystem layouts and cover ambiguity, missing output,
record/scope/revision/seal mismatches, substituted directories, symlinked files,
FIFOs, duplicate JSON fields and unchanged public-proof admission. These fixtures
are not substitutes for actual CLI/MCP or four-reader acceptance.

See [the proof-repair guide](proof-repair.md) for the public commands exercised
by these workloads.

## Recorded setup failure

The frozen `8e09eaa` reader-stage attempt timed out during ordinary baseline
preparation at its unchanged 600-second bound. Neither reader census ran. See
[the retained setup record](validation/m7-reader-entry-setup-timeout.md); this
failure remains separate from the older live-census latency failures and idle
retained-job diagnostics.

## Recorded single-proof diagnosis

A separately granted [detailed sysroot diagnosis](validation/m7-sysroot-proof-diagnostic.md)
completes one fresh full HumanEval proof within the original 600-second budget,
with public Current and accepted evidence. It uses two actual compiler
invocations, with no preparation or retry; the focus result remains provisional.
It does not reproduce or explain the earlier D20 coverage loss. Neither reader
census nor retained CLI/MCP acceptance runs in that diagnostic. A new original
reader workflow still requires its own genuine Current proofs, unchanged
collectors and complete owned lifecycle.

## Current complete reader attempt

The [earlier frozen `4472279` run](validation/m7-current-reader-workflow.md) completed genuine Current full preparation in independent baseline and active workspaces, then passed all four original bounded setup checks. The first collector completed all 200 RPC samples through four readers while the same selected-crate verifier remained live. Status p95 was 148.430 ms; complete Result p95 was 394.713 ms, exceeding the unchanged 200 ms requirement. Each Result carried a 4,312,245-byte complete report. The controller stopped there, leaving the Rust census and retained CLI/MCP integration unattempted. This is a retained failure, not reader acceptance. Preparation costs, samples, worker continuity and post-run seals accompany the record.

The [current-product observation-only stage run](validation/m7-rpc-stage-workflow.md)
retains another complete Current baseline and active proof, then a failed
original Python Result p95 (403.341 ms; Status 147.290 ms). All 200 admission and
per-call verifier checks passed; Rust and retained integration were unattempted.
Its bounded server tail attributes only 43 completed pairs after cleanup
polling. It is diagnostic evidence, not reader acceptance or source qualification.

The subsequent [native-binding product workflow](validation/m7-rpc-binding-reader-terminal.md)
establishes independent complete Current preparation proofs, then times out in
the original 600-second baseline ordinary-full check. No census or retained
integration is attempted. The successful preparation does not substitute for
that failed prerequisite. Its separately retained process snapshot is partial:
main-task child traversal omitted descendants launched by other Cargo tasks.
The [all-task observer control](validation/m7-reader-process-observation.md)
now demonstrates that coverage on a real worker-thread child and rejects PID
birth changes. It improves future diagnosis, without recapturing old state or
admitting a timed-out proof.

One separately scoped [focused ordinary diagnosis](validation/m7-ordinary-wait-diagnostic-terminal.md)
then completed under the same 600-second bound, FORCE policy and ordinary
context, with a Current full report and unchanged post-run seals. Its successful
cached solver path did not reproduce the historical wait. Partial all-task
snapshots and a capped closure scan remain explicitly incomplete. No reader
census, retained integration or acceptance claim follows from this diagnostic.
