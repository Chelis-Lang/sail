# sail-daemon

Sail's long-lived local service, including cache protocol handling and runtime request dispatch. Service state and cached replies support verification clients without turning provisional evidence into confirmed verdicts.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo install sail-solve --version 0.2.0-rc.1 --locked
```

This first crates.io release is a prerelease; full release qualification continues.
See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Runtime guide](../../docs/runtime.md) · [Rust API reference](https://docs.rs/sail-daemon/0.2.0-rc.1/)

