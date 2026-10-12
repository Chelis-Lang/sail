# Changelog

All notable user-visible changes are recorded here. The project follows
semantic versioning once the API is published.

## Unreleased

- The optional interactive engine is built from Sail's Verus fork
  (`Chelis-Lang/verus`, pinned in `engine/verus/fork.toml`) instead of the
  upstream commit plus `engine/verus/interactive.patch`. `sail engine status`
  reports `fork_commit` in place of `patch_sha256` (status version 2), and the
  runtime pin records the fork commit. The fork fixes nine known Verus issues
  (VI-0001, 0003, 0004, 0005, 0008, 0009, 0010, 0012, 0015): stable
  declaration, trigger and progress-note order, correct
  `--output-json` success and selection, a `function-path` timing field, and
  per-crate log directories and argument-change re-verification under
  `cargo verus`. Default `sail verify` still runs the stock release.
- Managed runs are 1–3 s faster per edit cycle and warm unchanged runs take
  0.4–1.6 s. Sail now checks a result's currentness by rehashing exactly the
  files the verified target depended on, and identifies toolchain files by
  their file stats. Large workspaces such as Vest now read back Current
  instead of Unknown. A result is never shown as Current when any input it
  depended on may have changed; in doubt it reads back Unknown with the note
  "inputs could not be confirmed unchanged; re-run". See
  [freshness](docs/freshness.md) for the rare cases this does not detect.
- Concurrent managed runs on one workspace now wait for a busy target
  (`SAIL_MANAGED_LOCK_WAIT_SECS`) instead of failing with `SAIL3104`.
- Removed the Python kernel-lease helper, the daemon sysroot handoff endpoint
  (`.sysroots-v1` socket) and the kernel-lease notes on `SAIL3114`.

- Managed verification binds only the environment that `cargo verus` treats
  as input: the variables Cargo, rustc, Verus and Z3 read, plus every variable
  the project declares through `env!`/`option_env!` or
  `rerun-if-env-changed`, recorded per receipt. `PATH` and `HOME` count only
  through the tools and homes they resolve to. Agent sessions, terminals and
  the CLI therefore share one proof context per project: an agent's first
  `sail mcp` verify after a CLI run, or in a new session, no longer verifies
  vstd again. Receipts are version 3; older receipts are not reused.
- MCP `verify` no longer forces the requested targets fresh. It reuses a
  Current report for unchanged inputs, as `sail verify --incremental` does,
  and names the run that established it.
- The project job service runs jobs from different Cargo workspaces
  concurrently, so several agents sharing one daemon no longer wait for each
  other. `SAIL_JOB_WORKERS` sets the bound (default: a quarter of the available
  cores, from 1 to 4). Jobs in the same workspace still run one at a time, in
  submission order, and queue rather than fail.
- Repair job results carry the full report once, as `result`; `repair.batch`
  no longer repeats it as `full`, halving the frame every reader receives
  (4.9 MB to 2.5 MB on a HumanEval repair). Foreground `sail repair-check
  --output json` still prints a standalone repair document with
  `batch.full`.

- `sail-bench repair` no longer records peak RSS or writes a raw sidecar;
  `--raw` is removed. Each invocation's command, stdout and stderr stay under
  `--out`. The report drops its preparation and peak-RSS columns.

- `sail_api::jobs::Client` no longer caches decoded Results. Removed
  `Client::with_result_decoding_cache`, `without_result_decoding_cache`,
  `with_socket`, `call_cancellable` and `CancellableError`; use `Client::new`
  and `call`, or `call_with` to abandon a call by shutting down its stream.
- `sail mcp` direct job reads report plain errors: `SAIL0813: job read failed:
  …` (`{"error":"read_failed"}`) and `SAIL0813: job read cancelled`
  (`{"error":"cancelled"}`), instead of reproducing the CLI's error text and
  exit code.

- Remove the `SAIL_JOB_RPC_TIMINGS` diagnostic and the daemon's
  `rpc-timings.json` export. Job reads are unchanged: every Status and Result
  read still re-reads and hashes the job's output and checks input freshness.

