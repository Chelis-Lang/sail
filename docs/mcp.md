# `sail mcp`: the MCP server

`sail mcp` serves verification to agents over the
[Model Context Protocol](https://modelcontextprotocol.io) on stdin and
stdout. It is built for agents that verify after every edit. The design
decisions are in ADRs 0016 and 0024.

## Registering it

The server is a stdio command: `sail mcp`, with no arguments. It needs what
`sail verify` needs: `cargo-verus` on `PATH` (or `SAIL_CARGO_VERUS`), and
`sail-z3` next to `sail`. Start it from the project directory. Submission and
project-oriented lookup tools accept `project_dir` where their schemas show it;
durable job/evidence reads and cancellation do not.

Claude Code:

```sh
claude mcp add --transport stdio sail -- sail mcp                  # for you
claude mcp add --transport stdio --scope project sail -- sail mcp  # shared, in .mcp.json
```

Any client that takes a JSON server list (`.mcp.json` and similar):

```json
{
  "mcpServers": {
    "sail": {
      "type": "stdio",
      "command": "sail",
      "args": ["mcp"],
      "env": { "SAIL_CARGO_VERUS": "/path/to/cargo-verus" }
    }
  }
}
```

The `env` entry is needed only if `cargo-verus` is not on the client's
`PATH`. Use an absolute path to `sail` if it is not on `PATH` either.

For interactive repair, install the engine in the same Sail cache used by the
MCP process and CLI. If overriding `SAIL_CACHE_DIR` or `SAIL_DAEMON_SOCKET`, set
the same absolute values in the MCP launch environment and CLI environment.
`start_repair_validation` can change the submission project, but subsequent
job/status/result/cancel, event, explanation and comparison calls use the server's
working directory to resolve its service. They reject `project_dir`. A relative
cache override can route these calls to another service after switching projects.
The [repair walkthrough](proof-repair-walkthrough.md) shows engine setup and the
complete submission-to-baseline sequence.
Keep MCP client transcripts and redirected tool output outside watched package
roots and declared inputs during verification. Creating or updating those files
inside the project can correctly invalidate the proof's input freshness.

## Reading completed evidence

Tool replies retain the full structured result and its complete JSON text for
clients that read text only. The server moves owned result graphs through the
response envelope and encodes each complete JSON-RPC frame before taking the
stdout write lock. Concurrent frames remain atomic. Cancellation remains
registered through encoding and is checked again before publication; a
cancelled response is suppressed before its write begins. Slow stdout consumers
can still block writes. This does not establish a live-reader latency gain or a
bound on temporary response memory. See [ADR 0034](decisions/0034-mcp-response-encoding-ownership.md)
and the [scoped controls](validation/m7-mcp-output-ownership.md).

Job status, completed results, counterexamples and cached-result lookups use
persistent service clients inside `sail mcp`. Each lookup still opens a bounded
connection and reads the complete fresh service reply. The service determines
source currentness; an earlier decoded report cannot authorize a later lookup.
Changing a model, job ID, currentness or any other reply byte invalidates decoded
reuse. Historical evidence remains labelled historical.

Relative `SAIL_CACHE_DIR` and `SAIL_DAEMON_SOCKET` settings resolve from each
operation's project directory, just as they do for the CLI. Switching projects
or restarting a service does not reuse an earlier authorization. Missing or refused connections use the existing CLI read path, including
service startup and legacy support. Once a direct request starts, it has an
absolute five-second transport deadline and is never retried through the CLI.
Cancellation shuts down the owned read connection and joins its monitor; a late
response is discarded. Invalid versions, response variants and job IDs fail
without a retry.
Verification submission, repair submission and cancellation retain the existing
CLI process ownership. Proof events and explanation/comparison tools retain
their current CLI paths.

Eight endpoint handles share one 128 MiB decoded-frame budget, including entries
still used by concurrent calls after eviction. This bounds retained cache entry
ownership; returned tool graphs, temporary parsing/serialization and process RSS
are outside that budget. Replies larger than the cache
eligibility limit still work normally without retention. No MCP latency gain is
claimed from the separate client-codec measurements; product measurements remain
necessary. See [the client contract](result-client.md).

## Protocol

- **Transport.** Newline-delimited JSON-RPC 2.0 on stdio. stdout carries
  only MCP messages, and the server writes nothing to stderr while serving.
- **Versions.** The server is dual-era:
  - Clients that open with `initialize` get revision `2025-11-25`,
    `2025-06-18`, `2025-03-26` or `2024-11-05`: the one they ask for, or
    `2025-11-25` if they ask for another.
  - Clients on revision `2026-07-28` send the version in each request's
    `_meta` and may call `server/discover`. An unknown version gets
    `UnsupportedProtocolVersionError` (`-32022`) listing the supported ones.
- **Methods.** `initialize`, `notifications/initialized`, `server/discover`,
  `ping`, `tools/list`, `tools/call` and `notifications/cancelled`. There
  are no resources or prompts.
- **Errors.** Malformed messages get the JSON-RPC errors (`-32700`,
  `-32600`), unknown methods `-32601`, and an unknown tool `-32602`. Invalid
  tool arguments and failed runs are tool results with `isError: true` and a
  message the model can act on.
- **Results.** Every tool returns `structuredContent` and two text blocks: a
  short summary, then the same structured result as JSON text (for clients
  that ignore `structuredContent`).
- **Concurrency and cancellation.** Tool calls run on their own threads, so
  `ping` and cancellation are served while a verification runs. Runs in one
  project are serialised through the local job service in default mode. Lookups
  inspect completed evidence without waiting for a newer verifier to finish.
  `notifications/cancelled` kills the run's whole process group (`cargo
  verus`, Verus, shims, solvers), as `sail watch` does, and the call gets no
  response. Cancelling is always safe: the cache stores only complete
  replies.
- **Shutdown.** When stdin closes, the server finishes the calls in flight
  and exits.

## Persistent verification jobs

Use `start_verification` with the same default verification arguments as
`verify`; it returns a durable job status immediately. It does not enable
portfolio or fragility experiments. Pass its returned `structuredContent.id`
as the `job_id` argument to:

| Tool | Result |
|---|---|
| `verification_status` | Queued/running/completed/cancelled/superseded/infrastructure-error, separately from proof outcomes. |
| `verification_result` | Job status and the versioned project envelope, with per-target reports, requested scope, establishing run, reuse and currentness. Running jobs return without waiting. |
| `cancel_verification` | Cancel queued/running work and all verifier descendants; retain its history. |

Jobs survive MCP-client restarts and share the same single local verifier worker
as `sail verify --background` and `sail watch`. Read `cached_result` and
`counterexample` while a newer run proceeds. They inspect completed jobs first;
older MCP records remain historical evidence and cannot establish complete
workspace/runtime freshness.

After submission returns, use `cancel_verification` with its durable ID to stop
the job. A cancelled read request or MCP disconnect does not cancel a submitted
background job.

`current` applies to the recorded verification context and a revalidated declared
input revision. `historical` applies to earlier inputs; `unknown` means coverage
could not be established. Neither historical nor provisional evidence is a new
verified claim about current code. Run `verification_result` to obtain complete
scope, target identity, original report provenance, and counterexample data.

## Tools

Charter rule 7 holds in every tool. A function whose tier is `provisional`
is reported as `provisional`, never `verified`, in summaries and in looked-up
verdicts. `unknown` means verification was not confirmed and is never a settled
failure. It does not by itself establish that Z3 exhausted a resource limit.

### `verify`

Default verification submits a forced-fresh managed workspace job and waits
for its result. It retains dependency artifacts and returns its schema-v1
report (`docs/output-schema.md`). The report has per-function verdicts with
tier and provenance, obligation ids, counterexample models, diagnostics,
timings and cache sources. The run is recorded for the lookups. Explicit module/function selectors use
`cargo verus focus`, which is the pinned verifier’s supported partial entry
point; whole-project requests use `cargo verus verify`.

| Argument | Meaning |
|---|---|
| `project_dir` | The project. Default: the server's working directory. Relative paths resolve against it. |
| `package` | `-p <package>`. |
| `module` | Verus `--verify-module <module>`: this module and its descendants (with `function`, `--verify-only-module` selects that function’s owning module), relative to the crate root (`arith`, `foo::bar`). |
| `function` | Verus `--verify-function <function>`: a unique substring of the name, or a pattern with `*` at either end. It is looked up in `module`, or in the crate root (`--verify-root`) if no module is given. |
| `portfolio` | Optional `accept-first-proof`: sequential complete Verus attempts; every winning function comes from one complete run, with seed/configuration and attempt evidence. Never enabled by default. |
| `measure_fragility` | Optional boolean: capture and replay solver sessions locally, attaching exact per-function scores; incomplete or ambiguous attribution stays null with a note. Cannot be combined with a portfolio. |
| `cargo_args` | Further `cargo verus verify` arguments, before `--`. They may not contain `--` or `--output`. |
| `verus_args` | Further Verus arguments, after `--`. |

Request:

```json
{"jsonrpc": "2.0", "id": 2, "method": "tools/call",
 "params": {"name": "verify", "arguments": {"project_dir": "/work/demo"}}}
```

Summary text (first content block):

```text
sail verify: 3 functions: 2 verified, 1 failed, 0 unknown, 0 provisional (exit 1); checks: 0 solved fresh, 6 from cache.
Failed: demo::bad.
Counterexamples (use the counterexample tool): demo::bad#Function-Def/0.
First of 1 error(s): postcondition not satisfied at src/lib.rs:10:13.
```

Default `structuredContent` contains `project_dir`, `sail_args`, `exit_code`,
`duration_ms`, `stored`, `summary` and the schema-v1 `report`, together with:

- `job`: the durable job status, including its id, lifecycle and currentness;
- `project_result`: the complete version-1 project envelope, including requested
  targets, execution counters and their establishing reports;
- `sources`: `input_revision`, `currentness` and `changed_during_run`. The last
  value is `false` for current evidence and `null` when historical or unknown
  currentness cannot establish when an input changed.

The summary also identifies the job and its currentness. If Cargo fails before
any verification target produces a report, the response contains `job` and
`project_result` with its diagnostics rather than inventing a function report.
Explicit portfolio/fragility paths and older Sail installations retain their
existing report response; their legacy source snapshots cannot establish complete
workspace/runtime freshness.

**Complete reports.** Default blocking verification forces the requested
verification targets fresh, so Cargo's unchanged-target shortcut cannot produce
an empty successful report. Workspace dependencies remain warm. Partial
requests preserve their explicit scope. Explicit portfolio/fragility requests
use their established paths. Builds that fail before verification report their
diagnostics without inventing per-function verdicts.

If `sail verify` could not run Verus at all (the report's `error` is set), or
printed no report, the result has `isError: true` and the end of its stderr.

### `counterexample`

Returns counterexample evidence for a function or obligation from the latest
matching completed job, falling back to older MCP records. Nothing is re-run.
The full establishing report, including diagnostics, is available through
`verification_result`.

| Argument | Meaning |
|---|---|
| `project_dir` | As for `verify`. |
| `function` | A full path (`demo::arith::mul_pos`), or a unique `::` suffix (`mul_pos`). |
| `obligation` | An obligation id (`demo::bad#Function-Def/0`). The function part may be a suffix too. |

At least one of `function` and `obligation` is required.

Both `counterexample` and `cached_result` return this shape for job-backed
lookups, including when only one function is selected:

```json
{
  "project_dir": "/work/demo",
  "job": {"version": 1, "id": "JOB", "state": "completed", "currentness": "historical", "…": "remaining status fields"},
  "functions": [{
    "package_id": "…", "target": "demo", "establishing_run": "RUN", "reused": false,
    "function": "demo::bad", "verdict": "failed", "tier": "confirmed",
    "mode": "default", "configuration": null,
    "function_report": {"…": "complete schema-v1 function report"},
    "obligations": [], "counterexamples": []
  }]
}
```

The example abbreviates the job and report fields. Read `job.currentness` to
assess whether the evidence applies to current inputs. `establishing_run` names
the original proof run even when a later job reused its target. Counterexample
lookups populate `obligations` and `counterexamples`; compact verdict lookups
set those two fields to `null` and retain the full `function_report`. Obligation
selectors restrict counterexample evidence to that obligation. A missing
obligation or ambiguous function is an error, rather than an empty model.

Older MCP records use the following legacy format. Its `freshness` remains
unknown unless tracked source changes establish that it is stale; those records
cannot prove complete workspace/runtime currentness.

```json
{"jsonrpc": "2.0", "id": 3, "method": "tools/call",
 "params": {"name": "counterexample", "arguments": {"function": "bad"}}}
```

```text
demo::bad: failed
1 obligation(s) with a counterexample model:
  demo::bad#Function-Def/0: r! = 0, x! = 0
Established by run 2 (sail verify --output json) at 2026-10-08T22:41:21.259Z; Tracked sources are unchanged; complete workspace/runtime freshness is unknown for this historical record.
```

`structuredContent` holds:
- `function`, `verdict` and `tier`;
- `obligations`: the function's unproved obligations, or those with a model
  (only the named one, given `obligation`). Each is the schema's obligation,
  with `counterexample.assignments` as data and `counterexample.model`
  verbatim. The text summary lists only source variables (`x!`, `i@0`).
- `diagnostics`: the run's diagnostics located in the function. These are
  diagnostics with a span in the function's file, from its first line up to
  the next function's first line. Verus's spans say where a function starts
  but not where it ends. `diagnostics_matched_by` says how they were
  matched.
- `attribution`: the run's attribution status and notes. Without
  attribution there are no obligations, and `diagnostics` holds every
  diagnostic of the run.
- `freshness`: as for `cached_result`.

### `cached_result`

Compact lookup responses and counterexample responses retain each function's
`mode` and `configuration` (including a portfolio seed). Their human summaries
label opt-in modes and seeds too. Run freshness includes the originating mode
and winning seed; recalling a portfolio proof never relabels it as default.

Looks up the latest matching completed job for a function or obligation, or
its requested functions when given neither. If no completed job matches, it
falls back to an older MCP record. Nothing is re-run. Arguments are as for
`counterexample`, all optional.

```json
{"jsonrpc": "2.0", "id": 4, "method": "tools/call",
 "params": {"name": "cached_result", "arguments": {"function": "demo::max"}}}
```

The following example shows the legacy-record fallback; job-backed responses
use the `job` and `functions` shape above.

```text
demo::max: verified
Established by run 2 (sail verify --output json) at 2026-10-08T22:41:21.259Z; The sources changed since this run: its results apply to the earlier sources only. Re-run verify to confirm.
```

The structured response below is schematic: ellipses omit nested report fields.

```text
{
  "project_dir": "/work/demo",
  "function": "demo::max",
  "verdict": "verified",
  "tier": "confirmed",
  "result": { "function": "demo::max", "verdict": "verified", "obligations": [ … ], … },
  "freshness": {
    "status": "stale",
    "run": 2,
    "established_at": "2026-10-08T22:41:21.259Z",
    "established_by": "sail verify --output json",
    "exit_code": 1,
    "scoped": false,
    "sources_changed_since": true,
    "sources_changed_during_run": false,
    "changed_files": [{"path": "src/lib.rs", "change": "modified"}],
    "changed_files_total": 1,
    "note": "The sources changed since this run: its results apply to the earlier sources only. Re-run verify to confirm."
  }
}
```

With `obligation`, `result` is replaced by `obligation` (the schema's
obligation). With neither argument, `functions` lists each recorded
function's verdict, tier, `unstable` flag, provenance source, run and
status, and `runs` holds each run's freshness.

### `fragility`

Runs `sail fragility --output json` (ADR 0018) with `verify`'s arguments
(the `FRAGILITY_COMMAND` line in `crates/sail-mcp/src/tools.rs`). The
command captures one Verus run through the pass-through shim, re-runs every
solver session under 8 seeds, and returns per-check answer and effort
distributions, stability classes and scores in `result`. It changes no
verdict and never touches the cache. Expect it to take several times as
long as `verify`. Selecting a `module` or `function` runs `cargo verus
focus`, and checks are attributed to a function only when exactly one
function was verified.

The tool checks `sail --help` on every call. A `sail` without the command
gets `isError: true` with `{"available": false, "error": "not_available"}`.

## The result store

Every `verify` that produces results is recorded per project. A record holds
the report, when the run finished, its `sail` arguments, its exit code, and
a snapshot of the project's sources taken just before the run.
- **Snapshot.** The SHA-256 of every `*.rs` and `Cargo.toml` under the
  project directory, skipping `target/` and hidden directories.
- **Per function.** For each function the store keeps the latest run that
  reported it. A scoped run (a package, module or function) updates only its
  functions. An unscoped run replaces the set, forgetting functions it no
  longer reports.
- **Where.** In memory, and in `<cache>/mcp/<hash of the project path>.json`
  (`<cache>` is `$SAIL_CACHE_DIR` or `~/.cache/sail`). So results survive a
  server restart and are shared by servers on one machine. A store file that
  cannot be read or written is ignored, and the `verify` result says so.
  Store format 2 requires reverification of older records: those reports
  predate conservative handling of incomplete attribution (VI-0003) and
  cannot establish current confirmed results.
- **Staleness.** A lookup compares the run's snapshot with the sources now.
  Any difference, or a change while the run was in progress, makes the
  result `stale`. The changed files are listed. A stale verdict applies to
  the earlier sources. Files outside the project directory (path
  dependencies elsewhere, the toolchain) are not tracked.
- **Never the source of a verdict.** The store only reports what a recorded
  `sail verify` run said, with its provenance.

## Proof repair tools

The optional interactive engine is selected explicitly and must be installed as a
validated Sail-managed pinned build. `start_verification` accepts `engine: stock`
or `engine: interactive`; stock is the default.
Use `sail engine status --output json` to inspect the selected installation;
see the [repair walkthrough](proof-repair-walkthrough.md) for setup and a complete
focus/full regression example.

- `start_repair_validation` requires `package`, `kind` (`lib` or `bin`), exact
  crate-qualified `function`, and `engine: interactive`. A binary also requires
  `target`; a library omits it. Optional `project_dir`, `cargo_args` and
  `verus_args` preserve verification configuration. It returns a durable job ID
  immediately. Read the independent focused/full bundle with
  `verification_result`; a focus success never authorizes the repair.
- `verification_events` takes `job_id`, optional `stream_id` and unsigned
  `after`. It returns one bounded page and a reconnect cursor. Observe `gap` and
  `terminal`; provisional observations are not confirmed proofs.
- `explain_failure` takes exact `job_id`, `package_id`, `target`, `kind` and
  `function`, optionally `obligation`. It preserves verifier reports, raw models,
  diagnostics and provenance without generating proof edits.
- `compare_verification` takes `after_job` and exactly one `before_job` or
  immutable `baseline`. Context/scope mismatches and unobserved functions stay
  explicit; they are not labelled proof regressions.
- `save_baseline` takes `job_id` and `name`. The job must have completed under
  the existing service checks. An existing name cannot be overwritten. Failed,
  unknown, provisional or incomplete target evidence is retained as reported;
  saving does not accept a repair or create a proof receipt.
- `list_baselines` takes no arguments and returns sorted saved names.
- `get_baseline` takes `name` and returns the complete immutable snapshot,
  including the recorded job, input revision, context, targets and reports.
  Its currentness is always `historical`; it never revalidates today's workspace.

Baseline operations resolve the same local job service as job reads. They do not accept
`project_dir`; use the server's working directory and the same absolute cache
and socket overrides as submission. Names use 1–80 ASCII letters, digits, dots,
underscores or hyphens, excluding `.` and `..`. The CLI equivalents are
`sail baseline save NAME --job JOB`, `sail baseline list` and
`sail baseline get NAME --output json`. For a literal name that looks like an
option, place output options first: `sail baseline get --output json -- --json`.

Save/get return the existing job response envelope:
`structuredContent.response` is `baseline`, with a version-one snapshot in
`structuredContent.value`. List returns `response: baselines` and a string array
in `value`. No baseline operation reports a new Current proof. MCP rejects a
response with the wrong requested name or, for save, the wrong original job.
The existing local RPC payload limit remains; oversized evidence fails rather
than returning a truncated graph. A cancelled save call may already have
published its immutable record; inspect the name before retrying.

`get_baseline` adds a read-only job operation without changing the protocol
version or existing save/list operations. An older running service rejects the
unknown request with `SAIL0801`; it does not return an empty baseline. Update and
restart that service before using get. Saving and listing remain compatible with
the existing service operations.

See
[`proof-repair.md`](proof-repair.md) for currentness, compiler-seal and journal
retention contracts.

Submission returns `structuredContent.id`; it only acknowledges a durable job.
On the completed `verification_result`, inspect
`structuredContent.value.repair.accepted` and the independent full report.
`isError: false` means the tool operation succeeded, not that a proof succeeded.
Use the exact package/target/function identities in the report for explanations
and comparisons. MCP events are paged reads; CLI `--follow` is not an MCP argument.

Current `b89546c` passes the [original repair fixture](validation/m7-compiler-script-bootstrap/scheduling-recovery.json).
The [complete 730 reader/retained workflow](validation/m7-reader-730a321-terminal.md)
is historical; current-product reader replay remains required.
The reader p95 values come from the unchanged Python and Rust collectors, rather
than an MCP-specific timing census; retained MCP scenarios establish functional
behavior. Real-project repair performance and complete standard/cache coverage
remain pending. PR #74 is draft, and M7 remains experimental.

## Live read transport validation

On Linux, `python3 ci/test-mcp-live-service.py PATH_TO_SAIL --out EVIDENCE_DIR
--execution MACHINE_LABEL` runs the installed MCP server against an independent
real Sail job service. The installed `sail-z3` must be beside `sail`. The script
stops an actual verification worker and cancels it before proof completion; it
does not substitute a verifier or a CLI adapter. Removing the owned executable
pathname after MCP starts makes subprocess fallback unavailable during the read
checks. It checks the same job's changed running/cancelled replies, restart
identity, and cancellation of a blocked service read through MCP notifications.
Success requires the MCP server to exit normally with code zero within five
seconds after stdin EOF, followed by complete stdout EOF without a cancelled
reply. Forced termination is failure cleanup and cannot establish success.
All owned actors are stopped before their scratch directory is removed.
`ci/local.sh e2e managed` includes this control before the unchanged managed
proof acceptance. Three separate Python-child controller controls reject an
abnormal exit or ignored EOF and accept a normal completed stream; they do not
stand in for installed MCP or verifier acceptance. CI retains raw messages,
stderr and summary under
`managed-acceptance/mcp-live-service`, alongside the original proof evidence.

The [frozen b36 installed run](validation/m7-installed-mcp-b36.md) passed all five
unchanged controls on the hardened product, including normal EOF exit. Its
source, product and controller hashes are retained. The earlier
[portable record](validation/m7-mcp-live-service.json) preserves its older
controller scope and initial missing-program fixture error; neither is replaced
by the later pass.

This is supplementary transport evidence. It establishes neither a current
proof, counterexample correctness, live reader latency, nor the repair-loop
speed target. The original installed CLI/MCP verifier acceptance and reader
collector limits remain separate required checks. Raw replies and failures are
retained in the specified evidence directory.
