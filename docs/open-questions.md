# Open questions for the owner

Numbered and append-only. When a question is answered, record the answer
inline with the date and link the ADR that captures it.

**Questions 1–8 were answered by the owner on 2026-10-08.** See ADR 0010
(1–6), ADR 0009 (7) and ADR 0010's update to ADR 0008 (8).

1. **License.** The workspace has no license set, and `publish = false`. Which
   license should `sail` use, and when?
2. **Reference platform.** Baselines are first measured on the lead's machine
   (macOS arm64). Is macOS arm64 the reference platform for published
   benchmarks, or should Linux x86-64 be? This affects which Verus and Z3
   artifacts are pinned as primary.
3. **Function attribution source.** Function names never reach the solver
   pipe. Attribution can come from Verus's `--log smt-transcript` (which leaves
   the pipe bytes unchanged) combined with structure, or from `--log smt`
   (which changes the pipe's whitespace, and so cache keys). Is it acceptable
   for `sail verify` to add Verus logging flags, given that this changes what
   Z3 receives? It would still match plain Verus run with the same flags.
4. **Corpus scope** (from `docs/survey/corpus.md`):
   - include `rust_verify_test` through a source build of Verus, or not;
   - make Singular a required or optional dependency;
   - allow pins of commits on personal fork branches (verita uses two);
   - pin a second Verus release for the older agent corpora.
5. **Baseline definition.** Plain Verus may vary slightly between identical
   runs (`rlimit-count` and scheduling across solver processes, per the
   prior-art survey). Conformance should compare per-function verdicts and
   diagnostics. Should benchmark baselines be the median of several runs?
   The current plan is a median of 3 runs.
6. **Sources for the shared tier.** The same Z3 binary can report different
   rlimit counts on different machines. Should the shared tier be scoped per
   machine class, or should entries be re-validated locally before use?
   *Answered* (ADR 0010, item 6): scoped by solver binary hash and platform
   class. Built with signed entries in ADR 0017; a finer machine class
   waits for measurement.
7. **Plain Verus is not perfectly deterministic.** Z3 4.16.0's rlimit
   consumption varies slightly between runs on byte-identical nonlinear
   queries (`corpus/repros/z3-nla-rlimit-nondeterminism/`). A query near its
   limit could therefore flip between `unsat` and `unknown` in plain Verus.
   How should rule 1 ("the same verdict as plain Verus, every time") be read
   for such queries? Proposal:
   - conformance compares against plain Verus run on the same machine;
   - any unit whose plain outcome varies across repeated runs is classified
     *unstable* and reported, not counted as a sail difference;
   - the fragility census measures how common this is.
8. **Default-mode key after the history experiment.** On the captured
   corpus, answers were a function of the assertion stack, but model bytes
   and `rlimit` effort were not (`docs/survey/solver-behavior.md`). Under
   ADR 0004, the default-mode key must then bind the full command prefix of
   the process. ADR 0008 records this. Two choices remain for
   the owner:
   - whether a stack-only key may serve a named opt-in mode, one that
     accepts verdicts across histories and recomputes models on demand;
   - how prefix keys should treat the `rlimit` jitter of nonlinear spinoffs
     (question 7).
9. **Verus's nondeterministic declaration order.** Verus emits some
   declarations in a run-dependent order
   (`corpus/repros/verus-nondeterministic-declaration-order/`), which caps
   warm hit rates at about 95% of checks on the fast tier. The options are:
   - report it upstream to Verus, offering a fix (deterministic iteration
     where it emits declarations);
   - build an opt-in normalization mode under charter rule 5.

   Which should come first? The proposal is upstream first: it costs nothing
   in correctness and helps every Verus user. The root cause is now located:
   `builtin_macros/src/enum_synthesize.rs` iterates a `HashMap`.

   **Answered 2026-10-08 (owner):** do not file bot issues upstream. Track
   Verus issues locally as documentation (`docs/verus-issues/`, VI-0001).

10. **Unknown serving threshold after the census.** Answered 2026-10-09:
    keep 50%. Resource-exhaustion reasons remain unservable. The observed
    far-from-limit quantifier failures do not change this policy.
11. **Seed portfolio.** Answered 2026-10-09: build `accept-first-proof` as
    sequential complete independent Verus runs, baseline then seeds 1–7;
    never combine per-function successes across attempts (ADR 0020).
