# Verus chooses automatic triggers in a run-dependent order

**Found:** 2026-10-12, by the source-engine conformance arm's same-input byte
oracle on `verus-examples/examples/guide/traits.rs` (rescue lane L4).

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

**Machine:** Linux x86-64 (EC2 c7i.4xlarge, CI image `sail-ci:8e24fb5f7f490c01`).

## What happens

Verifying the unchanged `examples/guide/traits.rs` (Verus repository at the
pinned commit) twelve times with the stock release sent Z3 eight distinct byte
streams. Among the differences, the automatically chosen trigger set of
`forall |x: &Self, y, z| x.dist(y) <= x.dist(z) + z.dist(y)` varies between
runs: the sixth `Distance.dist` pattern is `(x, y)` in some runs and `(x, z)`
in others.

```
$ ./repro.sh <install dir> <verus source> 12
      2 115912cea21a0774 xy yx xy xy xy xy xz
      1 333238af51bbd487 xy yx xy xy xy xy xz
      2 3a6b261dcbceecee xy yx xy xy xy xy xz
      2 4ead600c2580dc68 xy yx xy xy xy xz xz
      1 7e68ba3cf0ae0fd3 xy yx xy xy xy xy xz
      2 828f4cb14ac53e83 xy yx xy xy xy xy xz
      1 b1e1630f230758e8 xy yx xy xy xy xz xz
      1 f31eda7fa29acc86 xy yx xy xy xy xy xz
```

Each line is: run count, the first 16 hex digits of the SHA-256 of the bytes
sent to Z3, and the argument pairs of every `traits!Distance.dist` pattern in
order. Verdicts and diagnostics were identical in every run.

The variation also happens **within one process**: two lowerings of the same
bucket in one compiler invocation can choose differently. The source engine's
same-input oracle (ADR 0027 owner amendment) restored the focused phase's
lowering of this bucket and compared it with a fresh lowering in the same
callback; in ten repetitions, seven reported `ordered_wire_equal: false` and
`lowering_effects_equal: false` (the inspected one with exactly this pattern
difference), and three passed. Without the oracle, every engine transcript
equalled a transcript stock Verus also produced.

**Root cause (by source reading):**
`verus@426d8b0:source/vir/src/triggers_auto.rs`, `compute_triggers`. The
search picks the next variable with `state.remaining_vars.iter().next()` (a
`HashSet`) and iterates candidate terms over `ctxt.pure_terms_by_var[&x]` (a
`HashMap`). When candidate trigger sets tie on score, the first one found is
kept. `std` hash maps use a per-instance `RandomState`, so the order, and
therefore the chosen tie, varies between processes and between map instances
in one process.

## Run it

```sh
./repro.sh <verus-install-dir> <verus-source-dir> [runs]
```

`<verus-install-dir>` contains `verus` and `z3` (or set `VERUS_Z3_PATH`).
`<verus-source-dir>` is the pinned Verus checkout (for example
`$SAIL_CACHE_DIR/corpus/verus-426d8b01e7ff`).
