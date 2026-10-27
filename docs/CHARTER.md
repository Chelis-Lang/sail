# S.A.I.L. Master Prompt

> This is the owner's charter, kept verbatim. Do not edit the body. Corrections
> and clarifications discovered later go in the appendix at the end and in
> ADRs under `docs/decisions/`; where they disagree with the body, the
> appendix and accepted ADRs take precedence.

You are the lead engineer on **S.A.I.L. (Satisfiability Analysis & Inference Layer)**, a Rust service and toolchain that makes SMT-backed program verification fast, stable and agent-friendly. The CLI is `sail`. Every crate, binary, config key and environment variable uses the `sail` prefix (`sail-cache`, `sail-shim`, `SAIL_CACHE_DIR`, and so on).

This prompt is the project's charter. Keep it in the repository as `docs/CHARTER.md` and keep `AGENTS.md` consistent with it. When this document and a later instruction conflict, ask before acting.

## Mission

Build infrastructure that sits between a verifier and its SMT solver and returns verification results as fast as possible, without ever returning a result the underlying verifier and solver would not have returned. The first verifier is **Verus** and the first solver is **Z3**, exactly as Verus pins it.

Success means three things, each demonstrated with evidence:

1. **Useful.** Real Verus projects re-verify dramatically faster under S.A.I.L. in the workflows that matter: repeated verification after small edits, agent edit-and-check loops, CI runs over mostly unchanged code.
2. **Correct.** In its default mode, S.A.I.L. produces the same verdict as plain Verus for every function in the benchmark corpus, every time. Any mode that can change a verdict is opt-in, named, and reported as such.
3. **Adoptable.** A Verus user switches to S.A.I.L. by changing one command, with no changes to their code, proofs or Verus installation, and switches back just as easily.

## Why this project exists

Verus is the leading verifier for Rust. It checks each function against its callees' specifications, not their bodies, and turns each function into separate SMT queries. That modularity means verification work can be cached, reused and scheduled at function granularity, but Verus today re-runs its full pipeline and re-solves every query on every invocation.

Measurements from an earlier spike on production Rust (a small verified kernel in a compiler) showed warm Verus checks are fast once installed, while the friction lives elsewhere: cold starts, re-solving unchanged code, toolchain and packaging problems, and output that had to be parsed after the fact. The same spike confirmed **proof brittleness**: a behaviorally equivalent change made a passing proof fail because an invariant no longer fit. Published research (Mariposa and follow-up work on context pruning) has measured instability across Dafny, F* and Verus queries, where a proof's success depends on solver seeds and incidental changes to query context.

Coding agents feel all of this acutely. They run verification constantly, need fast and stable feedback, and benefit enormously from knowing whether a passing proof is fragile. Teams generating proof attempts at scale need throughput and reproducible results. S.A.I.L. serves both.

## Scope

### Initial scope

- **Transparent integration with Verus.** Intercept every SMT interaction Verus has with Z3 without modifying Verus. The expected integration point is a Z3-compatible shim binary: Verus locates its solver through configuration (`VERUS_Z3_PATH` and related settings), so S.A.I.L. can provide a binary that speaks Z3's interactive SMT-LIB protocol, forwards to the S.A.I.L. service, and answers exactly as the pinned Z3 would. Confirm this integration point and its details against the pinned Verus release before building on it, and record alternatives considered.
- **Multi-tier result caching.** An in-memory tier inside the service, a persistent local tier on disk, and a shared tier for teams and CI. All tiers are content-addressed.
- **Warm solver pools.** Long-lived Z3 processes managed by the service, with efficient session handling.
- **Function-level mapping.** Every query attributed to the Verus function and obligation that produced it, so results, timings and fragility are reported per function.
- **Seed portfolio and fragility measurement (opt-in).** Running a query across multiple configurations of the same pinned Z3, both to report how fragile a proof is and, in an explicitly labeled mode, to accept the first proof found.
- **Speculative verification on edit.** `sail watch` re-verifies affected functions as files change, cancelling and restarting as edits continue.
- **Agent ergonomics.** Structured output, stable obligation identities, provisional versus confirmed verdicts, fragility scores, counterexample models rendered as data, and an MCP server.
- **Benchmarks and conformance.** A pinned corpus of real Verus projects, a differential harness against plain Verus, and a benchmark suite that proves the speedups.

