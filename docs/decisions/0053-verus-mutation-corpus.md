# 0053: A seeded mutation corpus for Verus programs

- **Status:** proposed
- **Date:** 2026-10-16

## Context

Conformance (ADR 0006, ADR 0009) compares Sail with plain Verus on pinned
projects that mostly verify. The failure path is exercised far less: the
edit catalogue (`corpus/edits.toml`, ADR 0012) has a handful of hand-written
edits, few of them breaking. Yet the one outcome Sail must never produce is
a function reported as verified that plain Verus rejects, and the most
likely way to produce it is a cache answer served for a query that differs
only slightly from one already seen. Default-mode keys bind the exact solver
input bytes (ADR 0008), so such a hit should be impossible; nothing tests it
at scale.

Seeded program mutation tests exactly this. The method is taken from the
Voyage project's invariant-inference probes, which mutate Chelis programs
checked with cvc5. Those mutants cannot be reused: they are not Verus, and
their ground truth is LLM-generated properties. The method carries over:

- mutations are textual rewrites of one function, chosen and ordered by a
  seeded hash, so a catalogue regenerates identically;
- mutations fall into named classes;
- *rare-region* mutations change behaviour on a tiny part of the input
  space (for example, a wrong result for one input value). The solver
  catches them where testing would not, and their solver queries differ
  very little from the original's, which is the case that stresses keys.

Plain Verus is the oracle, so a mutant needs no hand-written expected
result. A mutant that leaves the program correct (an equivalent mutant)
still verifies on both sides and is still a valid comparison.

The edit workload already runs the needed sequence: a fill on the original,
then the edited copy, with every sail run compared with plain Verus. It
copies and refills the project for every edit, which does not scale to
hundreds of mutants.

### Pilot: FPBench

A pilot ran the method on FPBench (commit `d7ade34e`, MIT), translated to
Verus by a generator outside this repository, in two translations and two
layouts each ([record](../validation/mutation-corpus.md)):

- `f64`: 23 of FPBench's 136 kernels in 5 crates. The pinned Verus ships no
  floating-point axioms (`examples/float.rs`: projects define their own), so
  the translation carries a small axiom set over the biased binary exponent
  `ex(x)` of a finite `f64` (sum, difference, product and quotient upper
  bounds, and a product lower bound for divisors). Each axiom follows from
  round-to-nearest; one million random IEEE-754 samples, including subnormals
  and values near overflow, found no counterexample. Each kernel `ensures`
  that its result is finite and below a bound derived with the same rules.
  The rest of FPBench needs `sqrt` or transcendental functions, loops,
  single precision, an input without a finite range, or a divisor without a
  lower bound.
- `int`: 20 kernels evaluated exactly over `int` at a scale `10^k`, with
  interval bounds proved by `nonlinear_arith`. Seven lemmas that never
  finish are excluded; four that end with an rlimit error sit in a crate
  without mutants.
- Each translation also comes split, with one module per kernel and the
  same mutants.

379 mutants in five classes (`arith`, `constant`, `spec-weaken`,
`spec-strengthen`, `rare-region`). On native Linux (Sail `c848259`),
conformance was `same` for every unit and function in all four arms, and
the edit workload was `same` for every mutant in every round, with no false
verified. Findings that shape the decision below:

- **Edit-loop gain depends on module structure.** Keys bind the session's
  whole input so far (ADR 0008) and the first miss hands the session to the
  solver (ADR 0011). With one module per crate the edit workload ran at
  0.99× (`f64`, 21% of replies served; none after a `rare-region` mutant)
  and 1.03× (`int`, 17%). With one module per kernel, `f64` ran at 1.81×
  and `int` at 2.63×, each with 72% served. The runner therefore reports served share per class.
