# Prior art: verification caching, proof stability and solver proxies

Survey for charter §"Survey before building", item 2 (prior art) and part of
item 4 (Z3 determinism). Sources were read on 2026-10-08. Where this document
states something about a tool's behaviour, it links the primary source:
paper, documentation or source file at a named commit. Where a statement is
our own inference rather than a source's claim, it says so.

Each section has the same three parts:

- **Summary.** What the system does and what it keys or records.
- **Adopt / avoid.** What `sail` should copy and what it should not.
- **Contract.** How the finding bears on the charter's correctness contract.
  Rule numbers refer to `docs/CHARTER.md` §"Correctness contract". The most
  relevant rules are rule 2 (keys bind the full assertion stack, every option
  and the solver's identity), rule 4 (deterministic limits only) and rule 5
  (normalization off by default).

The last two sections are "Lessons for sail" and a bibliography.

## Contents

1. [Dafny and Boogie verification caching](#1-dafny-and-boogie-verification-caching)
2. [F* hints, query hashes and checked files](#2-f-hints-query-hashes-and-checked-files)
3. [Instability research: Mariposa and follow-ups](#3-instability-research-mariposa-and-follow-ups)
4. [Result caches and solver proxies in other ecosystems](#4-result-caches-and-solver-proxies-in-other-ecosystems)
5. [Z3 determinism](#5-z3-determinism)
6. [Lessons for sail](#6-lessons-for-sail)
7. [Bibliography](#7-bibliography)

## At a glance

| System | Cache unit | Key or validity check | Solver identity in key? | Who checks a hit? |
|---|---|---|---|---|
| Boogie `/verifySnapshots` (Dafny IDE) | Boogie implementation | Implementation id, then entity and dependency checksums computed from the AST | No | Nobody: the cached outcome is trusted |
| F* `--use_hint_hashes` (removed 2026) | One SMT query | Digest of the query text before `check-sat`, plus the configured Z3 version string, rlimit and seed | Version string only | Nobody: a hash match admits the query |
| F* `.checked` files | Module | Digest of the source file and of each dependency's checked file | No | Nobody, beyond the digests |
| Why3 sessions | Proof attempt | Goal checksums and "shapes", prover name and version | Name and version | `why3 replay` re-runs the prover |
| Isabelle `smt_certificates` | Solver call | SHA-1 of the input bytes | No | The Isabelle kernel replays the proof |
| KLEE solver chain | Constraint set | Exact match, then subset and superset reasoning | Not needed: decidable logic, models validated | Optional validating solver |
| Green, GreenTrie, Recal, Utopia, Cache-a-lot | Constraint (sliced, often canonicalized) | Canonical form, implication, or substitution matching | No | Reuse is justified by logic, not by solver behaviour |

No system we found keys a cache on the exact solver binary, the platform and
the full process history. Each one either trusts its cached outcome, or makes
the cache safe by checking every hit independently.

---

## 1. Dafny and Boogie verification caching

### Summary

Boogie has an in-process **verification result cache**, described by Leino and
Wüstholz in "Fine-grained Caching of Verification Results" (CAV 2015) and
exposed in Dafny's IDE ("The Dafny Integrated Development Environment", F-IDE
2014). It works as follows:

- **What is cached.** Dafny computes an *entity checksum* for each function,
  method and method specification from its AST, so editing a comment does not
  change it. It then computes a *dependency checksum* from the entity's own
  checksum and the dependency checksums of the entities it directly depends on.
  Both are attached to Boogie declarations as attributes, so any front end
  targeting Boogie can use the mechanism.
- **Where it lives.** `Source/ExecutionEngine/Caching/VerificationResultCache.cs`
  in Boogie is a .NET `MemoryCache` keyed by `impl.Id`, with a 10-minute
  sliding expiration.
- **How a lookup is decided.** A lookup compares `result.Checksum` with
  `impl.Checksum` and `result.DependenciesChecksum` with
  `impl.DependencyChecksum`. Only when both match is the cached result reused
  (`Priority.SKIP`).
- **Modes.** `/verifySnapshots:1` is "basic" caching (skip unchanged
  implementations). Levels 2 and 3 are "advanced" caching. They instrument the
  new program with information from the cached run, so that only the parts of
  a changed implementation affected by the edit are re-checked. Level 3 also
  maps errors to new source locations.
- **Where Dafny uses it.** In Dafny the same mechanism is the experimental
  `--cache-verification <level>` language-server option, default 0 (off). The
  command-line driver constructs its execution engine with
  `EmptyVerificationResultCache`, so `dafny verify` never caches.

Dafny's separate answer to instability is **measurement**, not caching:

- `dafny measure-complexity --iterations N` (formerly `/randomSeedIterations`,
  now `/randomizeVcIterations`) re-runs each goal with derived seeds. The seed
  "affects the structure of the SMT queries sent to the solver, changing the
  ordering of SMT commands, the variable names used, and the random seed the
  solver itself uses". These are the same three mutations Mariposa uses.
- The Dafny reference manual recommends bounding the coefficient of variation
  of Z3's "more deterministic 'resource count' metric" (for example
  `dafny-reportgenerator --max-resource-cv-pct 10`).
- It recommends `-rlimit` as "a more deterministic approach than setting a
  time limit".
- Boogie also normalizes names and declaration order *at generation time*
  (`/normalizeNames`, and `/normalizeDeclarationOrder`, which is on by
  default). The aim is that source-level renames and reorderings produce
  byte-identical SMT input.

**Known pitfalls:**

- **Caching changed a verdict.** Dafny issue #1860 ("Inconsistent timeouts
  with and without caching, on a single snapshot") shows a lemma that times
  out without caching and verifies with caching on. Our reading is that this
  follows from the advanced mode's design: the instrumented program sends
  Z3 a different query, so Z3 can give a different answer.
- **Diagnostics went stale.** Dafny issue #2523: cached diagnostics are not
  moved to new locations when the document changes, because Boogie's cache
  entries keep the old tokens.
- **Missing key inputs.** The cache key is `impl.Id` plus AST checksums. It
  includes no solver binary, solver options or Z3 version. That is tolerable
  only because the cache is in-process and short-lived.
- **Timeouts are cached.** `TimedOut` outcomes are cached too, and are re-run
  only when `runDiagnosticsOnTimeout` is set.
- **Correctness depends on dependency tracking.** If the dependency checksum
  misses an edge (an axiom, a trigger, an opaque or reveal change, a
  module-level option), a stale result is reused.

### Adopt / avoid

- **Adopt.** Prioritize what was directly edited when scheduling. The Dafny
  IDE paper's priority order (unchanged entities first, so their cached
  results display at once; then added; then edited; then dependents) suits
  `sail watch`.
- **Adopt.** Prefer resource counts to wall time when measuring brittleness,
  and report the coefficient of variation.
- **Avoid.** Do not let dependency checksums decide whether a cached answer
  is reused. Do not instrument queries with cached facts in default mode: it
  changes the query Z3 sees. Do not cache timeouts.
- **Note.** Boogie's generation-time normalization is the right layer for
  normalization. The verifier changes what it emits, and the solver sees the
  normalized bytes in *both* the plain and the cached run. A proxy cannot do
  this without breaking observational identity (see §6, lesson 7).

### Contract

- **Rule 6.** Boogie's design is the opposite of rule 6: dependency
  information decides correctness. `sail` must keep dependency information for
  scheduling only.
- **Rules 1 and 8.** Issue #1860 is direct evidence that "incremental" reuse
  that alters the query can change a verdict. Any `sail` equivalent belongs in
  an opt-in, named mode.
- **Rule 2.** The missing solver identity and options in Boogie's key are
  exactly what rule 2 forbids for a persistent or shared tier.

---

## 2. F* hints, query hashes and checked files

### Summary

**Hints (2016 to August 2026).**

- **Recording and using.** `--record_hints` wrote a `<Module>.fst.hints` file.
  Each entry recorded the query name and index, the fuel and ifuel that
  succeeded, the names of the facts in Z3's **unsat core**, and a hash of the
  query. `--use_hints` replayed a query by pruning the context to the recorded
  core at the recorded fuel, in a fresh Z3 process.
- **Purpose.** Hints made proofs *faster* and more robust to irrelevant
  context growth: a later edit adding unrelated lemmas does not perturb a
  query restricted to its core.
- **Replay can fail.** The F* book notes that a core that is logically unsat
  can still fail, because "Z3 may have relied on quantifier triggers from
  facts outside the core". F* then falls back to the full context. Shake
  (§3.2) reports the same "incomplete core" phenomenon on about 8% of F*
  queries.
- **Masking.** The book warns that hints can mask proof-performance problems.
- **Freshness.** Hints went stale silently and were refreshed periodically and
  checked into repositories.

**Query-hash cache.** `--use_hint_hashes` was a true result cache: "Admit
queries if their hash matches the hash recorded in the hints database". The
pre-removal source of `src/smtencoding/FStarC.SMTEncoding.Z3.fst` at
`72b078e4` (`mk_input`, `cache_hit`) works as follows:

- **What is hashed.** The digest covers the printed declarations *before*
  `check-sat`. It ignores captions and source ranges, and drops the
  post-`check-sat` suffix (`get-model` and similar) as "semantically
  irrelevant".
- **Extra inputs.** It appends the configured Z3 version string, rlimit and
  seed. The source comment says why:

  > Add the Z3 version to the string, so we get a mismatch if we switch
  > versions. Same for rlimit and seed; otherwise we may save a hash with a
  > large rlimit (or particular seed), then decrease the limit (change the
  > seed) and the proof would still succeed with the old hash (seed), but be
  > broken.

- **Hit behaviour.** A hit returns `UNSAT` without running Z3, with *fake*
  statistics (`fstar_cache_hit = 1`).
- **Gaps (our reading of that code).** The hash omits:
  - the binary (it uses only `Options.z3_version()`, the version F* was told
    to expect);
  - `--z3cliopt` command-line options;
  - the platform;
  - any history in a long-lived Z3 process, because the hint is keyed on the
    query text sent, not on the process state.

**Checked files.** `--cache_checked_modules` writes a `.checked` file per
module. `src/fstar/FStarC.CheckedFiles.fst` validates it against a cache
format version (`cache_version_number`), the digest of the source file and the
digests of direct dependencies' checked files. A valid checked file skips
re-verification entirely. Nothing in the record identifies the Z3 binary.

**Robustness tooling:**

- `--query_stats` prints per-query outcome, fuel, rlimit and Z3 statistics.
- `--quake N/M[/k]` requires N successes out of M runs.
- `--retry N` accepts the first success, which is verdict-changing.
- `--z3refresh` restarts Z3 after each query, "useful for ensuring proof
  robustness".
- `--z3seed` and `--z3rlimit` set the seed and resource limit.
- `--log_queries` writes `.smt2` files.

The F* book states that Z3 is deterministic only "in a very strict sense".
Renaming variables, or even repeating a query in the same session, can change
the answer.

**Removal (2026).** F* PR #3680 (January 2025) removed all `.hints` files from
the F* repository. Commit `8e641479` (2 August 2026, "Remove SMT hints and
unsat cores") then removed the mechanism, with this rationale:

- Context pruning (`--ext context_pruning`) "does the job better".
- `(set-option :produce-unsat-cores true)` "disables preprocessing and is a
  significant performance hazard".

The commit message adds:

> Without `:produce-unsat-cores` Z3 takes a different search path and three
> proofs became brittle.

### Adopt / avoid

- **Adopt.** F*'s query-hash design shows that its authors saw why rlimit,
  seed and solver version must be in the key. `sail` should go further and
  bind the binary hash, the command line, the platform and the session
  history (§5, §6).
- **Adopt.** `--quake`-style N-of-M checks are a good fragility report shape
  for `sail-portfolio`. `--z3refresh` shows that verifier authors already
  treat process history as a robustness variable.
- **Avoid.** Never synthesize statistics or other follow-up responses on a
  hit. Replay the recorded bytes (rule 3). F*'s fake statistics would be
  observable to any client that reads `rlimit-count` or `get-info`.
- **Avoid.** Unsat-core replay as a default-mode speed-up. It changes the
  query (and requires `:produce-unsat-cores`, which itself changes Z3's
  search), so it is a divergence. It fits the charter's roadmap item
  "speculative pruning" only as an opt-in mode with provisional results.

### Contract

- **Rule 2.** The F* hash is the closest prior art to `sail-key`, and its gaps
  are the ones rule 2 closes: binary identity, every option including
  command-line ones, and the full logical state.
- **Rule 3.** Faked statistics are what rule 3 exists to prevent.
- **Rules 2 and 5.** The removal commit is direct evidence that an option
  which looks observational changes verdicts. Option values belong in the key,
  and "harmless" options cannot be stripped from it.

---

## 3. Instability research: Mariposa and follow-ups

### 3.1 Mariposa (Zhou et al., FMCAD 2023)

**Summary.**

- **Mutations.** Mariposa treats a query-solver pair (Q, S) as unstable if
  semantically equivalent, syntactically isomorphic mutants of Q get
  inconsistent results. It uses three mutation methods:
  - *assertion shuffling* (reordering `assert` commands);
  - *symbol renaming* (consistent α-renaming);
  - *reseeding* (changing the solver's random seed).
- **Classification.** For each method it samples 60 mutants and estimates the
  success rate r with a Z-test at α = 0.05. A query is:
  - *unsolvable* if r < 5%;
  - *stable* if r ≥ 95%;
  - *unstable* in between;
  - *inconclusive* otherwise.

  A tolerance test (ω = 0.8 of a 60 s limit) avoids calling near-timeout
  queries unstable. Per-method verdicts are combined, and disagreement means
  unstable.
- **Time, not rlimit.** Mariposa uses wall time rather than rlimit. Its
  reasons: rlimit "often counts only some of the resources used (e.g., it may
  ignore resources spent inside a theory solver)", and there is "no guarantee
  of consistency across solver versions".
- **Corpus.** 17,043 queries from six Dafny, F* and Serval projects, run on
  eight Z3 versions (4.4.2 to 4.12.1): over 3 million mutants and about 578
  CPU days.

**Findings.**

- 2.6% of queries are unstable on Z3 4.12.1, and up to 5.0% in one project
  (Komodo-Dafny).
- Upgrades often made projects *less* stable. There is a jump between Z3 4.8.5
  and 4.8.8. Bisection blamed two small commits that changed the ordering of
  flattened disjunctions for 67% of the regressions.
- Shuffling found the most instability, then renaming, then reseeding.
  Seed-only checks (Dafny's and F*'s at the time) "only capture a fraction of
  the problem".
- Stable queries can still have large time variance.
- Longer time limits give diminishing returns.
- One carefully engineered F* project (vWasm) was almost entirely stable,
  through discipline: no nonlinear arithmetic, low fuel, few ambient lemmas
  and hand-picked triggers.
- cvc5 could parse none of the Dafny or F* queries unmodified. After
  conversion it solved only about 14% of one project.

**Adopt / avoid.**

- **Adopt** the methodology for `sail`'s fragility census: three mutation
  families, statistical classification, and reporting r rather than a binary
  verdict.
- **Note.** Mutation-based fragility measurement is fine in an opt-in
  measurement mode because it reports a property and changes no verdict.
- **Note.** For Verus, `sail` should measure under rlimit as well as time,
  since Verus bounds queries with rlimit. Mariposa's caveat about rlimit
  counting applies.
- **Avoid** assuming that a seed portfolio measures fragility fully. The
  charter's seed portfolio sees only the reseeding axis.

**Contract.** Mariposa is the empirical basis for rule 5. Renaming and
reordering, the two most obvious normalizations, are exactly the mutations
that change Z3's answer most often. Solver upgrades changing stability
supports keying on the exact binary (rule 2) and treating a Z3 upgrade as a
full cache invalidation.

### 3.2 Shake: context pruning (Zhou et al., FMCAD 2024)

**Summary.**

- **How much context is irrelevant.** Using Z3 unsat cores as an oracle,
  Shake finds that typically 96% to 99% of a query's assertions are not in
  the core (median relevance ratios range from 0.06% to 3.76% by project).
- **Irrelevant context causes instability.** Replacing an unstable query with
  its core stabilizes 78.3% of unstable instances, and keeps 99.5% of stable
  ones stable (Z3 4.12.5).
- **The algorithm.** Shake treats a verification query as goal plus axioms. It
  layers axioms by symbol reachability from the goal:
  - quantified axioms are expanded lazily, only when a trigger pattern's
    symbols are all reachable;
  - very frequent symbols (such as F*'s `ApplyTT`) can be ignored;
  - a distance limit cuts the layering short.
- **Results.** Shake reduces context 3 to 10 times and improves stability by
  29% (Z3) and 41% (cvc5).
- **Oracle mode.** Shake works best with an *oracle* distance taken from a
  previous successful run's core.
- **Option side effects.** The paper documents cases where "the solver returns
  unsat on a query, but returns unknown when core production is enabled". It
  also finds solver-produced cores that are incomplete: the core alone does
  not re-prove the goal.

**Adopt / avoid.**

- Shake is the best-documented form of the charter's roadmap item
  "speculative pruning using the unsat core of a previous successful proof".
- Pruning can only drop axioms, so an `unsat` on a pruned query implies
  `unsat` on the original. In that logical sense it is sound for proofs.
- It is still *not* observationally identical to plain Verus. It can prove
  what plain Verus times out on, which is a verdict change. A `sat` or
  `unknown` from a pruned query also says nothing about the original.
- So pruning belongs in an opt-in, labeled mode whose results are provisional
  until confirmed (rules 7 and 8).
- Verus already prunes unreachable definitions per module before sending them
  to Z3 (Verus, SOSP 2024), so the remaining gain on Verus queries may be
  smaller than on Dafny or F*.

**Contract.** Core production is itself an option that changes answers. So
`sail` may not turn on `:produce-unsat-cores` in a default-mode session to
collect cores for later use. Cores can only be collected in a separate,
non-authoritative replay.

### 3.3 Cazamariposas (Zhou et al., CADE 2025)

**Summary.**

- **Technique.** Cazamariposas debugs a specific unstable query. It compares
  quantifier-instantiation profiles (Z3 traces and proofs, parsed with a fork
  of SmtScope) between passing and failing mutants, and finds over- or
  under-instantiated quantified formulas.
- **Benchmark.** It adds a **Verus benchmark** of 7,584 queries from ten Verus
  projects, of which about 1% (70) are unstable. This is lower than
  Mariposa's Dafny and F* sets. The queries were also submitted to SMT-LIB
  (SMT-LIB benchmark-submission PR #12).
- **Results.** On 615 unstable queries (545 Mariposa and 70 Verus) it repairs
  70%, and 87% of those fixes involve a single quantifier.
- **Failure modes are bimodal.** Mutants either fail fast with `unknown`
  (under-instantiation) or time out (over-instantiation). Verus queries used
  a 10 s artifact timeout.
- **Option side effects.** "Enabling proof production often causes Z3 to take
  a different path, completely failing on an otherwise solvable query."
- **On normalization.** The paper reports that the input-normalization work
  (§3.4) gave mixed results: −20% instability on one benchmark, +76% on
  another, and +4% overall.

**Adopt / avoid.**

- `sail`'s fragility report should separate *fast unknown* from *exhausted
  limit*. They point to different source-level fixes (add an assertion versus
  hide a definition), which is exactly what an agent needs.
- A ~1% unstable rate on Verus means fragility is rare per query, but common
  per project.

**Contract.** More evidence that proof-production and tracing options must be
in the key, and that profiling runs must never share keys with normal runs.
Verus `--profile` collects quantifier-instantiation data. We infer, but have
not confirmed, that it does so through Z3 tracing options. The
Verus-internals survey should confirm the options Verus actually sets.

### 3.4 Input normalization (Amrollahi et al., arXiv 2410.22419)

**Summary.**

- **Technique.** The authors normalize SMT-LIB inputs so that equivalent
  inputs map to one form. The ideal normalizer would cover assertion order,
  commutative operands, symbol names and converse operators. It is at least
  as hard as graph isomorphism. The practical algorithm handles shuffling and
  renaming, using "patterns" and "super-patterns".
- **No guarantee.** The paper says plainly: "our algorithm does not guarantee
  the normalization property".
- **Results.** Stability is measured by runtime. Mean absolute deviation drops
  in most families, but average PAR-2 is "generally comparable" rather than
  identical.

**Adopt / avoid.** This is the strongest available normalizer, and it still
neither guarantees a canonical form nor preserves solver outcomes per query.
`sail` must not adopt it in default mode. At most, use it to *measure* how
many cache misses are "renaming or reordering only".

**Contract.** This is a direct case for rule 5 staying as written.
Normalization changes the bytes Z3 sees, and therefore the answer, on some
queries.

### 3.5 Other follow-ups (2024 to 2026)

- **Instability track proposal (Shah, Zhou, Heule, Parno; SMT 2025).** The
  authors propose an SMT-COMP instability track with PAR-2I and "instability
  score" metrics, and goal annotations. They contribute the Verus queries.
  This is relevant as a source of Verus-derived benchmark queries for the
  `sail` corpus.
- **Stable generation accounting (Cebeci, Bjørner, Candea, Pit-Claudel;
  arXiv 2609.26345, September 2026).** The authors argue that Z3's
  quantifier-instantiation "generation" (depth) accounting is non-confluent
  and causes unsat/unknown fluctuations. A confluent replacement implemented
  in Z3 reduces outcome instability by 94% on Mariposa's unstable core. We
  could not confirm whether this has shipped in a Z3 release. If it ships, it
  will change answers across a Z3 upgrade, which is one more reason to key on
  the binary.
- **Solver-side mitigations at source level.** These include "Free Facts"
  (Bordis and Leino, FM 2024), which replaces quantified axioms with specific
  instantiations, and trigger-selection strategies (Leino and Pit-Claudel, CAV
  2016). They are not `sail`'s concern, but explain why instability will
  shift as Verus evolves.
- **Talk.** "Towards Proof Stability in SMT-based Program Verification"
  (Zhou and Parno, Dafny 2025 workshop) summarizes the programme above.

### 3.6 Unsat-core-based reuse outside verification

Cache-a-lot (Sadykov, Abdullin, Akhin; arXiv 2504.07642, 2025) reuses unsat
cores across symbolic-execution queries. It finds a variable substitution σ
such that σ(core) is contained in a new formula, using hash footprints and
Bloom filters. It reports up to 74% unsat-core reuse against Utopia's 41%.
Its soundness rests on "a formula obtained through a variable substitution on
an unsatisfiable formula remains unsatisfiable".

**Contract.** This is logically sound for `unsat`. It is not observationally
identical: it can return `unsat` where the pinned Z3 would return `unknown`,
and it has no answer for the follow-up commands Verus sends after a failure.
It is a candidate opt-in mode only.

---

## 4. Result caches and solver proxies in other ecosystems

### 4.1 Why3 proof sessions and `why3 replay`

**Summary.**

- **What is recorded.** Why3 records proof sessions in `why3session.xml`, with
  goal checksums and "shapes" in a separate `why3shapes` file. Each proof
  attempt records the prover's full name (name, version and optional
  alternative), the limits given, the result and the time.
- **Changed goals.** When goals change, Why3 matches old to new goals by name
  and shape, and marks unmatched attempts *obsolete* (Bobot, Filliâtre,
  Marché, Melquiond and Paskevich, "Preserving User Proofs Across
  Specification Changes", VSTTE 2013).
- **Replay.** `why3 replay` is explicitly "to play non-regression tests". It
  re-runs every stored proof and reports differences, with exit code 1 if any
  differ.
- **Smoke detection.** `--smoke-detector` checks for inconsistent contexts by
  proving the negated goal.

**Adopt / avoid.**

- **Adopt** the replay-as-regression-test idea. `sail cache verify` (or the
  conformance harness) should be able to re-run a sample of cached entries
  against the pinned solver and report any difference as a bug.
- **Adopt** recording prover name *and* version per result.
- **Avoid** shape-based matching for default-mode reuse: it is approximate by
  design.

**Contract.** Why3 trusts nothing it has not re-run, which matches rule 9 for
the shared tier. It is also a model for periodic auditing of the local tier.

### 4.2 Isabelle SMT certificates

**Summary.**

- **What is cached.** Setting `smt_certificates` to a file caches every SMT
  call's output: "Any further application of the same SMT solver (using the
  very same configuration) re-uses the cached certificate instead of invoking
  the solver."
- **The key.** In `src/Tools/cache_io.ML` the key is `SHA1.digest_bytes input`,
  the problem bytes only, with no solver binary.
- **Read-only mode.** `smt_read_only_certificates` makes a missing certificate
  an error. This lets theories in the AFP and the distribution check without
  Z3 installed.
- **Why it is safe.** The cached output is a *proof* that Isabelle's kernel
  reconstructs. A wrong or stale certificate fails reconstruction and cannot
  produce a false theorem.
- **History.** Certificates went stale when Z3 or the reconstruction code
  changed, and a 2012 isabelle-dev thread records pinning an old Z3 for
  certificate regeneration.

**Adopt / avoid.**

- **Adopt** the read-only mode as a CI feature: `sail verify --cache
  read-only` fails, or falls back, rather than writing.
- **Adopt** the principle that *checked* cache entries can be trusted from any
  source. That is the charter's future certificate tier (Alethe proofs checked
  by Carcara).
- **Avoid** keying on input bytes alone for *unchecked* entries. Isabelle can
  afford a weak key only because the kernel checks every hit.

**Contract.** This is the clearest illustration of rule 9's two tiers.
Unchecked entries need a strong key and integrity checks. Certificate-checked
entries can tolerate a weaker key because they are verified on use.

### 4.3 KLEE's solver chain

**Summary.**

- **The chain.** `lib/Solver/ConstructSolverChain.cpp` builds a chain of
  solver decorators, from the outside in:
  1. an independence solver (slices out constraints that share no variables
     with the query);
  2. a branch cache (exact-match `CachingSolver`);
  3. the counterexample cache (`CexCachingSolver`);
  4. a fast counterexample solver;
  5. an optional assignment-validating solver;
  6. logging solvers (KQuery and SMT-LIB);
  7. optional cross-checking against a second solver.
- **The counterexample cache.** Introduced in the KLEE paper (OSDI 2008), it
  maps constraint sets to satisfying assignments or to "unsat". It answers
  new queries by subset and superset reasoning: a subset of a satisfiable set
  is satisfied by the same model, and a superset of an unsat set is unsat.

**Adopt / avoid.**

- **Adopt** the decorator architecture: logging, caching and validating
  layers composed around a core solver, each optional. Also adopt the
  *validating solver* as a debugging mode: re-ask the real solver and abort
  on disagreement.
- **Avoid** subset and superset reuse. KLEE works in QF_ABV, which is
  decidable and has checkable models, so "unknown" is rare and a model is a
  certificate. Verus queries are quantified and incomplete. A superset of a
  query Z3 answered `unknown` may still get `unsat`, and Verus's follow-up
  commands after a failure (models, `reason-unknown`) cannot be synthesized.

**Contract.** Semantic reuse is a verdict-changing mode under rule 8. The
validating-solver idea maps onto rule 10 and the conformance harness.

### 4.4 Symbolic-execution constraint caches: Green, GreenTrie, Recal, Utopia

**Summary.**

- **Green** (Visser, Geldenhuys and Dwyer, FSE 2012) slices a constraint to
  its relevant part, canonizes it (renaming and reordering variables, with
  simplification rules for QF_LIA), and looks results up in a key-value store
  (Redis), shareable across runs, programs and analyses.
- **GreenTrie** (Jia, Ghezzi and Ying, ISSTA 2015) adds implication-based
  reuse over a trie.
- **Recal** (Aquino et al., ISSTA 2015) uses a canonical matrix form for
  QF_LIA.
- **Utopia** (Aquino, Denaro and Pezzè, ICSE 2017, extended in TSE 2021 as
  "Reusing Solutions Modulo Theories") reuses solutions across formulas that
  are likely to share one, found heuristically and checked by evaluation.

**Adopt / avoid.**

- **Adopt** the observation that a shared, cross-run store (Green's Redis
  tier) pays off. This supports the charter's shared tier.
- **Avoid** canonicalization. Green's canonical form is sound for its purpose
  because *satisfiability* is invariant under renaming and reordering. But
  `sail` must reproduce Z3's *output*, which is not invariant (Mariposa).
  Utopia-style reuse checks a candidate model against the new formula. That
  can only certify `sat`, which in Verus means a failed proof whose
  diagnostics depend on Z3's own model, so it cannot reproduce plain Verus's
  output.

**Contract.** These systems answer "is it satisfiable?", while `sail` must
answer "what would this Z3 have printed?". That difference is why rule 5
exists.

### 4.5 Query logging and benchmark tooling

- **Boogie `/proverLog:<file>`** logs the SMT input with filename macros
  (`@TIME@`, `@PREFIX@`, `@FILE@`). It is the standard way to extract Dafny
  queries for offline study.
- **Verus `--log smt` and `--log smt-transcript`** write `.smt2` and
  `.smt_transcript` files under `.verus-log` (`source/rust_verify/src/config.rs`).
  The transcript mode logs the "complete SMT transcript", which is the natural
  oracle for checking `sail-shim` capture byte for byte.
- **The SMT-COMP scrambler** (`SMT-COMP/scrambler`) is "the official SMT-COMP
  benchmark scrambler, used as a pre-processor for all tracks". Mariposa notes
  that scrambling resembles its mutations. Our reading: the competition
  scrambles partly so that solvers cannot recognize benchmarks, which concedes
  that names and order are observable to solvers. It is useful as an
  off-the-shelf mutation tool for the fragility census.

### 4.6 Proxies, portfolios and solver-abstraction layers

- **`smt2proxy`** (`facebookincubator/smt2utils`, last released 2021) is "an
  experimental binary tool to intercept and pre-process SMT2 commands before
  they are sent to an SMT solver", acting as a command-line replacement for
  Z3. It is the closest structural precedent for `sail-shim`. It rewrites
  commands rather than caching them. The same project's `z3tracer` parses Z3
  trace logs.
- **pySMT `Portfolio`** (`pysmt/solvers/portfolio.py`) runs one process per
  solver and returns the first answer. It subclasses
  `IncrementalTrackingSolver`, which tracks the assertion stack so it can
  replay it to each backend. This is the session-model-plus-replay design
  `sail-session` and `sail-solver` need.
- **smt-switch** (Stanford) is "a generic C++ API for SMT solving" over
  several backends. It is relevant to the roadmap's portable-query layer, not
  to default-mode caching.
- **CBMC's incremental SMT2 backend** (`--incremental-smt2-solver 'z3 -smt2
  -in'`) drives an external solver interactively over a pipe. That is the same
  integration shape as Verus, and a likely second client for a Z3-compatible
  shim.
- **lean-smt** sends goals to cvc5 and replays cvc5's proof in Lean, so the
  solver is not trusted. It is relevant to the certificate tier.
- **Z3, Bitwuzla and Boolector servers.** We found no documented network or
  daemon server mode. Z3's interactive `-smt2 -in` stdin protocol is the de
  facto "server", and Bitwuzla is used as a library or a process. A `sail`
  daemon has no upstream protocol to reuse beyond SMT-LIB over pipes.
- **No existing cache found.** Searches of GitHub and the literature found no
  SMT result cache or caching proxy for Verus, and no general-purpose
  *byte-faithful* SMT-LIB result cache. This is our search result, not a
  proof of absence.

### 4.7 Verus-specific tooling

- **Solver interaction.** Verus sets a per-query rlimit (`--rlimit`, "roughly
  in seconds", default 10) and passes extra options through `--smt-option`
  (for example `smt.random_seed=7`). It can isolate a function in its own Z3
  process (`#[verifier::spinoff_prover]`, or `-V spinoff-all`). `--profile`
  ("collect and report prover performance data when resource limits are hit")
  and `--profile-all` collect quantifier-instantiation data, which means Z3
  runs with different options. `-V no-solver-version-check` skips the Z3
  version check. `--num-threads` sets verification parallelism, which defaults
  to the available parallelism.
- **Z3 pin.** Verus pinned Z3 4.12.5 from February 2024 until PR #2578 (30
  July 2026) moved it to **Z3 4.16.0**. The motivation was memory-corruption
  bugs in 4.12.5 and a broken Linux-Arm release.
- **Upgrade effect.** The upgrade broke 1 to 7 proofs in most `verita`
  projects. Total resource counts changed by −2% to +277% per project.
- **Run-to-run variation.** Three runs per version on one machine gave
  *identical* total resource counts for every project except one: `page-table`
  differed by 57 units (out of 3.1 billion) between two Z3 4.16.0 runs. The
  PR does not explain this. We note it as possible evidence of
  run-to-run nondeterminism in plain Verus (for example from scheduling
  across Z3 processes). This is an inference that needs measurement.
- **Cargo-level caching bugs (2026).**
  - Issue #2737: cargo's mtime-based freshness skipped verification after a
    file was atomically renamed into place, and exited 0 on broken code.
  - Issue #2799: `--verify-module` made cargo's cache treat all modules as
    verified.

  PR #2794 mitigated #2737. These are live examples of the failure rule 6
  prevents: freshness decided by something other than content.
- **verus-analyzer** is an IDE front end. We found no result caching there or
  in Verus itself beyond cargo's build cache.

**Contract.**

- **Rule 1.** The `page-table` observation matters directly. If plain Verus is
  itself nondeterministic across runs, "the same verdicts as plain Verus"
  needs a precise definition, for example per-query identity given the same
  process history. The differential harness also needs repeated baseline
  runs.
- **Rule 6.** The cargo bugs are the clearest argument for rule 6 in Verus's
  own ecosystem.

---

## 5. Z3 determinism

This section concentrates on Z3 4.12 to 4.16 (Verus's past and current pins)
and records what is and is not deterministic. It is a literature and source
review. Measurements on the pinned build belong in the solver-behaviour survey
and the M3 profile.

### 5.1 Seeds

- **The seed options.** `smt.random_seed` seeds the SMT core and arithmetic
  (`smt_params.cpp`, `theory_arith_params.cpp`). `sat.random_seed` seeds the
  SAT core, and when it is 0 it falls back to the global `random_seed`
  (`sat_config.cpp`). The SMT-LIB `(set-option :random-seed n)` sets a
  separate `cmd_context` seed. Other components (`sls`, `nlsat`, the
  `qflia` tactic) carry their own seeds.
- **What a fixed seed gives.** It makes the solver's *choices* reproducible.
  It does not make the outcome independent of anything else (the following
  subsections).
- **Seeds are a weak mutation.** Mariposa found reseeding the least effective
  mutation, so a seed portfolio explores only part of the instability space.

### 5.2 rlimit

- **Mechanics.** In `src/util/rlimit.cpp` the resource counter `m_count` is
  cumulative for the `reslimit` object, which is per context and so per
  process in the CLI. Each `check-sat` pushes a limit of `m_count + rlimit`
  (`cmd_context.cpp`, `scoped_rlimit`). The budget is therefore per query,
  while the `rlimit-count` statistic grows across the session.
- **Not deterministic across builds and platforms:**
  - **Allocator.** Issue #6736 shows the same Z3 4.11.2 binary giving
    `rlimit-count` 7,284 on one machine and 13,110 on another. The
    maintainers trace it to a preprocessing step (`cofactor-term-ite`) that
    bails out on a *memory* estimate. The allocator reports different
    allocation sizes per platform (`malloc_usable_size`, kernel and libc). The
    maintainers say current master still diverges.
  - **Compiler.** Issue #8841: with Z3 4.16.0 official binaries, the resource
    count needed for a proof was 702,274 on Windows and 929,760 on Linux, so
    an rlimit between the two gives different verdicts. Nikolaj Bjørner names
    "one fundamental platform divergence": C++ leaves argument evaluation
    order unspecified, so compilers differ. Some `std::sort` versus
    `std::stable_sort` uses also matter, "but these are mostly weeded out".
  - **Platform performance.** Issues #9311 (4.15.3: unsat in under 1 s on
    Linux, no answer after 6 hours on macOS) and #7457 (4.13.3) show platform
    divergence in default-option runs. #9311 was resolved only in a 4.17.1
    nightly.
- **Counting is incomplete.** Issue #10939 (2026): in some paths, such as
  legacy Gröbner basis computation, rlimit is not charged in proportion to
  work. A query can run for 30 minutes under an rlimit that should bind. Only
  a wall-clock timeout stops it. Mariposa makes the same point.
- **Messages.** In `rlimit.cpp`, a limit hit reports `max. resource limit
  exceeded`, and a cancellation reports `canceled`
  (`reslimit::get_cancel_msg`). With the default threaded timer, a wall-clock
  timeout arrives as a cancellation. `sail` can therefore separate the two
  through `(get-info :reason-unknown)`, as long as it knows whether it armed a
  timer itself. Caveat: in builds with `POLLING_TIMER`, a timeout is detected
  inside `not_canceled()` without raising `m_cancel`. From our reading of the
  source, such a timeout could be reported as `max. resource limit exceeded`.
  Official release builds should be checked.

### 5.3 Wall-clock timeouts

Timeouts (`-t`, `timeout`, `smt.timeout`, `-T`) fire on a timer thread,
independent of solver progress (Nikolaj Bjørner in issue #7525). A result
produced under, or cut short by, a timeout is not reproducible. Rule 4 already
forbids caching such outcomes.

### 5.4 History dependence within one process

This is the most important finding for `sail`'s key design.

- **Earlier checks change later answers.** Issue #7525 (Z3 4.13.4, macOS): the
  first two `check-sat-assuming` commands in a file change the answers of the
  later ones. Removing them changes the later results.
- **The maintainer's explanation:**

  > The behavior of the solver is determined by a number of factors, such as
  > the current set of expressions that are created. If you create expressions
  > in different order, it results in a different solver state. If you run a
  > solver call and interrupt it, the state after interruption is not the same
  > as the state before the solver was invoked.

- **A second check on the same context differs.** Issue #10939: on Z3 5.1.0
  and master, the *second* check on a fresh solver in the same context runs
  for over 30 minutes where the first takes 0.04 s. Copies using `(reset)`,
  `(push)/(pop)` or `(reset-assertions)` return quickly. So both history and
  the reset mechanism matter.
- **Verifier authors already know.** The F* book says repeating a query in the
  same session can change its answer, and F* offers `--z3refresh` to restart
  Z3 after each query.

The consequence: Z3's response to a `check-sat` is a function of at least the
binary, the platform, the options and **the entire sequence of commands the
process has received since it started**. That includes popped scopes, earlier
`check-sat`s and whether they were interrupted, and the order in which
symbols were declared. It is not a function only of the assertion stack in
effect at that `check-sat`.

### 5.5 Options that look observational but change search

These options look as if they only affect output, but each has been shown to
change Z3's search:

- `:produce-unsat-cores`: F* commit `8e641479`, and Shake.
- Proof production (`:produce-proofs` or `proof=true`): Cazamariposas.
- Tracing for profilers (`trace=true`), probably used by Verus `--profile`
  (unconfirmed): not measured by us, but it is set through the same context
  parameters (`context_params.cpp`).
- Model production and `model_validate`: not measured by us.

No option value can be assumed irrelevant to the answer without measurement.

### 5.6 Threads

Z3's parallel modes (`parallel.enable`, `smt.threads`, `sat.threads`) are off
by default. Several issues report nondeterministic crashes or results with
them (#4856 for `smt.threads=8`, #3898 for `parallel.enable`). Verus does not
enable them as far as we know, which the Verus-internals survey should
confirm. `sail` should treat any thread option as making a session
uncacheable.

### 5.7 Across versions

- Mariposa's bisection (§3.1) and the Verus 4.16.0 upgrade (§4.7) show that
  answers change between versions, even for "small, localized" commits.
- The stable-generation work (§3.5) is a deliberate future change to Z3's
  instantiation behaviour.
- Version strings are not enough. Z3's version output can include a build
  hashcode, F* strips it, and the same version string covers different builds
  per platform (the Verus `get-z3.sh` artifacts are per OS, architecture and
  libc).

### Contract

- **Rule 4** is confirmed and needs sharpening. Only `max. resource limit
  exceeded` outcomes, with no timer armed by anyone, are cacheable as
  deterministic. `canceled`, `max. memory exceeded` (memory-dependent),
  crashes and timeouts are not.
- **Rule 2 needs two refinements:**
  - "Solver identity" must include the platform (OS, architecture, libc and
    allocator), not just the binary hash. Even a byte-identical binary
    diverges across machines (#6736).
  - "The full assertion stack in effect at `check-sat`" is not enough to
    determine Z3's answer. The key must bind the full command history of the
    solver process (§6, lesson 1).

---

## 6. Lessons for sail

These are recommendations, not decisions. The ones that refine the charter
should become ADRs before `sail-key` is built.

1. **Key on the process transcript, not only the logical state.**
   - *Evidence.* Z3's answer depends on everything the process has seen
     (§5.4).
   - *Default key.* Hash the byte-exact sequence of commands sent to that Z3
     process since spawn, up to and including the `check-sat`, together with
     the solver identity (lesson 2) and the process's command line.
   - *Charter wording.* The charter's "full assertion stack in effect at
     `check-sat`" is then a *projection* of this key. Treating two sessions
     with equal assertion stacks but different histories as the same key is a
     normalization under rule 5, needing an ADR and oracle validation. ADR
     candidate: amend the appendix to say so.
   - *Effect on solver pools.* On a miss, `sail` must reproduce the plain-Verus
     answer, so it must replay the same history into a fresh process. A warm
     pool can then only save process start-up and pre-spawning, not reuse a
     process that has seen a different history. This is a design constraint
     on `sail-solver`.
   - *Effect on hit rate.* A key over the full history makes later queries in
     a long Verus session miss after any earlier change in that session. This
     is the price of rule 1. The M3 profile should measure it before anyone
     proposes relaxing it.

2. **Make solver identity include the platform.**
   - Bind:
     - the binary hash;
     - the full `--version` output, including any build hashcode;
     - OS, architecture, libc version and allocator.
   - Shared-tier entries from a different platform fingerprint are misses by
     default (#6736, #8841, #9311).
   - Record the fingerprint with every confirmed result's provenance (rule 7).

3. **Cache only `max. resource limit exceeded`, never `canceled`.**
   - Cache an `unknown` only when the reason is `max. resource limit exceeded`
     and no wall-clock timer was set by Verus, `sail` or the command line.
   - Never cache `canceled`, `max. memory exceeded`, crashes, or results from
     sessions with threads enabled.
   - Boogie caches timeouts. Do not copy that.
   - Verify how `POLLING_TIMER` builds report timeouts for the pinned binary.

4. **Every option is in the key, including ones that look like output
   switches.**
   - Unsat cores, proofs, models and tracing all change search (§5.5).
   - `sail` must never add an option to a default-mode session, for example to
     harvest unsat cores for a later pruning mode.
   - Harvesting runs are separate, non-authoritative replays under their own
     key namespace.

5. **Replay recorded bytes for every follow-up, and never synthesize them.**
   - F*'s cache hit returned fake statistics. Dafny's cached diagnostics went
     stale (#2523).
   - `sail` stores and replays the exact responses to everything Verus asks
     after `check-sat` (rule 3).
   - A follow-up Verus has never asked before is a cache miss that needs a
     full replay to the same point.

6. **Use content addressing everywhere, and dependency information only for
   scheduling.**
   - Boogie's dependency checksums and cargo-verus's mtime freshness (#2737,
     #2799) both produced wrong "verified" results when the dependency or
     freshness signal was wrong.
   - `sail`'s design under rule 6 avoids this class of bug. Keep it so, and
     add a regression test modeled on #2737 (an atomic rename of an older
     file into place) to the conformance corpus.

7. **Keep normalization off. If it is ever introduced, introduce it in the
   verifier, not the proxy.**
   - Mariposa, Cazamariposas and the normalization paper all show renaming and
     reordering changing Z3's answers. The best normalizer has no canonicity
     guarantee and raised net instability by 4%.
   - Boogie normalizes at generation time, where the solver sees normalized
     bytes in the plain run too. A proxy that normalizes sends Z3 different
     bytes than plain Verus would have, so it breaks rule 1 by construction.
   - The only plausible default-mode candidate is lexical: whitespace and
     comments, which do not reach Z3's AST. Even that needs an ADR and
     whole-corpus oracle validation (rule 5).
   - The normalized key is still useful as a *diagnostic*: count how many
     misses are "the same up to renaming or reordering" to inform Verus
     upstream.

8. **Semantic reuse and pruning are opt-in modes.**
   - KLEE subset and superset reuse, GreenTrie implication, Cache-a-lot core
     substitution, F* hint replay and Shake pruning are all logically sound for
     `unsat`.
   - All of them can turn a plain-Verus `unknown` into `unsat`, and none can
     reproduce Verus's post-failure follow-ups.
   - They are verdict-changing modes (rule 8), listed in
     `docs/divergences.md`. Their results are provisional until confirmed by
     the pinned solver on the exact query (rule 7), and they are cached under a
     separate key namespace.

9. **Fragility measurement should follow Mariposa, not seeds alone.**
   - The portfolio's seed axis finds the least instability.
   - The fragility census should add shuffling and renaming mutants (the
     SMT-COMP scrambler or Mariposa's mutators), use Mariposa's statistics
     (60 mutants per method, α = 0.05, r_solvable 5%, r_stable 95%), report
     both rlimit and time, and separate fast-unknown from exhausted-limit
     failures (Cazamariposas).
   - Report the Dafny-style resource-count coefficient of variation for
     queries that are stable but noisy.
   - Expect about 1% of Verus queries to be unstable (Cazamariposas'
     ten-project Verus set).

10. **Treat a solver upgrade as a full key change, and use the cache to measure
    upgrades, not to bridge them.**
    - Upgrades change verdicts (Mariposa; Verus 4.12.5 to 4.16.0 broke 1 to
      7 proofs per project) and change resource use by large factors.
    - Old entries must never answer for a new binary.
    - Keeping both lets `sail bench` report exactly which queries changed
      across an upgrade. That would have given Verus PR #2578 its data for
      free.

11. **Audit cached entries by replaying them.**
    - Following `why3 replay`, `sail` should re-run a random sample of
      local-tier hits against the pinned solver, in the deep tier and
      optionally in the background. Any mismatch is a bug (or a
      determinism finding for `docs/survey/`).
    - Following Isabelle's `smt_read_only_certificates`, offer a read-only
      cache mode for CI.
    - Following KLEE's validating solver, offer a debugging mode that answers
      from the solver and checks it against the cache.

12. **Check-on-use entries can have weaker keys. Trusted entries cannot.**
    - Isabelle and lean-smt can key loosely because every hit is checked by a
      kernel.
    - Until `sail` has certificate checking (roadmap: Alethe and Carcara),
      every entry is trusted only as "the pinned solver said so". That is why
      lessons 1 and 2 are strict.
    - When certificates arrive, they form the stronger, distinct tier rule 9
      describes.

13. **Define the baseline precisely.**
    - If plain Verus can itself differ between runs, then rule 1's "the same
      per-function verdicts as plain Verus" needs a definition. Possible
      sources are scheduling across Z3 processes (the unexplained 57-unit
      resource difference in Verus PR #2578), and history dependence
      combined with parallel verification.
    - One option: identical per query, given the same per-process command
      history. Another: within the range observed over repeated plain-Verus
      runs.
    - The Verus-internals survey should establish how Verus assigns queries to
      Z3 processes. The M3 baseline should run plain Verus several times per
      project. This is a candidate entry for `docs/open-questions.md`.

---

## 7. Bibliography

### Dafny and Boogie

- K. R. M. Leino and V. Wüstholz. *Fine-grained Caching of Verification
  Results.* CAV 2015, LNCS 9206.
  <https://www.microsoft.com/en-us/research/publication/fine-grained-caching-verification-results/>
- K. R. M. Leino and V. Wüstholz. *The Dafny Integrated Development
  Environment.* F-IDE 2014. <https://arxiv.org/abs/1404.6602>
- Boogie, `Source/ExecutionEngine/Caching/VerificationResultCache.cs`.
  <https://github.com/boogie-org/boogie/blob/master/Source/ExecutionEngine/Caching/VerificationResultCache.cs>
- Boogie, `Source/ExecutionEngine/CommandLineOptions.cs` (`/verifySnapshots`,
  `/randomizeVcIterations`, `/normalizeNames`, `/normalizeDeclarationOrder`,
  `/proverLog`).
  <https://github.com/boogie-org/boogie/blob/master/Source/ExecutionEngine/CommandLineOptions.cs>
- Dafny, `Source/DafnyLanguageServer/LanguageServer.cs` (`--cache-verification`).
  <https://github.com/dafny-lang/dafny/blob/master/Source/DafnyLanguageServer/LanguageServer.cs>
- Dafny, `Source/DafnyDriver/CliCompilation.cs` (`EmptyVerificationResultCache`).
  <https://github.com/dafny-lang/dafny/blob/master/Source/DafnyDriver/CliCompilation.cs>
- Dafny Reference Manual, §13.7.6 "Debugging brittle verification".
  <https://dafny.org/dafny/DafnyRef/DafnyRef#sec-brittle-verification>
  (source: <https://github.com/dafny-lang/dafny/blob/master/docs/DafnyRef/UserGuide.md>)
- Dafny issue #1860, "Inconsistent timeouts with and without caching, on a
  single snapshot". <https://github.com/dafny-lang/dafny/issues/1860>
- Dafny issue #2523, "LSP server verification diagnostics that are cached
  using basic caching, are not migrated when the document is changed".
  <https://github.com/dafny-lang/dafny/issues/2523>
- T. Bordis and K. R. M. Leino. *Free Facts: An Alternative to Inefficient
  Axioms in Dafny.* FM 2024. <https://doi.org/10.1007/978-3-031-71162-6_8>
- K. R. M. Leino and C. Pit-Claudel. *Trigger Selection Strategies to
  Stabilize Program Verifiers.* CAV 2016.

### F*

- F* book, "Understanding how F* uses Z3" (hints, `--query_stats`, `--quake`,
  `--log_queries`). <https://fstar-lang.org/tutorial/book/under_the_hood/uth_smt.html>
- F*, `src/smtencoding/FStarC.SMTEncoding.Z3.fst` before hint removal (query
  hash and `cache_hit`).
  <https://github.com/FStarLang/FStar/blob/72b078e477aff99b1cde9d1ef40b0d404bf4aaf0/src/smtencoding/FStarC.SMTEncoding.Z3.fst>
- F*, `src/smtencoding/FStarC.SMTEncoding.Solver.fst` before hint removal
  (hint recording and replay).
  <https://github.com/FStarLang/FStar/blob/72b078e477aff99b1cde9d1ef40b0d404bf4aaf0/src/smtencoding/FStarC.SMTEncoding.Solver.fst>
- F* commit `8e641479`, "Remove SMT hints and unsat cores" (2 August 2026).
  <https://github.com/FStarLang/FStar/commit/8e6414790e929993f450bb5ecf165380b4d40ad4>
- F* PR #3680, "Removing hints from the repo" (January 2025).
  <https://github.com/FStarLang/FStar/pull/3680>
- F*, `src/fstar/FStarC.CheckedFiles.fst`.
  <https://github.com/FStarLang/FStar/blob/master/src/fstar/FStarC.CheckedFiles.fst>
- F*, `CHANGES.md` (master section, "SMT").
  <https://github.com/FStarLang/FStar/blob/master/CHANGES.md>

### Instability

- Y. Zhou, J. Bosamiya, Y. Takashima, J. Li, M. Heule and B. Parno.
  *Mariposa: Measuring SMT Instability in Automated Program Verification.*
  FMCAD 2023. <https://www.andrew.cmu.edu/user/bparno/papers/mariposa.pdf>.
  Technical report: <https://doi.org/10.1184/R1/23905905>.
  Code and benchmark: <https://github.com/secure-foundations/mariposa>
- Y. Zhou, J. Bosamiya, J. Li, M. Heule and B. Parno. *Context Pruning for
  More Robust SMT-based Program Verification.* FMCAD 2024.
  <https://doi.org/10.34727/2024/isbn.978-3-85448-065-5_12>
  (PDF: <https://www.andrew.cmu.edu/user/bparno/papers/shake.pdf>)
- Y. Zhou, A. Shah, Z. Lin, M. Heule and B. Parno. *Cazamariposas: Automated
  Instability Debugging in SMT-based Program Verification.* CADE 2025.
  <https://www.andrew.cmu.edu/user/bparno/papers/cazamariposas.pdf>
- A. Shah, Y. Zhou, M. Heule and B. Parno. *Instability Track for SMT-COMP.*
  SMT 2025, CEUR-WS Vol-4008. <https://ceur-ws.org/Vol-4008/SMT_paper19.pdf>
- D. Amrollahi, M. Preiner, A. Niemetz, A. Reynolds, M. Charikar, C. Tinelli
  and C. Barrett. *Towards SMT Solver Stability via Input Normalization.*
  arXiv 2410.22419. <https://arxiv.org/abs/2410.22419>
- C. Cebeci, N. Bjørner, G. Candea and C. Pit-Claudel. *A Deeper Look at
  Depth: Stable Generation Accounting for Quantifier Reasoning.* arXiv
  2609.26345 (September 2026). <https://arxiv.org/abs/2609.26345>
- Y. Zhou and B. Parno. *Towards Proof Stability in SMT-based Program
  Verification.* Dafny 2025 workshop talk.
  <https://popl25.sigplan.org/details/dafny-2025-papers/5/Towards-Proof-Stability-in-SMT-based-Program-Verification>
- SMT-LIB benchmark-submission PR #12, "Adding benchmarks from Verus".
  <https://github.com/SMT-LIB/benchmark-submission/pull/12>

### Other ecosystems

- Why3 manual, `why3 replay`. <https://www.why3.org/doc/manpages.html>
- F. Bobot, J.-C. Filliâtre, C. Marché, G. Melquiond and A. Paskevich.
  *Preserving User Proofs Across Specification Changes.* VSTTE 2013.
  <https://hal.science/hal-00875395>
- Isabelle, `src/HOL/SMT.thy` (`smt_certificates`), `src/Tools/cache_io.ML`
  and `src/HOL/Tools/SMT/smt_solver.ML`.
  <https://github.com/isabelle-prover/mirror-isabelle/blob/master/src/HOL/SMT.thy>,
  <https://github.com/isabelle-prover/mirror-isabelle/blob/master/src/Tools/cache_io.ML>,
  <https://github.com/isabelle-prover/mirror-isabelle/blob/master/src/HOL/Tools/SMT/smt_solver.ML>
- C. Cadar, D. Dunbar and D. Engler. *KLEE: Unassisted and Automatic
  Generation of High-Coverage Tests for Complex Systems Programs.* OSDI 2008.
  <https://www.usenix.org/legacy/event/osdi08/tech/full_papers/cadar/cadar.pdf>
- KLEE, `lib/Solver/ConstructSolverChain.cpp`.
  <https://github.com/klee/klee/blob/master/lib/Solver/ConstructSolverChain.cpp>
- W. Visser, J. Geldenhuys and M. B. Dwyer. *Green: Reducing, Reusing and
  Recycling Constraints in Program Analysis.* FSE 2012. Code:
  <https://github.com/green-solver/green-solver>
- X. Jia, C. Ghezzi and S. Ying. *Enhancing Reuse of Constraint Solutions to
  Improve Symbolic Execution* (GreenTrie). ISSTA 2015.
  <https://arxiv.org/abs/1501.07174>
- A. Aquino, G. Denaro and M. Pezzè. *Heuristically Matching Solution Spaces
  of Arithmetic Formulas to Efficiently Reuse Solutions* (Utopia). ICSE 2017.
  <https://doi.org/10.1109/ICSE.2017.46>. Extended as *Reusing Solutions Modulo
  Theories*, IEEE TSE 47(5), 2021.
- R. Sadykov, A. Abdullin and M. Akhin. *Cache-a-lot: Pushing the Limits of
  Unsatisfiable Core Reuse in SMT-Based Program Analysis.* arXiv 2504.07642.
  <https://arxiv.org/abs/2504.07642>
- SMT-COMP scrambler. <https://github.com/SMT-COMP/scrambler>
- smt2utils (`smt2proxy`, `z3tracer`). <https://github.com/facebookincubator/smt2utils>
- pySMT, `pysmt/solvers/portfolio.py`.
  <https://github.com/pysmt/pysmt/blob/master/pysmt/solvers/portfolio.py>
- smt-switch. <https://github.com/stanford-centaur/smt-switch>
- CBMC incremental SMT2 backend.
  <https://github.com/diffblue/cbmc/blob/develop/doc/cprover-manual/smt2-incr.md>
- lean-smt. <https://github.com/ufmg-smite/lean-smt>

### Verus

- A. Lattuada et al. *Verus: A Practical Foundation for Systems Verification.*
  SOSP 2024. <https://www.andrew.cmu.edu/user/bparno/papers/verus-sys.pdf>
- Verus, `source/rust_verify/src/config.rs` (`--rlimit`, `--smt-option`,
  `--log smt`, `--log smt-transcript`, `--profile`, `-V spinoff-all`).
  <https://github.com/verus-lang/verus/blob/main/source/rust_verify/src/config.rs>
- Verus, `source/tools/get-z3.sh` (Z3 4.16.0 pin).
  <https://github.com/verus-lang/verus/blob/main/source/tools/get-z3.sh>
- Verus PR #2578, "Upgrade to Z3 4.16.0". <https://github.com/verus-lang/verus/pull/2578>
- Verus issue #2737, "Result caching by `cargo` may improperly skip
  verification". <https://github.com/verus-lang/verus/issues/2737>
- Verus issue #2799, "Verifying with `--verify-module` makes `cargo`'s cache
  think all modules are OK". <https://github.com/verus-lang/verus/issues/2799>
- Verus PR #2794, "[cargo-verus] fix: mitigate known caching issue".
  <https://github.com/verus-lang/verus/pull/2794>
- verus-analyzer. <https://github.com/verus-lang/verus-analyzer>

### Z3

- Z3, `src/util/rlimit.cpp`, `src/cmd_context/cmd_context.cpp`,
  `src/sat/sat_config.cpp` and `src/util/common_msgs.cpp`.
  <https://github.com/Z3Prover/z3/blob/master/src/util/rlimit.cpp>,
  <https://github.com/Z3Prover/z3/blob/master/src/cmd_context/cmd_context.cpp>,
  <https://github.com/Z3Prover/z3/blob/master/src/sat/sat_config.cpp>,
  <https://github.com/Z3Prover/z3/blob/master/src/util/common_msgs.cpp>
- Z3 issue #7525, "Nondeterministic behavior observed despite seed set".
  <https://github.com/Z3Prover/z3/issues/7525>
- Z3 issue #6736, "rlimit-count depends on machine characteristics".
  <https://github.com/Z3Prover/z3/issues/6736>
- Z3 issue #8841, "behavior difference windows/linux".
  <https://github.com/Z3Prover/z3/issues/8841>
- Z3 issue #9311, "Z3 4.15.3 instability on identical input across macOS and
  Linux". <https://github.com/Z3Prover/z3/issues/9311>
- Z3 issue #7457, "Unexpected performance divergence on identical inputs
  between macOS and Linux". <https://github.com/Z3Prover/z3/issues/7457>
- Z3 issue #10939, "Legacy grobner (smt.arith.solver=2) does not charge rlimit
  for coefficient size". <https://github.com/Z3Prover/z3/issues/10939>
- Z3 issues #4856 and #3898 (parallel-mode nondeterminism).
  <https://github.com/Z3Prover/z3/issues/4856>,
  <https://github.com/Z3Prover/z3/issues/3898>