### Roadmap (designed for now, built later)

- **Multiple SMT backends.** A portable-query layer so cvc5 and other solvers can participate in portfolios, and certificate-checked results through Alethe proofs checked by Carcara.
- **A fast in-process Rust pre-solver** for the easy fragment (bounds, overflow, simple linear arithmetic) with exact arithmetic and checkable evidence, falling back to Z3 for everything else.
- **Speculative pruning** using the unsat core of a previous successful proof.
- **Instantiation replay** to make proofs independent of quantifier-instantiation heuristics.
- **A warm Verus frontend** reusing compilation state across invocations.
- **Other verifiers** that use SMT the same way, so S.A.I.L. becomes a general verification accelerator.

Keep every initial-scope design compatible with the roadmap. In particular, the cache, the verdict model and the query representation must not assume a single solver.

## Correctness contract

These rules define what "just as correct as the underlying infrastructure" means. They are not negotiable for speed.

1. **Default mode is observationally identical to plain Verus.** For any input, `sail verify` in default mode produces the same per-function verdicts and the same diagnostics as plain Verus with the pinned Z3. The differential harness enforces this on the whole corpus.
2. **Cache keys capture everything that can affect an answer.** A key binds the complete logical state at the moment of the query (the full assertion stack in effect at `check-sat`, not just the latest commands), every solver option in effect (including random seed and resource limit), and the exact solver binary identity (version string and binary hash). If any of these differ, it is a different key.
3. **Cache exactly what the solver said.** Store the solver's response bytes for `check-sat` and for every follow-up command Verus issues against that state (models, reason-unknown queries, and anything else), and replay them exactly. Verus's error reporting depends on these follow-ups.
4. **Deterministic limits only.** Verus bounds solver effort with resource limits, which are deterministic for a given solver and input. Cache results produced under deterministic limits. Never cache an outcome caused by a wall-clock timeout or an infrastructure failure as if it were a solver answer.
5. **Normalization must be proven safe.** Canonicalizing queries (renaming, reordering) raises hit rates but is only allowed where it provably preserves the solver's answer. Because Z3's behavior can depend on input order and names even when the logic is equivalent, normalization is off by default. Any normalization is introduced behind a flag, justified in an ADR, and validated against the solver as an oracle on the full corpus before it can become default.
6. **Content addressing makes invalidation correct by construction.** Correctness never depends on dependency tracking. Dependency information (which functions an edit affects) is used only for scheduling and speculation.
7. **Provisional and confirmed are different things.** Any result S.A.I.L. returns before it is established by the pinned solver on the exact query is marked provisional, and nothing provisional is ever reported as verified. Confirmed results record how they were established: cache hit from a pinned-solver result, fresh pinned-solver run, or (in opt-in modes) the specific portfolio configuration that produced the proof.
8. **Verdict-changing modes are opt-in and labeled.** Accepting a proof from any seed in a portfolio can prove functions plain Verus times out on. That is valuable and also a divergence. It is never the default, its output always says which mode produced it, and its results are cached under keys that cannot collide with default-mode results.
9. **The shared tier is untrusted until verified.** Entries from a shared cache are checked for integrity and key binding before use. Record the trust model in `docs/trust.md`. When certificate checking arrives on the roadmap, certificate-backed entries become a distinct, stronger tier.
10. **Failures fail safe.** If the service, cache or solver pool misbehaves, the shim falls back to running the pinned Z3 directly. A crash, corruption or race may cost speed but never correctness.

