# 0079: Source-shared executable acceptance refinements

- **Status:** proposed
- **Date:** 2026-10-25

## Context

The acceptance models establish conditional safety properties, but a copied
predicate cannot establish that production Sail executes the proved logic.
Production also needs explicit concrete-to-model relations for its structured
evidence and state transitions.

## Decision

Place pure executable validation in a dependency-free `sail-acceptance` leaf.
Allow direct edges from `sail-api`, `sail-verus`, `sail-shim`, `sail-cache` and `sail-daemon`.
The leaf cannot reach the verifier, cache, daemon or solver processes. Existing
shim isolation and solver/verifier independence remain enforced by layering.

The same committed Rust bodies compile through small syntax envelopes in
`sail-acceptance/src/syntax.rs`. Cargo erases contracts and proof hints; direct
stock Verus selects the checked envelopes with the fixed `sail_verus` cfg.
Executable arguments, return types, conditions and bodies are preserved.
Proof-only loop invariants and assertions must themselves be checked; they
cannot supply unchecked implementations. No Verus runtime dependency enters
the production dependency graph.

The proof runner captures the complete production source closure and generates
only reviewed module bindings. API declaration stencils must preserve the
authority-read fields and enum variants from their actual declarations, bind
the full original source, and state the representation relation. Dropping
presentation-only fields is not permission to assume report validity, input
completeness, process identity or cache origin.

Each refinement proves concrete operations against the existing models and
identifies its actual runtime callers. Pure-source proofs do not establish
unverified callers or I/O behavior merely because those callers use a checked
helper. The compiler-input, filesystem/process, cryptographic and persistent
writer assumptions retain their named boundaries. Shared and bundled origins
need their own writer contract before entering an origin theorem.

## Consequences

The new leaf and five edges are the dependency-graph change. The leaf remains
small and has no third-party dependencies. The Rust compiler and the small
syntax envelopes remain build dependencies; source capture and declaration
stenciling remain reviewed, tested proof-runner logic.

Production selection requires direct stock proofs, guard-removal controls,
native conformance and cost qualification. A checked helper or first adapter
does not complete the service-wide theorem. Runtime and proof scope, remaining
adapters and actual qualification belong in the owning design and validation
record.
