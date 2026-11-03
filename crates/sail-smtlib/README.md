# sail-smtlib

Byte-oriented SMT-LIB tokenization, parsing and incremental command/response splitting. Spans refer to the original bytes; this crate does not normalize solver input.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
```

See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Trust model](../../docs/trust.md) · [Rust API reference](https://docs.rs/sail-smtlib/0.2.0/)