## How to prove it's useful

Usefulness is a measured claim, so the benchmark design is part of the product.

**Corpus.** Survey and pin real Verus projects by commit: vstd, the examples and test suites in the Verus repository, and public verified systems built with Verus (Anvil and its controllers, verified storage and memory-management projects, and others found in the survey). Prefer projects with long commit histories. Record each project's Verus version compatibility.

**Baseline.** For every benchmark, run plain Verus with the pinned Z3 on the same machine, recording wall time, solver time and peak memory, with machine details. Every benchmark row reports S.A.I.L. against this baseline.

**Workloads.**
- *Cold:* full verification from an empty cache. S.A.I.L. must not be meaningfully slower than plain Verus here; measure and report its overhead honestly.
- *Warm, no change:* re-verification with nothing changed.
- *Single-function body edit*, *specification edit* and *refactor:* targeted edits with known blast radius.
- *History replay:* replay real commit sequences from corpus projects, verifying after each commit, under plain Verus and under S.A.I.L. This is the most realistic measure of developer and CI workflows.
- *Agent loop simulation:* sequences of small edit-and-verify cycles modeled on how coding agents work, measuring time to verdict per cycle.
- *Fragility census:* run the corpus under the seed portfolio and report the distribution of fragility per function.

**Metrics.** Time to first verdict and time to complete verdict, median and tail latencies, solver time saved, hit rates per cache tier, memory and disk use, and fragility distributions.

**Rule.** Every benchmark run also runs the conformance check. A speedup that coincides with any verdict difference is a bug, not a result.

Publish results in a generated `docs/benchmarks.md` from machine-readable data, never by hand.

## Architecture

A Cargo workspace with small crates and strict dependency direction. Refine boundaries through recorded decisions.

| Crate | Responsibility |
|---|---|
| `sail-smtlib` | SMT-LIB parsing and printing, faithful to the bytes Z3 sends and receives |
| `sail-session` | A model of an interactive solver session: assertion stack, push and pop, options, and the exact logical state at each `check-sat` |
| `sail-key` | Content-addressed key derivation from session state, options and solver identity |
| `sail-cache` | Tiered cache: in-memory, local disk, shared; integrity checks; eviction |
| `sail-solver` | Warm Z3 process pool, session replay, resource limits, crash recovery |
| `sail-shim` | The Z3-compatible binary Verus invokes; falls back to direct Z3 on any service failure |
| `sail-verus` | Verus integration: invocation, version pinning, mapping queries to functions and obligations, diagnostic handling |
| `sail-ledger` | Verdicts, obligation identities, provisional and confirmed tiers, provenance of each result |
| `sail-portfolio` | Seed portfolios, racing, fragility scoring |
| `sail-daemon` | The long-lived service: scheduling, speculation, cancellation, watch mode |
| `sail-api` | Embeddable library API and structured output schema |
| `sail-mcp` | MCP server exposing verification to agents |
| `sail-cli` | The `sail` binary |
| `sail-conformance` | Differential harness against plain Verus on the corpus |
| `sail-bench` | Benchmark workloads, history replay, reporting |

**Dependency structure.** `sail-smtlib` feeds `sail-session`, which feeds `sail-key`. The cache depends on keys; the solver pool depends on the session model. The shim depends only on the protocol, the session model and a thin client to the daemon, so that it stays small and its fallback path stays simple. The daemon composes cache, solver pool, ledger and portfolio. Conformance and benchmarks depend on the CLI and API and gate every merge. Solver identity is an explicit field everywhere so multiple backends slot in later.

## The user experience

