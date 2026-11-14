# 0139: Explain actual optional-retention publication failures

- **Status:** implemented; native Linux integration and focused review pass
- **Date:** 2026-11-14
- **Extends:** ADR0136
- **Tracking:** #712 package C, #595

## Context

Optional publication ignores archive failures so verification stays independent
of retention. A caller consequently cannot distinguish unavailable compiler-read
bytes, a failed storage admission or an archive write failure. The existing
missing build-script-export explanation already landed in #708; recovery from
that missing export remains separate work.

## Decision

Return private fixed-stage declines from the actual publication checks. Preserve
all predicates, check order, filesystem operations and proof/source authority.
An entry-level archive failure continues to the next candidate as before;
outer publication-admission failures stop publication as before. Successful
publication stays silent. Accumulate at most one counter for each of sixteen
fixed stages, with no per-operation journal or captured paths.

Append aggregate SAIL3146 notes to the existing result after optional publication.
Human stderr shows them only with SAIL_EXPLAIN=1. These observations cannot alter
currentness, exit status, receipts or a verdict. No additional file is read to
infer a cause after failure. A stage names the check that refused and does not
claim a more specific operating-system cause.

## Consequences

Publication refusals become visible without making storage mandatory for proof.
No recovery, cost policy, cache key, dependency or platform interface changes.
Typed restoration reasons and any changed publication policy remain separate.
Focused archive/refusal tests, full native gates and independent differential
review are required before landing this slice. No corpus or performance pass is
inferred from contributor or fixture checks.

Qualification and preserved attempts are recorded in
[the validation record](../validation/retention-publication.md).
