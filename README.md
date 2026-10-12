# sail

**S.A.I.L.** (Satisfiability Analysis & Inference Layer) is a Rust service and toolchain that sits between the [Verus](https://github.com/verus-lang/verus) verifier and its pinned Z3. It caches solver replies and complete target proofs, with durable verification jobs for developers and agents. Default-mode correctness follows the pinned verifier and the charter’s policy for unstable queries; verdict-changing portfolios are explicitly opt-in.

Status: **live workspace verification and team caching implemented.**
Complete proof receipts, durable jobs/watch/MCP and authenticated HTTP caching
pass the retained acceptance in [the M6 report](docs/reports/2026-10-10-live-workspace.md).
The report records performance costs, platform limits and emulation failures.
See the [documentation index](docs/README.md), [known issues](docs/known-issues.md)
and [roadmap](docs/ROADMAP.md) for guides, limitations and follow-up work.

## Using it

```sh
cargo build --release -p sail-cli -p sail-shim   # builds `sail` and `sail-z3`
export PATH="$PWD/target/release:$PATH"
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

## Experimental proof repair

The optional pinned source engine adds exact-function focus followed by independent
full-target validation. The selected source engine passes its memory and lowering
qualification. Current `387ba80` passes the [complete workspace gate (673 tests),
original solver suite (41 tests), product installations and original installed
CLI/MCP repair fixture](docs/validation/m7-compiler-script-bootstrap/source-inventory.json)
with unchanged checks. Named-baseline feature smoke passed separately on `57a319d`.
The [historical b895 full reader workflow](docs/validation/m7-compiler-script-bootstrap/scheduling-recovery.json)
passes both unchanged collectors: each retains 200 total samples across four
concurrent clients (100 Result and 100 Status reads), with complete Current
graphs and p95 below 200 ms, followed by retained CLI/MCP scenarios. The
[730 reader pass](docs/validation/m7-reader-730a321-terminal.md) remains a separate historical scope.
The original 387 reader workflow fails Python Result p95 (436.008 ms against
the unchanged 200 ms bound); Status p95 is 137.084 ms. All 200 samples retain
Current evidence and the same continuing verifier. Rust and retained CLI/MCP
checks are unattempted, so no current reader acceptance is claimed. Twofold
real-project repair-loop improvement and
complete standard/cache coverage remain outstanding;
M7 is not accepted and PR #74 remains draft. Stock verification stays the default.
See [current validation](docs/validation/m7-compiler-script-bootstrap/source-inventory.json) and
[known issues](docs/known-issues.md).

Optional ordinary-build recovery passes its local controls and installed repair
fixture. The earlier c2 [real-project benchmark](docs/validation/m7-benchmark-c2a4b23-warmup-terminal.md)
fails Current warmup admission despite exit-zero verification and produces no
measured samples. The recovered 387 benchmark also fails its original one-driver requirement:
its first measured GCD full result is Current/accepted but uses three compiler
invocations. Zero accepted samples are retained and Vest is unreached; no
real-project speedup or invocation cause is claimed.
The earlier b895 [whole benchmark](docs/validation/m7-compiler-script-bootstrap/scheduling-recovery.json)
also fails admission: a later repair verifies but its full evidence is Historical.
It has zero measured samples; the cause remains unestablished.
It can establish dependency contracts after initial regeneration, but never
promotes an Unknown proof or replaces full verification. Unsupported coverage
retains original cleaning. See [build recovery](docs/managed-cold-start.md).

From the Sail checkout, build/register the selected optional source variant with
`cargo xtask build-verus-engine`, then inspect `sail engine status --output json`.
Alternatively, `sail engine install --from ENGINE_BUILD_DIR --output json`
imports a recognized build after validating its provenance and artifact bytes.
A candidate directory or successful install is not M7 acceptance evidence.
If you override `SAIL_CACHE_DIR`, use one absolute value for engine installation,
project CLI commands and the MCP server; changing directories must not select
another cache. The walkthrough explains MCP job-read routing.

From a project, submit a library repair and retain the returned job ID:

```sh
sail repair-check --package demo --lib --function demo::arith::lemma \
  --engine interactive --background --output json
sail jobs events JOB_ID --follow --output jsonl
sail jobs result JOB_ID --output json
# Save only after the completed result has value.repair.accepted = true.
sail baseline save before-repair --job JOB_ID
sail baseline list
sail baseline get before-repair --output json
sail compare LATER_JOB_ID --baseline before-repair --output json
```

Submission success and provisional focus observations do not establish a proof.
Inspect `value.repair.accepted` in the completed result; acceptance requires
independent complete Current full-target success after compiler completion.
Keep redirected event logs and result files outside watched package roots while
verification runs; the walkthrough shows a separate capture directory. Saved
baselines retain establishing evidence and never prove today's inputs.
`baseline get` retrieves the complete Historical snapshot even after its job is
pruned; provisional reports remain provisional. See the [baseline guide](docs/mcp.md).
[The complete walkthrough](docs/proof-repair-walkthrough.md) includes a runnable
project, focus-pass/full-failure example, exact failure identities, event cursors,
cancellation and MCP calls. [The repair contract](docs/proof-repair.md) and
[frozen acceptance guide](docs/bench/m7-repair-acceptance.md) distinguish usable
workflow from unfinished qualification and performance claims.

## Reports and team caching

`sail verify --output json [ARGS...]` also prints one JSON document on stdout.
It holds per-function verdicts, with tier and provenance, and per-obligation
results with stable identities, effort and counterexample models. Diagnostics
still go to stderr unchanged. See `docs/output-schema.md`.

The daemon keeps replies in memory in front of the disk cache, stops after
30 minutes without connections or active jobs, and logs to `daemon.log` in the
cache root.
`sail status` shows its state and counters. `sail cache stats|verify|gc|clear`
inspects, checks, shrinks or empties SMT reply entries (ADR 0015). These commands
do not limit or remove managed compiler targets. Focused repair generations
trade additional dependency storage and cold setup for isolated certified
artifacts; their observation migration remains a source candidate with native
qualification pending. See [workspace hygiene](docs/workspace-hygiene.md).

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
Experimental repair tools add `start_repair_validation`, `verification_events`,
`explain_failure`, `compare_verification`, `save_baseline`, `list_baselines` and
`get_baseline`. Baseline tools expose the same immutable Historical evidence as
the CLI, independently of job pruning. Their separate installed
[feature smoke](docs/validation/m7-mcp-baseline-product-smoke-plan.md) passes on
`57a319d`, including overwrite refusal and full CLI/MCP snapshot equality.
Durable read-only lookups keep
bounded persistent clients while authorizing each read with a complete fresh
service reply; writes and verification lifecycle ownership retain their existing
paths. Register the stdio command `sail mcp`, for example
`claude mcp add --transport stdio sail -- sail mcp`. See [MCP setup and
schemas](docs/mcp.md), or [the persistent Rust SDK](docs/result-client.md).
Neither client decoding reuse nor offline timing grants proof authority or a
live-reader latency guarantee.

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

Retire completed local run scratch after verified evidence retention and holder
checks. The [workspace hygiene guide](docs/workspace-hygiene.md) describes the
automatic local-gate cleanup and separate reader/benchmark handoff.

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
