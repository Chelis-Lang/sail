# 0082: Check query-key framing in the acceptance leaf

- **Status:** proposed
- **Date:** 2026-10-26

## Context

The default v2 key binds exact solver identity, mode and raw solver-input bytes.
The shim's checked prefix operation supplies exact slices, but the production
key layer previously framed and fed those slices outside the stock proof.
A copied encoder would leave that production boundary open.

## Decision

Allow the direct `sail-key` → `sail-acceptance` dependency. Put the incremental
key controller in the existing dependency-free acceptance leaf and invoke its
same Rust bodies from the production `PrefixHasher`. The private SHA adapter
retains only fresh-state, exact update, state copy and finalization primitives.
Their cryptographic implementation remains an explicit primitive boundary;
no hash or digest establishes a truthful solver origin.

Preserve the exact v2 domains, field lengths, solver/mode byte fields, raw byte
order, end marker, cumulative length and nested silence-key framing. Counter
overflow permanently removes exact-length authority and disables cache work;
it never changes the forwarded solver-input bytes.

Extend `sail_trait!` only with the literal optional `: Sized` supertrait needed
for checked primitive methods returning an owned state. Both Cargo and stock
Verus retain that same bound. General supertrait syntax is outside this change.

## Consequences

The leaf still has no dependencies and cannot launch a process, reach a cache
or interpret a verdict. Existing cache and shim paths already depend on the
leaf and key crates; the added edge introduces no cycle and preserves shim
isolation from the daemon and verifier. Layering checks enforce the one edge.

The direct stock proof checks production framing, counter transitions and
unambiguous byte encoding. Native golden controls retain the preexisting key
format. SHA-256 behavior and collision resistance retain their existing trust
boundary. Cache writer truthfulness, authenticated launch binding and the
service-wide refinement require their own proofs and provider contracts.
