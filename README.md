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
