# Scope: Sail as a general verification runtime

- **Status:** planned for releases after 0.1.0. Nothing here changes 0.1.0.
- **Source:** owner direction, 2026-10-15.
- **Tracking:** milestone *M10: Verification runtime*, umbrella issue
  [#146](https://github.com/Chelis-Lang/sail/issues/146). Ordering
  against other post-release work is in the [roadmap](../ROADMAP.md#after-010).

Sail today serves Verus. It runs Sail's Verus (built from the fork,
[ADR 0043](../decisions/0043-default-mode-runs-the-fork.md)) with the pinned
Z3 behind a caching shim, and returns the same verdicts and diagnostics as
plain stock Verus. This document records where the owner wants Sail to go
next: one verification server for a family of tools. The
[charter](../CHARTER.md) and its correctness contract govern all of it
unchanged (charter appendix entry 5). Where this scope and the charter
conflict, the charter wins; the conflicts found so far are listed under
[Conflicts](#conflicts-with-the-charter-and-current-decisions).

## Current state

None of this scope is implemented. What exists and is reused:

- one verifier (Verus) and one solver (Z3), with solver identity already
  explicit in keys, the cache and the verdict model;
- content-addressed keys over the exact process prefix (ADR 0008), memory,
  disk and signed shared tiers;
- the ledger and `--output json` schema v1: per-function verdicts,
  `confirmed`/`provisional` tiers, provenance (ADR 0014);
- obligation identities for Verus functions;
- sequential, complete-run seed portfolios and fragility measurement
  (ADRs 0018, 0020); racing is deferred;
- the daemon, persistent jobs, watch and the MCP server;
- the differential conformance harness and benchmark suite.

No interface specification under `docs/spec/` has been written yet. Each is a
tracked work item.

## Mission

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

Two projects are built concurrently in their own repositories. Sail designs
for them and never depends on their internals.

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
  cannot.
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
- **Counterexamples and witnesses** use ITF's value encoding everywhere,
  including SMT models and Rust-level counterexamples, so clients handle one
  value format.

### TLA+ job interface

Defined in Sail, with a TLC adapter behind it (see
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
  underneath.
- **Vermilion.** A Lean back end behind Verus semantics, adding a
  kernel-checked tier for Verus-expressible obligations.
- **Creusot.** Annotations on ordinary Rust, discharged through Why3. Why3's
  driver and session architecture is the most mature multiprover design in
  the field; a decision records whether Sail drives Why3 as a
  sub-orchestrator or reaches its provers directly.
- **Prusti.** Annotations on ordinary Rust, discharged through Viper.
- **Kani.** Function contracts plus generated proof harnesses. It is
  SAT-based through CBMC, so caching keys on harness and model identity, not
  SMT queries. Verdicts are bounded-tier, with bounds as qualifiers.
- **Kernel-checked provers.** One generic prover interface, not a
  Lean-specific one. Lean via Aeneas, and Rocq via Aeneas, hax or
  RefinedRust (which reaches unsafe code), are peers under it. Proofs are
  agent-written in the prover's language; Sail runs translation and kernel
  checking, caches compiled artifacts and keeps warm prover processes.
  Earlier measurements found Lean-backed checking too heavy for an edit loop;
  that is the performance problem to attack.
- **Sampled tier.** Results of requirement-tagged property tests run under
  `cargo test` with runtime contract checks, recorded as sampled-tier
  verdicts.

### Backend selection and portfolios

The seed-portfolio machinery generalizes from Z3 seeds to backends. A caller
may request a specific backend, a minimum tier, or the strongest available
within limits. Fragility is recorded per obligation per backend. Under racing,
a verdict's trusted base stays exact: the winning backend's identity, never a
blend.

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

### TLA+ jobs through D.I.V.E.

Embedded through D.I.V.E.'s library API, for these job kinds:

- model check, with invariants, temporal properties and fairness;
- simulate;
- validate an ITF trace against a spec;
- replay spec-generated traces.

Jobs are cached by spec content, configuration and engine identity. Results
become ledger verdicts: bounded tier where configuration bounds the state
space, with bounds as qualifiers. A TLC adapter behind the same interface lets
the job path, ledger integration and caching be built and tested against the
canonical tool first, and later serves as a parity check on D.I.V.E. through
Sail.

### Mutation gate

A property that kills no mutants of the code it constrains is vacuous. The
gate runs mutants of the bound implementation through the same backend that
verified the property, records per property which mutants it killed, and
flags properties that killed none as vacuous in the ledger.

`cargo-mutants` covers code outside `verus!` but cannot see inside the macro.
A **Verus-aware mutator** mutates executable code inside `verus!` blocks and
leaves ghost and specification code intact. It serves every Verus user,
independent of the rest of this scope.

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

1. **Interface specifications first:** the contract syntax, the request and
   verdict API, and the TLA+ job interface. Oyster and D.I.V.E. build against
   them concurrently.
2. **Then in parallel:** each verifier backend independently; the SMT-LIB
   service; certificate checking (on the SMT-LIB service); the TLA+ job path
   with its TLC adapter; the Verus-aware mutator (needs no specification);
   packaging modes.
3. **Dependent work:**
   - the mutation gate's generic layer depends on the backends it drives;
   - the assurance view depends on the ledger carrying requirement IDs and
     tiers;
   - portfolio selection across backends depends on at least two backends
     sharing obligations.

Each workstream lives in its own crates with strict dependency direction. New
crates and edges are decisions (ADR) and enter `xtask/src/layering.rs`.

### Tracking issues

All are on milestone *M10: Verification runtime*, under umbrella
[#146](https://github.com/Chelis-Lang/sail/issues/146).

| Stage | Item | Issue |
|---|---|---|
| Specification | Neutral contract syntax crate | [#127](https://github.com/Chelis-Lang/sail/issues/127) |
| Specification | Request and verdict API, evidence tiers, ITF values | [#128](https://github.com/Chelis-Lang/sail/issues/128) |
| Specification | TLA+ job interface and TLC adapter | [#129](https://github.com/Chelis-Lang/sail/issues/129) |
| Parallel | Verus lowering | [#130](https://github.com/Chelis-Lang/sail/issues/130) |
| Parallel | Kernel-checked prover interface (Lean, Rocq) | [#131](https://github.com/Chelis-Lang/sail/issues/131) |
| Parallel | Vermilion | [#132](https://github.com/Chelis-Lang/sail/issues/132) |
| Parallel | Creusot through Why3 | [#133](https://github.com/Chelis-Lang/sail/issues/133) |
| Parallel | Prusti through Viper | [#134](https://github.com/Chelis-Lang/sail/issues/134) |
| Parallel | Kani | [#135](https://github.com/Chelis-Lang/sail/issues/135) |
| Parallel | Sampled tier | [#136](https://github.com/Chelis-Lang/sail/issues/136) |
| Parallel | SMT-LIB service (Z3, cvc5) | [#137](https://github.com/Chelis-Lang/sail/issues/137) |
| Parallel | Proof certificates (Alethe, Carcara) | [#138](https://github.com/Chelis-Lang/sail/issues/138) |
| Parallel | TLA+ jobs through D.I.V.E. | [#139](https://github.com/Chelis-Lang/sail/issues/139) |
| Parallel | Verus-aware mutator | [#140](https://github.com/Chelis-Lang/sail/issues/140) |
| Parallel | Library mode and configuration surface | [#144](https://github.com/Chelis-Lang/sail/issues/144) |
| Dependent | Mutation gate | [#141](https://github.com/Chelis-Lang/sail/issues/141) |
| Dependent | Backend selection and portfolios | [#142](https://github.com/Chelis-Lang/sail/issues/142) |
| Dependent | Per-requirement assurance view | [#143](https://github.com/Chelis-Lang/sail/issues/143) |
| Evaluation | Dafny reach | [#145](https://github.com/Chelis-Lang/sail/issues/145) |

## Decisions to record

ADRs, each written when its workstream starts: the Verus lowering strategy,
Why3 integration depth, the kernel-prover interface, neutral quantifier
forms, and the certificate cost policy. Related owner questions are in
[open questions](../open-questions.md) 16–27.

## Guardrails

- Do not regress the Verus serving path's performance or parity.
- Do not modify users' source; all lowering happens in shadow copies.
- Do not modify the Oyster or D.I.V.E. repositories; interface needs are
  drafted here as proposals, and a human decides how they are passed on
  (question 21).
- Do not add a dependency from Sail onto Oyster, or from D.I.V.E. onto Sail.
- Agents do not publish releases or crates; publishing is the owner's
  decision.
- Where this scope and the charter conflict, the charter's correctness rules
  win: record the conflict and ask.

## Conflicts with the charter and current decisions

Each is resolved in the charter's favour until the owner decides otherwise.

1. **Rule 1 names Verus only.** For a new backend, default mode means the same
   verdicts as that backend invoked directly on the same (lowered) input,
   with the pinned versions. This generalizes rule 1; it does not relax it.
2. **Rule 8 and cross-backend selection.** "Strongest available within
   limits", a minimum tier, or racing across backends can return a verdict
   the requested backend alone would not. Any such selection is a
   verdict-changing mode: opt-in, named, recorded in `divergences.md`, with
   separate keys. Racing is deferred today (ADR 0020). Question 23.
3. **Rule 4 and wall-clock-bounded tools.** TLC, CBMC, Lean and SMT-LIB
   clients may bound work by time. Outcomes caused by a wall-clock timeout or
   an infrastructure failure are never cached as answers. Question 26.
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
   confirmed/provisional field and testing tiers. Questions 22 and 25.
7. **Publishing.** The scope describes the contract syntax as "published from
   this workspace" and also says not to publish crates. The workspace keeps
   `publish = false` (ADR 0010). Question 20.
8. **Filing outside this repository.** The scope asks for interface needs to be
   raised as drafted issues or proposals for Oyster and D.I.V.E.; this
   repository's rules forbid agents filing in other projects' repositories.
   Drafts stay here. Question 21.
9. **Roadmap review deferral.** The 2026-10-13 review deferred more SMT
   backends, certificates and other verifiers for lack of user pull. This
   owner direction schedules them after 0.1.0.

ADR 0043 is compatible: the Verus backend's "existing fast Verus path" is
Sail's Verus on the fork, and stock Verus on the same lowered crate is the
conformance oracle (question 24 asks the owner to confirm, and to say which
Verus Vermilion runs on).

## Reporting

When the work starts, it reports: an audit of the code against this scope,
the interface specifications, per-backend conformance status, decisions and
their evidence, disagreements with this scope, and open questions for the
owner.
