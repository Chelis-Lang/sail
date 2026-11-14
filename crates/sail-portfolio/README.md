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

The `quantifiers` module decodes bounded complete pinned-Z3 traces. It counts
E-matching/model-based discoveries and instances, preserving unknown or conflicting
fingerprint ownership explicitly. Its results describe that traced history only;
no solver/input authentication, source identity or proof/currentness follows.
The complete profile worker and CLI remain planned (ADR0144).

The diagnostic quantifier counter checks framing, supported identity fields and
resource bounds. Opaque trace bodies and quantifier binding cardinality are not
fully validated; complete attribution never authenticates a solver/session or
establishes a proof. Full profiling and its separate session binding remain work
in progress.
