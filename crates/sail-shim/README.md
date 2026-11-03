# sail-shim

The Z3-compatible solver shim used by Sail's verification path. It forwards the solver byte stream and can consult Sail's cache service; service failures fall back to the pinned Z3. The shim requires the matching solver and Sail configuration.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
```

See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Trust model](../../docs/trust.md) · [Rust API reference](https://docs.rs/sail-shim/0.2.0/)

