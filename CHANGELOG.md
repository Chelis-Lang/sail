# Changelog

All notable user-visible changes are recorded here. The project follows
semantic versioning once the API is published.

## Unreleased

- Require reports for every requested verifying Cargo target before accepting
  a portfolio proof; reject ambiguous or unsupported target coverage.

- Exercise a real seeded proof, cold and warm, in the fast end-to-end check.
  Bound daemon readiness to sixty seconds and fail early when the daemon exits.

- Exclude skipped timing successes from function-scoped reports and portfolio
  acceptance (pinned Verus issue VI-0009).

- Explicit complete-run seed portfolio mode, retained attempt evidence, MCP
  selection, and isolated protocol-v2 cache namespaces (ADR 0020).
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
  - `fragility`: reports "not available" until `sail fragility` exists.

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
