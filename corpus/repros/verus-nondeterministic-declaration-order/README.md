# Verus emits declarations in a run-dependent order

**Found:** 2026-10-08, while investigating the 5% of checks that missed in a
warm-run hit-rate estimate (`sail-bench hits`).

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

**Machine:** macOS arm64.

## What happens

Verifying `examples/syntax.rs` (Verus repository at the pinned commit) twice
with identical input sends Z3 different bytes. Six runs produced two
distinct sets of solver input streams:

```
$ ./repro.sh <install dir> <verus source> 6
   2 b8edd9ab7cb9ed04
   4 cfef10d8e514f8b5
```

The differences are in the order of declarations and fuel lists for the
accessor functions that Verus generates for datatype fields used with `->`
(`impl&%2.arrow_v`, `arrow_0`, ...). For example:

```
< (declare-const fuel%syntax!impl&%2.arrow_v. FuelId)
...
>   fuel%syntax!impl&%2.arrow_0.
>   fuel%syntax!impl&%2.arrow_v. ...
```

This matches the survey's prediction of hash-ordered iteration in Verus's
SMT emission (`docs/survey/verus-z3.md` §8).

**Root cause:** `verus@426d8b0:source/builtin_macros/src/enum_synthesize.rs`.
- At line 92, the macro collects enum fields into a
  `std::collections::HashMap` (`direct_fields`).
- At line 155, it generates the `arrow_<field>` accessor methods by iterating
  over that map.
- `HashMap`'s randomised hashing (`RandomState`) changes the iteration order
  from process to process. That changes the order of the generated items,
  and so the order of their SMT declarations and fuel lists.

An insertion-ordered map, or sorting before emission, would make this
deterministic. A draft upstream report is in `upstream-issue.md`; it has not
been filed. Other units affected in the fast
tier include `debug_expand.rs`, `guide/traits.rs`, `guide/datatypes.rs` and
`state_machines/*`. vstd is affected too: 72 of 2,060 checks.

## Why it matters

- **Correctness: unaffected.** Keys bind the exact bytes (ADR 0008), so a
  reordered session is simply a different key.
- **Hit rate: capped.** In the warm, no-change estimate, about 5% of checks
  missed for this reason. Each affected bucket misses from its first
  reordered byte onwards.
- **The options:**
  - an upstream Verus fix (deterministic iteration);
  - an opt-in, labelled normalization mode. Charter rule 5 requires that mode
    to be validated against the solver on the whole corpus, because Z3's
    behaviour can depend on declaration order.