- **Expected outcomes are platform- and layout-specific near the rlimit.**
  The unmodified `f64/fptaylor_extra` crate verifies on macOS arm64 and
  fails at one function on Linux x86-64 with the same Z3 4.16.0
  ([reproduction](../../corpus/repros/z3-rlimit-platform-difference/README.md)),
  so five `f64` mutants have different expected outcomes on the two
  platforms. On Linux, nine `f64` mutants fail in the per-crate layout and
  verify in the per-kernel layout, which also raises two kernels' rlimits;
  one `int` mutant finishes per crate and runs past 900 s per kernel.
  Expected outcomes are therefore measured on the reference platform for
  each catalogue.
- **A run that does not finish is not an outcome.** Some nonlinear lemmas
  neither finish within 900 s at the default rlimit nor stop at it, yet stop
  at `--rlimit 1`; one ended with an rlimit error in some runs and ran past
  900 s in others on identical input
  ([reproduction](../../corpus/repros/z3-nla-rlimit-enforcement/README.md)).
  These are excluded and listed rather than given a capped outcome.
- **Near-limit mutants can be very slow.** One `int` mutant takes about
  770 s per plain run, which dominates that catalogue's run time.

## Decision

### Catalogue

- Mutants live in `corpus/mutants/<project>.toml`, one file per base
  project, in exactly the `corpus/edits.toml` format (`schema_version = 1`;
  per entry `id`, `unit`, `kind`, `file`, `description`, `expected`,
  `find`, `replace`), so `sail-bench edit --edits` reads it unchanged.
- Generation metadata lives beside it in `corpus/mutants/<project>.meta.json`,
  keyed by mutant id: the class, the mutated function, the seed and
  generator version, the functions plain Verus rejected, every plain run's
  outcome, the platform the outcomes were measured on, and whether the
  outcome is near the rlimit.
- Catalogues are generated, checked in and reviewable. A regenerated
  catalogue with the same seed, generator version and base commit must be
  byte-identical.
- `expected` comes from three plain Verus runs **on the reference platform**,
  Linux x86-64 in the CI image on a native host (ADR 0010, ADR 0038).
  Outcomes measured elsewhere are provisional and are never committed. A
  mutant whose plain outcome varies across those runs is dropped from the
  catalogue and counted in the generation summary (ADR 0009); it would only
  ever classify as unstable.
- A wall-clock cap is never an outcome. An rlimit error is `errors`; a run
  that does not finish is left out of the catalogue and listed in the
  metadata with its measured time.

### Mutation classes

Each class is a token-level rewrite inside one function body or one
`requires`/`ensures` clause. Signatures and every other function are left
unchanged.

| Class | Rewrite |
|---|---|
| `arith` | swap `+`/`-`, `*`/`/`, `%`/`/` |
| `compare` | swap `<`/`<=`, `>`/`>=`, `==`/`!=` |
| `constant` | replace a literal `k` with `k + 1`, `k - 1` or `2k` |
| `logic` | swap `&&`/`||`; drop a `!` |
| `statement` | delete one statement from an exec body |
| `invariant` | delete one loop `invariant` clause |
| `spec-weaken` | delete one `requires` clause |
| `spec-strengthen` | delete one `ensures` clause from a callee, keep its callers; or tighten a stated bound |
| `rare-region` | guard a `return` with `if <param> == <k> { <wrong value> }` for a `k` inside the declared preconditions |

`statement`, `invariant` and `spec-*` mutants usually fail in a different
function from the one edited. That is intended, and the metadata records
where.

### Generator

- `cargo xtask gen-mutants --project NAME --seed S --count N` fetches the
  pinned base, enumerates candidate rewrite sites per class, orders them by
  `hash(seed, file, function, class, site)`, takes up to `N` spread evenly
  across classes, runs plain Verus three times on each and writes the
  catalogue and its metadata.
- It lexes with the lexer `xtask` already uses, or a new third-party
  dependency if that cannot find function and clause spans inside `verus!`.
  Adding that dependency goes in this ADR's implementation note, with the
  reason; no internal edge is added to `xtask/src/layering.rs`.
- Bases generated outside this repository, such as the FPBench translation,
  bring their own generator and must meet the same catalogue rules.

### Running

