# sail-verus

Integration with Sail's pinned Verus fork, including toolchain identity, invocation and managed verification evidence. Verification requires the matching installed runtime; this library does not bundle the verifier executables.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo install sail-solve --version 0.2.0-rc.1 --locked
```

This first crates.io release is a prerelease; full release qualification continues.
See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Verification guide](../../docs/workspace-verification.md) · [Rust API reference](https://docs.rs/sail-verus/0.2.0-rc.1/)