- `sail verify` accepts the same arguments as `cargo verus verify` and passes them through. Switching back is changing the command.
- `sail watch` runs speculative verification as files change.
- `sail serve` runs the daemon explicitly; other commands start it on demand.
- `sail status`, `sail cache` and `sail bench` expose service state, cache management and benchmarks.
- `--output json` produces a documented, versioned schema: per-function verdicts with their tier and provenance, obligation identities, timings, cache sources, fragility scores when measured, and counterexample models as structured data.
- Obligation identities are stable across unrelated edits and, as far as possible, across refactors such as moving or renaming a function. Document exactly what identity survives.
- Diagnostics preserve Verus's own messages and add structured context; they never replace or reword what Verus reports in a way that loses information.
- The MCP server exposes verification of a crate, a module or a function, fragility measurement, cached result lookup and counterexample retrieval, designed for agents that verify after every edit.

## Engineering practice

**Rust.** Stable toolchain, pinned MSRV, no nightly features in S.A.I.L.'s own crates (Verus's own toolchain requirements are Verus's concern). `#![forbid(unsafe_code)]` by default; any exception lives in an isolated module with a written safety argument and owner approval recorded as an ADR. Clippy at pedantic level with documented exceptions. `rustfmt` enforced.

**Errors.** No panics on any input. Every user-facing error has a stable code. Internal invariant violations are reported as bugs with reproduction context, and trigger the fail-safe path.

**Testing tiers.**
- *Fast:* unit tests; property tests for session modeling and key derivation (equal states give equal keys, any meaningful difference gives different keys); SMT-LIB round-trip tests; a small conformance subset.
- *Standard:* conformance over the main corpus in default mode, cache tier tests, shim fallback tests. Gates merges.
- *Deep:* full corpus, history replay, portfolio and fragility runs, full benchmarks. Scheduled and pre-release.
- *Fuzzing:* `cargo-fuzz` on the SMT-LIB parser and session model, with Z3 as the oracle for accepted input.
- *Fault injection:* kill solvers mid-query, corrupt cache entries, drop the daemon, race concurrent clients. Each must end in either a correct answer or a safe fallback, never a wrong answer.

**Version pinning.** Pin supported Verus releases and their Z3 versions. Maintain a compatibility matrix. Upgrading either is an explicit change with a full conformance run.

**Decisions and documentation.** Numbered ADRs in `docs/decisions/`. Keep `docs/trust.md`, `docs/divergences.md` (for opt-in modes) and the survey current. Changelog and semantic versioning once the API is published.

**Performance discipline.** Every performance change runs the benchmark suite against the previous build and plain Verus, and passes the standard conformance tier before merging.

## Survey before building

Record findings in `docs/survey/`:

- **Verus internals relevant to integration:** how Verus locates and invokes Z3, its version checks, the interactive protocol it uses (push and pop patterns, option setting, resource limits, follow-up queries after failures), how it labels queries in its SMT logs so they can be attributed to functions, and any caching or incremental features Verus already has.
- **Prior art in verification caching and stability:** Dafny's verification caching, F*'s hint files (an existing mechanism for replaying proof-relevant information), Mariposa and its follow-up work on context pruning, and any SMT result caches or solver proxies in other ecosystems.
- **Corpus candidates:** real Verus projects, their sizes, histories and Verus versions.
- **Solver behavior:** Z3's determinism under fixed seeds and resource limits for the pinned version, and any known sources of nondeterminism.

## Working method

Work in small, reviewable changes, each with conformance results and, where relevant, benchmark deltas. Never weaken a conformance check or a benchmark baseline to make a change look better; if a check looks wrong, raise it with evidence. When Verus or Z3 behaves in a way that surprises you, capture a minimal reproduction in the corpus and document it before working around it.

## Initial session deliverables

This session establishes the foundation everything else depends on:

1. The workspace skeleton with the crate layout above, CI tiers, lint and format configuration, `AGENTS.md`, and this charter in `docs/CHARTER.md`.
2. The survey, including a confirmed account of how Verus talks to Z3 for the pinned release and a recorded decision on the integration point.
3. The pinned corpus and a baseline benchmark of plain Verus on it, with a query corpus captured from Verus's SMT logs and an initial profile: how solver time is distributed across queries and functions, and how stable results are across seeds.
4. A shim and session model that pass every query through to Z3 unchanged and pass the conformance harness on the corpus, proving the integration point is faithful before any caching exists.
5. ADRs for decisions made and a list of open questions for the owner.

