# 0086: Share actual streaming list framing with the acceptance leaf

- **Status:** proposed
- **Date:** 2026-10-27

## Context

The cached shim checks proposed commands and replies with the acceptance byte
grammar, but `sail-smtlib::split` independently recognizes list boundaries and
owns the bytes it emits. Hash-binding that implementation does not check its
operations. The existing delimiter and `frame_step` functions already describe
the same byte rules, including input comments, bar symbols and doubled quotes.

## Decision

Propose one direct dependency, `sail-smtlib` → `sail-acceptance`, recorded in
`xtask/src/layering.rs`. The acceptance leaf remains dependency-free. Move the
actual pending-list and top-level atom buffers, lexical transitions and absolute
cursor/chunk loop into a private-field shared controller. Native splitters consume
its original owned completion bytes. Pair the input controller with the existing
owned raw transcript in QueryPrefix, returning command items from its actual
append operation.
Delegate the native word-delimiter operation to the existing checked function.
No cross-crate source includes or third-party dependencies are introduced.

Preserve the public splitter/item API, original byte order, arbitrary chunking,
comments, quoted symbols, doubled quotes, malformed/incomplete finish results
and output atom/CRLF behavior. This is a reversible source proposal, not an
accepted runtime or architectural selection.

## Consequences

The added edge cannot form a cycle because the leaf has no dependency on the
parser or another workspace crate. The shim still cannot reach the daemon,
cache, solver pool, portfolio or Verus through this edge. The checked kernel
establishes exact pending bytes and first-root list completion against the
existing `single_list` predicate, with no solver meaning or origin assertion.

The newer outer/raw-input operations are source candidates requiring new direct
whole-root/native qualification. Full transitive emission inventory, output-tape
composition, complete lexical classification and parser/solver interpretation
remain separate obligations under #371. Representable buffer append is an explicit capacity
premise, not evidence of stream completeness or EOF. Normal native, whole-root
proof/control, conformance and added-cost qualification are required; none is
inherited from earlier captures.
