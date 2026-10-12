# Managed Cargo feature alias parity

[Sail issue 78](https://github.com/Chelis-Lang/sail/issues/78) records a source-proven
metadata mismatch. The pinned
[Cargo feature argument definition](https://github.com/rust-lang/cargo/blob/5f94df4789f005f9a352888e8355ffc645b7ed0e/src/util/command_prelude.rs#L261-L273)
accepts `-F` as the short form of `--features`. Before this correction,
`input::metadata` forwarded only the long forms while verification received all
original Cargo arguments. `expected_targets` checks metadata resolve-node feature
lists against each target's required features; omitting this flag can therefore
remove a feature-gated target from Sail's expected set.

Minimal source fixture, saved as `Cargo.toml` with `main.rs` below:

```toml
[package]
name = "sail-feature-fixture"
version = "0.0.0"
edition = "2024"
[package.metadata.verus]
verify = true
[features]
enabled = []
[[bin]]
name = "sail-feature-fixture"
path = "main.rs"
required-features = ["enabled"]
```

```rust
fn main() {}
```

The relevant managed request is `sail verify --incremental -Fenabled`. Cargo
arguments go before the first `--`; the existing separator remains reserved for
verifier arguments. This fixture is a source reproduction, not an executed
current-product or wrong-verdict claim.

The correction factors the actual metadata command constructor and forwards
only the known `-F` forms unchanged. Draft controls inspect that exact constructor
for `-F enabled`, `-Fenabled`, `-F=enabled` and both original long forms, repeated
mixed selectors, manifest/locked flags, the first `--` cutoff and a missing value.
The request vector is unchanged, so proof/compiler context keys do not normalize
feature arguments. Native controls, strict and complete-product validation are
pending. This issue does not explain the separate b895 benchmark failure:
its original arguments use `--package` and `--lib`, without a feature alias.
