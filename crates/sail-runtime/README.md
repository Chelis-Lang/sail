# sail-runtime

Bounded exact SMT execution for embedded and service clients, independently usable without a daemon. Execution requires an installed solver matching the embedded pin; constructing the runtime does not install tools. Temporal execution is unavailable in the default core build.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo install sail-solve --version 0.2.0-rc.1 --locked
```

This first crates.io release is a prerelease; full release qualification continues.
See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Runtime guide](../../docs/runtime.md) · [Rust API reference](https://docs.rs/sail-runtime/0.2.0-rc.1/)