- Interactive engine: remove prepared-lowering reuse and the same-input
  oracle. Repairs no longer pay for preparation, and progress events no longer
  rebuild every function name per obligation. A one-round native candidate
  measured 1.70× M6 on HumanEval and 1.54× on Vest. Removed
  `batch.preparation`, `engine.persistent_preparation`,
  `SAIL_ENGINE_PERSISTENT_PREPARATION` and `sail-conformance engine`'s
  `--oracle*` options; `sail-bench repair` no longer reports `SAIL7105`.

- Fix managed focused checks (`sail verify --incremental -- --verify-function
  …`) failing with `SAIL3107` when the source matched the last focused build.
  Sail now cleans `<target>/verus-partial`, where `cargo verus focus` builds.
- Fix managed runs that were always Unknown (`SAIL3114
  native-observer-setup-unavailable`) on workspaces, such as Vest, whose
  members resolve dependencies the selected package never builds. Proof-time
  coverage now spans the selected targets' dependency closure; a Cargo unit
  outside it reports `native-observer-scope-exceeded`.
- `sail-bench repair` reports `SAIL7110` for an M6 check that ended in a Sail
  error; such a sample never counts as M6 time.

- Add `sail-bench repair prepare|run`, the M7 repair acceptance workload. It
  replaces `scripts/m7-repair-bench.py`, which stopped at the first refusal.
  The new workload records every M6 (focused then full) and M7 (one repair
  batch) sample, keeping its refusal, flags and timings, and continues the run.
  `sail-bench report` applies the gate per catalogue in `docs/benchmarks.md`
  and lists every failing sample:
  - `SAIL7101`: an invocation timed out, failed to start or printed no JSON;
  - `SAIL7102`: the full evidence is not Current;
  - `SAIL7103`: M7 used other than one compiler driver;
  - `SAIL7104`: M7 is not a real engine batch;
  - `SAIL7106`: the outcome differs from the catalogue;
  - `SAIL7107`: the M6 and M7 outcomes differ;
  - `SAIL7108`: fewer than three complete rounds;
  - `SAIL7109`: the median whole-loop speedup is below 2×.

  Committed results are compact. A raw sidecar (`--raw`, by default under
  `--out`) holds arguments, full errors and canonical outcomes, and its sha256
  is recorded in the result. Use `--resume` to continue an interrupted run.
  See [M7 repair acceptance](docs/bench/m7-repair-acceptance.md).

