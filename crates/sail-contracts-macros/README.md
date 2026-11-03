# sail-contracts-macros

Procedural macros implementing runtime expansion of Sail's supported neutral Rust contracts. Applications normally use the attributes re-exported by `sail-contracts`. Runtime checks are not SMT verification evidence.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo install sail-solve --version 0.2.0-rc.1 --locked
```

This first crates.io release is a prerelease; full release qualification continues.
See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Neutral contracts guide](../../docs/neutral-contracts.md) · [Rust API reference](https://docs.rs/sail-contracts-macros/0.2.0-rc.1/)

