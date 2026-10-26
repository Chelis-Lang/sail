# VI-0036: Ghost-only environment reads are missing from the final dep-info

- **Status:** open in stock Verus and in the fork (tracked here; not filed
  upstream). Sail works around it (#386).
- **Verus:** `0.2026.10.18.7d82994` (pinned stock) and fork `36d65c3f`.
- **Found:** 2026-10-26, issue #386.
- **Reproduction:** [verus-ghost-env-dep-info](../../corpus/repros/verus-ghost-env-dep-info/)
  (`repro.sh CARGO_VERUS`).
- **Impact on Sail:** a receipt bound only the variables the final dep-info
  declares, so a proof established with a ghost-only variable set was reused
  as Current after the variable was removed, while a fresh verification
  fails.

## Description

Under Cargo, `rust_verify` runs rustc twice in one process: the verification
phase with `--cfg verus_keep_ghost` and the erased compilation without it.
Both write the crate's dep-info to the same path, and the second write
replaces the first (the same mechanism as
[VI-0005](VI-0005-forwarded-args-do-not-re-verify.md)). The erased phase never
expands macros inside ghost code, so an `env!` or `option_env!` read that
occurs only in a `proof fn`, a spec function or an assertion is not declared:

```rust
proof fn ordinary_flag_is_present() {
    assert(match option_env!("ORDINARY_PROOF_FLAG") { Some(_) => true, None => false });
}
```

1. `ORDINARY_PROOF_FLAG=7 cargo verus verify` verifies (exit 0). The final
   `.d` file declares only the erased phase's variables, not
   `ORDINARY_PROOF_FLAG`.
2. With the variable removed, the same command in the same target prints only
   `Finished` and exits 0: Cargo finds the crate fresh.
3. With the variable removed, a new target fails the assertion (exit 101).

Issue #386 records the same split from the fork's compiler-input report: the
verification phase tracks `ORDINARY_PROOF_FLAG=7`, the compilation phase only
`VERUSDOC`.

## Sail workaround

For every verified target the managed driver requests the fork's existing
compiler-input report (`SAIL_COMPILER_INPUTS`, which lists rustc's tracked
environment of both phases and needs `-Zchecksum-hash-algorithm`). After a
successful verification it appends each read missing from the dep-info, in
rustc's own form, and records the observed names in the invocation; a receipt
requires that record ([freshness](../freshness.md#which-environment-variables-are-proof-inputs)).
Variables the proof context already binds, including the forwarded driver
arguments that made VI-0005's fork fix rebuild every repair, are not appended.

## Fork

Not fixed. A fork fix would carry the verification phase's tracked
environment (without the forwarded-argument names) into the erased
compilation's dep-info. It needs a runtime re-pin; the Sail workaround needs
none.
