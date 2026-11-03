# sail-daemon

Sail's long-lived local service, including cache protocol handling and runtime request dispatch. Service state and cached replies support verification clients without turning provisional evidence into confirmed verdicts.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
```

See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Runtime guide](../../docs/runtime.md) · [Rust API reference](https://docs.rs/sail-daemon/0.2.0/)

