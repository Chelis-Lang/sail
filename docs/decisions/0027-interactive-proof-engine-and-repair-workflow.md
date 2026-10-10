# 0027: Interactive proof engine and repair workflow

- Status: accepted; implementation and acceptance pending
- Date: 2026-10-10

## Decision

M7 adds an optional Sail-managed source variant of the primary pinned Verus.
Stock verification remains the default. `sail engine install` installs an
isolated variant whose manifest binds the Verus commit, Rust toolchain, Sail
patch digest and runtime artifact hashes. `sail engine status` validates that
manifest. Interactive requests reject missing or incompatible installations;
an engine fault invalidates its preparation state and falls back to stock
verification without manufacturing a successful receipt.

`sail repair-check --function F --engine interactive` performs a focused check
and an independent full validation in one compiler callback for one source
revision. Package and library/binary selection are explicit. Rust checking
starts afresh after an edit. Rust compiler contexts, source maps and borrowed
diagnostic contexts never survive into a different source revision. Focused
evidence is provisional. The full phase preserves stock operation ordering,
mandatory termination and safety checks, deferred diagnostics and the final
erased EXEC compilation. Only a complete successful full result whose input
revision is still Current can accept a repair. Successful functions from
different phases, revisions or portfolio attempts are never stitched together.

The private source adapter uses `SAIL_ENGINE=interactive`,
`SAIL_ENGINE_FUNCTION` and a separate `SAIL_PROOF_EVENTS` JSONL sidecar. The
engine's ordinary full output remains authoritative. A source progress event
cannot confirm a target before final compiler sealing. Focused diagnostics
must not suppress an independently emitted full-phase diagnostic through
compiler deduplication. Raw compiler span data remain bound to their owning
callback and thread.

Preparation reuse caches lowering, not verification verdicts. Keys bind all
semantic global inputs, exact bucket membership and order, settings, source
and span bindings, and naming/counter state. Entries restore all mutable
lowering and diagnostic effects. Unknown coverage, changed spans or incomplete
checkpoint support cause a miss. The global preparation budget is 512 MiB,
with one active workspace verification worker. The engine preserves stock Verus
bucket concurrency; a global preparation gate serializes cache admission,
encoding and restoration. Batch-local reuse and reuse across edits
are reported separately: an ephemeral Cargo driver must not claim persistent
cross-edit cache hits.

## Product contract

Repair output retains separate focused and full ProjectResults, exact target
identity (package ID, name and kind), revision/context identity and engine
provenance. Background repair validation uses the same durable job machinery
as CLI and MCP verification. Submission options negotiate a new worker
capability/socket version so an old worker cannot silently ignore interactive
engine or repair semantics.

Proof events use a separate version-one journal; existing job event variants
retain their meaning. A producer supplies phase and target evidence. The
daemon wraps it with job, stream, revision/context and monotonic sequence
identity. Cursor reads expose gaps and completion. Reconnects and duplicates
are idempotent; stale, cancelled or unsealed events cannot update a newer
revision's confirmed projection. CLI JSONL follow and MCP event reads consume
that same projection.

`sail explain` and MCP `explain_failure` return deterministic evidence bundles:
the attributed obligation, spans, original diagnostics, available structured
and raw models, solver effort/reason and provenance. Missing or uncertain
attribution is explicit. Model assignments are evidence, not automatically
generated executable tests. No automated proof editing is introduced.

Named baselines save immutable result documents independently of the rolling
job retention limit. `sail compare` and MCP compare classify newly failed,
unknown, recovered, removed and unobserved obligations, and expose scope or
context incompatibility rather than guessing equivalence after a refactor.

## Freshness and responsiveness

A shared daemon observer owns content snapshots and currentness. Reuse of
digests requires uninterrupted complete precise native observation, including
runtime files, aliases and absent configuration paths, with synchronous
barriers before publication and reads. Timestamps alone never identify input
content. Overflow, unavailable coverage, restarts or observation loss
invalidate the projection until revalidation. macOS retains conservative
validation where equivalent coverage is unavailable.

Status and result requests use that projection, independently of the active
verification worker. Established reads must meet less than 200 ms p95 on
Linux under active verification and concurrent clients. The existing client
deadline remains unchanged.

## Acceptance

Freeze the human-eval-verus and Vest repair catalogues before optimization.
Both include body, specification and refactor edits, failing attempts and
repairs. Alternate M6 focused-then-full runs and M7 batches over identical
scopes, dependencies and warm SMT caches. Record at least three rounds,
median and spread, first actionable evidence, tails, RSS and preparation
hits/misses. Linux x86-64 must show at least a twofold median improvement on
**each** catalogue. On 2026-10-10 the owner directed implementation and
acceptance to run in the local Linux container, including its emulation;
native-host reference measurements will be rerun by the owner later. Record
the actual execution environment with every result. Native-host availability
does not block this phase. Synthetic fixtures do not replace the two real
catalogues, and a failed speed gate remains visible.

Correctness compares full verdicts and diagnostics with independent stock
runs, and cached versus fresh lowering solver input bytes from the same
compiler input (the owner amendment below). Include full regressions after a
successful focus, late borrow/EXEC
errors, bitvector failures, recursive obligations, duplicated diagnostics,
stale span checkpoints, preserved-mtime content edits, aliases/configuration
changes, observation loss, corrupt preparation, cancellation, restart and
event reconnects. Run the full local fast and standard corpus and affected
source/deep tests. Long source builds and benchmarks remain local-only; each
required CI job stays below ten minutes. CI uses validated pinned engine
artifacts and rejects a manifest with a stale patch digest.

## Owner amendment: the source-engine byte oracle (2026-10-10)

VI-0001 makes independent stock compiler invocations emit different
`arrow_1`/`arrow_2` declaration orders for identical source. The failed
qualification and four fixed-order controls are retained under
`engine/verus/candidates/qualifications/`; no old failure becomes a pass.
The owner approved comparing cached lowering with fresh lowering from the
**same compiler input**, keeping stock verdict and diagnostic checks and
reporting stock transcript instability.

The byte oracle observes actual ordered bytes written to every pinned solver
session, including framing and follow-up requests. Both arms begin with
identical current compiler inputs and independently owned mutable lowering,
naming, reporting and solver state. Compare lowering and final effects,
grouped/immediate diagnostics and outcomes as additional checks. The fresh
arm performs stock lowering directly. Shadow checks cannot mutate production
proof events, counters, preparation entries or the authoritative result.
Incomplete capture, unsupported state or a mismatch fails qualification.
Post-validation corruption of a real restored hit must be detected; rejecting
an entry before restoration is insufficient negative evidence.

This changes the source-engine byte comparison baseline, not query bytes or
default-mode replay. No normalization is authorized. Full stock verdicts and
diagnostics, final compiler sealing, Current acceptance, memory limits, local
corpus coverage, real-reader deadlines and both twofold speed gates remain
mandatory. A new source patch requires its own exact build and qualification
before selection; the existing runtime is unchanged by this decision.

## Consequences

This phase delivers an evidence-driven repair workflow through CLI and MCP.
Editor integration, automated proof edits, new SMT backends, certificates,
pruning, instantiation replay and a Rust pre-solver remain later work. The
default verdict contract and unknown serving threshold do not change.
