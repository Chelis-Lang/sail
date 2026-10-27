# 0087: Scope long conformance gates to runtime behavior

- **Status:** accepted
- **Date:** 2026-10-27

## Context

The charter says standard conformance gates merges, while the operating guide
names selected components. Treating every proof, documentation or diagnostic
tooling edit as a runtime change imposes a long corpus run without exercising
the changed behavior. The owner approves a scoped policy rather than standard
conformance on every PR.

## Decision

Every PR retains its required fast CI checks and relevant focused tests.
Changes to models, source-shared proved bodies or their checkers retain the
affected complete direct Stock proof suites and ordinary failure controls.

Full fast and standard corpus conformance gate changes to selected runtime
verification behavior: solver interaction, query/reply association, cache reuse
and keys, parsers, verdict/diagnostic production and the differential oracle.
Classify the actual behavior changed, rather than applying this requirement
because a file belongs to a particular crate or contains a cache-related name.

Proof/model-only, documentation and diagnostic-tooling changes do not
automatically require the long corpus. Neither do additions of unselected
proved operations that leave existing runtime paths unchanged. Selecting those
operations for production is a separate change that requires the runtime gate.
Runtime performance changes retain native benchmarks against the prior build
and plain Verus, plus standard conformance. Tooling timing claims require
measurements of that tooling; they are not product performance evidence.

PRs state this classification and identify actual tests and limits. Evidence
may be reused only for demonstrably identical tested inputs and implementation;
state its original source and scope. Reused evidence is not a new execution.
No check, oracle, expected result, proof obligation or resource limit is weakened
to obtain a pass. An interrupted or unrun suite never becomes a success.

## Consequences

Long corpus runs remain meaningful runtime acceptance gates. Small proof and
tooling deliveries can merge on their relevant evidence without repeating an
hour-long suite whose runtime inputs and implementation did not change.
Native runtime adoption, solver/runtime re-pinning and releases retain their
own broader qualification requirements.
