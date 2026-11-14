# sail-portfolio

Observation and fragility measurement across captured solver sessions and selected configurations. This crate reports measurements; it does not change verification verdicts or default-mode cache keys. Solver racing is deferred.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
```

See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Documentation index](../../docs/README.md) · [Rust API reference](https://docs.rs/sail-portfolio/0.2.0/)


The `profile` module prepares an exact borrowed session prefix through one
selected check for future diagnostic replay. It preserves every earlier byte
and check, refuses unclassified/incomplete or exited histories, and adds no
execution or proof/cache authority. The profile CLI and trace worker remain
planned; see ADR 0138 and `docs/validation/profile-session-prefix.md`.
