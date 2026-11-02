# sail-solver

Execution of the pinned Z3 on complete, exact input, with process and wall-time handling for measurement clients. Infrastructure failures are reported separately from solver answers. A warm solver pool is not implemented.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo install sail-solve --version 0.2.0-rc.1 --locked
```

This first crates.io release is a prerelease; full release qualification continues.
See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Trust model](../../docs/trust.md) · [Rust API reference](https://docs.rs/sail-solver/0.2.0-rc.1/)

