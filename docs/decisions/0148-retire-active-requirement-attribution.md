# 0148: Retire active requirement attribution after its evidence bar

- **Status:** implemented; native Linux qualification and independent review pass
- **Date:** 2026-11-14
- **Supersedes:** active product surface in ADR0104
- **Tracking:** #712 F, #143; follows ADR0109

## Context

The post-Oyster design requires a real five-function hand join, an owner or
outside-user specification improvement beyond the per-function view, and
actual demand for grouped CI output before adding requirement grouping.
Otherwise ordinary requirement attribution and `sail assurance` are removed
in this release. The hand join now uses original IronKV/Vest campaign data;
its unchecked illustrative tags add no new verifier or mutation observation.
No outside-user improvement or grouped-CI demand is demonstrated. An agent's
judgment cannot substitute for that evidence. The qualification record preserves
missing mutation coverage and Vest's unknown result rather than inventing a
successful requirements pilot.

## Decision

Apply the planned retirement to active entry points. Reject `--requirement`
before project resolution or worker launch. Remove `sail assurance` help and
execution, MCP requirement-assurance listing/dispatch and `requirements` input.
The daemon advertises no attribution capability and rejects both legacy raw
attributed-submission and assurance-read requests with `SAIL3161` before job
lookup or creation. Never silently submit an unattributed replacement.

Keep legacy wire/data types and SDK negotiation for older services and journal
compatibility. Current services refuse those SDK calls through negotiation;
a direct legacy RPC also refuses. Preserve historical ordinary job tags on
disk and allow original ordinary job results to be read. They acquire no new
assurance tier or currentness. Neutral source records remain retired under
their existing rule. Existing ordinary coalescing still distinguishes retained
nonempty tags from new empty-tag submissions.

General compiler/function attribution, evidence tiers, ledger, raw-runtime
requirement IDs and counterexample decoding remain supported. This retirement
adds no requirements language or replacement grouping. A later user-backed
proposal needs its own evidence and ADR.

## Qualification

The corrected implementation qualifies on native Linux with independent review.
Actual CLI/MCP/service refusal and no-worker controls, historical journal/result
restart preservation, complete workspace/solver/E2E/PR gates and book checks
pass at the exact source recorded in the
[qualification record](../validation/requirement-attribution-disposition.md).
Default untagged Verus behavior stays identical; no verdict, conformance check
or proof rule is relaxed. Final required PR checks and release qualification
remain separate.
