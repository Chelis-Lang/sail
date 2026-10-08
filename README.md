# sail

**S.A.I.L.** (Satisfiability Analysis & Inference Layer) is a Rust service and toolchain that sits between the [Verus](https://github.com/verus-lang/verus) verifier and its pinned Z3. Its goal is to return verification results as fast as possible, through caching, warm solver pools and function-level scheduling, without ever returning a result plain Verus would not have returned.

Status: **early.** Pass-through and local result caching work and pass the
conformance harness (`docs/reports/2026-10-08-bootstrap.md`). See
`docs/ROADMAP.md` for what comes next.

## Using it

```sh
cargo build --release -p sail-cli -p sail-shim   # builds `sail` and `sail-z3`
sail verify [ARGS...]    # instead of: cargo verus verify [ARGS...]
```

`sail verify` runs your own `cargo verus verify` with the same arguments.
Its only change is putting `sail-z3` in front of the Z3 your Verus already
uses. It starts the cache daemon (`sail serve`) on demand. To switch back,
run `cargo verus verify` again.

`sail watch [ARGS...]` re-verifies whenever sources change, cancelling the
run in flight. With the cache warm, only edited buckets are re-solved.

`sail verify --output json [ARGS...]` also prints one JSON document on stdout.
It holds per-function verdicts, with tier and provenance, and per-obligation
results with stable identities, effort and counterexample models. Diagnostics
still go to stderr unchanged. See `docs/output-schema.md`.

The daemon keeps replies in memory in front of the disk cache, stops after
30 minutes without connections, and logs to `daemon.log` in the cache root.
`sail status` shows its state and counters. `sail cache stats|verify|gc|clear`
inspects, checks, shrinks or empties the cache (ADR 0015).

`sail mcp` is an MCP server on stdio for agents that verify after every
edit. Its tools are `verify` (a crate, module or function, returning the
JSON document above), `counterexample`, `cached_result` (latest verdicts,
saying whether sources changed since) and `fragility` (once available).
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
