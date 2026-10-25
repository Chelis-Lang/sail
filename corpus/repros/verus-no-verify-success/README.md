# Compile-only success is not proof evidence

Use the stock Verus runtime and its Rust/Z3 toolchains pinned by
`corpus/toolchain.toml`, directly rather than through Sail:

```sh
"$SAIL_STOCK_VERUS" corpus/repros/verus-no-verify-success/main.rs --output-json
"$SAIL_STOCK_VERUS" corpus/repros/verus-no-verify-success/main.rs --output-json --no-verify
```

The ordinary invocation fails the false assertion with exit 1, zero verified
checks and one error. The second invocation deliberately skips verification:
it exits 0, reports `success: true`, zero verified checks, zero errors,
`is-verifying-entire-crate: true` and an empty `func-details` object.

This is an option/observation boundary, not a demonstrated Verus translation
defect. A successful process and an entire-crate JSON flag do not establish
that verification was enabled. Sail must preserve that distinction before
minting receipt, whole-target or repair authority, including options injected
through effective package-specific driver arguments.

The source is nonempty and its assertion is false. This control must remain
distinct from a genuine successful whole crate containing zero verified
functions. See [VI-0035](../../../docs/verus-issues/VI-0035-no-verify-success.md).
