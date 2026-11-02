# sail-session

An observational model of interactive solver sessions, including assertion stacks and solver options. Unsupported or malformed state changes mark snapshots inexact so consumers can decline reuse.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo install sail-solve --version 0.2.0-rc.1 --locked
```

This first crates.io release is a prerelease; full release qualification continues.
See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Trust model](../../docs/trust.md) · [Rust API reference](https://docs.rs/sail-session/0.2.0-rc.1/)

