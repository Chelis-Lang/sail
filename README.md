# sail

**S.A.I.L.** (Satisfiability Analysis & Inference Layer) is a Rust service and toolchain that sits between the [Verus](https://github.com/verus-lang/verus) verifier and its pinned Z3. Its goal is to return verification results as fast as possible, through caching, warm solver pools and function-level scheduling, without ever returning a result plain Verus would not have returned.

Status: **bootstrap (M0).** Nothing is usable yet. See `docs/ROADMAP.md`.

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