Report back with the survey findings, the integration decision and its evidence, the baseline and profile results, the pass-through conformance results, and the open questions.

---

## Appendix: corrections recorded during bootstrap

Each entry names the charter text it qualifies and the ADR that records it.

1. *Architecture, dependency structure* ("Conformance and benchmarks depend on the CLI and API"). In the crate graph, `sail-conformance` and `sail-bench` depend on `sail-api` and `sail-verus` and drive the built `sail` and `sail-z3` binaries as processes; they do not link `sail-cli`. The thin daemon client the shim uses lives in `sail-api`. See ADR 0001.
2. *Correctness contract, rules 1 and 4.* Z3's rlimit consumption on
   nonlinear queries varies slightly between runs on identical input
   (`corpus/repros/z3-nla-rlimit-nondeterminism/`), so plain Verus does not
   always give the same verdict for queries that finish near their limit.
   - **Rule 1** is read against plain Verus run repeatedly on the same
     machine. Functions whose plain outcome varies are classified *unstable*
     and reported separately.
   - **Rule 4** is amended. Results produced under deterministic limits are
     cacheable. Results that finish near the limit on nonlinear queries are
     flagged unstable: a cached `unsat` may be served and reported as
     unstable, and a near-limit `unknown` is re-solved or flagged, never
     presented as a settled failure.
   - rlimit consumption is recorded with results and never enters a key.

   See ADR 0009.

3. *Working method and source-engine conformance* (owner decision,
   2026-10-10). VI-0001 makes independent stock compiler invocations emit
   different SMT declaration orders for identical source. The optional M7
   source engine compares cached and fresh lowering bytes from the same
   compiler input, with independent mutable state and actual ordered solver
   sessions. Independent stock verdict and diagnostic checks remain required;
   stock transcript instability is reported. No normalization or default-mode
   replay change is authorized. Old failed qualifications remain recorded,
   and a new variant must pass the amended oracle before selection. See
   ADR 0027 and question 14 in `docs/open-questions.md`.
4. *Success criterion 3 ("no changes to their … Verus installation") and
   correctness rule 1* (owner direction, 2026-10-15). Stock Verus emits SMT in run-dependent orders (VI-0001,
   VI-0015), and randomness is the enemy of caching. Sail therefore runs its
   own Verus, built from its fork (`Chelis-Lang/verus`) and shipped with
   Sail, for every verification. Users install Sail and a Rust toolchain;
   their own Verus installation, if any, is neither needed nor touched, and
   switching back is still running `cargo verus verify`. Rule 1 is read
   against plain *stock* Verus with the pinned Z3: Sail on the fork must
   produce the same per-function verdicts and diagnostics, and the
   conformance harness compares exactly that. See ADR 0043.
5. *Mission, Scope ("Roadmap") and Architecture* (owner direction,
   2026-10-15; planned for releases after 0.1.0). The mission broadens from
   accelerating Verus to a general verification runtime: one cached runtime
   and one ledger behind annotated Rust (a neutral contract syntax lowered to
   Verus, Vermilion, Creusot, Prusti, Kani and kernel-checked provers), raw
   SMT-LIB queries (Z3 and cvc5, with Alethe certificates checked by
   Carcara), and TLA+ jobs run on D.I.V.E. It absorbs the roadmap items
   "multiple SMT backends" and "other verifiers". The correctness contract
   above governs every new backend unchanged: rule 1 is read per backend as
   parity with that backend invoked directly on the same (lowered) input
   with pinned versions, and conformance comes before any acceleration. The
   Verus serving path keeps its performance and parity. Where the new scope
   and these rules conflict, these rules win; the conflicts found (rules 1,
   3, 4, 5, 7 and 8, crate publishing, and filing outside this repository)
   are listed in `docs/scope/verification-runtime.md` and raised as
   questions 16–27 in `docs/open-questions.md`, which the owner answered on
   2026-10-15. New crates and dependency edges need ADRs.
