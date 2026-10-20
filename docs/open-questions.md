# Open questions for the owner

Numbered and append-only. When a question is answered, record the answer
inline with the date and link the ADR that captures it.

**Questions 1–8 were answered by the owner on 2026-10-08.** See ADR 0010
(1–6), ADR 0009 (7) and ADR 0010's update to ADR 0008 (8).

1. **License.** The workspace has no license set, and `publish = false`. Which
   license should `sail` use, and when?

   **Answered 2026-10-08 (owner):** Dual MIT OR Apache-2.0 at the first public release; keep `publish = false` until then (ADR 0010).

   **Updated 2026-10-14 (owner):** MIT, matching the public Chelis-Lang repositories. `LICENSE` and the workspace `license` field are set now; `publish = false` stays (ADR 0010 update).

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

    **Moot 2026-10-12:** the same-input oracle and prepared-lowering reuse
    were removed (ADR 0038). Engine conformance compares the engine's full
    results and diagnostics with plain Verus.

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
    (stock also varies there); its other 109 restored hits passed. On the
    standard tier, the oracle could not fork `vest-generated`'s initial
    lowering state at all, so that unit also fails with the oracle on. Options include keeping the
    engine unqualified until trigger choice is deterministic, disabling
    preparation reuse for buckets whose lowering is order-sensitive, or
    classifying such mismatches as unstable under explicit evidence. No
    normalization or classification change has been made.

    **Answered 2026-10-12 (lead, under the owner's delegation):** on a
    mismatch, the oracle lowers the bucket again from an independent owner of
    the same compiler input in the same callback. If the two fresh lowerings
    differ, the unit is unstable (reported separately, like ADR 0009) and an
    engine run without the oracle must still match plain Verus's verdicts and
    diagnostics. If they agree, the mismatch is a hard failure. Fresh
    lowerings from separate processes are not evidence (VI-0001). See ADR
    0038's amendment of 2026-10-12.

    **Moot 2026-10-12:** prepared-lowering reuse and the same-input oracle
    were removed from the engine (rescue L7a; ADR 0038's removal note), so
    there is no restored lowering to compare.

## Verification runtime scope (owner direction, 2026-10-15)

Questions 16–27 come from the [verification runtime scope](scope/verification-runtime.md),
planned for releases after 0.1.0. The owner answered all twelve on
2026-10-15.

16. **Why3 integration depth.** For Creusot, should Sail drive Why3 as a
    sub-orchestrator (its sessions and drivers own prover calls) or reach
    Why3's provers directly (Sail's cache and keys see every query)? The
    first is less work and matches Creusot users; the second is what makes
    caching and identity exact. To be recorded in an ADR.

    **Answered 2026-10-15 (owner):** As recommended. Start with the
    sub-orchestrator; move hot paths to direct prover calls only where
    measurements justify it. To be recorded in the Why3 ADR.
17. **Kernel-prover interface.** What is the generic interface's unit of work
    (a translated module, a theorem, a proof file) and what is pinned per
    prover (Lean or Rocq toolchain, Aeneas, hax, RefinedRust versions)? Is
    one interface for Lean and Rocq peers acceptable even if it fits neither
    perfectly?

    **Answered 2026-10-15 (owner):** As recommended: the unit is a translated
    module plus its proof files; the prover toolchain and translator are
    pinned exactly; one interface serves Lean and Rocq. Addition: the cache
    key also includes the hash of the source crate and of the proof files.
18. **Neutral quantifier forms and mathematical integers.** Which quantifier
    forms are in the neutral layer (bounded only, or unbounded too), and how
    are mathematical integers written in stable Rust? Trigger hints are
    excluded from the neutral layer, so Verus lowering relies on automatic
    trigger choice, whose run-dependence is VI-0015: is that acceptable, or
    must the Verus lowering emit explicit triggers?

    **Answered 2026-10-15 (owner):** (a) Both bounded and unbounded
    quantifiers; unbounded forms are marked verification-only and are not
    runtime-checked. (b) As recommended: the neutral `Int` is `i128` at
    runtime, panicking on overflow, and a true mathematical integer in
    verifiers; panicking is the right failure mode ("the program stops with
    an error rather than producing a wrong number"). (c) Changed: the fork's
    deterministic trigger choice is fine for production runs, but stock
    Verus, the conformance oracle (24), picks triggers differently run to
    run, so oracle comparisons would be flaky. The fork emits the triggers it
    selected as part of the lowering output, and Sail passes them explicitly
    when running the stock oracle. Production still relies on automatic
    choice; the oracle comparison becomes deterministic.
19. **Certificate cost policy.** Certificates are opt-in per request. May a
    certificate-checked entry be shared through the signed shared tier as a
    distinct, stronger tier (charter rule 9)? Is there a cost bound beyond
    which Sail declines to produce one?

    **Answered 2026-10-15 (owner):** As recommended. Certificate-checked
    results are their own tier and never upgrade an existing entry. No hard
    ceiling: cost is reported and callers set limits.
20. **Publishing the contract-syntax crate.** The scope calls it "published
    from this workspace" and also says not to publish crates; the workspace
    keeps `publish = false` (ADR 0010). Until a publishing decision, may
    Oyster and users depend on it (and on Sail's library mode) by git
    revision from the private repository?

    **Answered 2026-10-15 (owner), then corrected:** A git dependency until
    the open-source release; "published from this workspace" meant where the
    crate lives. The owner's correction governs: "we're not putting anything
    in open source or crates until I orchestrate a rollout. all of this work
    is intended for oss but still staged (beyond the verus fork)." The
    workspace keeps `publish = false`; see the publishing ground rule in
    `AGENTS.md`.
21. **Exchanging interface proposals with Oyster and D.I.V.E.** Agents may not
    file in other projects' repositories. Are the sibling `Chelis-Lang`
    repositories covered by that rule? Proposal: interface drafts stay in
    `docs/spec/` with a proposals section, and a human passes them on.

    **Superseded 2026-10-15 (owner):** "the agents can do whatever, gh issues
    and prs and such, on repos in this line of work (verus fork (but not verus
    proper), dive, sail, oyster), but they shouldnt do stuff on wider repos of
    any kind." Agents may raise interface proposals directly in the D.I.V.E.
    and Oyster repositories; the filing ground rule in `AGENTS.md` lists the
    repositories.
22. **Evidence tiers versus the current schema.** Schema v1 has a `tier`
    field (`confirmed` or `provisional`) and `provenance.source`. Proposal:
    keep those as the establishment axis, add `evidence_tier` as a separate
    field in a new schema version, and render ITF values alongside the raw
    solver bytes that rule 3 requires. Does the owner agree with two axes and
    a schema v2 rather than reusing `tier`?

    **Answered 2026-10-15 (owner):** Rename now, before 0.1.0. Ship 0.1.0 on
    schema v2 with two clearly named axes: establishment (confirmed or
    provisional) and evidence tier. Nobody depends on v1 yet; "shipping a
    schema you already plan to break is worse than a small change before
    release." Counterexamples get ITF rendering alongside the exact solver
    output. Implemented in ADR 0044 (#149).
23. **Default mode for cross-backend selection.** Is a verdict obtained by
    "strongest available within limits", a minimum tier, or racing across
    backends a verdict-changing mode under rule 8? Proposal: a request for a
    named backend is default mode; anything that can choose among backends is
    opt-in, named and in `divergences.md`, and racing waits for its own ADR
    (ADR 0020 deferred it).

    **Answered 2026-10-15 (owner):** Choosing a backend is not a
    verdict-changing mode if three conditions hold: every verdict is
    attributed to exactly the backend that produced it; each verdict is
    exactly what that backend returns on its own, with parity applying per
    backend; and the backend's identity appears in provenance. Selection
    modes are then labeled rather than gated as opt-in. Two backends
    returning contradictory conclusive verdicts (one verified, one refuted)
    on the same obligation is a soundness alarm: surface it loudly and never
    resolve it automatically. Verified on one backend and unknown on another
    is not a contradiction.
24. **Which Verus runs lowered code.** Under ADR 0043, the Verus backend runs
    lowered shadow crates on Sail's Verus (the fork) with stock Verus on the
    same lowered crate as the conformance oracle. Confirm. Which Verus does
    Vermilion run on, and do its pins move with `fork.toml`?

    **Answered 2026-10-15 (owner):** Yes to both. The Verus backend runs
    lowered shadow crates on the fork, with stock Verus as the oracle.
    Vermilion also runs on the fork, and its pins move whenever `fork.toml`
    moves.
25. **Sampled tier provenance.** Does Sail run `cargo test` itself, or ingest
    results that a client reports? Ingested results are not established by
    Sail; should they be recorded with a distinct provenance and never count
    toward the assurance view's "established" relationships?

    **Answered 2026-10-15 (owner):** As recommended. Support both: Sail runs
    `cargo test`, and clients may report results. Ingested results are
    labeled as reported and never count as established in the assurance
    view.
26. **Wall-clock-bounded and nondeterministic backends.** TLC, CBMC and Lean
    have no deterministic resource limit like Z3's rlimit. Proposal: cache
    only completed outcomes, never timeouts (rule 4), and measure and report
    nondeterminism as ADR 0009 does. Sail forks Verus to drive randomness out
    (ADR 0043); for other backends, is measure-and-report enough, or should
    the same fork-and-fix approach apply?

    **Answered 2026-10-15 (owner):** As recommended: measure and report. Fork
    a backend only if its measured nondeterminism costs real cache hits. The
    general forking rule: "we'll fork whatever we need to, if the perf story
    is there, but we should be trying to rigorously prove out that there is a
    gain in doing so, before shipping any sail usage of a forked backend or
    whatever."
27. **Priority against the Verus path.** The 2026-10-13 roadmap review
    deferred more backends, certificates and other verifiers for lack of user
    pull. The [roadmap](ROADMAP.md#later) now orders the Verus-path items
    (B1, B2, B4, #118) before the verification runtime. Is that the owner's
    order, and which runtime workstream comes first after the interface
    specifications?

    **Answered 2026-10-15 (owner):** Don't serialize. The Verus-path items and
    the interface specifications share no dependencies, so they run in
    parallel; the specifications are documents that Oyster and D.I.V.E. are
    blocked on, so they do not wait behind the Verus items. After the
    specifications, the SMT-LIB service comes first (Oyster depends on it
    first), and the backend workstreams fan out alongside it. Start
    condition: none of the M10 work starts until the 0.1.0 performance scope
    (the vstd fix in #125, managed-by-default mode and pre-verified vstd) has
    landed and is ready for brittonr's review of the fully performance-improved
    Sail.

    **Superseded 2026-10-15 (owner):** "We're doing the core verus and perf
    features first and then backburnering the growth to multibackends and
    prep for oyster and such." M10, including the interface specifications,
    waits until M11 (Verus core and performance, #168) completes.

## 0.1.0 release (owner direction, 2026-10-15)

28. **`sail new` or exposing `cargo-verus`.** A first-time user needs a project
    that verifies against the matching vstd. Should Sail add a `sail new`
    command that creates a ready-to-verify project, or put its bundled
    `cargo-verus` on `PATH` so users run `cargo verus new` themselves?

    **Answered 2026-10-15 (owner):** Add `sail new`, and keep the bundled
    `cargo-verus` off `PATH`. "It gives the best first run without shadowing
    anyone's stock Verus."
29. **What 0.1.0 is.** Which release candidate becomes 0.1.0, and what goes to
    testers first?

    **Answered 2026-10-15 (owner):** Send rc.4 to testers now, once its
    validation passes. Cut 0.1.0 from rc.5. Managed-by-default mode and
    pre-verified vstd are the headline speed story, and a user's first
    public impression should be of that build.

    **Superseded 2026-10-19 (owner):** rc.5 is not accepted (#184, #187,
    #189). 0.1.0 is the joined candidate on PR #227: shipped verified vstd,
    fork runtime `cbbcdf44` and proof coexistence. Nothing ships without
    real, robust performance wins over stock Verus, measured on the final
    candidate.
30. **Process-loader variables now that managed verification is the only
    `sail verify`** (ADR 0046). Managed verification cannot bind what
    `LD_LIBRARY_PATH`, `LD_PRELOAD` or a macOS `DYLD_*` loader variable changes
    in the compiler and verifier to a proof receipt, and the first version of
    ADR 0046 refused such runs (`SAIL3112`). `LD_LIBRARY_PATH` is common on
    Linux (CUDA, conda, module systems), so that locked those users out.

    **Decided 2026-10-15 (lead):** fail safe, never refuse. A run with a loader
    variable set is verified as plain Cargo-Verus verifies it, prints one
    `note[SAIL3112]` line, reuses no proof receipts and records none (nor
    dependency records); the solver cache stays on, since its keys bind query
    bytes and solver identity. No check is weakened. The same applies to
    Cargo `--config`, `--target-dir`, Verus log options, `--no-verify`,
    tests, benches and examples: nothing plain Cargo-Verus accepts is
    refused (ADR 0046, decision 4).

## Solver-continuation research

31. **Existing stored timing versus a new literal continuation oracle.**
    On actual pinned Linux Z3, three fresh complete runs each of an IronKV
    and an Anvil body-edit trace exit 0 with empty stderr and identical check
    replies (24 and nine `unsat`). Every changed stdout line in those samples
    is a `:time` field. Historical fork/resume `num-allocs` differences remain
    a separate unresolved failure, and one IronKV fresh run retains a memory
    observer's exit race. Both live samples were skipped under the unchanged
    strict raw gate; no gain is claimed.

    [ADR 0011](decisions/0011-local-cache-and-lazy-solver-sessions.md) already
    permits replaying actual stored time/memory figures. What exact timing
    behavior should a future continuation gate require relative to that
    existing contract? This asks for a precise acceptance decision, not
    permission to mask fields, alter resource limits or waive diagnostics,
    models, statistics or runtime identity. Until decided and qualified,
    the current gate remains failed and the prototype stays unselected.
    [Evidence](validation/solver-snapshots.md).
32. **How much slower counts as "materially slower".** The release goal says
    no scenario may be materially slower than stock Verus.

    **Answered 2026-10-19 (owner):** At most 5% slower than stock, or at most
    0.1 s on sub-second runs. Known losses are fixed, not documented.
33. **Which Cargo runs managed verification.** The shipped vstd proof is keyed
    to the compiler arguments Cargo 1.98.1 emits; a newer default Cargo misses
    it and verifies vstd again.

    **Answered 2026-10-19 (owner):** Sail is released as a CLI. Managed builds
    use the engine's pinned Cargo unless the user or project selects a
    toolchain (`RUSTUP_TOOLCHAIN`, a toolchain file, a rustup override or
    `$CARGO`). Stock comparisons run with the same toolchain.
34. **Fork base for 0.1.0.** The fork is based on Verus
    `0.2026.10.04.426d8b0` (Rust 1.98.1). Upstream released
    `0.2026.10.18.7d82994` (Rust 1.99.1) after moving to Rust 1.99 on
    2026-10-17, and Sail supports one Verus release. Rebase before qualifying
    0.1.0, or qualify on the current base and rebase after?
    *Answered 2026-10-19:* rebase before qualifying, so the gates run once on
    the shipped runtime (owner decision). See the ROADMAP rebase row.
35. **Prebuilt macro crates under ADR 0053.** Serving release-built
    proc-macro and build-script crates from the archive would save an
    estimated 1.5–3 s per fresh project, but a broken local compiler (for
    example a user's `cc`) would then no longer fail Sail as it fails stock
    Verus (#187). Is that divergence acceptable for exact-match pinned tools?
