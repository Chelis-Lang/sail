# 0086: Share actual stream framing and lexical operations

- **Status:** proposed
- **Date:** 2026-10-27

## Context

The native SMT parser owns stream buffers and token cursors. Pure acceptance
predicates alone do not establish that these operations follow their byte
grammar. The acceptance leaf already supports identical Cargo/Stock bodies
(ADR 0079), but the parser currently has no dependency on that leaf.

## Decision

Propose the single edge `sail-smtlib` → `sail-acceptance`, recorded in
`xtask/src/layering.rs`. Keep the leaf dependency-free and add no third-party
crate or cross-crate source include. The edge cannot form a cycle or give the
shim a path to the daemon, cache, solver pool, portfolio or Verus.

Move actual list/atom buffers, lexical state, absolute cursor, ordered chunk
emission, copy and explicit finish operations into private-field shared types.
Share the actual borrowed lexer cursor, token declarations, endpoint/digit
operations and sticky error handling. Native public splitters and lexer retain
their existing methods and raw data fields through thin adapters. Native error
conversion remains bound source, outside the direct Stock theorem.

The structural grammar is a separate leaf module. The existing acceptance
query/input/authority modules and their captured programs remain unchanged.
Command classification, command-head convenience operations, original raw
owners, Session policy, replay and cache publication remain later slices.

## Consequences

The theorem describes the actual supplied byte sequence and exact ordered
emitted items/token spans. It preserves comments, quoted symbols, doubled
quotes, arbitrary chunk boundaries, malformed forms, input delimiters, output
CRLF/atom behavior and incomplete finish results without normalization.

Representable cursor, buffer and output-vector bounds remain explicit
preconditions. A native wrapper call does not prove those lifetime bounds;
explicit finish does not prove physical EOF. Solver interpretation, external
raw-owner/queue joins and truthful origins remain open. Runtime selection
requires new complete-root Stock discovery/controls, native compatibility,
conformance and measured cost. This reversible source proposal is not that
selection or completion of #371.
