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
changes, including resolved registry/git dependencies and compiler-declared
external includes. Newly discovered inputs prevent that establishing run from
claiming currentness until they can be observed. Failed, interrupted or partial
target proofs cannot become reusable whole-target receipts. Publication waits
for a coherent input check and an atomic run commit, so an interrupted publisher
cannot leave usable unvalidated receipts. Function/module selection runs fresh;
its report describes that requested scope. Managed mode owns its target and
observation logs, and does not accept an external `--target-dir`, direct Verus
source selection, tests/benches/examples, a chained workspace rustc wrapper,
portfolio, fragility measurement, custom sysroots or JSON target specifications.
Ordinary Cargo library and binary targets are supported. Cargo `--config`,
configuration includes, and compiler, wrapper, loader or sysroot overrides that
cannot be bound safely are rejected with `SAIL3112`. Use ordinary `sail verify`
for those unsupported managed configurations. Cargo configuration discovery
uses the original invocation directory, including member-directory ancestors;
relative Cargo/cache homes are resolved against that directory.

On the M7 development branch, Linux managed commands also attempt automatic
sysroot-digest reuse through the trusted local daemon. Each command first
installs its own original file and namespace guards. It accepts an available
snapshot only while both owners protect the same complete inventory; executable
identities and all final proof checks remain local. A cold, unavailable, busy or
invalid service falls back to full local hashing. Cold commands can request a
bounded background fill after their own hash barrier; they do not wait for that
fill. No new configuration is required. Original cold/warm integration and
performance validation remain pending; see [the roadmap](ROADMAP.md) and
[the handoff contract](decisions/0031-guarded-sysroot-digest-handoff.md).

Linux is the reference platform for frontend proof reuse. Its ordered native
event queue tracks transient creation and removal of missing Cargo configuration
files before receipts are published. On macOS, present inputs retain content
and mutation checks, and missing configuration paths retain conservative parent
directory checks. Unrelated directory activity can make a completed result
historical and require another proof run. Sail does not relax those checks to
claim current evidence. Ordinary verification and the SMT cache remain available
on both platforms.

