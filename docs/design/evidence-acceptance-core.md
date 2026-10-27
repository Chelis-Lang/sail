# Proposed verified evidence-acceptance core

**Status: checked component and composed models; selected production operations
now use source-shared concrete refinements. Native and hosted integration gates
pass; the service-wide safety theorem remains open.**
See the [production refinement guide](acceptance-production-refinements.md). Tracked in
[#368](https://github.com/Chelis-Lang/sail/issues/368).

This proposal targets silent false acceptance in Sail's existing Verus path.
It takes Verus's translation and obligation generation as trusted, including
Sail's pinned fork, just as an ordinary Verus user trusts the underlying
verifier. It asks what Sail must establish before reusing or reporting that
verifier's evidence.

The [charter](../CHARTER.md), [trust model](../trust.md),
[freshness contract](../freshness.md) and [divergences](../divergences.md)
remain authoritative. This proposal does not relax conformance, choose a new
crate dependency graph, select a backend or authorize a runtime change.

## Problem and scope

Fallback handles detected failures. It cannot help when a wrong answer has a
plausible shape, a stale result looks Current, or missing target evidence is
mistaken for complete verification. Four boundaries can cause false acceptance:

| Boundary | Failure to exclude | Required evidence |
|---|---|---|
| Query association | A valid `unsat` from one query is served for another | Exact command prefix, reply association, solver identity and mode |
| Input reuse | A prior proof is reused for different effective code or configuration | Complete input identity, path resolution and observations at the acceptance point |
| Requested scope | A subset or sibling target is accepted as the requested complete run | Unambiguous target identity and complete requested-scope accounting |
| Result authority | Failed, unknown, provisional, stale or inconsistent evidence becomes success | A validated result whose reported verdict follows its evidence |

These obligations apply to immediate verification, durable jobs, repair
acceptance and the structured responses consumed by CLI and MCP clients.
Historical successful results may still be displayed as history; they cannot
authorize a claim about the present inputs.

The following are separate obligations, outside the proposed initial theorem:

- Correctness of Verus translation, its axioms and its encoding of Rust.
- Mathematical soundness of the pinned solvers and trusted verifier runtime.
- Termination, bounded latency, cache effectiveness and performance gains.
- Complete default-mode diagnostic and verdict parity with stock Verus. The
  differential harness continues to enforce that stronger operational contract.
- Other backend families, neutral-contract lowering and temporal execution.
  They need their own refinement before entering this theorem.
- Arbitrary hardware corruption, a compromised OS, or modification of trusted
  executables and local evidence by an actor inside the existing trust boundary.

Timeouts, cancellation, crashes and disconnection remain in the modeled
transition system wherever they can affect evidence publication or reuse. A
proof that acceptance remains safe need not prove that such executions finish.

## The theorem

Let `R` identify the requested package/target/function scope, effective
verification options, mode and runtime. Let `I` describe the effective inputs
at a defined observation point `t`. Let `E` be the retained evidence history.

The proposed safety theorem is:

```text
AcceptCurrentComplete(R, report, t)
  implies there exists E such that:
    BoundToEffectiveInputs(E, R, I, t)
    and CoversRequestedScope(E, R)
    and EstablishedByTrustedVerification(E, I, R)
    and FaithfulReport(report, E, R, t)
```

`EstablishedByTrustedVerification` means that the trusted verifier completed
the requested work successfully, using only solver replies justified for
their exact queries. It does not mean that every function has exactly one
SMT query: Verus may issue multiple queries and follow-ups or discharge work
without a solver query. Absence of recorded obligation detail is not itself
proof that no obligation existed.

The theorem is about all executions permitted by the model, not a selected
corpus. It is conditional on the assumptions below. The initial implementation
target is default mode; a named opt-in mode needs its own acceptance relation
and disjoint namespace before it can be included.

`t` must be an actual observation/acceptance point defined by the API. A
response cannot promise that a shared writable workspace stays unchanged after
the final observation, while the response travels to its reader. Stronger
atomic-snapshot semantics would require a correspondingly stronger filesystem
interface. Existing documented freshness exclusions cannot be silently omitted
from the theorem's assumptions.

## Assumptions and evidence origins

The proof's assumption inventory must name the following boundaries rather
than hiding them behind an abstract successful-result predicate:

1. **Trusted verifier and solver.** Their pins, executable identities,
   arguments and session mode identify the actual processes that ran. A
   version/hash observation followed by an unrelated path-based launch is not
   an atomic identity guarantee. Runtime immutability or a stronger launch
   binding must justify this interface.
2. **Compiler input observation.** The modeled closure includes every input
   the actual compilation consumes, including selected configuration,
   environment, build-script inputs and generated includes. Completeness is
   an obligation at this interface, not an assumed consequence of hashing.
3. **Filesystem/process interfaces.** Their modeled behavior includes path
   lookup, symlinks and ancestor resolution, process output and termination,
   publication ordering and failures. Unsupported observations refuse reuse
   or remain explicit assumptions.
4. **Cryptography.** Byte equality is the logical identity relation. Replacing
   it with a digest comparison needs an explicit collision-resistance
   assumption, or an additional byte comparison. Hashes are not
   mathematically injective. Signatures authenticate writers; they do not
   prove an SMT answer.
5. **Initial persistent evidence.** Either it satisfies the invariant when
   loaded, or the implementation validates it into an admissible state.
   Preserving a valid in-memory cache does not prove a restarted process's
   initial cache valid.
6. **Build and deployment.** The verified executable logic is the logic
   compiled into the shipped program. The Rust compiler, linker and any
   unverified adapters remain named trust dependencies until separately
   covered.

For existing reply caches, two proof strategies must be distinguished:

- **Verified-origin strategy:** prove that every admitted writer associates
  an answer with the exact query and that persistence/recovery preserves that
  association. Assume the protected local store is initially valid. Shared
  and bundled signers additionally need an explicit trusted-writer contract;
  verifying a signature alone does not establish that contract.
- **Independently checked strategy:** require a certificate or other evidence
  whose correctness a smaller checker establishes for the exact query. This
  would reduce writer trust, but it is separate work
  ([#138](https://github.com/Chelis-Lang/sail/issues/138)).

A handshake repair is useful admission hardening. It cannot make a plausible
wrong `unsat` independently detectable and does not complete either strategy.

## Proposed implementation boundary

The core is an acceptance authority over typed evidence. The production
candidate places pure shared logic in a dependency-free leaf and typed report
factories in the API crate. [ADR 0079](../decisions/0079-source-shared-acceptance-refinements.md)
records the accepted graph; native integration and the bounded cost result are
recorded in the production qualification. Broader release performance remains
separate.

The intended logical interfaces are:

| Concept | Meaning | Construction authority |
|---|---|---|
| `QueryBinding` | Solver/mode identity plus exact session prefix and command endpoint | Verified session/association logic |
| `BoundReply` | A reply justified for that query by an admitted origin | Verified writer/recovery path or an independent checker |
| `InputBinding` | Effective input bytes, lexical path bindings, relevant environment and runtime | Validated observation adapter plus verified binding logic |
| `RequestedScope` | Exact targets and partial/full verification selection | Validated request and Cargo/compiler identity mapping |
| `CompletedEvidence` | Successful complete work for a particular binding and scope | Verifier-output validation and scope accounting |
| `ValidatedResult` | Consistent verdicts, establishment, scope and currentness | The acceptance core; not arbitrary deserialization |

These conceptual interfaces cover a broader boundary than the implemented
source-shared authority tokens. Constructors must
prevent arbitrary callers or deserializers from minting established evidence.
Wire/storage structs remain candidates until validated. A provenance string,
checksum, signature, process exit code or `complete: true` field cannot alone
construct `CompletedEvidence`.

The runtime shell may request candidates, schedule work and miss caches. It
must obtain authority from the core before claiming current complete success.
Presentation may render historical or provisional observations separately,
but cannot promote them through an independent success path. Every CLI, job,
repair and MCP success surface needs an explicit mapping to this authority.

## Component proof obligations

### 1. Query and reply association

Prove a state-machine invariant linking each output-producing command to its
exact raw-input endpoint and its corresponding response bytes. Include
fragmented reads, empty queues, multiple replies of the same plausible shape,
follow-up replies, unsupported commands and failure transitions.

After a cache-to-solver transition, the solver receives the exact original
prefix and later input. Suppressed output corresponds exactly to replies
already served. A failed connection cannot supply a reply to a later request.
Publication requires complete admitted output and the existing clean-session
diagnostic policy. Restart and shared/bundled admission preserve the same
query association, solver and mode identity.

Initial code to relate to this model includes `sail-key::PrefixHasher`, the
shim's cached-session state machine and pairer, the daemon cache protocol,
and cache persistence/recovery. See
[#342](https://github.com/Chelis-Lang/sail/issues/342) for the handshake gap.

### 2. Effective input identity and freshness

Prove that evidence reuse binds the actual compiler input, not merely a set
of file contents. Include lexical source paths, source and ancestor symlink
resolution, resolved file bytes, declared absence, target selection,
configuration and relevant environment/runtime inputs.

If two files are already hashed, changing `src/lib.rs` from one file to the
other still changes the effective input. A map containing only canonical
file contents loses this distinction. A content-revert proof must establish
restoration of the full effective binding, not only a subset of digests.

Prove the relationship between fresh snapshots, scoped target revisions,
compiler-declared inputs, receipts and read-time refresh. Mutation observations
and scheduling hints must not become independent proof authority. Explicitly
account for generated inputs and observations learned only after compilation.
See [#343](https://github.com/Chelis-Lang/sail/issues/343).

### 3. Requested-scope coverage

Prove that requested targets are accounted for with an unambiguous identity
including their source/compiler invocation where necessary. Rust's normalized
crate name is not a unique Cargo target identity: `foo-bar` and `foo_bar` can
be different targets in the same package.

Coverage must establish that every requested target has matching completed
evidence; a first match cannot cover two distinct requests. Failed invocations
without Cargo artifacts remain accounted for. Partial reports authorize only
their exact scope and cannot construct whole-target evidence. Dependency
reports cannot substitute for requested-target evidence. See
[#344](https://github.com/Chelis-Lang/sail/issues/344).

The trusted verifier supplies the obligation-completeness fact for the exact
invocation it executed. Sail must prove that this invocation is the one the
request required. Merely checking that all reported functions passed does not
prove that the complete requested invocation occurred.

### 4. Report and acceptance authority

Derive summaries from validated function/target evidence or prove their
consistency before consuming them. Provisional, failed and unresolved evidence
cannot authorize verified acceptance. A Current label cannot convert a failed
proof into a successful one, and an old success cannot establish Current.

Prove the whole-target and repair predicates, including exit/termination,
verifier-error, engine/fallback, scope and invariant checks. Retained result
frames must preserve the validated evidence and use a freshly justified
currentness projection. Check the correspondence of human output, JSON, MCP
and repair acceptance rather than assuming all callers use one predicate.
See [#345](https://github.com/Chelis-Lang/sail/issues/345).

## Models, implementation proofs and refinement

TLA+ is suitable for the association, publication, concurrency and recovery
state machines. Finite model checking is a bug-finding stage over the stated
configuration; it is not an unrestricted proof. An inductive invariant proof
can establish safety for the modeled state space. Any checker used for design
research is separate from Sail's temporal backend; D.I.V.E. remains that
backend under ADR 0065.

Stock Verus is the proposed implementation-proof checker for executable Rust
logic. Its state-machine and tracked-state facilities can connect transitions
to code. Supported Rust/library scope must be checked against the selected
pin; arbitrary existing I/O, serialization and service code does not become
verified simply by being called from a proved function.

Each implementation module needs a refinement obligation: concrete states
represent modeled states, constructors establish invariants, every concrete
transition preserves the relation, and every acceptance output maps to the
theorem's acceptance event. Copies or simplified sketches prove only their
own algorithms unless this relation is established.

External specifications must describe low-level observable behavior. For
example, assuming that a filesystem function returns "all effective inputs"
would assume away the very completeness property being proved. The same
applies to an external function assumed to return "a correct cache entry."
Every `assume`, `external_body` and external specification needs a reviewed
justification and an entry in the assumption inventory.

## Direct stock-Verus checking

Proof checking must be independent of Sail's own acceptance path:

1. Pin an upstream stock Verus release, source revision, archive/runtime
   digests, compatible Rust/vstd and Z3 identity. The stock pin in
   `corpus/toolchain.toml` is the starting candidate, not an unqualified
   assertion that it accepts the eventual proof code.
2. Invoke the validated stock executable directly on proof-bearing modules
   with the approved complete verification scope. Do not call `sail verify`,
   Sail's managed driver, the fork engine or `sail-z3` to establish this proof.
3. Set the solver path explicitly to the validated pinned Z3. Clear inherited
   shim, engine, cache and injected verification-argument settings. Audit
   Cargo/launcher arguments so focus, `--no-verify` or similar options cannot
   silently reduce the proof scope. Required trusted assumptions must be
   explicit rather than hidden behind flag choices.
4. Require both normal successful termination and complete verification
   evidence for the expected modules. Bind executable source, proof source,
   dependencies, assumptions and invocation to the checked commit. A test
   fixture, stale build or cached receipt cannot substitute for this run.
5. Confirm the proof gate fails when a required assertion/invariant is broken,
   a proof module is omitted, scope is narrowed, or the selected verifier or
   solver is replaced. Track assumption-inventory changes as reviewable proof
   changes. Never use an inconsistent assumption to make a negative control pass.

This avoids using Sail to certify its own acceptance mechanism. It does not
remove trust in stock Verus, its solver, the Rust toolchain or the declared
environment interfaces.

## Delivery and acceptance

| Stage | Deliverable | Completion criterion |
|---|---|---|
| A | Repair the audited defects and admission gaps | Regression controls plus existing native gates; evidence levels remain explicit |
| B | Freeze theorem, interfaces and assumption inventory | Reviewed models include path bindings, unique target identities and admitted evidence origins |
| C | Prove pure acceptance and coverage logic | Direct stock-Verus proofs plus positive and deliberately broken controls; production paths use that logic |
| D | Prove query association and cache-origin preservation | Session, writer and recovery refinements cover every admitted origin, or unsupported origins are excluded explicitly |
| E | Establish input-observation refinement | Actual compiler-input closure and freshness behavior match the model on Linux/macOS within stated assumptions |
| F | Compose the safety theorem and qualify integration | Every success surface is covered at the exact source/runtime; conformance and performance requirements still hold |

Stages C and D may expose a smaller useful proved boundary before E and F.
That boundary must be named accurately: a verified predicate conditional on
input evidence is not an end-to-end verified service. [Component and composed models](acceptance-proof-checker.md) provide
machine-checked model evidence, including explicit origin/observation lifecycles. Stage C now has source-shared executable operations and typed factories with
concrete refinements. Its final integration gates remain open; stages D–F
still need complete cache origins, observation adapters and composition.
Those stages are not complete.

Fast proof checks should enter CI only after their measured duration fits the
repository's fast-tier budget. Longer model exploration and whole-corpus
acceptance remain local native-host gates. Proof timeouts fail the gate;
smaller scopes or assumed lemmas cannot silently replace the selected theorem.
Code changes keep the existing conformance gates and performance requirements;
any extra hashing, validation or copies need native cost measurements before
selection. This document proposes no performance result.

Evidence follows ADR 0038: one concise owning acceptance record per gate,
actual source/runtime/environment and scope, with failed attempts visible and
raw data outside the repository. Do not label a model-check pass as an
implementation proof or a proof build as integrated acceptance.

## Remaining decisions before production integration

- Approve the first theorem and observation-point semantics, including how
  current documented filesystem exclusions enter the assumptions.
- Select the verified-origin or independently checked cache strategy, and
  state which shared/bundled writers the first theorem includes.
- Review the source-sharing candidate and ADR 0079; complete its layering,
  conformance and added-cost qualification before selection.
- Qualify final changed source with the compatible pinned stock checker on
  both native platforms and review its explicit trust dependencies.
- Define the scope and measured cost budget for each integration stage.

The initial component models do not select a new runtime architecture or
establish the proposed composed theorem.

## Primary references

- [Verus overview](https://verus-lang.github.io/verus/guide/).
- [Verus transition systems](https://verus-lang.github.io/verus/state_machines/).
- [Verus assumptions and trusted components](https://verus-lang.github.io/verus/guide/tcb.html).
- [Supported Rust features](https://verus-lang.github.io/verus/guide/features.html).
- [TLA+ specifications and finite model instances](https://lamport.azurewebsites.net/tla/model-popup.html).
- [TLA+ Proof System](https://proofs.tlapl.us/doc/web/content/Home.html).
