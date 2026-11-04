# 0100: Mutation-first roadmap and design-first Oyster hold

- **Status:** accepted by owner, 2026-11-04
- **Date:** 2026-11-04
- **Amends:** work ordering in ADR 0065 and charter appendix entry 8;
  existing implementations and correctness contracts remain in force

## Context

Sail 0.2.0 is published. The owner selected the supplied colleague feedback as
the deciding input: Oyster's open question is product design despite substantial
integration. Current Verus users can gain specification-strength feedback from
mutation testing without waiting for that design or another backend.

## Decision

Make bounded Verus-aware mutation testing the next release headline through CLI,
JSON and MCP together. Mutate executable code in copies, preserving contracts,
proof and ghost code. Report killed, survived and inconclusive observations with
explicit budgets, omissions and unsupported scope. Start with the native Unknown
cases. Never convert Unknown into a kill or infer adequacy or vacuity from
sampled survivors/zero kills. Existing native-campaign classification is
unchanged; scalar witnesses remain a separately qualified evidence family.

Run changed-proof performance, the named Mac Anvil cold regression and the
service-wide acceptance proof alongside it. Give frontend reuse a bounded
go/no-go study; it is never a release dependency. First-use/full-path work
remains useful backlog. Measure total campaign cost before claiming cache gains.

Then evaluate Kani on a real ordinary Rust crate before choosing a production
backend, including the experimental contracts interface in the go/no-go. Kani
precedes cvc5. Do not widen the neutral subset for that pilot.

Hold Oyster integration expansion and SDK migration. Maintain delivered
experimental interfaces and tests. A short Oyster design must identify user,
current problem, one successful session and the value of requirements over
direct specs/contracts/mutation feedback. Product direction remains unsettled.

Hold temporal packaging until D.I.V.E.'s distribution decision. Preserve native
worker boundaries and the no-TLC policy. Keep original plans and evidence intact,
adding superseding current instructions rather than rewriting history.

## Consequences

The [roadmap](../ROADMAP.md), runtime scope and trackers distinguish shipped
0.2.0 work from actual follow-ups. Mutation has no Oyster, neutral-expansion or
new-backend dependency. This decision introduces no dependency, wire protocol,
solver policy or product behavior. Implementation requires its own interfaces,
native qualification and green protected checks. Final publication requires
explicit owner approval of the candidate.
