# 0016: The MCP server (`sail mcp`)

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter asks for an MCP server that exposes four things to agents that
verify after every edit:
- verification of a crate, a module or a function;
- fragility measurement;
- cached result lookup;
- counterexample retrieval.

`sail verify --output json` (ADR 0014) already produces everything a
verification tool needs to return, as one schema-v1 document. `sail-mcp` is
permitted to depend only on `sail-api` (`xtask/src/layering.rs`).
`sail fragility` is being built in parallel and is not on `main`.

**The protocol** (checked 2026-10-08):
- The current MCP revision is `2026-07-28`. It is stateless: there is no
  `initialize` handshake. Every request carries its protocol version in
  `_meta`. Servers must implement `server/discover`, and reject an unknown
  version with `UnsupportedProtocolVersionError` (`-32022`).
- Revisions `2025-11-25` and earlier ("legacy") open with `initialize` and
  `notifications/initialized`.
- The specification allows a server to support both eras.
- Deployed clients still use the legacy handshake.
- `tools/list` and `tools/call` are the same in both eras, apart from the
  modern `resultType` field.

## Decision

**Protocol.**
- `sail-mcp` implements newline-delimited JSON-RPC 2.0 over stdio itself,
  with `serde_json`. There is no MCP SDK.
  - The surface needed is small: about a dozen message shapes.
  - The Rust SDKs pull in an async runtime and macro machinery. That is
    more dependency and more `deny.toml` surface than the protocol, and
    the protocol is moving faster than those SDKs.
  - Framing and dispatch are about 300 lines, with unit tests.
- **Dual-era.**
  - `initialize` negotiates a legacy revision (`2024-11-05` to
    `2025-11-25`), echoing the client's if known.
  - Modern requests are served statelessly. Their `_meta` version is
    checked, and `server/discover` is implemented.
  - Results carry `resultType: "complete"`, which legacy clients ignore.
- No resources or prompts. Tools are the whole surface.
- Tool calls run on threads, so `ping` and `notifications/cancelled` are
  served during a run. A cancelled call's process group is killed, and the
  call gets no response, as the specification asks.

**Verification by subprocess.**
- Every tool that verifies runs the `sail` binary (`sail verify --output
  json …`) in the project directory, in its own process group.
- `sail-mcp` links only `sail-api`, for the report types. The layering
  table is unchanged.
- The CLI passes the server its own executable, a store directory and the
  working directory. The server inherits `sail verify`'s behaviour exactly,
  including the daemon, the fallback and attribution. It cannot drift from
  the CLI.

**Tools.**
- **`verify`** takes `project_dir`, `package`, `module`, `function`,
  `cargo_args` and `verus_args`.
  - `module` and `function` become Verus's `--verify-module` and
    `--verify-function`, with `--verify-root` when no module is given:
    Verus requires one of the two.
  - It returns the report unchanged, plus a short summary and run metadata.
- **`counterexample`** and **`cached_result`** read the latest recorded
  results and never re-run.
  - Functions match by full path, or by a unique `::` suffix.
  - Obligations match by the ADR 0014 identity.
  - Diagnostics are attributed to a function by location: from its first
    obligation span up to the next function's. Verus gives no end span.
- **`fragility`** probes `sail --help` for a `fragility` command on every
  call.
  - When it exists, the tool runs `sail fragility --output json` with
    `verify`'s arguments. That argument vector is one constant, so adapting
    to the final interface is a one-line change.
  - Until then the tool returns `isError: true` with
    `available: false`.

**Rule 7.**
- Summaries and lookups present a function by its effective verdict:
  `provisional` whenever the tier is provisional, whatever the verdict field
  says.
- `unknown` is described as "Z3 gave up near its limit; not a settled
  failure".

**The result store.**
- Each `verify` that produces results is recorded per canonical project
  directory. A record holds the report, the finish time, the arguments, the
  exit code, and a SHA-256 snapshot of the sources taken before the run.
- The snapshot covers `*.rs` and `Cargo.toml` under the project, skipping
  `target/` and hidden directories. It leaves out `Cargo.lock`: a first build
  writes it, which would make the run look edited while it ran.
- Each function points at the latest run that reported it.
  - Scoped runs merge.
  - Unscoped runs replace, so deleted functions are forgotten.
  - Runs nothing points at are dropped.
- **Where.** The store is kept in memory, and written atomically to
  `<cache>/mcp/<sha256(project)[..16]>.json`.
  - This lets results survive a restart, and be shared between servers.
  - Failures to read or write the file are ignored and reported. The
    store never decides a verdict.
- **Staleness.** A lookup compares the run's snapshot with the sources now.
  - Any difference, or a change during the run, makes the result `stale`,
    with the changed files listed.
  - Content hashes are used, not modification times. A `touch` (which the
    e2e step uses to make Cargo re-verify) does not make a result stale.
- A run with no crates in its report is not recorded. Either Cargo skipped
  Verus (VI-0005), and the tool points at the latest recorded run, or the
  build failed first, and the tool says so.
- Runs in one project are serialised. Lookups wait for a run in progress in
  their project, so a pipelined `verify` then `counterexample` is
  consistent.

**Registration.** `sail mcp` is a stdio command with no arguments
(`docs/mcp.md`).

**Testing.**
- Unit tests cover framing, protocol errors, version negotiation, argument
  validation, the store, staleness, summaries and cancellation.
- `crates/sail-cli/tests/mcp.rs` drives `sail mcp` over stdio through every
  tool. It uses a stand-in `cargo-verus` that replays the ADR 0014
  attribution fixture. This test is in the fast gate.
- `ci/run.sh e2e` makes one real `verify` call on the e2e demo project
  (`examples/check_mcp.rs`). It checks that the call's verdicts equal
  `sail verify --output json`'s, and that its report passes
  `check_output` against Verus's own JSON. This adds seconds.

## Consequences

- **Agents get verification as tool calls.** Per-function verdicts,
  obligation ids and counterexample models come back as data, with
  provenance and staleness stated.
- **The server is as correct as `sail verify`,** because it is
  `sail verify`. Each call pays a process start and a `cargo metadata`, which
  is small next to Verus.
- **Speculative (provisional) results are not produced yet.** The tier is
  carried through, and rule 7 is enforced in presentation.
- **Fragility lights up when `sail fragility` lands,** if it accepts
  `--output json` and `verify`'s arguments. Otherwise `FRAGILITY_COMMAND`
  changes.
- **Revisit** if:
  - a stable, lightweight Rust MCP SDK appears;
  - clients need resources (for example the report as a resource) or
    progress notifications for long runs;
  - legacy clients disappear.
