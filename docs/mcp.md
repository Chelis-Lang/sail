# `sail mcp`: the MCP server

`sail mcp` serves verification to agents over the
[Model Context Protocol](https://modelcontextprotocol.io) on stdin and
stdout. It is built for agents that verify after every edit. The design
decisions are in ADRs 0016 and 0024.

## Registering it

The server is a stdio command: `sail mcp`, with no arguments. It needs what
`sail verify` needs: `cargo-verus` on `PATH` (or `SAIL_CARGO_VERUS`), and
`sail-z3` next to `sail`. Start it from the project directory, or pass
`project_dir` to each tool.

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
portfolio or fragility experiments. Pass its `job_id` to:

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

`current` applies to the recorded verification context and a revalidated declared
input revision. `historical` applies to earlier inputs; `unknown` means coverage
could not be established. Neither historical nor provisional evidence is a new
verified claim about current code. Run `verification_result` to obtain complete
scope, target identity, original report provenance, and counterexample data.

## Evidence tools

These tools read recorded evidence from the local job service through the same
CLI commands described in
[workspace verification](workspace-verification.md#evidence-from-completed-jobs).
They do not accept `project_dir`; they use the server's working directory and
environment to find the service. No tool here verifies current inputs or
generates proof edits. `isError: false` means the operation succeeded, not
that a proof did.

| Tool | Arguments | Result |
|---|---|---|
| `verification_events` | `job_id`, optional `stream_id` and unsigned `after` | One bounded page of the proof-event journal, with a reconnect cursor, `gap` and `terminal`. Stock jobs write no proof events today: a finished stock job returns an empty terminal page with a `SAIL0903` gap. |
| `explain_failure` | `job_id`, `package_id`, `target`, `kind` (`lib` or `bin`), `function`, optional `obligation` | The exact function report, raw models, run diagnostics and establishing provenance. |
| `compare_verification` | `after_job` and exactly one of `before_job` or `baseline` | A scoped comparison. Context or scope mismatches are `incomparable`, never regressions. |
| `save_baseline` | `job_id`, `name` | Saves a completed job under a new immutable name. An existing name is refused. |
| `list_baselines` | none | Sorted saved names. |
| `get_baseline` | `name` | The complete immutable snapshot, always `historical`. |

Save and get return `structuredContent.response: "baseline"` with the snapshot
in `structuredContent.value`; list returns `response: "baselines"`. MCP rejects
a reply whose name, or for save whose job, differs from the request. A
cancelled save may already have published its record, so inspect the name
before retrying.

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

```json
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
