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

## Additional exact producer bindings, 2026-11-15

Recognize the campaign producer `5d0349944af5fae5931d2902c3ae954d07ffaa9b`
and composed producer `e58c3e51ff6f9e54133aa994401352788fb452c6` alongside
`a3109bed`. Their AIR identity format follows the same unchanged consumer
contract. The complete AIR helper, timing emitter, query-emission call sites
and AIR context files are byte-identical between `5d034994` and `e58c3e51`.
Actual native composed output contains 3,127 distinct timing paths, including
207 rows with colliding friendly names. The original consumer named neither
new source and consequently retained conservative ambiguity handling.

Require each complete report to name exactly one recognized producer. Keep
all bounded-path, query/timing agreement, multiplicity and complete-attribution
checks. Unknown sources keep legacy handling; matching path fields alone grant
no authority. Short hashes, changed hashes and mixed producers cannot activate
the named join. Classification and the VI-0003 safeguard are unchanged.
Repository/embedded runtime pins, source lock and qualified manifest constants
are unchanged. This consumer capability does not select either compiler.
Native qualification of the final main-based slice remains required separately
from the combined development candidate.

The main-based capability qualification is recorded separately in the owning
validation record. ADR0087 classifies the two new bindings as support for
unselected producers: the public parser and development override gain precise
attribution, while selected-runtime behavior stays unchanged. That slice uses
native workspace/focused/solver gates, independent source/data review and fast
hosted checks. Full fast/standard and the other compiler/platform gates remain
mandatory before adopting a supporting runtime; this is no runtime gate waiver.
