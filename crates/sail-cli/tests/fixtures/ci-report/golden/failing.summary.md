## Sail verification: failed

**5 failed, 1 verified, 0 unknown (functions; Verus counts verification units: 1 verified, 6 errors)** in 2 requested targets.

Exit code 101 · 4.9 s · 1 verifier invocation · 2 targets reused from proof receipts · solver checks in this run: 0 from cache, 18 solved

| Target | Result | Functions | Verified | Failed | Unknown | Evidence |
|---|---|--:|--:|--:|--:|---|
| alpha (lib) | failed | 5 | 0 | 5 | 0 | failed in this run |
| beta (lib) | passed | 1 | 1 | 0 | 0 | reused whole-target proof |

### Functions that did not verify (5)

| Function | Verdict | Target | Location | Solver evidence |
|---|---|---|---|---|
| `alpha::add_one` | failed | alpha | [proofs/alpha/src/lib.rs:17:1](https://github.com/example/demo/blob/0123abc/proofs/alpha/src/lib.rs#L17) | Function-Def/0 failed, with a solver model |
| `alpha::call_add_one` | failed | alpha | [proofs/alpha/src/lib.rs:42:1](https://github.com/example/demo/blob/0123abc/proofs/alpha/src/lib.rs#L42) | Function-Def/0 failed, with a solver model |
| `alpha::count_to` | failed | alpha | [proofs/alpha/src/lib.rs:32:5](https://github.com/example/demo/blob/0123abc/proofs/alpha/src/lib.rs#L32) | Function-Def/0 failed, with a solver model; Function-Def/1 failed, with a solver model |
| `alpha::lemma_double_monotone` | failed | alpha | [proofs/alpha/src/lib.rs:49:1](https://github.com/example/demo/blob/0123abc/proofs/alpha/src/lib.rs#L49) | Function-Def/0 failed, with a solver model |
| `alpha::max` | failed | alpha | [proofs/alpha/src/lib.rs:9:1](https://github.com/example/demo/blob/0123abc/proofs/alpha/src/lib.rs#L9) | Function-Def/0 failed, with a solver model |

### Compared with the baseline

Baseline revision `7e6f83cac1a1` (run `378015-1794262634477018790`); this run's revision `840f476a5810`.

**5 regressed, 0 became unknown, 0 recovered**; 0 added, 0 removed, 0 changed otherwise, 1 unchanged, 0 not observed in this run.

| Function | Target | Change | Verdict |
|---|---|---|---|
| `alpha::add_one` | alpha | regressed | verified → failed |
| `alpha::call_add_one` | alpha | regressed | verified → failed |
| `alpha::count_to` | alpha | regressed | verified → failed |
| `alpha::lemma_double_monotone` | alpha | regressed | verified → failed |
| `alpha::max` | alpha | regressed | verified → failed |

### Notes from Sail

- SAIL3146: instant revert did not apply because alpha last compiled in 1083 ms, less than 16 times its measured retention (9996 ms), so retention would cost more than it saves; verification is unaffected

### Verus diagnostics (6 errors, 0 warnings)

Not listed: 1 closing compiler line(s) that restate the errors, and 0 top-level note or help block(s).

<details><summary>error: postcondition not satisfied (proofs/alpha/src/lib.rs:12:9)</summary>

```text
error: postcondition not satisfied
  --> alpha/src/lib.rs:12:9
   |
12 |         r > b,
   |         ^^^^^ failed this postcondition
13 | {
14 |     if a > b { a } else { b }
   |     ------------------------- at the end of the function body
```

</details>

<details><summary>error: possible arithmetic underflow/overflow (proofs/alpha/src/lib.rs:21:5)</summary>

```text
error: possible arithmetic underflow/overflow
  --> alpha/src/lib.rs:21:5
   |
21 |     x + 1
   |     ^^^^^
```

</details>

<details><summary>error: invariant not satisfied at end of loop body (proofs/alpha/src/lib.rs:34:13)</summary>

```text
error: invariant not satisfied at end of loop body
  --> alpha/src/lib.rs:34:13
   |
34 |             i < n,
   |             ^^^^^
```

</details>

<details><summary>error: invariant not satisfied before loop (proofs/alpha/src/lib.rs:34:13)</summary>

```text
error: invariant not satisfied before loop
  --> alpha/src/lib.rs:34:13
   |
34 |             i < n,
   |             ^^^^^
```

</details>

<details><summary>error: assertion failed (proofs/alpha/src/lib.rs:45:12)</summary>

```text
error: assertion failed
  --> alpha/src/lib.rs:45:12
   |
45 |     assert(r == x);
   |            ^^^^^^ assertion failed
```

</details>

<details><summary>error: postcondition not satisfied (proofs/alpha/src/lib.rs:51:9)</summary>

```text
error: postcondition not satisfied
  --> alpha/src/lib.rs:51:9
   |
49 | pub proof fn lemma_double_monotone(a: int, b: int)
   | -------------------------------------------------- at the end of the function body
50 |     ensures
51 |         double(a) <= double(b),
   |         ^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition
```

</details>

