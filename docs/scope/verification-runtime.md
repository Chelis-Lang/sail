# Scope: Sail as a general verification runtime

- **Status:** delivered surfaces maintained. Owner decision, 2026-11-09
  ([ADR 0109](../decisions/0109-oyster-retired-v030-scope.md)): Oyster is
  abandoned, mutation testing leads v0.3.0, and an experimental Kani backend
  is followed by cvc5 in the SMT runtime inside that release.
  Neutral-contract surfaces, including the three contract crates, were
  removed for v0.3.0.
  Temporal packaging is held for distribution. The 2026-10-21 active
  integration ordering and the Oyster descriptions below are historical.
- **Owner answers:** questions 16–29 in [open questions](../open-questions.md),
  answered 2026-10-15.
- **Source:** owner direction, 2026-10-15.
- **Tracking:** milestone *M10: Verification runtime*, umbrella issue
  [#146](https://github.com/Chelis-Lang/sail/issues/146). Ordering
  against other post-release work is in the [roadmap](../ROADMAP.md#after-010-parked).

Sail today serves Verus. It runs Sail's Verus (built from the fork,
[ADR 0043](../decisions/0043-default-mode-runs-the-fork.md)) with the pinned
Z3 behind a caching shim, and returns the same verdicts and diagnostics as
plain stock Verus. This document preserves the longer-term ambition of
one verification server for a family of tools; ADR 0109 controls current
order. The
[charter](../CHARTER.md) and its correctness contract govern all of it
unchanged (charter appendix entry 5). Where this scope and the charter
conflict, the charter wins; the conflicts found so far are listed under
[Conflicts](#conflicts-with-the-charter-and-current-decisions).

## Current state

Raw Z3 execution is a supported command
([ADR 0108](../decisions/0108-supported-smt-runtime.md)). Native temporal
execution exists in the internal extension, absent from public archives.
Maintain these surfaces and tests. Neutral source jobs, source-bound
assurance, neutral lowering and the contract crates were removed for v0.3.0;
the last implementation is tagged `archive/neutral-contracts-final`. The
neutral sections below are historical. The backend list
below is long-term scope, not simultaneous implementation work. The next
release's Verus-aware mutation feature does not depend on neutral contracts.
The existing Verus path supplies these reusable components:

- one verifier (Verus) and one solver (Z3), with solver identity already
  explicit in keys, the cache and the verdict model;
- content-addressed keys over the exact process prefix (ADR 0008), memory,
  disk and signed shared tiers;
- the ledger and `--output json` schema v2: per-function verdicts, the
  establishment (`confirmed`/`provisional`) and evidence tier
  (`smt_verified`) axes, provenance and ITF counterexample values (ADRs 0014,
  0044);
- obligation identities for Verus functions;
- sequential, complete-run seed portfolios and fragility measurement
  (ADRs 0018, 0020); racing is deferred;
- the daemon, persistent jobs, watch and the MCP server;
- the differential conformance harness and benchmark suite.

The first raw-SMT interface is specified in [runtime v1](../spec/runtime.md).
Contract and temporal specifications land with their implementations; a
specification alone does not establish backend support.

## Mission

The longer-term ambition below is not a list of delivered capabilities or
simultaneous implementation commitments. Existing scope is described above.

Sail becomes one cached runtime and one ledger behind three kinds of input:

1. **Annotated Rust source.** Contracts in a neutral syntax, tagged with
   requirement IDs. Sail lowers them per verifier and dispatches them to
   Verus, Vermilion, Creusot, Prusti, Kani and kernel-checked provers.
2. **Raw SMT-LIB queries.** A generic, cached solver service for clients that
   formulate their own queries.
3. **TLA+ jobs.** Model checking, simulation, trace validation and replay,
   run on D.I.V.E., embedded as a library.

Every verdict, from every input, lands in one ledger keyed by obligation
identity and, where present, requirement ID.

## Sibling projects

These sibling projects have their own repositories. Oyster engineering is
held for design and temporal packaging for distribution. Existing dependency
boundaries remain; Sail never depends on Oyster's internals.

- **Oyster** is a requirements tool. It refines EARS-style requirements with
  stakeholders, formalizes each into a typed model, and checks requirements
  for ambiguity, conflict and gaps. As a Sail client it:
  - sends many small, repetitive SMT-LIB queries: sampled formalizations of a
    requirement are clustered by logical equivalence, conflicts come back as
    minimal unsat cores, gaps as models;
  - sends TLA+ jobs that check multi-step consistency and liveness;
  - writes neutral contracts and requirement predicates, tagged with
    requirement IDs, into users' Rust source, and relies on Sail to verify
    them and record per-requirement assurance.
- **D.I.V.E.** is a Rust port of the TLA+ toolchain, conformant with a pinned
  TLC, speaking ITF (Informal Systems' JSON trace format) for traces,
  counterexamples and witness values. It ships standalone for the TLA+
  community; Sail embeds it as a library backend.

**Dependency direction.** Oyster depends on Sail (linked in-process or through
the daemon). Sail embeds D.I.V.E. Sail never depends on Oyster, and D.I.V.E.
never depends on Sail.

| Relationship | Question | Sail's role |
|---|---|---|
| Requirements on their own | Consistent, complete, unambiguous? | Answers Oyster's SMT-LIB queries and TLA+ jobs |
| Contracts against requirements | Is each contract faithful to its requirement? | Answers Oyster's entailment queries |
| Design against requirements | Does a TLA+ design satisfy requirement properties? | Runs the D.I.V.E. job |
| Contracts against code | Do the contracts hold? | Lowers and dispatches to verifiers |
| Code against design | Is observed behavior allowed by the design? | Runs D.I.V.E. trace validation on ITF traces |

## Interfaces Sail owns

Each is written as a specification in `docs/spec/`, with examples, before any
implementation against it. External projects build against them
concurrently, so they must be precise, versioned and stable.

### Neutral Rust contract syntax

A crate in this workspace that users and Oyster write contracts with. Sail
owns the semantics of every construct, because it lowers them.

- **Base syntax** mirrors `core::contracts` exactly: `#[requires(expr)]` and
  `#[ensures(|ret| expr)]` over side-effect-free Rust boolean expressions. It
  works on stable Rust now and swaps to `core::contracts` when that
  stabilizes. Under normal compilation contracts become runtime checks, so
  ordinary tests exercise them.
- **Traceability:** `#[satisfies("REQ-ID")]` on implementations;
  `#[requirement("REQ-ID")]` on pure requirement predicates, which contracts
  call directly, for example `#[ensures(|ret| req::ord_003(&order, ret))]`.
- **Neutral forms** for constructs verifiers share but spell differently: old
  values, universal and existential quantification over bounded and
  unbounded domains, and mathematical integers in specifications. Each form
  has precise semantics and a per-backend account of how it lowers, or why it
  cannot. Unbounded quantifiers are marked verification-only and are not
  runtime-checked. The neutral `Int` is `i128` at runtime, panicking on
  overflow, and a true mathematical integer in verifiers (question 18).
- **Contract cases,** after ACSL's behaviors: named cases with their own
  assumptions and guarantees, plus optional declarations that the cases are
  complete and disjoint. Cases mirror requirements one to one; Sail
  discharges completeness and disjointness as obligations of their own.
- **Proof scaffolding stays out of the neutral layer.** Loop invariants,
  termination measures, lemmas and trigger hints are written in each
  verifier's own syntax. The specification defines how such
  verifier-specific blocks attach to a function without breaking compilation
  for other backends.

### Request and verdict API

One schema serves the library API, the CLI's structured output, the daemon
protocol and the MCP server.

- **Requests** name an input kind (Rust target, SMT-LIB query or batch, TLA+
  job), the backends and tiers wanted, resource limits and caller metadata.
  Requirement IDs travel in source attributes or job metadata.
- **Verdicts** carry:
  - **obligation identity,** stable across runs and unchanged by unrelated
    edits, extending the existing scheme to every backend;
  - **requirement IDs,** where present;
  - **outcome:** verified, refuted with counterexample, unknown, error or
    timeout;
  - **evidence tier,** ordered: *sampled* (runtime-checked contracts under
    property tests), *bounded* (Kani, D.I.V.E. within declared bounds),
    *SMT-verified*, *certificate-checked* (solver proof certificates checked
    independently), *kernel-checked* (Lean, Rocq);
  - **qualifiers:** bounds, arithmetic models, trusted components, and exact
    tool and solver versions;
  - **provenance:** inputs by content hash, backend identity, and whether the
    result came from cache.
- A verdict is never upgraded across tiers; each tier's result stands on its
  own evidence.
- **Two named axes:** establishment (`confirmed` or `provisional`) and
  evidence tier. Output schema v2 introduces both before 0.1.0, so the
  runtime extends v2 rather than breaking it (question 22).
- **Counterexamples and witnesses** use ITF's value encoding everywhere,
  including SMT models and Rust-level counterexamples, so clients handle one
  value format. ITF is a rendering alongside the exact solver output, never
  a replacement.

### TLA+ job interface

Defined in Sail, with D.I.V.E. as its exclusive backend (see
[TLA+ jobs](#tla-jobs-through-dive)), so D.I.V.E. and Sail build against it
concurrently.

### Per-requirement assurance view

For each requirement ID, the ledger answers which relationships in the table
above are established, at what tier, under what qualifiers, and whether each
property is mutation-verified. It is exposed through the API, the CLI and the
MCP server.

## Workstreams

### Verifier backends for annotated Rust

Each backend lowers neutral contracts into its verifier's dialect in a
**shadow copy** of the crate, never modifying the user's source. It runs the
verifier, maps results back to obligation identities with source spans, and
has a conformance harness showing that Sail's verdict equals the verifier's
own verdict on the lowered code.

- **Verus.** A source transformation into `verus!` form, with requirement
  predicates becoming `spec fn`. This is the hardest lowering and needs an
  explicit design. Shadow-crate generation preserves spans for diagnostics.
  The existing fast Verus path (Sail's Verus with the cache) stays the engine
  underneath: lowered shadow crates run on the fork, with stock Verus on the
  same lowered crate as the conformance oracle (question 24). The fork's
  automatic trigger choice serves production runs; the fork also emits the
  triggers it selected as part of the lowering output, and Sail passes them
  explicitly to the stock oracle so oracle comparisons are deterministic
  (question 18).
- **Vermilion.** A Lean back end behind Verus semantics, adding a
  kernel-checked tier for Verus-expressible obligations. It runs on the fork,
  and its pins move whenever `fork.toml` moves (question 24).
- **Creusot.** Annotations on ordinary Rust, discharged through Why3. Why3's
  driver and session architecture is the most mature multiprover design in
  the field. Sail starts by driving Why3 as a sub-orchestrator and moves hot
  paths to direct prover calls only where measurements justify it (question
  16; recorded in an ADR).
- **Prusti.** Annotations on ordinary Rust, discharged through Viper.
- **Kani.** Function contracts plus generated proof harnesses. It is
  SAT-based through CBMC, so caching keys on harness and model identity, not
  SMT queries. Verdicts are bounded-tier, with bounds as qualifiers.
- **Kernel-checked provers.** One generic prover interface, not a
  Lean-specific one. Lean via Aeneas, and Rocq via Aeneas, hax or
  RefinedRust (which reaches unsafe code), are peers under it. Proofs are
  agent-written in the prover's language; Sail runs translation and kernel
  checking, caches compiled artifacts and keeps warm prover processes. The
  unit of work is a translated module plus its proof files; the prover
  toolchain and translator are pinned exactly; the cache key also includes
  the hash of the source crate and of the proof files (question 17).
  Earlier measurements found Lean-backed checking too heavy for an edit loop;
  that is the performance problem to attack.
- **Sampled tier.** Results of requirement-tagged property tests run under
  `cargo test` with runtime contract checks, recorded as sampled-tier
  verdicts. Sail runs `cargo test` itself, and clients may also report
  results; reported results are labeled as reported and never count as
  established in the assurance view (question 25).

### Backend selection and portfolios

The seed-portfolio machinery generalizes from Z3 seeds to backends. A caller
may request a specific backend, a minimum tier, or the strongest available
within limits. Fragility is recorded per obligation per backend. Under racing,
a verdict's trusted base stays exact: the winning backend's identity, never a
blend. Selection is labeled, not gated as a verdict-changing mode, because
every verdict is attributed to the one backend that produced it, equals what
that backend returns on its own, and names it in provenance (question 23). Two
contradictory conclusive verdicts on the same obligation (one verified, one
refuted) raise a loud soundness alarm and are never resolved automatically;
verified on one backend and unknown on another is not a contradiction.

### SMT-LIB service

A generic, cached solver service for external clients: `check-sat`,
`get-model`, `get-unsat-core`, incremental sessions, and batched submission so
a client can send a cluster of related queries in one request. At minimum Z3
and cvc5, with solver identity in every cache key. Queries are content
addressed the same way Verus queries are. Oyster's equivalence-clustering
traffic is highly repetitive across sessions, so hit rates are measured on
synthetic workloads shaped like it.

### Proof certificates

A certificate-checked tier for SMT verdicts: solvers emit Alethe proofs (cvc5
natively) and Carcara checks them independently. Isabelle's in-kernel
reconstruction of SMT proofs is the conceptual precedent. A cached verdict
with a checked certificate is auditable rather than trusted. Certificate
production is opt-in per request, and its cost is recorded in benchmarks.
Certificate-checked results are their own tier and never upgrade an existing
entry. There is no hard cost ceiling: cost is reported and callers set limits
(question 19).

### TLA+ jobs through D.I.V.E.

Embedded through D.I.V.E.'s library API, for these job kinds:

- model check, with invariants, temporal properties and fairness;
- simulate;
- validate an ITF trace against a spec;
- replay spec-generated traces.

Future temporal caching would bind spec content, configuration and engine
identity, with separately qualified ledger tiers and bounds. Current internal
execution does not cache temporal evidence or establish those assurance tiers.
A Sail-owned worker links the pinned D.I.V.E. Rust library. Sail controls cancellation and deadlines by terminating
and reaping that worker, including during binding. No TLC backend, fallback
or Java execution is part of this route. Backend conformance compares against
the pinned D.I.V.E. library directly.

### Mutation gate

The next release runs bounded executable-code mutations in copies, preserving
contracts, proof and ghost code. Report per-function killed, survived and
inconclusive observations, explicit limits and unsupported/omitted work through
CLI, JSON and MCP together. Unknown is never a kill; survivors and zero observed
kills do not prove adequacy or vacuity. Start with existing native Unknown
diagnosis. Scalar contract witnesses retain their separate evidence family.
See the [feature brief](../design/verus-mutation-release.md).

The general requirement-assurance mutation gate is future scope. Existing seeded
benchmark catalogues and bounded neutral campaigns do not establish completion
of the general Verus product.

### Packaging and modes

- **Library mode:** Sail is embeddable, with an in-process mode, so a client
  such as Oyster links it without running a daemon.
- **Daemon mode:** started on demand when available, for caches that stay
  warm across runs (as today).
- **One configuration surface** names which backends are installed and
  where. A missing backend gives a clear, structured "unavailable" result,
  never a silent skip.

### Reach extension to evaluate: Dafny

Dafny invokes Z3 through Boogie as a subprocess, the pattern Verus uses, so the
shim and cache might accelerate Dafny with little added work. Measure the cost
and report it; build it only if the cost is small, since it serves a
different audience from this mission.

## Correctness discipline for new backends

- **Conformance first, per backend.** Before any acceleration or caching on a
  backend, its harness establishes verdict parity with the backend invoked
  directly on a pinned corpus.
- **A speedup that coincides with any verdict difference is a bug,** not a
  result, on every backend.
- **Versions are pinned.** Tool and solver versions are part of every cache
  key and every verdict's qualifiers.
- **Lowering correctness is tested separately from verification.** Each
  backend has a corpus of neutral contracts with known meaning, checked so
  that lowered contracts are equivalent to their neutral source: by SMT where
  both sides are SMT-expressible, otherwise by differential runtime checks.
- **Nondeterminism** follows the charter's handling of Z3 resource-limit
  nondeterminism, generalized (ADR 0009): fragility is measured and
  reported, never hidden.
- **The Verus serving path** keeps its performance and parity throughout;
  every change runs the existing conformance and benchmark gates.

## Dependency graph

**Current order (owner, 2026-11-09, ADR 0109).**

1. Verus-aware mutation (#140, #141), independent of neutral contracts.
2. Alongside: proof health and profiling, changed-proof costs (#158, #244),
   Mac Anvil cold (#232), bounded frontend go/no-go (#162), reach,
   correctness-test gaps and acceptance proof (#368).
3. Kani (#135) as an experimental backend for hand-written harnesses, with
   `arrayvec` 0.7.8 as the pilot fixture; Kani function contracts are not
   adopted as a front end. cvc5 follows inside v0.3.0, in the supported SMT
   runtime only. Certificates get one measurement spike and no product work.
4. Maintain the supported SMT runtime. Neutral contracts, the three contract
   crates and source jobs are removed.
   Requirement attribution stays experimental for now, with keep-or-remove
   decided when `sail mutate` exists. Oyster is abandoned.
5. Temporal packaging waits for D.I.V.E.'s distribution decision.

The old interface-first/SMT-first fan-out is historical. Delivered interfaces
are not a new prerequisite chain for mutation testing.

Each workstream lives in its own crates with strict dependency direction. New
crates and edges are decisions (ADR) and enter `xtask/src/layering.rs`.

### Tracking issues

The broader program remains under
[#146](https://github.com/Chelis-Lang/sail/issues/146). Mutation (#140/#141)
has the next-release milestone; performance uses M11. Other rows are maintained
implemented scope or future/deferred capabilities, as their issues record.

| Current disposition | Item | Issue |
|---|---|---|
| Next-release headline | Verus-aware mutator | [#140](https://github.com/Chelis-Lang/sail/issues/140) |
| Next-release headline | Per-function campaigns through CLI/JSON/MCP | [#141](https://github.com/Chelis-Lang/sail/issues/141) |
| First executable task | Native Unknown diagnosis | [#664](https://github.com/Chelis-Lang/sail/issues/664) |
| Later pilot | Kani on a real ordinary Rust crate | [#135](https://github.com/Chelis-Lang/sail/issues/135) |
| Removed in v0.3.0 | Neutral runtime syntax and IR | [#127](https://github.com/Chelis-Lang/sail/issues/127) |
| Maintain bounded implementation | Versioned request/results, tiers and ITF | [#128](https://github.com/Chelis-Lang/sail/issues/128) |
| Removed in v0.3.0 | Verus profile-one/two lowering | [#130](https://github.com/Chelis-Lang/sail/issues/130) |
| Removed in v0.3.0 | Source-job requirement assurance | [#143](https://github.com/Chelis-Lang/sail/issues/143) |
| Maintain bounded implementation | Embedded runtime and SDK; wider configuration deferred | [#144](https://github.com/Chelis-Lang/sail/issues/144) |
| Maintain Z3; cvc5/handles deferred | SMT-LIB service | [#137](https://github.com/Chelis-Lang/sail/issues/137) |
| Maintain internal extension; packaging held | Native temporal interface / execution | [#129](https://github.com/Chelis-Lang/sail/issues/129), [#139](https://github.com/Chelis-Lang/sail/issues/139) |
| Deferred future option | Kernel provers, Vermilion, Creusot, Prusti | [#131](https://github.com/Chelis-Lang/sail/issues/131), [#132](https://github.com/Chelis-Lang/sail/issues/132), [#133](https://github.com/Chelis-Lang/sail/issues/133), [#134](https://github.com/Chelis-Lang/sail/issues/134) |
| Deferred future option | Sampled tier, certificates, cross-backend portfolios, Dafny | [#136](https://github.com/Chelis-Lang/sail/issues/136), [#138](https://github.com/Chelis-Lang/sail/issues/138), [#142](https://github.com/Chelis-Lang/sail/issues/142), [#145](https://github.com/Chelis-Lang/sail/issues/145) |

## Decisions to record

Existing contract, source-job and profile-two boundaries are recorded in ADRs
0066, 0070 and 0076. New mutation interfaces, any selected Kani adapter, and
future Why3/kernel/certificate designs need their own decisions when selected.
Historical answers to questions 16–27 do not override ADR 0100's current holds.

## Guardrails

- Do not regress the Verus serving path's performance or parity.
- Do not modify users' source; all lowering happens in shadow copies.
- Interface needs may be raised as issues or PRs in the Oyster and D.I.V.E.
  repositories, which are in this line of work; never in upstream or wider
  repositories (question 21; the filing rule in `AGENTS.md`).
- Do not add a dependency from Sail onto Oyster, or from D.I.V.E. onto Sail.
- Nothing is open-sourced or published to crates.io until the owner's
  rollout; the workspace keeps `publish = false` (question 20).
- A backend is forked only with a rigorously measured gain, proven before any
  Sail use of the fork ships (question 26).
- Where this scope and the charter conflict, the charter's correctness rules
  win: record the conflict and ask.

## Conflicts with the charter and current decisions

Each was resolved in the charter's favour until the owner answered on
2026-10-15; the resolutions are recorded below.

1. **Rule 1 names Verus only.** For a new backend, default mode means the same
   verdicts as that backend invoked directly on the same (lowered) input,
   with the pinned versions. This generalizes rule 1; it does not relax it.
2. **Rule 8 and cross-backend selection.** "Strongest available within
   limits", a minimum tier, or racing across backends can return a verdict
   the requested backend alone would not. Any such selection is a
   verdict-changing mode: opt-in, named, recorded in `divergences.md`, with
   separate keys. Racing is deferred today (ADR 0020). **Resolved (question
   23):** selection is not a verdict-changing mode when every verdict is
   attributed to exactly the backend that produced it, equals what that
   backend returns on its own (parity per backend), and names the backend in
   provenance; selection modes are then labeled rather than opt-in. Two
   contradictory conclusive verdicts on one obligation are a soundness alarm,
   surfaced loudly and never resolved automatically. Racing still waits for
   its own ADR.
3. **Rule 4 and wall-clock-bounded tools.** TLC, CBMC, Lean and SMT-LIB
   clients may bound work by time. Outcomes caused by a wall-clock timeout or
   an infrastructure failure are never cached as answers. **Resolved
   (question 26):** measure and report nondeterminism; fork a backend only if
   its measured nondeterminism costs real cache hits.
4. **Rule 3 and ITF everywhere.** Sail keeps caching and replaying the
   solver's exact response bytes; ITF is a rendering on top, never a
   replacement. The current schema already carries the raw `get-model` reply.
5. **Rule 5 and equivalence clustering.** Clustering by logical equivalence is
   Oyster's work. The SMT-LIB service keys exact bytes and does no
   normalization by default.
6. **Rule 7 and evidence tiers.** `confirmed`/`provisional` (how a result was
   established) and the evidence tier (how strong the evidence is) are
   separate axes; nothing provisional is reported as verified at any tier.
   Results Sail ingests rather than produces, such as sampled-tier test runs,
   need their own provenance. "Tier" already names cache tiers, the
   confirmed/provisional field and testing tiers. **Resolved (questions 22
   and 25):** output schema v2 names the two axes, establishment and evidence
   tier, and moves into 0.1.0; reported results are labeled as reported and
   never count as established.
7. **Publishing.** The scope describes the contract syntax as "published from
   this workspace" and also says not to publish crates. **Resolved (question
   20, historical):** publication waited for the owner's rollout. The owner
   subsequently approved eighteen packages at 0.2.0 (ADR 0098); root
   `publish = false` remains a default with explicit selected-crate opt-ins.
   Private temporal extensions stay unpublished.
8. **Filing outside this repository.** The scope asks for interface needs to be
   raised as issues or proposals for Oyster and D.I.V.E. **Resolved (question
   21):** agents may open issues and PRs in this line of work's repositories
   (Sail, the Verus fork, D.I.V.E.), never in upstream Verus or wider
   repositories. `AGENTS.md` lists them. Oyster left the list when the owner
   abandoned it (question 49).
9. **Roadmap review deferral.** The 2026-10-13 review deferred more SMT
   backends, certificates and other verifiers for lack of user pull. This
   earlier owner direction scheduled them after 0.1.0 (question 27).
   ADR 0109 now prioritizes mutation, then experimental Kani followed by cvc5
   in the SMT runtime inside v0.3.0, with the temporal distribution hold kept.
10. **Rule 1 and trigger choice in the oracle.** Stock Verus, the
    conformance oracle for lowered crates, picks triggers differently run to
    run (VI-0015), which would make oracle comparisons flaky. **Resolved
    (question 18):** the fork emits the triggers it selected as part of the
    lowering output, and Sail passes them explicitly to the stock oracle.
    Production runs keep automatic choice.

ADR 0043 is compatible: the Verus backend's "existing fast Verus path" is
Sail's Verus on the fork, and stock Verus on the same lowered crate is the
conformance oracle (confirmed in question 24; Vermilion also runs on the
fork).

## Reporting

When the work starts, it reports: an audit of the code against this scope,
the interface specifications, per-backend conformance status, decisions and
their evidence, disagreements with this scope, and open questions for the
owner.