- `sail-bench mutants --project NAME [--rounds R]` runs per round:
  1. copy the project once and start a fresh daemon;
  2. fill: plain Verus and sail with the cache on the original;
  3. for each mutant: apply it, run plain Verus and sail with the same
     filled cache, then restore the file;
  4. revert: run plain Verus and sail on the original again.

  The daemon is not restarted between mutants. Its cache accumulates every
  mutant's answers, so step 4 checks that none of them is served for the
  original.
- Every sail run is compared with that round's plain run under ADR 0009 and
  ADR 0042. The report counts `same`, `unstable` and `different`. Inside
  `different`, it counts **false verified** separately: sail reports a
  function verified that plain Verus rejected. Any false verified is a
  correctness defect and gets an issue with the mutant as the reproduction.
- The report also gives each class's share of replies served, so a class
  whose edits defeat the cache is visible.
- Results use the `BenchFile` format with `workload = "mutants"` and the
  timings for fill, each mutant and revert, so the same run gives the
  edit-loop speedup.
- It is local only (ADR 0013). `ci/local.sh mutants PROJECT [ROUNDS]` runs
  it in the CI image; timings are taken only on a native host (ADR 0038).
  Until it exists, `sail-bench edit --edits corpus/mutants/<project>.toml`
  runs a catalogue with a refill per mutant.

### Base projects

- First base: `human-eval-verus`, already in the corpus (fast tier). It has
  many small specified functions and is cheap to verify. It tests the
  failure path, but it is not independent: Sail has been measured and tuned
  on it.
- FPBench, in two translations, neither of which makes a claim about
  floating-point correctness; plain Verus is the only oracle:
  - `f64`: the pilot above. It covers Verus's float encoding (literals,
    bit specifications, broadcast axioms), which the corpus otherwise
    exercises in one example file. It carries little solver load.
  - integer and fixed-point: the polynomial kernels evaluated exactly over
    `int` at a scale `10^k`, with interval and sampled range bounds proved
    by `nonlinear_arith` at the default rlimit. It is meant to load Z3's
    nonlinear arithmetic and produce near-limit outcomes. Lemmas too slow
    to run per mutant go in separate crates without mutants.

  Where the translation lives, and so the URL `corpus/corpus.toml` pins, is
  an owner decision (`docs/open-questions.md`).
- Independent base: one Verus project that has never been in the corpus,
  the edit catalogues or any benchmark. It must:
  - verify with the pinned Verus release without source changes;
  - be pinned by full commit SHA under a licence `deny.toml` allows;
  - have specifications on most functions, so mutants are detectable;
  - be solver-heavy enough that verification time is dominated by Z3
    (quantifiers, triggers, sequences and maps, nonlinear arithmetic);
  - verify cleanly at the pin, with few functions unstable across three
    plain runs;
  - not need Singular;
  - fit a full `mutants` run with three rounds on one c7i.4xlarge within
    the host's eight-hour lifetime.

  The owner chooses it (`docs/open-questions.md`). Once chosen, it is added
  to `corpus/corpus.toml` like any other project, and its first mutation
  run is recorded before any Sail change is made in response to it.

### Evidence

`docs/validation/mutation-corpus.md` records each catalogue's base commit,
seed, generator version and size, and each run's host, counts and timing
summary, under the ADR 0038 evidence policy.

## Consequences

- The failure path and near-identical queries get systematic coverage in
  default mode, with plain Verus as the only oracle.
- The edit-loop benchmark gains a large, unbiased set of edits, many of
  which fail, instead of a few hand-picked ones.
- Generation needs plain Verus runs (three per mutant) on the reference
  platform, so catalogues are produced on a native host and checked in,
  never generated in CI.
- Mutants that change a function's signature, add code or need type
  inference are out of scope; the classes can grow by amending this ADR.
- Results on `human-eval-verus` are not evidence of generalisation; only the
  independent base's first run is.
- The pilot shows the edit-loop gain depends on module structure: an edit
  ends cache reuse for the rest of its solver session. Measuring that per
  class makes it visible; changing it is separate work (ADR 0040, ADR 0051).
- Only `verus` units can be edited today (ADR 0012). A Cargo-built
  independent base needs Cargo units in the edit machinery first.
