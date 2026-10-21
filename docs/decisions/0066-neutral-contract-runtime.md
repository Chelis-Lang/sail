# 0066: Shared neutral contract syntax and checked runtime

- **Status:** runtime implemented; backend qualification separate
- **Date:** 2026-10-21

## Context

Oyster needs contracts on ordinary Rust source, with explicit requirement IDs,
before a backend proves those contracts. Existing Sail input is Verus source;
the established Verus path must retain its verdicts, diagnostics and performance.
The owner authorized the Oyster critical path alongside performance work.

## Decision

Implement the [version-one interface](../spec/neutral-contracts.md) in three
crates with their first implementations: pure shared syntax in
`sail-contracts-ir`, procedural expansion in `sail-contracts-macros`, and the
small checked runtime in `sail-contracts`. The runtime has no verifier or daemon
dependency. Both expansion and later lowering use the shared validator.

Start with scalar formulas, owned results and one requirement guarantee per
function. Predicate purity is checked structurally. Whole-source admission must
resolve and recheck predicate calls; neither an attribute nor a generated name
alone establishes purity. Old values are copied scalar snapshots. All contract
checks and specification arithmetic remain checked in release mode. Runtime
`Int` is checked `i128`; backend `Int` is mathematical, as the owner decided.
Owned results are `Copy` scalars or `Copy` scalar structs. Runtime operator
dispatch and prestate captures use sealed scalar traits. Attribute expansion
is not typed whole-source admission: fields, types and calls must be resolved
and checked again before a backend runs, including user-defined autoderef.

The later lowering adapter produces a content-bound shadow project and source
map and reuses the existing managed Verus runner. It does not edit the caller's
source, change the ordinary Verus route, or claim an SMT proof from runtime
tests. Oyster continues to own contract-to-requirement entailment.

## Consequences

The shim and cache-key dependency graph cannot reach these crates through the
API. Unsupported Rust and contract forms fail explicitly. The first slice
does not claim quantified, trait, generic or arbitrary-effect support. A later
multi-property slice must isolate proof failures before attributing mutation
kills to individual requirements. Runtime fixtures and actual backend
conformance are separate acceptance gates.
