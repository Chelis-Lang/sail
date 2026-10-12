# VI-0015: Automatic trigger choice depends on hash iteration order

- **Status:** open in stock Verus (tracked here; not filed upstream);
  fixed in Sail's Verus fork by `404a1e30` (fix: choose automatic triggers in a deterministic search order).
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-12
- **Reproduction:** `corpus/repros/verus-nondeterministic-trigger-choice/`
- **Impact on sail:** the source engine's same-input cached-versus-fresh byte
  oracle (ADR 0027 owner amendment) reports mismatches on
  `verus-examples/examples/guide/traits.rs` (seven of ten repetitions) and
  `examples/summer_school/chapter-6-1.rs`, because two lowerings of the same
  bucket in one process can choose different triggers.
  Both choices are outputs stock Verus also produces, and verdicts and
  diagnostics match plain Verus. The oracle cannot tell this variation from a
  preparation defect, so the engine conformance arm fails on these units. Like
  VI-0001, it also lowers warm cache hit rates for affected buckets.
- **Workaround:** no normalization. Under ADR 0038's amendment of 2026-10-12,
  the oracle lowers a mismatching bucket again in the same callback; if the
  two fresh lowerings differ, the unit is reported as unstable (its verdicts
  and diagnostics, from an engine run without the oracle, must still match
  plain Verus). If they agree, the mismatch stays a failure. This needs the
  rebuilt engine runtime that records the confirming arm. In each set of ten
  engine runs of `traits.rs` on Linux x86-64, the fresh and confirming
  lowerings produced eight or nine distinct solver streams, and most restored
  lowerings were byte-identical to a fresh lowering from the same ten runs
  (`docs/reports/rescue/l4b-engine-parity.md`).

The description below is written so it could be filed upstream unchanged.

## Description

**Title:** Nondeterministic automatic triggers: `compute_triggers` breaks
score ties in `HashSet`/`HashMap` order (triggers_auto.rs)

**Body:**

Verifying the same input twice can choose different automatic triggers. With
release `0.2026.10.04.426d8b0`, `examples/guide/traits.rs` produced 8
distinct solver input streams in 12 runs. For the quantifier
`forall |x: &Self, y, z| x.dist(y) <= x.dist(z) + z.dist(y)`, one trigger is
`x.dist(y)` in some runs and `x.dist(z)` in others.

The cause appears to be `source/vir/src/triggers_auto.rs`,
`compute_triggers`: the next variable is taken from
`state.remaining_vars.iter().next()` (a `HashSet`), candidate terms are
iterated from `ctxt.pure_terms_by_var[&x]` (a `HashMap`), and among equally
scored covers the first found is kept. With `RandomState`, the iteration order
changes between processes and between map instances within one process.

**Impact:** verdicts were identical here, but trigger choice affects which
instantiations Z3 makes, so proofs near their limits can succeed in one run
and fail in another. Anything relying on reproducible solver input (caching,
SMT log comparison, instability studies) sees spurious differences.

**Suggested fix:** iterate in a deterministic order (for example sort the
remaining variables and candidate terms by their source position, as the
final `chosen_triggers` sort already does), or use insertion-ordered maps.

## Sail fork

Fixed in [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus) by
`404a1e30` (fix: choose automatic triggers in a deterministic search order),
one commit with its regression test
`source/rust_verify_test/tests/sail_vi0015_trigger_order.rs`, which fails on
the pinned stock source and passes on the fork. `compute_triggers` visits
variables in binder order and candidate terms in the order `gather_terms` met
them (the key that already sorts the final triggers). The search and scoring
are unchanged, so the chosen trigger is one stock can choose. The test checks
that eight identical quantifiers in one file get identical triggers; stock
gave different triggers within one process.

The fork builds only the optional interactive engine. Stock-mode workaround:
Default `sail verify` (stock Verus) keeps this nondeterminism; no workaround
is needed for correctness.
