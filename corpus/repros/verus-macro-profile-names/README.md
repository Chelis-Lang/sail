# Proc-macro build profile changes the fork's SMT names (VI-0033)

**Found:** 2026-10-19 (issue #247, Vest); reduced 2026-10-20.

**Versions:** fork `9c0ca7e7` (Verus `0.2026.10.18.7d82994`), Rust 1.99.1,
Z3 4.16.0.

## What happens

`lib.rs` has one closure and one trait impl, so its SMT input declares a
closure type and an aliased trait function. `repro.sh` creates a Cargo
package with path dependencies on a built Verus source tree, verifies it
with `cargo verus verify -- --log smt` with the proc-macro crates at
`[profile.dev.build-override] opt-level = 0` and then `3`, and compares the
two log trees:

```
$ ./repro.sh <fork 9c0ca7e7 source dir>
verification results:: 2 verified, 0 errors
verification results:: 2 verified, 0 errors
Files …/o0/log/vi_macro_profile/root.smt2 and …/o3/log/vi_macro_profile/root.smt2 differ
974c974
< (declare-const fuel%%sail%fn%…000000086e59fd94872e55a4…
> (declare-const fuel%%sail%fn%…000000089bad7d20fb2b9298…
```

Only the verified crate's log differs; vstd's 427 logs are equal. With fork
`912052eb` the script prints `SMT logs identical`. Stock Verus is not
affected (its names omit the stable crate ID).

The script needs a Verus source tree built with `vargo build --release`
(`target-verus/release`, `vstd`, `builtin`, `builtin_macros` and `z3`). Each
arm uses its own `CARGO_TARGET_DIR`, so the arms do not share artifacts.

## Impact

Cargo's metadata for every crate depending on the macros changes, so its
stable crate ID changes, and the fork spelled that ID into closure and alias
names. See `docs/verus-issues/VI-0033-crate-metadata-in-smt-names.md`.
