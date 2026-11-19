# Raw SMT runtime

Use `sail runtime` to submit complete SMT-LIB transcripts through Sail's bundled
pinned Z3. This command is independent of `sail verify`: a satisfiability answer
is evidence about the submitted formula, not proof that Rust code implements it.

Inspect actual installation and implemented capabilities:

```sh
sail runtime capabilities
```

A missing or changed solver is reported as unavailable. Release installations
use Sail's bundled runtime. In a source checkout, install the existing pinned
engine as described by `sail engine --help`; no separate Z3 installation is
needed. `PATH` is not searched for a solver.

Save a version-one request in `query.json`:

```json
{
  "version": 1,
  "input": {
    "kind": "smt",
    "queries": [
      {
        "obligation_id": "requirements/consistency",
        "requirement_ids": ["REQ-001"],
        "backend": "z3",
        "script": "(set-option :produce-models true)\n(declare-const count Int)\n(assert (> count 2))\n(check-sat)\n(get-model)\n"
      }
    ]
  }
}
```

```sh
sail runtime execute --file query.json
cat query.json | sail runtime execute --file -
```

Both commands print one JSON response. Query order and caller identities are
preserved. Each result separates the process outcome from ordered satisfiability
checks and their model, value, unsat-core or reason-unknown products. Raw solver
output is retained alongside its pinned identity and exact input hash.
Requirement IDs must be sorted and unique. Unknown JSON fields are rejected.

A batch admits at most 64 queries and 16 MiB of total script bytes. Each query
may set `limits`: `resource_limit`, `wall_time_ms`, `input_bytes` and
`output_bytes`. Defaults are 2,000,000 Z3 resource units, a 30-second safety
deadline, 16 MiB input and 16 MiB per output stream. Limits must be positive;
the maximum wall deadline is five minutes. The sum of three times each query's
output budget must fit 128 MiB across the batch, reserving space for both
streams and extracted products. Large batches must choose smaller per-query
output limits. The JSON envelope is bounded at
128 MiB before decoding. Resource options in a script must agree with its
request limit, and solver seeds must remain zero. See the
[complete admission rules](spec/runtime.md).

Transcripts may use push/pop and multiple checks. Follow-up products must belong
to the current check and the appropriate answer: a model requires `sat`, a core
requires `unsat`. There are no persistent session handles in this version.
Unsupported commands fail admission before solver execution. Missing, extra or
malformed output, nonzero process exit and solver stderr fail the whole
transcript. An apparently successful prefix is never returned as complete.

The default cache lives below Sail's cache directory in `runtime/`; `--cache DIR`
selects a different runtime cache root. Entries bind exact script bytes,
effective options, limits, protocol, fixed execution environment and pinned solver identity.
The solver environment is cleared and sets `LC_ALL=C`; ambient loader or solver
settings are not forwarded. Complete
transcripts containing only decided answers may be reused; timeouts,
cancellation, output overflow, infrastructure failures and unknown answers are
not cached as successful evidence. Reuse preserves original solver bytes and
reattributes them to the current caller's identities.

## Shared daemon and MCP

`sail serve` binds a separate version-one runtime endpoint beside its cache
and Cargo-job endpoints. The runtime shares the daemon's idle-activity guard
and shuts down with it. To use that service explicitly from the CLI:

```sh
sail serve --socket /path/to/cache.sock
# In another terminal:
sail runtime capabilities --socket /path/to/cache.sock
sail runtime execute --file query.json --socket /path/to/cache.sock
```

`--socket` names the cache socket, from which the runtime endpoint is derived.
It connects without autostart or local fallback. Without `--socket`, runtime
commands retain their embedded execution path. `--cache` and `--socket` are
mutually exclusive. Both paths use the same executor and cache grammar.

The MCP tools `runtime_capabilities` and `runtime_execute` use this service
directly. The first tool accepts no arguments; the second takes one `request`
object in the format above. MCP resolves `SAIL_CACHE_DIR` and
`SAIL_DAEMON_SOCKET` from its working directory and starts the configured
`sail serve` when no daemon is running. A running older daemon without a
compatible endpoint must be restarted; MCP does not bypass it with an embedded
solver. The shared daemon outlives an individual tool call.

