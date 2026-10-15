# Live workspace verification

Every `sail verify` is managed verification
([ADR 0045](decisions/0045-managed-verification-by-default.md)). Install
Sail; it runs its own Verus and starts its local daemon when needed. No team
server, credentials or container is required.

## Complete incremental reports

```sh
sail verify
sail verify --output json
sail verify -p my-package --output json
```

Without `--output json`, `sail verify` prints what `cargo verus verify` prints:
Cargo's status lines and Verus's diagnostics on stderr, Verus's own stdout
(`verification results:: N verified, M errors`, the `--output-json` document,
or the `--time` report, as you asked Verus for them), and the same exit code.
See [what the output does not repeat](#output-compared-with-cargo-verus-verify)
for the differences that remain.

`--incremental` was the opt-in for this mode before it became the default.
It is still accepted and does nothing.

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
its report describes that requested scope. Cargo configuration discovery
uses the original invocation directory, including member-directory ancestors;
relative Cargo/cache homes are resolved against that directory.

### What `sail verify` does not support

Sail owns the Cargo target and Verus's logs, and verifies Cargo library and
binary targets. It refuses, with an error and without running anything:

- `--target-dir`, Verus's `--log`, `--log-all` and `--log-dir`, and
  `--no-verify` (`SAIL3100`);
- tests, benches and examples (`--tests`, `--test`, `--all-targets`, …)
  (`SAIL3100`);
- a chained workspace rustc wrapper (`RUSTC_WORKSPACE_WRAPPER`), custom
  sysroots and JSON target specifications (`SAIL3100`, `SAIL3111`);
- Cargo `--config`, configuration includes, and compiler, wrapper or
  sysroot overrides it cannot identify (`SAIL3112`);
- a project with no target that sets `package.metadata.verus.verify = true`
  (`SAIL3100`).

`cargo verus verify` still runs these, without Sail.

A process-loader variable (`LD_LIBRARY_PATH`, `LD_PRELOAD`,
`DYLD_LIBRARY_PATH`, `DYLD_INSERT_LIBRARIES` or `DYLD_FRAMEWORK_PATH`) is
not refused. Sail cannot bind what it changes to a receipt, so such a run
prints one `note[SAIL3112]` line, verifies what Cargo rebuilds exactly as
`cargo verus verify` does, and neither reuses nor records proof receipts
or dependency records. The solver cache still answers queries Verus has sent
before. `--output json` reads `unknown`, with a `SAIL3112` note, and has no
report for a target Cargo found up to date. Unset the variable for the
`sail verify` command to get reuse back. `--verus-source FILE`
(a single file with `verus FILE`) is only for the opt-in portfolio.
`--measure-fragility` and `--portfolio accept-first-proof` are explicit solver
experiments: they run `cargo verus verify` with `sail-z3` directly and print
one schema-v2 report, and they do not take `--background` or `--engine`
(`SAIL0110`).

### Output compared with `cargo verus verify`

`sail verify` prints the same bytes as `cargo verus verify` for the same work.
What differs is when there is work:

- **Its own target.** The first run in a project, worktree or new
  combination of arguments compiles the dependencies and verifies vstd in
  Sail's target, so it prints Cargo's `Compiling` lines and vstd's
  `verification results::` line even when your own `target/` is up to date.
  After that, only what changed is verified.
- **Unchanged targets are skipped by content.** Cargo-Verus decides
  freshness by file content, and so does Sail's target: an unchanged target
  prints no `Checking` or `verification results::` line, in both. A target
  whose verification failed is always verified again, in both.
- **Arguments are part of the context.** Plain `cargo verus verify` treats a
  crate as fresh after you change only its Verus arguments (for example add
  `--rlimit` or `--time`) and prints nothing new. Sail verifies again with
  the new arguments, as a first run in that context.
- **Timings.** Cargo's `Finished … in Ns` and Verus's `--time` figures are
  measured on each run. Verus's `--time` report shows Sail's Verus's own
  build information (ADR 0043).
- **A retried target.** When Cargo finds a target fresh but Sail has no
  receipt for it (after a crash, or after `sail cache projects prune`
  removed receipts but not the target), Sail cleans that target and runs
  Cargo again, so Cargo's lines for that run appear twice.

Proof reuse works on Linux and macOS. On Linux, an ordered native event queue
also tracks transient creation and removal of missing Cargo configuration
files during a run. macOS has no equivalent, so there missing configuration
files are checked only at the start and end of a run; a configuration file
created and deleted again within one run is not noticed (see
[freshness](freshness.md#what-these-checks-do-not-catch)). Present inputs keep
the same content and mutation checks on both platforms.

`sail verify --output json` prints a `sail-project-result` envelope, version 2,
containing per-target schema-v2 reports. Its JSON Schema is
`crates/sail-api/schema/project-result-v2.json`. Each target identifies its package,
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

When Sail cannot confirm that a result's inputs are unchanged, the result is
Unknown and carries the note `SAIL3114: inputs could not be confirmed
unchanged; re-run for a current result` ([freshness](freshness.md)).
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
`sail verify` processes outside the daemon wait on the managed
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
  agree. A partial scope reports `not_observed`, never `removed`. A function
  that passed by Verus's totals (reported `unknown`, not attributed one by
  one) is classified as verified, with its reported verdict kept and flagged.
  A repeated friendly function name (VI-0010) is compared as one group only
  when every function under it has the same result on each side and the
  counts match; otherwise it is `incomparable`. See
  [MCP comparisons](mcp.md#comparisons). Without `--output json`, the
  command prints counts by change, the changed functions and the notes.

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
is `<cache>/daemon-v4-workspace-v2-proof-v1.sock`, and the job socket beside it
is `jobs-v1-proof-v1-<hash>.sock`.

Unix socket paths are limited to 107 bytes on Linux and 103 on macOS
(`SUN_LEN`). When a deep `HOME` or a long `SAIL_CACHE_DIR` would exceed that,
Sail puts both sockets in a private per-cache directory instead,
`/tmp/sail-<hash>/` (mode 0700; used only when no other user can access it).
If the daemon still cannot start, `sail verify` prints one
`warning[SAIL0003]` line saying why and verifies without the cache, with the
same results. An explicit `SAIL_DAEMON_SOCKET` is used as given, so keep it
short.
An explicitly selected older cache daemon can keep running: Sail starts a
private jobs-only sidecar alongside it instead of terminating it.
