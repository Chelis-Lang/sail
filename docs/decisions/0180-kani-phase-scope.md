# 0180: Kani phase scope, measured before built

- **Status:** accepted by owner (plan approved 2026-11-27); spike wave and checkpoint pending
- **Date:** 2026-11-27
- **Amends:** [ADR 0128](0128-kani-bounded-observations.md), [ADR 0174](0174-v030-native-temporal-scope.md)

## Context

Kani is the next backend after v0.3.1 and the D.I.V.E. symbolic pickup
(ADR 0174). `main` holds a strict decoder and pinned tool payloads (ADR 0128,
0137, 0149); a qualified single-harness executor and CLI sit on
`feat/v030-kani-cli` with no cache.

Kani has no cross-run result cache and runs one independent CBMC process per
harness, so exact per-harness reuse has no session-prefix constraint (ADR
0008). Published CI costs are 21 to 69 minutes per change on Firecracker,
s2n-quic, Hifitime and verify-rust-std. The only Sail measurement is the #135
pilot (18.5 s cold, 0.11 s warm on nine `arrayvec` harnesses), whose baseline
was nine separate `cargo kani` processes and whose key was the whole crate.

Stock Kani 0.68.0 names constant allocations after rustc's `AllocId` (Kani
#4821), so rebuilding the same source can produce different GOTO programs.
Exact-byte keys over CBMC's input stay sound and rarely hit. Most real corpus
harnesses use contracts, stubbing, loop contracts or a pinned SAT solver, all
of which the ADR 0128 decoder refuses.

The full analysis is in [the Kani phase design](../design/kani-phase.md).

## Decision

1. Kani is the headline candidate for v0.4.0. The claim is decided at one owner
   checkpoint after a spike wave; each spike has a numeric pass rule and a drop
   rule and leaves one record in `docs/validation/`.
2. The baseline for every Kani performance claim is one `cargo kani -j N` with
   a warm target, never per-harness processes.
3. Reuse is built in this order: batch execution on a shared target; exact
   receipts over an authenticated input closure; an exact-bytes shim at the
   CBMC boundary; a signed CI tier. ADR 0128's cache ban lifts per layer only
   when that layer's own ADR and conformance pass.
4. A Kani fork for deterministic output is permitted under ADR 0039's rules
   when a spike shows the nondeterminism costs real cache hits (open question
   26). Stock Kani remains the conformance oracle and the fork is dropped when
   a pinned stock release carries an equivalent fix. The owner decides where
   the fork lives and whether the fix is offered upstream; agents record the
   problem locally. This amends ADR 0128's "no backend fork".
5. Existing harness kinds (contracts, stubbing, loop contracts, should-panic,
   generated harnesses, pinned SAT solvers) are admitted for running, one kind
   at a time, each with named evidence qualifiers. Sail still generates no
   harnesses or contracts; the Form B bar in
   [requirements after Oyster](../design/requirements-after-oyster.md) is
   unchanged.
6. Results stay bounded evidence. Timeouts, out-of-memory, crashes and
   unsupported cases are no-claim and never cached. Reporting no-claim where
   direct Kani prints success is a downgrade-only divergence, registered in
   `docs/divergences.md` when it ships.
7. A whole-formula CNF cache and default-mode solver racing are not built.
   Racing may return as a named opt-in mode after its spike.
8. After Kani: SAT solver identity, Bitwuzla and cvc5 through CBMC's SMT path,
   then [Loom](../design/loom-after-kani.md) and
   [Creusot](../design/creusot-after-kani.md), each gated by its own spike.

## Consequences

If the cost-split or history-replay spike fails, Kani ships as a qualified
uncached backend with integrity supervision and the release's weight moves to
mutation testing. Follow-up ADRs 0181 to 0191 are listed in the design
document. Kani performance work needs a multi-node measurement method built on
`scripts/perf-host.sh`. The decoder's CaDiCaL-only and proof-only admissions
must widen before any real-project claim. No command, MCP tool or JSON field
is advertised until its gate passes (ADR 0174).
