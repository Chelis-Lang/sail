# 0128: Separate bounded Kani observations from ordinary proofs

- **Status:** accepted design; decoder implementation, executor and qualification pending
- **Date:** 2026-11-13
- **Extends:** [ADR 0109](0109-oyster-retired-v030-scope.md)

## Context

The selected experimental backend runs hand-written Kani harnesses over ordinary
Rust. The pinned pilot is arrayvec 0.7.8, Kani 0.68.0, CBMC 6.11.0 and its
nightly-2026-08-21 toolchain. A fresh native Linux direct pilot reproduces three
distinct outcomes: a passing try-push property, a genuine push panic and an
insufficient unwind bound. Kani's text, process status, summary, selected result
and individual checks must agree. Its JSON export is unstable and versioned.

The original spike is not a production cache design: it does not authenticate
all external compiler inputs. Importing its keyed results would invent reuse
authority. The first decoder slice must stay separate from execution and proof
receipts while the actual backend is built.

## Decision

Add an unpublished `sail-kani` crate, depending internally only on `sail-api`.
The workspace dependency and layering table record this edge; existing third-
party serde dependencies suffice. No daemon/cache/shim edge changes.

Decode only the exact pinned producer's single selected, ordinary hand-written
proof harness. Generated harnesses, contracts, should-panic and loop-contract
modes refuse. Preserve the original failed checks, diagnostics and multiplicity.
Admit `bounded_success` separately from `counterexample`; neither is an ordinary
`verified` result. An unwind-only failure is `bound_insufficient`, with no tier.
Missing, duplicate, contradictory, erroneous or incomplete native records refuse
as SAIL6301. Decode alone confers no execution, currentness or receipt authority.

An eventual executor must bind actual tool identities, original process results,
complete retained captures, declared bounds and backend attribution. CLI and
native direct-Kani conformance are separate implementation gates. Default Verus
and Z3 behavior remain unchanged. No result cache is enabled until exact input-
closure and reuse authority are established; diagnostic storage alone cannot
supply a proof. Kani precedes cvc5 in the implementation sequence.

## Consequences

Bounded checking cannot accidentally become SMT-verified evidence or a default-
mode cached proof. The strict decoder can be tested against native original
exports before subprocess and product integration. The library-only slice is
not an available CLI backend or completed pilot acceptance. Actual arrayvec
positive/negative/unwind, process fault/cancellation, direct conformance, native
Mac and the applicable integration gates remain required. No harness generation,
function-contract frontend or backend fork is adopted.
