# Live workspace verification

The managed workflow is local and automatic. Install `sail` and `sail-z3`
beside each other, with your pinned Cargo-Verus available as usual. Sail starts
its local daemon when needed. No team server, credentials or container is
required.

## Complete incremental reports

```sh
sail verify --incremental
sail verify --incremental --output json
sail verify --incremental -p my-package --output json
```

Cargo runs on every request. Sail owns a separate Cargo target directory, so it
never cleans the caller's artifacts. Successful complete target reports get
proof receipts bound to the verification context and actual Cargo artifacts.
When Cargo reports an artifact fresh and its receipt matches, Sail returns the
complete establishing report without invoking Verus for that target. A missing
or incompatible receipt requires fresh verification.

Verification arguments, features, toolchain and executable identity participate
in the context. Declared workspace and path-dependency inputs are checked for
changes. A proof from a failed, interrupted or partial verification cannot
become a reusable whole-target receipt. Function/module selection runs fresh;
its report describes that requested scope. Managed mode owns its target and
observation logs, and does not accept an external `--target-dir`, direct Verus
source selection, portfolio or fragility measurement.

The ordinary `sail verify` path keeps its existing drop-in output and behavior.
The explicit managed JSON format is a `sail-project-result` envelope, version 1,
containing per-target schema-v1 reports. Each target identifies its package,
target kind, establishing run, completeness and reuse. Embedded diagnostic,
timing and solver provenance fields describe that establishing run. The outer
execution counters describe the current Cargo request; historical timings are
never counted as fresh verification work.

`currentness` is `current`, `historical` or `unknown`. A changed revision makes
results historical. Incomplete input observation never claims that historical
proof evidence establishes a current workspace. Exit code and proof outcome
remain distinct from whether a job's execution finished.

## Jobs that survive the client session

```sh
sail verify --background
sail verify --background --output json
sail jobs list
sail jobs status JOB
sail jobs logs JOB --follow
sail jobs result JOB --output json
sail jobs cancel JOB
```

Background verification returns immediately with a durable job identity. One
worker runs by default, with identical in-flight requests coalesced. Status,
logs and previous results remain available while another verification runs.
Cancellation terminates the worker's process group, including Cargo-Verus and
its solvers. If the daemon restarts during verification, the unfinished job
becomes an infrastructure error rather than silently continuing.

The local job endpoint has its own protocol version and owner-only socket.
Job records and diagnostics are private files; the last twenty terminal jobs
per workspace are retained. Managed proof receipts are separate from that
history, so pruning old jobs does not discard evidence needed for current
incremental reports.

## Watch and agent tools

```sh
sail watch
sail watch --output jsonl
sail mcp
```

Watch follows the workspace, local path dependencies and configuration inputs.
It debounces edits, cancels superseded work and submits the latest revision.
Its JSON-lines stream carries job lifecycle and target completion/reuse events.
It verifies the requested Cargo scope; it does not guess semantic function
independence.

MCP exposes `start_verification`, `verification_status`, `verification_result`
and `cancel_verification`. Existing blocking verification and result tools
remain available. Status/result inspection does not wait behind a verification
worker. Results carry currentness, scope and establishing provenance so agents
can distinguish finished work from current proof evidence.