- Add evidence commands for completed jobs: `sail explain`, immutable named
  baselines (`sail baseline save|list|get`) and `sail compare` against an
  earlier job or baseline, plus the MCP tools `explain_failure`,
  `save_baseline`, `list_baselines`, `get_baseline` and
  `compare_verification`. Comparisons label transitions as regressions only
  when contexts and scopes agree. Baselines are always historical and survive
  job pruning. Add a version-one proof-event journal (`sail jobs events`, MCP
  `verification_events`) with resumable cursors. Stock verification writes no
  proof events yet, so a finished stock job returns an empty terminal page
  with an explicit `SAIL0903` gap until the optional interactive engine lands. The job endpoint socket is now
  `jobs-v1-proof-v1-*.sock`; a running older job service is not reused. See
  [workspace verification](docs/workspace-verification.md#evidence-from-completed-jobs)
  and [the MCP guide](docs/mcp.md#evidence-tools).

- Shared HTTP cache: move trust and configuration filesystem work onto owned
  helpers, so status and lookups never block on a stalled mount (#76). Start a
  configured HTTP client asynchronously when the daemon binds, let lookups wait
  for that setup within the existing 250 ms budget, and deliver verified replies
  directly to concurrent readers. Reuse an unchanged loopback connection pool on
  reload, read exact small bodies without an extra EOF read, and reject a
  private CA file that contains no PEM certificate.
  Lookup deadlines, signature and trust checks and the local-solver
  fallback are unchanged. Development and test builds optimize
  `curve25519-dalek` arithmetic; release builds are unchanged. See
  [the HTTP shared cache guide](docs/http-shared-cache.md).

- Add explicit managed workspace verification with complete per-target reports,
  successful proof receipts, and verification-context isolation. Linux supports
  precise missing-configuration observation; macOS remains conservative and may
  reverify after unrelated directory activity.
- Add persistent local verification jobs, background submission, status/results,
  cancellation, diagnostics, and workspace-aware watch and MCP job tools.
- Add optional authenticated HTTP shared-cache reading and signed publication,
  private connection settings, and a deployable local/private team server.
- Record complete frontend-reuse acceptance, full local conformance and measured
  freshness costs. Preserve conservative macOS currentness and explicit bounded
  failures; default verification and existing SMT verdict policy stay unchanged.


- Add feature-scoped Singular source builds, validated runtime identities, and
  an optional local conformance environment for integer-ring examples and the
  pinned memory allocator.

- Keep portfolio mode and seed provenance in compact MCP result lookups and
  summaries. Unknown summaries no longer infer an unobserved resource failure.

- Failing crates with incomplete attribution no longer confirm successful
  timing entries that may hide bit-vector failures (VI-0003). They report
  uncertainty while preserving Verus's raw fields and diagnostics. MCP retires
  old persisted result records; reverify to establish current results.

- `sail bench` runs the installed workload tool with unchanged arguments and
  standard streams.

- Fresh fragility measurements force reverification in a locked Sail-owned
  target, preserving the caller’s Cargo artifacts and dependency compilation.

- Join fragility captures to functions by exact transcript hashes and check
  order; expose conservative function scores in JSON and MCP on request.
- Require reports for every requested verifying Cargo target before accepting
  a portfolio proof; reject ambiguous or unsupported target coverage.

- Exercise a real seeded proof, cold and warm, in the fast end-to-end check.
  Bound daemon readiness to sixty seconds and fail early when the daemon exits.

- Exclude skipped timing successes from function-scoped reports and portfolio
  acceptance (pinned Verus issue VI-0009).

- Explicit complete-run seed portfolio mode, retained attempt evidence, MCP
  selection, and isolated protocol-v4 cache namespaces (ADR 0020).

- Avoid adding a second progress-suppression flag to the deep suite example
  that supplies it itself. Verification arguments remain identical on both sides.

- Exclude overlapping deep suites and source rebuilds with exclusive build
  ownership; reject concurrency before materialized input cleanup.

- Deep tier: Verus's own `rust_verify_test` suite (4,695 tests) runs
  plain and through `sail-z3`, test by test (ADR 0019).
  - `cargo xtask build-verus-source` builds the pinned Verus commit from
    source with vargo, the pinned z3 and toolchain, under
    `<cache>/verus-src-build/<commit>`, and prunes it to what the tests
    need.
  - `sail-conformance rust-verify-test [--filter S]... [--every N]
    [--jobs N]` compares outcomes, exit codes and diagnostics, classifies
    plain-Verus flakiness separately, and exports a reproduction for each
    difference.
  - `ci/run.sh deep-rvt [cache]` (local-only) runs both, optionally also
    with the cache daemon.

- Preserve solver stderr during cached verification. Publish replies only after
  a clean, silent session completes; warning sessions always re-solve. Refuse
  older daemon and cache formats that could suppress diagnostics.

- Required `CI completion` rejects failed, cancelled and skipped gates,
  including CI image failures. Branch protection requires an updated branch;
  every CI job now has at most ten minutes.

- A shared cache tier (ADR 0017, `docs/trust.md`). Its entries are signed
  with ed25519 and verified before use. Each signature binds the key, the
  reply hash, the command, the solver identity, the platform class, the
  effort and the format version.
  - `sail serve --shared DIR` (`SAIL_SHARED_STORE`) consults the store on
    local misses, with a 250 ms timeout (`SAIL_SHARED_TIMEOUT_MS`) and a
    back-off after a timeout. Verified hits are copied locally with
    provenance `shared:<key id>`. Every failed check is a miss. Entries
    for another solver or platform class are refused and counted.
  - `sail cache keygen --out FILE`, `sail cache trust add|list|remove`,
    and `sail cache publish --to STORE --key FILE [--filter ...]
    [--dry-run]`. Only local pinned-solver entries are published: never
    shared copies, and never `unknown`s the serving policy would not serve.
  - `STATS` gains `shared_*` counters and `INFO` a `shared` section;
    `sail status` shows both. The protocol version is unchanged.
  - The daemon now records each session's solver identity in the entries it
    stores. Entries stored earlier are not published.
- `sail mcp`: an MCP server on stdio (`docs/mcp.md`, ADR 0016). Its tools:
  - `verify`: a crate, module or function, through `sail verify --output
    json`;
  - `counterexample`: models and diagnostics from the latest run;
  - `cached_result`: the latest known verdicts, with when and how they were
    established and whether the sources changed since;
  - `fragility`: opt-in, exact captured-query replay with conservative
    per-function scores and explicit unavailable attribution.

  Calls can be cancelled. Results are kept per project under
  `<cache>/mcp`.
- `sail verify --output json`: one schema-v1 JSON document on stdout
  (`docs/output-schema.md`, ADR 0014). It has per-function verdicts with tier
  and provenance, and per-obligation results with stable identities, effort,
  cache sources and counterexample models. It also carries Verus's
  diagnostics as data. stderr and the exit code are unchanged.
- `sail-z3`: with `SAIL_PROVENANCE_DIR` set, a cached session records how many
  replies the cache served (ADR 0014).
- `sail status [--json]`: the daemon's versions, uptime, cache root, log,
  disk and memory tiers, write queue and counters. Exits 4 (`SAIL0401`)
  when no daemon is listening (ADR 0015).
- `sail cache stats|verify [--remove]|gc --max-bytes N|clear --yes`, on the
  daemon's cache root or `--cache DIR` (ADR 0015).
- Daemon: an in-memory tier in front of the disk tier
  (`--memory-bytes`, `SAIL_DAEMON_MEMORY_BYTES`, default 256 MiB). `PUT`s
  are written behind, through a bounded queue drained on shutdown. The
  daemon stops after 30 minutes without connections
  (`--idle-timeout`, `SAIL_DAEMON_IDLE_TIMEOUT`), logs to
  `<cache root>/daemon.log`, stops cleanly on SIGINT, SIGTERM and SIGHUP,
  and removes its socket on exit.
- Daemon protocol: an `INFO` operation; `STATS` reports `memory_hits` and
  `dropped_puts`. The protocol version is unchanged (ADR 0015).
- `sail watch [ARGS...]`: re-runs `sail verify` as sources change. Edits
  cancel the run in flight and restart it once they settle.
- `sail fragility`: opt-in fragility measurement. Runs `cargo verus verify`
  once through `sail-z3` in pass-through with tracing, then re-runs every
  captured solver session under N configurations of the same pinned Z3
  (reseeds, and optionally semantics-preserving assertion shuffles), and
  reports per-check answer and effort distributions, the share of the
  rlimit used, stability classes and scores (`--json` for the full
  report; `--output json` also works). Partial selections run
  `cargo verus focus`. It changes no verdict and never touches the cache
  (ADR 0018). The MCP `fragility` tool now runs it.
- `sail-bench fragility`: the fragility census over corpus units, written
  to `bench/results/fragility-census-*.json` and rendered by
  `sail-bench report` (ADR 0018).
- `sail verify`: runs `cargo verus verify` with the same arguments, through
  `sail-z3` and the cache daemon (started on demand).
- `sail-bench warm` and `sail-bench edit`: the warm (no change) and
  single-function body and specification edit workloads, each run against a
  fresh `sail serve` with conformance checked on every sail run. Edits come
  from `corpus/edits.toml`. `sail-bench report` renders both (ADR 0012).
- Daemon protocol: a `STATS` operation reporting the daemon's counters
  (ADR 0012).
- `sail serve`: the daemon, serving a local disk cache over a Unix socket.
  `sail-z3` with `SAIL_DAEMON_SOCKET` set serves replies from the cache and
  starts the pinned Z3 only on a miss, replaying the whole session prefix
  (ADR 0011).
- `SAIL_DAEMON_TIMEOUT_MS`: the shim's daemon client timeout (default
  10,000 ms).
- `sail-z3` in cached mode exits when its solver dies, as pass-through does,
  instead of waiting for stdin to close. A reply cut short by the solver's
  death is never stored. Fault-injection tests: `docs/fault-injection.md`.
- Workspace skeleton, CI fast tier and AI-authorship guard.
- `sail-z3`: a pass-through Z3 shim for Verus (`VERUS_Z3_PATH`), with an
  optional trace tap (`SAIL_TRACE_DIR`).
- `cargo xtask fetch-verus`: installs the pinned Verus release.
- `sail-bench profile|seeds|history`: query-time profile, seed stability and
  history-dependence analyses over shim traces.
- `sail-conformance`: `cargo-verus` corpus units (with Verus dependencies
  patched to the pinned source in a scratch copy), project requirements
  (skipped and reported when unavailable), and the standard-tier corpus.
