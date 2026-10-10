# Real warm-reader acceptance

`scripts/m7-reader-acceptance.py` runs the local reader workload against frozen
Sail binaries, a compatible installed source engine, and a prepared source-only
Cargo workspace. It prepares independent baseline and active copies without
copying prior receipts or Cargo output. Preparation verifies the full project
ordinarily, then establishes the exact repair-function context. Preparation
costs and failed attempts are retained separately from measured reader latency.

Pass the frozen product and probe directories, qualified source-runtime root,
prepared source workspace, exact package/target/kind/function, Python probe,
output directory and environment label. `--help` lists the required arguments.
The source-runtime manifest must be accepted by that exact Sail binary; an
incompatible engine is a setup failure. This workload is local-only.

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
have their original 600-second bounds. Ordinary `verify --incremental` may reuse
a legitimately established receipt; it does not claim a new ordinary compiler
invocation. The repair submission and public complete Current admission remain
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
