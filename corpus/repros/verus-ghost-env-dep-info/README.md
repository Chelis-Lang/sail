# Ghost-only environment reads are missing from the final dep-info

`lib.rs` reads `ORDINARY_PROOF_FLAG` with `option_env!` only inside a
`proof fn`. `repro.sh CARGO_VERUS [WORK_DIR]` creates a crate with
`cargo verus new --lib`, verifies it with the flag set, prints the final
dep-info's environment declarations, then verifies with the flag removed in
the same target and in a new one.

Observed 2026-10-26 with stock Verus `0.2026.10.18.7d82994` (the pinned
release in the CI image, `/opt/verus/verus-x86-linux/cargo-verus`) on native
x86-64 Linux:

```
1. flag present, new target:
   exit 0, assertion failures: 0
   final dep-info environment:
     # env-dep:VERUSDOC
2. flag removed, same target:
   exit 0, assertion failures: 0
3. flag removed, new target:
   exit 101, assertion failures: 2
```

Step 2 is stale: Cargo finds the crate fresh because the erased compilation
rewrote the dep-info without the verification phase's read. See
[VI-0036](../../../docs/verus-issues/VI-0036-ghost-env-reads-missing-from-dep-info.md).