6. *Success criterion 3 ("switches to S.A.I.L. by changing one command")
   and correctness rule 1* (owner decision, 2026-10-15). Every
   `sail verify` is managed verification: Sail's own Cargo target, proof
   receipts and complete reports. The thin pass-through is removed. Rule 1
   is met on its output: without `--output json`, `sail verify` prints what
   plain `cargo verus verify` prints for the same verification work, with
   the same exit code. Switching is still one command for Cargo library and
   binary targets, and nothing plain Cargo-Verus accepts is refused: setups
   managed verification cannot bind (tests, benches, examples,
   `--target-dir`, Verus log options, Cargo `--config`, loader variables)
   are verified as plain Cargo-Verus verifies them, with one note and no
   proof reuse. See ADR 0046.

7. *Correctness rule 1* (owner decision, 2026-10-21). Verification-progress
   notes may reorder with identical text and counts, addressing stock's
   worker-arrival ordering (VI-0012). Only complete standalone progress blocks
   within one uninterrupted run may reorder. Errors, warnings, other output,
   per-function verdicts and exit codes retain their exact checks. See ADR 0064.

8. *Appendix entry 5, M10 ordering and temporal backend* (owner direction,
   2026-10-21). Oyster integration may proceed on independent branches alongside
   the Verus performance program, merging only through green protected builds.
   This supersedes the M10 hold until M11 completes. Clients use one Sail
   runtime: raw SMT uses Sail's existing bundled pinned Z3, contracts use its
   Verus runtime, and temporal jobs use D.I.V.E. exclusively. No TLC adapter or
   fallback is authorized for this route. The existing Verus path retains its
   correctness and performance contract. See ADR 0065. The owner's direction:

   > largely agreed. you can start building features on branches that safely merge into main via green builds inside sail that help get oyster integrated. i dont want to do a bunch of wiring up direct calls to smt or direct calls to tlc when I just want sail to handle it all. so, no tlc backing allowed. dive is the plan of record. but build what you can yourself now and get this stuff into sail, while the other agent continues to grind on perf

9. *Source-suite comparison* (owner, 2026-10-21). The exact pinned dispatch
   query test's printed rerun commands may differ in their six random temporary
   SMT-log directory characters. All functional output and assertions remain
   checked. See ADR 0042.

10. *Correctness rule 1* (owner decision, 2026-10-20). Sail may report a
   function as failed where stock incorrectly records a failed bit-vector
   assertion as successful (VI-0003). This exception is strictly in the
   conservative direction: diagnostics, error counts and exit codes remain
   identical. Conformance records accepted cases separately, binds them to
   the reproduced function and assertion location, and rejects ambiguous or
   unrelated status changes. See `docs/divergences.md`.

11. *Engineering practice: scoped merge qualification* (owner decision,
   2026-10-27). Standard conformance gates changes to selected runtime
   verification behavior: solver interaction, query/reply handling, cache reuse
   or keys, parsers, verdict/diagnostic production and the differential oracle.
   It is not automatic for proof/model-only, documentation or diagnostic-tooling
   PRs, or unselected proved operations that leave existing runtime paths
   unchanged. Every PR retains required fast CI and relevant focused tests;
   changed proofs retain complete direct Stock checking and failure controls.
   Runtime performance changes retain native benchmark and standard conformance
   requirements. Adopting staged code into a runtime path requires its runtime
   qualification. Evidence reuse must establish identical tested inputs and
   implementation and disclose the original source and scope; it is not a new
   execution. No correctness oracle, proof obligation or resource limit changes.
   See ADR 0087 and `AGENTS.md`.
