# Cargo-Verus no-vstd metadata and launcher macro mode

Pinned source commit: `426d8b01e7ffb910a36c15e1f870ff985bb61eef`.
Tracked as [VI-0011](../../../docs/verus-issues/VI-0011-cargo-no-vstd-launcher-mode.md).

`input.rs` needs the builtin and builtin-macro crates, not vstd. The generated
Cargo package declares both `verify = true` and `no-vstd = true` in
`[package.metadata.verus]`. Run against a complete source checkout and an
installed Cargo-Verus for that pin:

```sh
python3 corpus/repros/verus-cargo-no-vstd/reproduce.py \
  --source PINNED_SOURCE_CHECKOUT --cargo-verus INSTALLED_CARGO_VERUS \
  --out REPRO_OUTPUT
```

The reproduction runs ordinary full `cargo verus verify`; it does not use
`cargo verus focus`, `--no-verify`, or an environment workaround. Its owned
process group has a 120-second invocation bound, including dependency builds.
A timeout is infrastructure failure, not reproduction success.

The captured Linux x86-64 run under emulation returned 101 in 69.72 seconds.
It used the manifest-validated Sail source variant recorded in
`observed-summary.json`, without enabling interactive engine execution. The
launcher code responsible for the mismatch is unchanged pinned source. The
stdout reports an entire-crate verification attempt, encountered compiler error,
and zero verified functions; the stderr contains E0433 for missing vstd. A
second cached-dependency probe with `assert(true)` and a Boolean body assertion
also failed E0433 in 6.87 seconds. This is not a claim that a native 120-second
ordinary-vstd acceptance run cannot fit its budget.

`observed.stdout.json` retains the original stdout. `observed.stderr` substitutes
only the machine-local source and scratch directory prefixes; original byte
hashes remain in `observed-summary.json`. The captured macro errors arise before
any SMT proof completes. No source-engine or production workaround is applied.

The optional engine's adapter reflects the same effective exact `--no-vstd`
option into literal launcher arguments (VI-0011). A one-off check of that
adaptation against the real launcher with identical Cargo argument bytes found
identical diagnostics, function verdicts and full-target SMT input; its script
and record are archived outside the repository (see VI-0011).
