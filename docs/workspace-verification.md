# Live workspace verification

Every `sail verify` is managed verification
([ADR 0046](decisions/0046-managed-verification-by-default.md)). Install
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
Reports and receipts bind Cargo's exact target name. Binaries named `foo-bar`
and `foo_bar` remain distinct even though Rust uses the crate name `foo_bar`
for both. An observation that cannot identify one Cargo target supplies no
reusable proof; a failing target retains its own report.
When the dependency store seeds a new Cargo target, each dependency's exact
validated displayed stdout is restored at its fresh Cargo artifact event,
before its dependents' output. An ordinary unchanged target emits nothing.
An entry with missing or damaged output is ignored and verified normally.
The store admits a verified root only when its closure contains no other
verified root, so it cannot suppress another proof's diagnostics.

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

### Setups verified without proof reuse

Sail owns the Cargo target and Verus's logs, and binds Cargo library and
binary targets to proof receipts. It never refuses what plain
`cargo verus verify` accepts. When an invocation asks for something it
cannot bind to a receipt, `sail verify` prints one `note[SAIL3112]` line on
stderr naming the reason, then verifies exactly as `cargo verus verify`
does, in your own target, with `sail-z3` and the solver cache in front of
Z3: the rest of the output, the verdicts and the exit code are Cargo-Verus's.
Nothing is reused or recorded. These setups are:

- `--target-dir`, Verus's `--log`, `--log-all` and `--log-dir`, and
  `--no-verify`;
- tests, benches and examples (`--tests`, `--test`, `--all-targets`, …);
- Cargo's `-v`/`--verbose` and `--message-format`, which would show Sail's
  driver and flags;
- `--sysroot`, a chained workspace rustc wrapper (`RUSTC_WORKSPACE_WRAPPER`),
  Cargo `--config`, configuration includes, and compiler, wrapper or sysroot
  overrides Sail cannot identify;
- a project with no target that sets `package.metadata.verus.verify = true`.

With `--output json` such a run prints one schema-v2 report (as
`--measure-fragility` does) instead of a `sail-project-result`.

Background jobs (`sail verify --background`), `sail watch` and the MCP tools
verify these setups the same way: the job's worker prints the
`note[SAIL3112]` line into the job's diagnostics and runs plain
Cargo-Verus in your own target. Since they return project results, the
result is a `sail-project-result` with:

- `no_reuse`: `{"code": "SAIL3112", "reason": …, "verified_as":
  "plain-cargo-verus"}`;
- one requested target named `cargo verus verify`, with empty `package_id`
  and `kind`, `reused` and `complete` false, holding the run's schema-v2
  report for every crate Verus verified. A crate Cargo found up to date
  is not verified again, as with plain Cargo-Verus, and has no report;
- currentness `unknown`, empty `context_sha256` and `input_revision`, and a
  note starting `SAIL3112:`. Such a result is never `current`, establishes no
  freshness record, and nothing reuses it.

For this aggregate target, `execution.verifier_invocations` is one when the
Cargo-Verus driver returns a report, including when Cargo verifies zero or
several crates, and zero when setup or the driver fails without a report.
It does not count actual per-crate Verus or solver work.

