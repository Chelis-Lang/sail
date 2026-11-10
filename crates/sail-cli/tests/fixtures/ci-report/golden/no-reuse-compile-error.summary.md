## Sail verification: compilation failed before verification

**compilation failed before verification** in 1 requested target.

Exit code 101 · 1.5 s · 1 verifier invocation · 0 targets reused from proof receipts

| Target | Result | Functions | Verified | Failed | Unknown | Evidence |
|---|---|--:|--:|--:|--:|---|
| cargo verus verify | compilation failed | 0 | 0 | 0 | 0 | did not compile |

### Compared with the baseline

Baseline revision `7e6f83cac1a1` (run `378015-1794262634477018790`); this run's revision not recorded.

**Not comparable:** a run without proof reuse (`SAIL3112`) binds no verification context to compare. The differences below are descriptive; none is reported as a regression.

| Function | Target | Change | Verdict |
|---|---|---|---|
| `alpha::add_one` | alpha | incomparable | verified → not observed |
| `alpha::call_add_one` | alpha | incomparable | verified → not observed |
| `alpha::count_to` | alpha | incomparable | verified → not observed |
| `alpha::lemma_double_monotone` | alpha | incomparable | verified → not observed |
| `alpha::max` | alpha | incomparable | verified → not observed |
| `beta::clamp` | beta | incomparable | verified → not observed |

- Requested scope, mode or runtime context differs; transitions are descriptive, not proof regressions.

### Notes from Sail

- SAIL3112: no proof reuse, because LD_LIBRARY_PATH is set, which a proof receipt cannot bind (unset it to reuse proofs); verified as plain `cargo verus verify`, with the solver cache. Nothing was reused or recorded.
- SAIL3112: LD_LIBRARY_PATH is set, which a proof receipt cannot bind (unset it to reuse proofs); verified as `cargo verus verify` does, in the invocation's own Cargo target, with Sail's solver cache; no proof receipt was reused or recorded
- cargo verus verify: the workspace has 2 members; Verus logs are shared by every package in a run, so sail does not request them

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

For more information about this error, try `rustc --explain E0308`.
```

</details>