The ordinary `sail verify` path keeps its existing drop-in output and behavior.
The explicit managed JSON format is a `sail-project-result` envelope, version 1,
containing per-target schema-v1 reports. Its JSON Schema is
`crates/sail-api/schema/project-result-v1.json`. Each target identifies its package,
target kind, establishing run, completeness and reuse. The `requested` field
distinguishes the selected Cargo scope from dependency reports. Embedded diagnostic,
timing and solver provenance fields describe that establishing run. The outer
execution counters describe the current Cargo request; historical timings are
never counted as fresh verification work.
An explicitly selected source engine adds an optional top-level `engine` object
(`name`, `manifest_sha256`), and each embedded report
records whether the engine or its stock fallback established it (see
[engine provenance](output-schema.md#engine-provenance)). Stock results omit
it.

`currentness` is `current`, `historical` or `unknown`. A changed revision makes
results historical. Currentness is evaluated under the job's original submitted
configuration; changing features or environment requires another request with
its own context. Incomplete input observation never claims that historical
proof evidence establishes a current workspace. Exit code and proof outcome
remain distinct from whether a job's execution finished.

For an Unknown Linux proof, `SAIL3114` coverage notes retain the first failed
barrier (`input-barrier` or `post-verification`). A
`native-observation-invalidated` note also retains the existing namespace detail,
such as `namespace-tracked-input-event` or
`namespace-event-drain-budget-exhausted`. These labels distinguish observed
input events from unavailable coverage; they do not identify a particular file
or explain an event's cause. Sysroot notes likewise distinguish the first failed
`namespace` or `explicit-files` observer and retain its existing detail. Earlier
coarse reports cannot be explained retrospectively by these new labels.
Job reads preserve the original proof notes. A
separate current-input-validation warning appears when a previously Current
report becomes Unknown during projection. See
[the diagnostic boundary](validation/m7.md).

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

Background verification returns immediately with a durable job identity.
Identical in-flight requests coalesce once complete
remembered input coverage establishes the same revision. Cold requests without
that evidence remain distinct, so an unseen edit cannot be swallowed. Status,
logs and previous results remain available while another verification runs.
Cancellation terminates the worker's process group, including Cargo-Verus and
its solvers. If the daemon restarts during verification, the unfinished job
becomes an infrastructure error rather than silently continuing.

### Concurrent jobs

Several agents can share one daemon. Jobs from different Cargo workspaces run
concurrently, up to `SAIL_JOB_WORKERS` at a time. The default is a quarter of
the available cores, between 1 and 4, because each job already runs Verus and
Z3 in parallel; values from 1 to 64 are accepted, and anything else uses the
default. The daemon reads the variable when it starts.

Jobs in the same workspace run one at a time, in submission order: a later job
waits in `queued` until the earlier one finishes, rather than failing on the
workspace's managed target. The daemon identifies a workspace by the outermost
directory above the project (or its `--manifest-path`) that holds a
`Cargo.toml`, so nested packages of one workspace share a queue. Separate
`sail verify --incremental` processes outside the daemon wait on the managed
runner's own lock for up to `SAIL_MANAGED_LOCK_WAIT_SECS` seconds (default
1,800) and then fail with `SAIL3104`.

The local job endpoint has its own protocol version and owner-only socket.
Job records and diagnostics are private files; the last twenty terminal jobs
per workspace are retained. Managed proof receipts are separate from that
history, so pruning old jobs does not discard evidence needed for current
incremental reports.

## Evidence from completed jobs

```sh
sail jobs events JOB [--stream ID] [--after N] [--follow] [--output jsonl|json]
sail explain JOB --package-id ID --target NAME --kind lib|bin --function F [--obligation O] [--output json]
sail baseline save NAME --job JOB
sail baseline list
sail baseline get NAME --output json
sail compare BEFORE_JOB AFTER_JOB [--output json]
sail compare AFTER_JOB --baseline NAME [--output json]
```

These commands read recorded evidence. None of them verifies current inputs or
generates proof edits.

- **Proof events.** `sail jobs events` reads one bounded page of the job's
  version-one proof-event journal, with a resumable cursor (`next.stream_id`
  and `next.after`). A gap is reported explicitly, and `terminal` is set once
  the job has finished. Event observations are provisional; only completed
  target reports carry final evidence. `--follow` streams JSONL pages until
  the job is terminal. **Stock verification writes no proof events today.** A
  finished stock job returns an empty terminal page whose `gap` is
  `SAIL0903: this job produced no interactive proof stream`. The producer
  arrives with the optional interactive engine (ADR 0027).
- **Explanations.** `sail explain` selects one function by exact package ID,
  target name, target kind and crate-qualified function name. It returns that
  function's complete report (obligations, models and provenance), the run's
  diagnostics and the establishing run. A missing or ambiguous identity fails
  with `SAIL0904`. Diagnostics are not assigned to a function by line-window
  heuristics, so `diagnostic_attribution_complete` is `false`. Diagnostics that
  a proof event explicitly links to the function are added to
  `attributed_diagnostics`.
- **Baselines.** `sail baseline save` copies a completed job's project result
  under an immutable name. Names use 1-80 ASCII letters, digits, dots,
  underscores or hyphens, excluding `.` and `..`. An existing name is never
  overwritten (`SAIL0905`). A baseline is always `historical` evidence for its
  recorded input revision. It survives job pruning and daemon restarts.
  Provisional, failed or incomplete evidence is kept as reported. To pass a
  name that looks like an option, put the output options first:
  `sail baseline get --output json -- --json`.
- **Comparisons.** `sail compare` classifies each target/function as
  `unchanged`, `regressed`, `became_unknown`, `recovered`, `added`, `removed`,
  `not_observed`, `incomparable` or `changed`. Transitions are labelled as
  regressions only when the verification context and the requested scopes
  agree. A partial scope reports `not_observed`, never `removed`. Repeated
  friendly function names are `incomparable` (VI-0010).

The job endpoint name includes its proof-contract version
(`jobs-v1-proof-v1-*.sock`), so a client never reaches an older job service
that lacks these operations. A `capabilities` request reports the supported
proof-stream version.

## Watch and agent tools

```sh
sail watch
sail watch --output jsonl
sail mcp
```

Watch follows the workspace, local path dependencies and configuration inputs.
It debounces edits for 300 ms, cancels superseded work and submits the latest
revision. Native events are preferred. If event observation becomes unavailable,
it announces a five-second content-scan fallback using remembered input roots;
unchanged inputs do not trigger another verification. A settled native watcher
does not poll completed jobs or rehash the verifier runtime.
Its JSON-lines stream carries job lifecycle and target completion/reuse events.
It verifies the requested Cargo scope; it does not guess semantic function
independence.

MCP exposes `start_verification`, `verification_status`, `verification_result`
and `cancel_verification`. Existing blocking verification and result tools
remain available. Status/result inspection does not wait behind a verification
worker. Results carry currentness, scope and establishing provenance so agents
can distinguish finished work from current proof evidence.

Existing solver-cache entries remain compatible. The automatic daemon socket
now includes the workspace-service capability (`daemon-v4-workspace-v1.sock`).
An explicitly selected older cache daemon can keep running: Sail starts a
private jobs-only sidecar alongside it instead of terminating it.
