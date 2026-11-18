# sail-dive

Native temporal jobs for Sail on the [D.I.V.E.](https://crates.io/crates/dive-api) TLA+ checker: exhaustive safety and temporal checking, seeded simulation and independent trace validation, run in the isolated `sail-runtime-worker` process. Every counterexample and sampled trace is replayed through D.I.V.E.'s independent trace validator before it is reported.

This component is part of [Sail](../../README.md). For the command-line tool,
install the `sail-solve` package:

```sh
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
```

See the [installation guide](../../docs/install.md) for runtime and toolchain setup.

[Temporal jobs guide](../../docs/temporal-jobs.md) · [Rust API reference](https://docs.rs/sail-dive/0.2.0/)
