# Inserting an item renames closure types in other modules

**Found:** 2026-10-15, lane L40 (real-project performance); reduced
2026-10-16, lane M11-5.

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

## What happens

`before.rs` and `after.rs` differ only by one empty function added to module
`m1`. Module `m2`, which has one closure, is identical in both. `repro.sh`
verifies each with `--log smt` and compares `m2.smt2` without its `;;`
comment lines:

```
$ ./repro.sh <stock install dir>
m2 differs:
< (declare-sort anonymous_closure%9. 0)
> (declare-sort anonymous_closure%10. 0)
< (declare-const TYPE%anonymous_closure%9. Type)
< (declare-fun Poly%anonymous_closure%9. (anonymous_closure%9.) Poly)
```

Stock `0.2026.10.04.426d8b0` and the fork's runtime `73dea021` print this
(macOS arm64). With the fork fix (VI-0019, `8d03ab54`) it prints
`m2 identical`.

`renaming-equal.py` checks the fix's one allowed difference: two
`--log smt` directories of the same input, from the fork before and after the
fix, must be equal up to a one-to-one renaming of closure-type numbers
(and line wrapping).

## Impact

Sail's cache keys bind exact solver bytes, so every module that uses a
closure after the inserted item misses from its first check. See
`docs/verus-issues/VI-0019-crate-wide-closure-numbering.md`.