The job's exit code is Cargo-Verus's. `sail jobs result` adds a
`no proof reuse (SAIL3112)` line, and `sail watch --output jsonl` emits a
`no-reuse` event (see [watch](#watch-and-agent-tools)).

A process-loader variable (`LD_LIBRARY_PATH`, `LD_PRELOAD`,
`DYLD_LIBRARY_PATH`, `DYLD_INSERT_LIBRARIES` or `DYLD_FRAMEWORK_PATH`) is
handled the same way inside managed verification, so it also works for jobs
and MCP: Sail cannot bind what it changes to a receipt, so such a run prints
one `note[SAIL3112]` line, verifies what Cargo rebuilds exactly as
`cargo verus verify` does, in Sail's target, and neither reuses nor records
proof receipts or dependency records. `--output json` reads `unknown`, with
a `SAIL3112` note and `no_reuse` whose `verified_as` is `managed`, and has
no report for a target Cargo found up to date. Unset the variable for the
`sail verify` command to get reuse back.

`--verus-source FILE` (a single file with `verus FILE`) is only for the
opt-in portfolio. `--measure-fragility` and `--portfolio accept-first-proof`
are explicit solver experiments: they run `cargo verus verify` with
`sail-z3` directly and print one schema-v2 report, and they do not take
`--background` or `--engine` (`SAIL0110`).

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
- **A fresh target without a receipt.** When Cargo finds a target fresh but
  Sail has no receipt for it (after a crash, or after `sail cache projects
  prune` removed receipts but not the target), `sail verify` prints what
  plain Cargo-Verus prints for an up-to-date target. `--output json`, jobs
  and MCP clean that target and run Cargo again, to return its report.
- **Partial verification.** `sail verify -- --verify-function F` (or
  `--verify-module`, `--verify-only-module`, `--verify-root`) gets plain
  Cargo-Verus's refusal ("Partial verification must use `cargo verus
  focus`") and its exit code. With `--output json`, and for jobs and MCP's
  scoped `verify`, Sail runs it focused and the report notes that its scope
  is partial.
- **Terminal.** On a terminal Sail asks Cargo for colour, as plain Cargo
  would print there; Cargo's progress bar is not shown. `-v` and
  `--message-format` are verified as plain Cargo-Verus does (with one
  `note[SAIL3112]` line), since Cargo would otherwise print Sail's driver and
  flags.
- **Combined output.** Verus's summary line reaches stdout once Verus has
  finished. With `2>&1`, a compile-phase error reported after verification
  (lifetime or erasure checks) appears before the summary instead of after
  it; proof failures and ordinary compile errors appear in plain's order.

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
never counted as fresh verification work. A run without proof reuse adds the
optional `no_reuse` object
([setups verified without proof reuse](#setups-verified-without-proof-reuse)).
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

Job text and MCP summaries call a target verified only when its whole-target
report passes the proof-authority checks and its inputs are Current. Historical
and Unknown proofs are labelled as recorded evidence for their establishing
revision. Compile-only runs (`--no-verify`) and incomplete or legacy results
without whole-target authority remain observations, even when their exit code
is zero or their inputs are Current. A genuine verification run with no
functions can still establish a whole-target proof.

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

`sail jobs` is supported. `sail explain`, `sail baseline` and `sail compare`
are experimental: `sail --help` does not list them, and they may change in
any release.

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
  agree. Runs marked `no_reuse` and results without a bound context are
  `incomparable`, including when saved as baselines: matching empty context
  fields do not establish that two setups agree.
  A partial scope reports `not_observed`, never `removed`. A function
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
unchanged inputs do not trigger another verification. Unrelated sibling removals
or renames delivered by ancestor subscriptions do not restart verification or
disable that startup check. A settled native watcher does not poll completed
jobs or rehash the verifier runtime.
Its JSON-lines stream carries job lifecycle and target completion/reuse events.
It verifies the requested Cargo scope; it does not guess semantic function
independence. A completed run without proof reuse
([`SAIL3112`](#setups-verified-without-proof-reuse)) adds one event
`{"kind": "sail-watch-event", "schema_version": 1, "event": "no-reuse",
"job_id": …, "no_reuse": …, "currentness": "unknown", "exit_code": …}`; in the
human stream the job's `note[SAIL3112]` line says the same. An explicit
`--target-dir` is Cargo's output, so writes there do not restart a run.
Watch retains its current input roots for these runs. Additional external
include and build-script paths require a managed run that supplies an input
record.

MCP exposes `start_verification`, `verification_status`, `verification_result`
and `cancel_verification`. Existing blocking verification and result tools
remain available. Status/result inspection does not wait behind a verification
worker. Results carry currentness, scope and establishing provenance so agents
can distinguish finished work from current proof evidence.

Existing solver-cache entries remain compatible. The automatic daemon socket
is `<cache>/daemon-v5-workspace-v2-proof-v1.sock`, and the job socket beside it
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

## Instant revert and research scope

Ordinary `sail verify` can restore a previously verified revision after an
A → B → A edit on the exact qualified Linux/public alloc/std dependency profile.
The original establishing run remains visible; fresh complete input, runtime,
compiler-certificate and Current checks are still required. Other resolutions,
platforms and unsupported contexts compile normally. `SAIL_MANAGED_FORCE=1`
keeps verification fresh. See the [guide](instant-revert.md) and
[ADR 0050](decisions/0050-retained-proofs-instant-revert.md).

`SAIL_RETAINED_CONTENT_RESEARCH=1` explicitly enables the other separately
closed provider profiles. It still requires actual source-observer documents
and never treats missing or unsupported evidence as reusable. Failed placement
rollback refuses with `SAIL3134`. Scoped native qualifications, the 13.45% Vest
cycle gain and increased cold/edit/unchanged and storage costs are in the
[validation record](validation/instant-revert.md).

The experimental retained arena refuses new admission at 8 GiB of file data or
65,536 entries, including failed writes and lifetime markers. It preserves
potentially live compiler generations and falls back to compilation on fullness
or unsupported inventory. Existing graph reuse can also refuse near capacity
because admission conservatively reserves a full graph. These limits cover the
retained arena, not ordinary managed-run sidecars or every Sail cache. The
complete native cycle includes the scan and storage costs for its stated scope.

The two compiler-reuse research switches are mutually exclusive. Setting both
`SAIL_VSTD_BOUNDARY_RESEARCH=1` and `SAIL_RETAINED_CONTENT_RESEARCH=1`
returns `SAIL3135` before compiler setup; unset either switch. Their
[source integration](validation/proof-reuse-integration.md) passes final-source
default gates and preserves each experiment's separate source authority and
remaining release limits.

Experimental portable vstd bundles also bind the configured solver-adapter bytes
and the exact Cargo-Verus package environment identity. Moving identical
installed binaries is distinct from selecting a different adapter; changed,
unknown or ambiguous observations safely run the compiler. Old bundle schemas
are refused. Same-host changed-root portable functionality passes; distributed
release/default fresh-machine first use remains pending. See the
[vstd boundary limits](validation/vstd-compile-boundary.md).
