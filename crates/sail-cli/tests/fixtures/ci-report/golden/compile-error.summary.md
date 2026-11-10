## Sail verification: compilation failed before verification

**1 verified, 0 failed, 0 unknown; compilation failed before verification in 1 crate** in 2 requested targets.

Exit code 101 · 183.2 s · 3 verifier invocations · 0 targets reused from proof receipts · solver checks in this run: 1 from cache, 0 solved

> **Currentness unknown.** Sail could not establish that the inputs stayed unchanged during the run.

| Target | Result | Functions | Verified | Failed | Unknown | Evidence |
|---|---|--:|--:|--:|--:|---|
| alpha (lib) | compilation failed | 0 | 0 | 0 | 0 | did not compile |
| beta (lib) | passed | 1 | 1 | 0 | 0 | whole-target proof, not established for the current inputs |

### Compared with the baseline

Baseline revision `7e6f83cac1a1` (run `378015-1794262634477018790`); this run's revision `db3dd8b2ded3`.

**0 regressed, 0 became unknown, 0 recovered**; 0 added, 0 removed, 0 changed otherwise, 1 unchanged, 5 not observed in this run.

### Notes from Sail

- SAIL3146: instant revert did not apply because alpha last compiled in 1083 ms, less than 16 times its measured retention (9996 ms), so retention would cost more than it saves; verification is unaffected
- SAIL3114: inputs could not be confirmed unchanged; re-run for a current result
- Build-script input declarations were established during this run

### Verus diagnostics (1 error, 0 warnings)

Not listed: 1 closing compiler line(s) that restate the errors, and 0 top-level note or help block(s).

<details><summary>error: mismatched types (proofs/alpha/src/lib.rs:10:27)</summary>

```text
error[E0308]: mismatched types
  --> alpha/src/lib.rs:10:27
   |
 5 | pub fn max(a: u64, b: u64) -> (r: u64)
   |                                   --- expected `u64` because of return type
...
10 |     if a > b { a } else { "b" }
   |                           ^^^ expected `u64`, found `&str`
```

</details>

