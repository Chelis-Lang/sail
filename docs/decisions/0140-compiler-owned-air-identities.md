# 0140: Join AIR queries with compiler-owned function identities

- **Status:** accepted consumer integration; native Linux qualification and bounded reviews pass; supporting runtime unselected
- **Date:** 2026-11-14
- **Tracking:** #712 package A, VI-0010

## Context

The native three-function VI-0010 reproduction and IronKV expose repeated
friendly names for distinct trait implementations. The fork already emits
`function-path` in timing JSON, but AIR headers still carry friendly names.
The existing ambiguity refusal consequently makes attribution partial. In a
failing crate the VI-0003 safeguard then refuses to confirm successful timing
entries. Neither refusal can safely be removed.

## Decision

Emit a single-line, JSON-escaped identity immediately before each AIR query,
after source-controlled comments and lazy AIR startup. Use the same compiler-owned VIR path already
present in timing JSON. Keep the legacy headers, SMT logs, solver pipe bytes,
verdicts and diagnostics unchanged. Generate metadata only when AIR logging is
requested. The ordinary selected runtime, repository and embedded fork pins, shipped
artifacts and qualified-manifest constants remain unchanged. This consumer
capability does not select the supporting compiler.

Consume this format only for the exact supporting compiler commit, established
by its structured build information. Require the metadata and timing row to
agree, with one unambiguous timing identity. Keep unique public friendly names;
use the distinct VIR path for colliding friendly names. Missing, malformed,
duplicate or mismatched identities must leave attribution incomplete. Legacy
runtime behavior stays covered by its existing tests. Keep the global VI-0003
safeguard unchanged; complete attribution must be established by actual query
joins, never inferred from timing success.

## Consequences

This corrects structured observation under ADR0039 without changing verification.
No dependency or normalization change. Native positive/negative minimal controls,
real IronKV, exact SMT/verdict/diagnostic comparisons, full corpus, determinism,
fork tests, independent review and both platforms remain qualification work.
No build, fixture or static review substitutes for those gates.


Current results and preserved attempts are in
[the identity validation record](../validation/compiler-owned-air-identities.md).
