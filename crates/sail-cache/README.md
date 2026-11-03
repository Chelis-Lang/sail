# sail-cache

In-memory, local-disk and signed shared solver-reply caches, with integrity checks and maintenance operations. Invalid cache entries are treated as misses, leaving callers responsible for executing the solver safely.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
```

See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Shared cache guide](../../docs/http-shared-cache.md) · [Rust API reference](https://docs.rs/sail-cache/0.2.0/)

