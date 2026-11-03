# sail-verus

Integration with Sail's pinned Verus fork, including toolchain identity, invocation and managed verification evidence. Verification requires the matching installed runtime; this library does not bundle the verifier executables.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
```

See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Verification guide](../../docs/workspace-verification.md) · [Rust API reference](https://docs.rs/sail-verus/0.2.0/)

