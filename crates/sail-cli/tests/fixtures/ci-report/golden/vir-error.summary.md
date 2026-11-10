## Sail verification: rejected by Verus before verification

**1 verified, 0 failed, 0 unknown; Verus rejected 1 crate before verification** in 2 requested targets.

Exit code 101 · 29.5 s · 3 verifier invocations · 0 targets reused from proof receipts · solver checks in this run: 0 from cache, 1 solved

| Target | Result | Functions | Verified | Failed | Unknown | Evidence |
|---|---|--:|--:|--:|--:|---|
| alpha (lib) | rejected by Verus | 0 | 0 | 0 | 0 | rejected by Verus |
| beta (lib) | passed | 1 | 1 | 0 | 0 | verified in this run |

### Compared with the baseline

Baseline revision `7e6f83cac1a1` (run `378015-1794262634477018790`); this run's revision `b70d3a4f7c53`.

**Not comparable:** the verification configuration or executable identity differs (context `720206fbc41d` against `3afb29760429`). The differences below are descriptive; none is reported as a regression.

| Function | Target | Change | Verdict |
|---|---|---|---|
| `alpha::add_one` | alpha | incomparable | verified → not observed |
| `alpha::call_add_one` | alpha | incomparable | verified → not observed |
| `alpha::count_to` | alpha | incomparable | verified → not observed |
| `alpha::lemma_double_monotone` | alpha | incomparable | verified → not observed |
| `alpha::max` | alpha | incomparable | verified → not observed |

- Requested scope, mode or runtime context differs; transitions are descriptive, not proof regressions.

### Notes from Sail

- Build-script input declarations were established during this run

### Verus diagnostics (1 error, 0 warnings)

Not listed: 1 closing compiler line(s) that restate the errors, and 0 top-level note or help block(s).

<details><summary>error: cannot call function `alpha::add_one` with mode exec (proofs/alpha/src/lib.rs:18:13)</summary>

```text
error: cannot call function `alpha::add_one` with mode exec
  --> alpha/src/lib.rs:18:13
   |
18 |     let y = add_one(x);
   |             ^^^^^^^^^^
```

</details>

