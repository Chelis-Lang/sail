# sail-cache

In-memory, local-disk and signed shared solver-reply caches, with integrity checks and maintenance operations. Invalid cache entries are treated as misses, leaving callers responsible for executing the solver safely.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo install sail-solve --version 0.2.0-rc.1 --locked
```

This first crates.io release is a prerelease; full release qualification continues.
See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Shared cache guide](../../docs/http-shared-cache.md) · [Rust API reference](https://docs.rs/sail-cache/0.2.0-rc.1/)

