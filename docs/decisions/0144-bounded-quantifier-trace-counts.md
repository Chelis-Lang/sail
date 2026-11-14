# 0144: Bounded quantifier trace counts

- **Status:** implemented diagnostic component; native qualification pending
- **Date:** 2026-11-14
- **Tracking:** #712 B

## Context

ADR0138 supplies exact whole-session prefixes; it does not execute a profiler.
The health spike's counter silently accepts malformed fields, omits model-based
instantiations and treats a fingerprint as a unique identity. Pinned Z3 4.16.0
uses a32-bit composite hash, with a separate equality comparison of quantifier
and bindings. A trace hash therefore cannot establish unique ownership.

Primary sources at Z3 tagz3-4.16.0, commitddb49568d3520e99799e364fb22f35fc67d887b1:
[quantifier discoveries](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/smt/smt_quantifier.cpp),
[instance emission](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/smt/qi_queue.cpp),
[fingerprint equality/hash](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/smt/fingerprints.cpp),
[quoted qid definitions](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/ast/ast.cpp).

## Decision

Add an independent bounded diagnostic decoder for this named trace format.
Require the exact tool header, complete newline-delimited EOF, balanced instance
records and modeled event tags. Bound trace/line bytes, qid count/label bytes,
tracked quantifier terms, fingerprint history and nesting. Any structural or
resource refusal is sticky; incomplete input never returns partial counts.

Track quoted qids through the existing lexer, E-matching and MBQI discoveries,
theory origins and actual instance events. Invalidate quantifier mappings when
Z3 reuses a term ID for an application, variable or proof. Group exact printed
qids, including rewrites and separate definitions sharing a label. Hash reuse
with contradictory ownership permanently withholds subsequent attribution for
that hash. Missing ownership is counted explicitly; it never picks a plausible
qid. Hash zero is a valid quantifier hash and differs from a null theory pointer.

A complete trace can have incomplete instance attribution. Report that explicitly
with unassigned counts and an attribution flag; never claim all work belongs to
the displayed qids. Measurements are cumulative over every begin-check in the
trace and carry no proof, function identity or currentness authority.

## Consequences

No solver execution, retention admission, worker lifecycle, CLI/MCP command or
default verification/cache behavior is added. The caller still must authenticate
the solver/session and compare actual plain/traced executions. Unsupported tags,
multiline labels or over-budget traces make measurements unavailable. Fresh
native fixtures and adversarial/boundary tests must qualify this component;
real-project profiling and Mac workflow acceptance remain separate work.
No dependency edge changes; existing proved parsing bodies remain unchanged.

## Qualification scope

The native exact component and fresh independent review pass as recorded in
[validation](../validation/quantifier-trace-counts.md). The API counts ownership;
it does not validate every opaque record or quantifier binding cardinality.
Completed counts, including `attribution_complete`, cannot authenticate a trace
or confer proof/source/currentness authority. Full profiler/worker/CLI and actual
session/plain-trace binding remain separate work. Original rejected lexer and
datatype-null candidates and their strict failures remain retained.
