# Exported VIR names the local rust-src copy of the standard library

`lib.rs` gives a specification to one standard library type. Export it with
a stock Verus release, once on a toolchain without the `rust-src` component
and once with it:

```sh
rustup component list --toolchain 1.99.1 --installed   # no rust-src
VERUS/verus --crate-type=lib lib.rs --export out.vir
strings -a out.vir | grep 'library/core/src/marker.rs' | sort -u
rustup component add rust-src --toolchain 1.99.1
VERUS/verus --crate-type=lib lib.rs --export out.vir
strings -a out.vir | grep 'library/core/src/marker.rs' | sort -u
```

Without `rust-src`, `out.vir` names the type's source by the toolchain's
virtual path, `/rustc/58cf8f9c35b355199727627d07ae34a4e2846d67/library/core/src/marker.rs:1025:10`
(18 occurrences). With it, the same 18 spans name the host's copy,
`<rustup home>/toolchains/1.99.1-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/marker.rs:1025:10`,
and no `/rustc/` path remains. Release 0.2026.10.04.426d8b0 with Rust 1.98.1
behaves the same way. Verification succeeds in all four runs.

Observed 2026-10-19 on native EC2 Linux x86-64 with the stock
`0.2026.10.18.7d82994` and `0.2026.10.04.426d8b0` release archives. See
[VI-0031](../../../docs/verus-issues/VI-0031-rust-src-std-paths.md).
