# Open questions for the owner

Numbered and append-only. When a question is answered, record the answer
inline with the date and link the ADR that captures it.

**Questions 1–8 were answered by the owner on 2026-10-08.** See ADR 0010
(1–6), ADR 0009 (7) and ADR 0010's update to ADR 0008 (8).

1. **License.** The workspace has no license set, and `publish = false`. Which
   license should `sail` use, and when?

   **Answered 2026-10-08 (owner):** Dual MIT OR Apache-2.0 at the first public release; keep `publish = false` until then (ADR 0010).

2. **Reference platform.** Baselines are first measured on the lead's machine
   (macOS arm64). Is macOS arm64 the reference platform for published
   benchmarks, or should Linux x86-64 be? This affects which Verus and Z3
   artifacts are pinned as primary.

   **Answered 2026-10-08 (owner):** Linux x86-64 is the published benchmark reference; macOS arm64 is a secondary pinned target (ADR 0010).

3. **Function attribution source.** Function names never reach the solver
   pipe. Attribution can come from Verus's `--log smt-transcript` (which leaves
   the pipe bytes unchanged) combined with structure, or from `--log smt`
   (which changes the pipe's whitespace, and so cache keys). Is it acceptable
   for `sail verify` to add Verus logging flags, given that this changes what
   Z3 receives? It would still match plain Verus run with the same flags.

   **Answered 2026-10-08 (owner):** Use `--log smt-transcript` plus structure; avoid `--log smt` in default mode. Any flag that changes solver input must also be supplied to the plain comparison (ADR 0010).

4. **Corpus scope** (from `docs/survey/corpus.md`):
   - include `rust_verify_test` through a source build of Verus, or not;
   - make Singular a required or optional dependency;
   - allow pins of commits on personal fork branches (verita uses two);
   - pin a second Verus release for the older agent corpora.

   **Answered 2026-10-08 (owner):** Source tests belong only in the local deep tier; Singular is optional and tagged. Commit-pinned forks are allowed with local archives. Include a second pinned Verus release (ADR 0010).

5. **Baseline definition.** Plain Verus may vary slightly between identical
   runs (`rlimit-count` and scheduling across solver processes, per the
   prior-art survey). Conformance should compare per-function verdicts and
   diagnostics. Should benchmark baselines be the median of several runs?
   The current plan is a median of 3 runs.

   **Answered 2026-10-08 (owner):** Report the median of three with its spread. Development-machine load must be labeled and does not block measurement (ADR 0010).

6. **Sources for the shared tier.** The same Z3 binary can report different
   rlimit counts on different machines. Should the shared tier be scoped per
   machine class, or should entries be re-validated locally before use?
   **Answered 2026-10-08 (owner):** Scoped by solver binary hash and platform
   class (ADR 0010, item 6). Built with signed entries in ADR 0017; a finer machine class
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

   **Answered 2026-10-08 (owner):** Accept the asymmetric policy: observed plain variation is reported as unstable; a cached proof remains sound; resource-exhaustion and near-limit unknowns are re-solved. Rlimit consumption is result metadata, never a key input (ADR 0009).

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

   **Answered 2026-10-08 (owner):** Default keys bind the full process prefix, solver identity and platform. Only the rlimit setting enters the key. A named stack-only mode is authorized in principle but remains deferred following the statistics-protocol analysis (ADRs 0008, 0010).

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

10. **Unknown serving cap after the census.** Should the cap drop from 50%
    to 1% after the eight-seed fast/page-table census?

    **Answered 2026-10-09 (approved completion plan):** keep 50%. Exhaustion,
    missing effort and the reason checks continue to prevent serving unsafe
    unknowns. The recommendation is retained as evidence, not implemented.

11. **Remaining scope and portfolio semantics.**

    **Answered 2026-10-09 (approved completion plan):** finish the initial
    product, with sequential `accept-first-proof` attempts and optional local
    Singular validation. One complete attempt must prove every requested
    verifying target; never assemble per-function successes across seeds.
    HTTP stores, pools, stack-only keys and later research remain deferred.

12. **Next product phase.**

    **Answered 2026-10-09 (approved M6 plan):** implement managed complete
    workspace reports, durable jobs shared by CLI/watch/MCP, and an optional
    authenticated HTTP store for signed SMT entries. Keep local setup automatic,
    signing trust independent of access credentials, and long validation local.
    Persistent Verus internals, pools, stack-only keys and new backends remain
    deferred. See ADRs 0023–0026 and milestone M6 in `docs/ROADMAP.md`.

13. **Interactive proof engine and repair workflow.**

    **Answered 2026-10-10 (approved M7 plan):** build an optional pinned
    source engine, CLI/MCP repair validation, deterministic explanations,
    immutable named baselines, separate proof events and shared safe
    freshness tracking. Retain a compiler callback only within one revision;
    focused evidence requires independent full validation. The completion
    gate is at least 2× median improvement on each of two frozen Linux
    real-project catalogues. The owner subsequently directed local-container
    execution now, with native-host reference measurements rerun later;
    native-host availability must not block implementation. Editor integration
    and automated proof edits are deferred. See ADR 0027.

14. **Source-engine byte oracle when stock emits unstable declaration order.**

    The graph candidate's real human-eval qualification failed the exact
    independent-stock SMT-byte check on unchanged source. Stock chose a
    different `arrow_1`/`arrow_2` declaration order (VI-0001). Cached and
    fresh-lowering engine runs matched all 114 sessions; four fixed-order
    controls reproduced both orders and missed when current compiler input
    order changed. The original failed qualification remains in
    `engine/verus/candidates/qualifications/2459-linux.json` (now in the
    `archive/m7-contracts-f7103bf` tag), with controls
    in `2459-order-controls.json` in that directory.

    **Answered 2026-10-10 (owner):** compare cached lowering with fresh
    lowering from the same compiler input; keep independent stock verdict
    and diagnostic checks and report stock transcript instability. The
    same-callback oracle compares actual ordered solver bytes with independent
    mutable owners and records diagnostic/effect/outcome comparisons. Its
    implementation must pass before it can qualify a new variant; the old
    failed qualification is not relabelled. See ADR 0027's amendment.

15. **Same-input oracle when stock lowering varies within one process.**

    The source-engine conformance arm (`sail-conformance engine`) runs the
    owner-amended same-input oracle on every restored preparation hit. On
    `verus-examples/examples/guide/traits.rs` it fails in most runs (seven of
    ten repetitions on Linux x86-64): the restored focused-phase lowering and
    a fresh lowering in the same callback chose different automatic triggers
    for one quantifier (VI-0015). Stock Verus makes both choices across runs,
    each engine transcript without the oracle equalled a stock transcript byte for byte, and
    verdicts and diagnostics match plain Verus. The oracle cannot distinguish
    this from a preparation defect, so the arm, as wired into
    `ci/run.sh conformance`, fails on this unit. The complete fast tier
    found the same kind of mismatch on `summer_school/chapter-6-1.rs`
    (stock also varies there); its other 109 restored hits passed. Options include keeping the
    engine unqualified until trigger choice is deterministic, disabling
    preparation reuse for buckets whose lowering is order-sensitive, or
    classifying such mismatches as unstable under explicit evidence. No
    normalization or classification change has been made.
