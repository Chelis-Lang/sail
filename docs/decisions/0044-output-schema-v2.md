# 0044: Output schema v2: establishment and evidence tier

- **Status:** accepted (owner answer to open question 22, 2026-10-15)
- **Date:** 2026-10-15
- **Amends:** [ADR 0014](0014-attribution-ledger-and-output-schema.md)
  (field names and versioning) and
  [ADR 0023](0023-managed-incremental-workspace-verification.md) (envelope
  version)

## Context

Schema v1 had a per-function `tier` field holding `confirmed` or
`provisional`. That records how a result was *established* (charter rule 7),
not how strong its evidence is. The verification runtime
(`docs/scope/verification-runtime.md`, #128) adds an ordered evidence tier:
sampled, bounded, SMT-verified, certificate-checked, kernel-checked. "Tier"
also already names cache tiers and testing tiers. Keeping `tier` would force a
breaking change right after 0.1.0. The owner's answer: rename now, before
0.1.0, and ship 0.1.0 on schema v2 with two clearly named axes. Nobody depends
on v1 yet. Counterexamples get ITF rendering alongside the exact solver
output.

## Decision

**Function fields.** `tier` is removed. Each function carries:
- `establishment`: `confirmed` or `provisional`, with v1's `tier` semantics
  unchanged. A provisional function's verdict is `provisional`, and nothing
  provisional is ever `verified`.
- `evidence_tier`: one of `sampled`, `bounded`, `smt_verified`,
  `certificate_checked`, `kernel_checked` (ordered weakest first), or `null`.
  It is set exactly when the verdict is settled (`verified` or `failed`), and
  is `null` for `unknown` and `provisional`. Every verdict Sail settles today
  comes from the pinned SMT solver through Verus, so it is `smt_verified`.
  The other values are reserved for the verification runtime, which extends
  v2 additively.

The report's invariant check enforces both rules. Live proof observations
(`function_observed` events) must be provisional with a `null` evidence tier.

**ITF values.** Every counterexample assignment gains `itf`, the value in the
ITF (Informal Trace Format) value encoding: `Bool` as a JSON boolean, `Int` as
`{"#bigint": "[-][0-9]+"}`, a bit vector of at most 128 bits as its unsigned
`#bigint`, and anything else as `{"#unserializable": "<verbatim value>"}`.
It is a rendering only (charter rule 3): the verbatim `get-model` reply
(`model`) and each verbatim `value` stay in the document unchanged, and the
cache still stores and replays the solver's exact bytes.

**Versions.**
- `sail verify --output json`: `schema_version` 2,
  `crates/sail-api/schema/verify-output-v2.json`.
- The managed `sail-project-result` envelope embeds these reports, so it
  moves to `schema_version` 2 (`project-result-v2.json`). Stored v1 project
  results and baselines are rejected by the existing version checks and
  re-established by a new run.
- The proof-event, proof-page and repair-result contracts keep their own
  version 1. They are first released with 0.1.0 and now reference the v2
  report definitions.

No v1 compatibility shim: v1 was never released. Within v2, fields are only
added, as ADR 0014 set for v1.

## Consequences

- Consumers read `establishment` where they read `tier`, and may read
  `evidence_tier` to tell settled SMT verdicts from unsettled ones.
- MCP lookups (`cached_result` and `counterexample`, local or job-backed)
  and the agent view's failure entries carry both fields.
  The MCP evidence tier follows the presented verdict: a function presented
  as provisional has no evidence tier.
- Goldens and fixtures changed with the schema; no verdict, diagnostic or
  solver input changed.
- The verification runtime adds evidence-tier values and ITF witnesses on top
  of v2 without renaming anything.
