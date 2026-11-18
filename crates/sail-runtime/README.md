# sail-runtime

Bounded exact SMT execution for embedded and service clients, independently usable without a daemon. Execution requires an installed solver matching the embedded pin; constructing the runtime does not install tools. Native temporal execution needs a linked `TemporalBackend` such as `sail-dive`'s, which the `sail` binary links.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
```

See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Runtime guide](../../docs/runtime.md) · [Rust API reference](https://docs.rs/sail-runtime/0.2.0/)

