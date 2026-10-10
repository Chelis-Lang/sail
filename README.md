# sail

**S.A.I.L.** (Satisfiability Analysis & Inference Layer) is a Rust service and toolchain that sits between the [Verus](https://github.com/verus-lang/verus) verifier and its pinned Z3. It caches solver replies and complete target proofs, with durable verification jobs for developers and agents. Default-mode correctness follows the pinned verifier and the charter’s policy for unstable queries; verdict-changing portfolios are explicitly opt-in.

Status: **live workspace verification and team caching implemented.**
Complete proof receipts, durable jobs/watch/MCP and authenticated HTTP caching
pass the retained acceptance in [the M6 report](docs/reports/2026-10-10-live-workspace.md).
The report records performance costs, platform limits and emulation failures.
See `docs/ROADMAP.md` for follow-up work.

## Using it

```sh
cargo build --release -p sail-cli -p sail-shim   # builds `sail` and `sail-z3`
sail verify [ARGS...]    # instead of: cargo verus verify [ARGS...]
```

`sail verify` runs your own `cargo verus verify` with the same arguments.
Its only change is putting `sail-z3` in front of the Z3 your Verus already
uses. It starts the cache daemon (`sail serve`) on demand. To switch back,
run `cargo verus verify` again.

The managed workflow adds complete incremental reports and durable local jobs:

```sh
sail verify --incremental --output json  # complete reports, including reused targets
sail verify --background                # submit and keep working
sail jobs list                         # status, result, logs and cancel also available
sail watch                            # follow workspace and path-dependency edits
```

One verifier worker runs by default. Completed target reports record which run
established them; current execution counters stay separate. Ordinary local use
needs no server configuration or credentials. Frontend receipt reuse targets
Linux; macOS uses conservative currentness checks that can require fresh proofs.
See [live workspace verification](docs/workspace-verification.md).

An optional source engine adds a repair workflow for one exact function:

```sh
cargo xtask build-verus-engine                       # local pinned source build
sail engine install --from PATH --output json       # validated build directory
sail repair-check --package demo --lib --function demo::arith::lemma \
  --engine interactive --background --output json
sail jobs events JOB --follow --output jsonl
sail explain JOB --package-id PACKAGE_ID --target demo --kind lib \
  --function demo::arith::lemma --output json
sail baseline save before-repair --job JOB
sail compare LATER_JOB --baseline before-repair --output json
```

The focused result is provisional. Acceptance requires independent, complete
full-target verification after compiler completion, with Current input evidence.
CLI and MCP share resumable proof events, failure evidence and immutable
baselines. The M7 implementation is undergoing source and real-project acceptance;
see [proof repair](docs/proof-repair.md) and
[the frozen acceptance workload](docs/bench/m7-repair-acceptance.md).

`sail verify --output json [ARGS...]` also prints one JSON document on stdout.
It holds per-function verdicts, with tier and provenance, and per-obligation
results with stable identities, effort and counterexample models. Diagnostics
still go to stderr unchanged. See `docs/output-schema.md`.

The daemon keeps replies in memory in front of the disk cache, stops after
30 minutes without connections or active jobs, and logs to `daemon.log` in the
cache root.
`sail status` shows its state and counters. `sail cache stats|verify|gc|clear`
inspects, checks, shrinks or empties the cache (ADR 0015).

Teams and CI can share results through a shared store: a directory such as
an NFS mount or a synced folder. Entries in it are signed, and a machine
serves one only if a key it trusts signed it for the same solver binary and
platform (`docs/trust.md`, ADR 0017):

```sh
sail cache keygen --out ci.key                          # once, where releases are made
sail cache publish --to /shared/sail --key ci.key       # after a CI run fills the cache
sail cache trust add ci.key.pub                         # on each reader
SAIL_SHARED_STORE=/shared/sail sail verify [ARGS...]    # or: sail serve --shared DIR
```

An optional HTTP server shares the same signed entries over HTTPS:
`sail cache serve`, `sail cache connect`, and `sail cache publish --to URL`.
Readers configure signing trust separately from access credentials. See
[HTTP team caching](docs/http-shared-cache.md) for a local trial and deployment.

`sail mcp` is an MCP server on stdio for agents that verify after every
edit. Its tools are `verify` (a crate, module or function, returning the
JSON document above), `counterexample`, `cached_result` (latest verdicts,
saying whether sources changed since) and `fragility` (local solver replay measurement).
The asynchronous tools `start_verification`, `verification_status`,
`verification_result` and `cancel_verification` use the same durable jobs as
the CLI, so an agent can inspect results while verification continues.
Register it as the stdio command `sail mcp`, for example
`claude mcp add --transport stdio sail -- sail mcp`. See `docs/mcp.md`.

- `docs/CHARTER.md`: mission, scope and the correctness contract.
- `AGENTS.md`: how to work in this repo (humans and agents).
- `docs/decisions/`: architecture decision records.
- `docs/survey/`: research on Verus internals, prior art, the corpus and solver behaviour.

## Getting started

```sh
cargo xtask install-hooks   # once per clone: the commit-msg authorship guard
cargo build --workspace
cargo nextest run --workspace
```

The toolchain is pinned in `rust-toolchain.toml`, and rustup installs it automatically.

## Portfolio and fragility

An explicit seed portfolio can try independent complete verification runs:

```sh
sail verify --portfolio accept-first-proof --portfolio-seeds 4 --output json
```

The baseline runs first. The first complete proof wins; function results from
separate attempts are never combined. Attempts and their diagnostics are retained
under the cache's `portfolio/` directory. This can take substantially longer and
is local-only. Every requested verifying Cargo library or binary must report
its result. Use exact package selectors; ambiguous crate names and test,
example or benchmark target scopes require an explicit supported selection.
See `docs/divergences.md` and ADR 0020.

Projects invoked directly through Verus can use the same portfolio with
`--verus-source path/to/lib.rs -- <Verus flags>`. The pinned Verus launcher
runs the complete requested scope independently on each attempt; Cargo target
cleanup applies only to Cargo projects.

For function scores in a verification report, request measurement explicitly:

```sh
sail verify --measure-fragility --fragility-seeds 8 --output json
```

This runs baseline verification without cached replies, then replays its exact
solver sessions. It can be expensive and is local-only; ordinary verification
runs no census. Incomplete or ambiguous attribution is reported as unavailable.

Explicit fragility measurements reverify in a Sail-owned Cargo target, reusing
dependency compilation between requests. They never clean the caller’s target.
Do not combine measurement with an external `--target-dir` or a portfolio.

`sail bench` forwards workload arguments to `sail-bench` installed beside it;
build both with `cargo build --release -p sail-cli -p sail-bench -p sail-shim`.
Benchmarks remain local-only and never form part of the CI gate.

## Optional Singular validation

Run `ci/local-singular.sh` locally to build the pinned Verus source with
Singular support and compare the tagged integer-ring examples and memory
allocator in pass-through, cache-fill and cache-warm modes. The separate image
pins the Singular package; its source build and cache use separate volumes.
This long validation is excluded from CI. To select an already built source
variant explicitly, set `SAIL_VERUS_SOURCE_BUILD` to its build directory; the
runner checks its commit, feature set, toolchain and runtime hashes (ADR 0021).
Singular obligations are tagged in the corpus and execute through Verus; the
Z3 cache only answers Z3 queries.

Project decisions are indexed in [docs/decisions/README.md](docs/decisions/README.md);
the completion evidence is in [the initial-product report](docs/reports/2026-10-09-initial-product.md).
