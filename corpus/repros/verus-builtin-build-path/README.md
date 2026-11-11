# Builtin definition paths in Rust diagnostics (#710)

The pinned stock Verus release and Sail's fork both reject this type error,
but the `function defined here` note names the source directory recorded when
their builtin library was compiled. The runtime archives record different
builder directories. This is a default-diagnostic difference, not an allowed
divergence or a solver disagreement.

Run each binary against the same source path, from the same directory:

```sh
"$STOCK_VERUS" corpus/repros/verus-builtin-build-path/repro.rs >stock.stdout 2>stock.stderr
"$FORK_VERUS" corpus/repros/verus-builtin-build-path/repro.rs >fork.stdout 2>fork.stderr
cmp stock.stdout fork.stdout
cmp stock.stderr fork.stderr
```

Both compiler invocations must exit 1 with `error[E0308]`. The issue is the
builtin definition note, not this intentional rejection. Preserve the original
streams; do not normalize their paths before comparison. This reproduction is
not yet in a conformance tier: the current selected runtime still differs.

Pins: Verus `0.2026.10.18.7d82994`, fork `a0e11cff`, pinned Z3 4.16.0.
Native Mac evidence and the unselected build-time correction are in the
[validation record](../../../docs/validation/builtin-source-paths.md).
