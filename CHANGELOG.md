# Changelog

All notable user-visible changes are recorded here. The project follows
semantic versioning once the API is published.

## Unreleased

- Fall back to the stock verifier when an optional engine emits a terminal event
  without a complete structured full result, or terminates by panic or signal.
  Conservatively rerun compiler-error documents that can also hide caught panics;
  preserve failed engine output, stock diagnostics and actual fallback counts.

- Reuse run-local sysroot byte hashes on Linux only while kernel leases and
  precise namespace observation remain continuously owned from the first hash
  through receipt publication. Lost coverage forces fresh byte checks and
  unavailable current evidence.

- Validate optional-engine artifacts in the same byte-hashing phase as the
  verification context, retaining complete inventory and manifest checks before
  any engine executable starts.

- Retain optional per-driver preparation counters with separate batch-local and
  persistent reuse; unavailable telemetry is distinct from observed zero hits.

- Decline ambiguous trait-implementation obligation joins when Verus repeats a
  friendly function name; preserve full results and partial-attribution evidence.

- Add explicit optional-engine focused repair checks with independent full-target
  evidence, resumable scoped proof journals, exact failure explanations, immutable
  verification baselines and regression comparisons through CLI and MCP.

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
