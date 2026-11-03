# sail-contracts

Neutral Rust contract attributes and checked scalar runtime support. Preconditions and postconditions are checked in release builds too. Passing runtime checks is not SMT verification evidence; verification requires a separate supported Sail workflow.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo install sail-solve --version 0.2.0-rc.1 --locked
```

This first crates.io release is a prerelease; full release qualification continues.
See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Neutral contracts guide](../../docs/neutral-contracts.md) · [Rust API reference](https://docs.rs/sail-contracts/0.2.0-rc.1/)

