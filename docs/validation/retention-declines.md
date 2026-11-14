# Optional retention decline reasons

Status: native Linux gate, solver, full end-to-end, PR conformance and bounded
independent review pass at `7c0aa003`. Required hosted CI remains before merge.
Tracks #712 package C and #595; ADR0136 records the boundary.

The candidate returns a typed reason directly from each failed optional
plan-admission check. Normal compilation, check order, original successful Plan
fields and existing profile/runtime notes remain. There is no thread-local
last-step state, publication journal, new cost policy, source observation or
reuse authority. Unrequested retention remains silent.

## Actual qualification

Source `7c0aa003567bf48cf04bb4b4bf559657c73cc25c`, c7i.4xlarge EC2 in
us-east-2, Ubuntu 24.04, Rust 1.99.0 and verifier 1.99.1, image
`6c19f5c8d397b61c-core` / complete MCP target. Selected runtime is the public
manifest `1d23c074efd3335a60f3ca24f97ab5eb130fafd2454a06f5fc9d279da188c03c`.

| Gate | Actual result |
|---|---|
| Full local workspace gate | 1,403 passed / 57 skipped; fmt, clippy, doctests, rustdoc, layering, dependency policy and generated page checks pass |
| Pinned-Z3/cvc5 solver tier | 53 passed |
| Complete local end-to-end | Pass, including Extra, retained reuse, failing first runs, ghost/generated inputs, build-script reuse and repair CLI/MCP |
| Four PR conformance arms | Each 33 same / 311 functions; zero different, unstable, infrastructure errors or skipped projects; seven resource-drift units retained |
| Independent actual retained tests | 50 passed; source mounted read-only |
| Independent extracted-production controls | 31 cases preserve optional admission, successful Plan data, helper evaluation order, original notes and real filesystem effects; three deliberately broken candidates fail their intended runtime assertions |
| Documentation | Book source, build and link checks pass |

The differential controls inject unchanged eligibility/runtime/environment
interfaces to force difficult paths. They are bounded refactor evidence, not
real-project acceptance; the separate full end-to-end suite exercises the real
service and verifier. The new malformed-metadata production test requires both
the correct failed stage and no storage change.

ADR0087 classification: explanation/diagnostic tooling. Independent source and
executable review establish identical admission predicates/order, filesystem
operations, selected runtime, Plan values, solver interaction, keys and reuse
conditions. No model/refinement body or dependency/platform interface changes.
A long standard corpus or benchmark run is not claimed or required for this
explanation-only change. No Mac execution or performance result is inferred.

All 878 gate/integration payloads were independently rehashed and retained
externally, archive
`08277c88e0239a6e6cdd4c0dd07d03972840a6eacffbacdc747bc8a6299d3601`.
The independent native archive is
`54c97f8862d7a728adf972ce573c8b0517eb6266c933e1eb9e71b33f2387cde9`;
all 59 native and 65 complete-review payloads were independently rehashed and
retained. Review report SHA-256:
`36da1dabdf0cb7643fb0fc71be045dda362de4f40d84b4060b0ac4ecd3f877ae`.

## Preserved attempts

| Source / attempt | Date, result and cause |
|---|---|
| `e5713197`, original native gate/review | 2026-11-14: rejected unused `skipped_note` import; denied warning. Successor removes only that import and records the failure |
| Review standalone controls, first setup | 2026-11-14: offline Cargo index unavailable before compilation; invalid setup, not a candidate failure |
| Review mutation-control runner, first setup | 2026-11-14: image lacks rg after the intended first assertion failure; equivalent grep checks permit all three runtime assertion controls to finish |

Documentation changed: ADR0136, this record, instant-revert guide and changelog.
The consumer skill needs no change because the existing SAIL_EXPLAIN=1 command
and released availability are unchanged. Typed restore/publication reasons,
missing-export recovery and cost-gate decisions remain open in #712. Subsequent
status-document amendments do not claim additional native execution.