The runtime socket is mode 0600 inside an owner-owned real directory of mode
0700. Symlink parents and permissive endpoints are refused. Each framed request
has a separate transport version and identity; incomplete frames, excess size,
changed attribution and incompatible responses fail closed. The default
service admits eight client connections and two active executions, reserving
capacity for cancellation and capability requests. These are service admission
bounds, not a global memory guarantee. Disconnecting an execution cancels its
work. MCP cancellation sends a separate control request and never publishes a
cancelled call as completed proof.

Exit 0 means every transcript completed with decided answers; either `sat` or
`unsat` qualifies. Exit 1 means unknown or incomplete execution. Exit 2 means
invalid input or setup failure. Ctrl-C cancels active work and reaps its solver
processes. Capabilities are descriptive and exit 0 even if a backend is missing.

The SMT family implements Z3 only. Native temporal execution is described
below. Requirement assurance remains a separate integration slice, and is not
implied by raw SMT completion. See [ADR 0068](decisions/0068-runtime-daemon.md).

## Native temporal jobs

Native temporal jobs are experimental. The `sail` binary links the
D.I.V.E. backend, and release archives ship its isolated worker.
The same runtime endpoint accepts `input: {kind: "temporal", job: ...}`.
`sail-api::temporal::Job` contains caller `obligation_id`, sorted unique
`requirement_ids`, an immutable native `request` and positive `wall_time_ms`
(up to five minutes). The native request supplies the closed source map,
root/configuration and check, finite simulation or trace validation operation.
See [native temporal jobs](temporal-jobs.md) for the admitted semantic scope.

`capabilities.temporal` reports implementation and authenticated worker
installation separately. The `sail` binary links the native backend
([ADR 0172](decisions/0172-native-temporal-in-main-workspace.md)); with no
authenticated worker beside it, it reports `installed: false` with the
`unavailable` reason "native temporal worker is not installed", and temporal
jobs return `Unavailable`. An embedder that runs `sail_cli::run_with` without
a backend instead reports "native temporal support is not included in this
sail build" ([ADR 0083](decisions/0083-temporal-extension-workspace.md)). Older services
omit this field; temporal clients
must restart or upgrade instead of falling back. Existing SMT requests and
version-one wire fields retain their meanings.

The result preserves caller identity, exact request identity, D.I.V.E.
release version, worker digest and deadline. Execution failure has no native response.
A completed execution includes the original typed native response, reports,
traces and validation evidence. Native stopped/unsupported/error outcomes are
explicit. Neither finite simulation nor trace validity is an exhaustive proof.
The runtime can reuse a completed exact check when its source closure, policy,
worker and limits match. On a hit the attributed result carries `cache_hit: true`
and the matched `cache_key`. Elapsed times and paths in its native report
describe the original execution. Changing the caller's obligation or requirement
IDs attaches the original evidence to the new call. Reverting all inputs can
recover the earlier result. [Temporal receipt contract](spec/temporal-jobs.md#exact-check-replay).

The service, the embedded runtime and preparation share
`$SAIL_CACHE_DIR/runtime/temporal-v1` by default. An explicit runtime cache
outside Sail's solver `results/` directory keeps its own receipts. An alias
of the default solver location uses the normal temporal store. An explicit
subdirectory of `results/` gets a separate `runtime-scope-<digest>` directory
beside it. This keeps temporal evidence out of unsigned solver-cache exports.

Rust embedders using `Config::default()` inherit the runtime cache location.
When updating an exhaustive `sail_runtime::Config` literal, add
`temporal_cache_directory: None` for that behavior. Clients constructing
temporal result or capability literals also supply the optional `cache_key`
or `replay_context`. Older socket clients continue to receive fresh results.

Release archives ship `bin/sail-runtime-worker` and its installation
manifest `bin/sail-runtime-worker.json` beside `sail`. A source build stages
the worker beside its `sail` with the same helper:

```sh
cargo build --locked -p sail-solve --bins
python3 scripts/stage-temporal-worker.py target/debug/sail-runtime-worker target/debug
target/debug/sail runtime capabilities
```

`cargo install sail-solve` installs the worker binary beside `sail` but not
its manifest, so temporal jobs report the worker not installed until the
manifest is staged. The manifest records the exact linked D.I.V.E. release,
payload protocol, platform and worker digest. Release staging uses the same
helper after stripping final bytes and binds both files in its release
inventory. No PATH search,
caller-selected executable, Java or TLC process participates.
