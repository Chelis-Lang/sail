# 0174: v0.3.0 ships native temporal checking; unfinished pillars move to v0.4.0

- **Status:** accepted by owner, 2026-11-19
- **Date:** 2026-11-19
- **Amends:** the release scope of
  [ADR 0109](0109-oyster-retired-v030-scope.md), the mutation-first headline
  of [ADR 0100](0100-mutation-first-roadmap.md), the owner's 2026-11-11
  confirmation in [`docs/ROADMAP.md`](../ROADMAP.md) that the whole v0.3.0
  program is completed through candidate staging, and charter appendix
  entries 16 and 17
- **Unchanged:** the correctness contract, ADR 0111's mutation evidence rule,
  ADR 0109's Oyster and neutral-contract decisions, and the owner's explicit
  approval of the exact final candidate (charter appendix entry 15)

## Context

ADR 0109 gave v0.3.0 five pillars: Verus mutation testing (headline), proof
health and profiling, never slower than stock, reach, and backends (Kani, then
cvc5). On 2026-11-11 the owner confirmed that the whole program was to be
completed before candidate staging.

On 2026-11-19, `main` holds this of that program:

| Pillar | On `main` |
|---|---|
| Mutation testing (#140, #141, #664) | Evidence rule (ADR 0111), exec-only generator, campaign evidence model and classifier. No `sail mutate` command, MCP tool or campaign job. The engine, CLI and MCP exist only on unmerged development branches, without real-project, cost or Mac acceptance |
| Proof health | `sail health` and MCP `proof_health` merged (#733), qualified on native Linux and Mac ([record](../validation/proof-health.md)) |
| Profiling | Session-prefix, prefix-replay and quantifier-count library groundwork. No history, stability worker or `sail profile` command |
| Never slower than stock (#158, #232, #558, #560, #288, #162) | Named rows still slower; frontend reuse is a no-go for v0.3.0 (ADR 0170) |
| Reach | `sail report` and the GitHub Action merged (#699, #702). Installer distribution and the editor bridge are not merged |
| Kani (#135) | Pilot passed; decoder and authenticated input fixtures only. No executor or CLI; the backend is not built or qualified on `main` |
| cvc5 / SMT-LIB service (#137) | Raw-SMT backend merged as an unadvertised development opt-in (`SAIL_RUNTIME_CVC5`); Mac qualification open |

Separately, D.I.V.E. 0.1.0 is about to be published on crates.io. Draft pull
request #763 prepares the 0.3.0 release source: native temporal checking in
the main workspace on D.I.V.E. 0.1.0 (its ADR 0172), genuine temporal
properties with an exact store and limits, and a measured comparison with
standard TLC (record `docs/validation/sail-dive-vs-tlc.md` on that branch).
On the measured host, small checks ran 27 to 39 times faster than TLC per
check and a 100-check MCP session about 13 times faster, with identical
verdicts and state counts.

## Decision

Sail 0.3.0 ships soon after D.I.V.E. 0.1.0 is on crates.io. Its headline is
native temporal checking with D.I.V.E. It also carries what is already done
and qualified on `main`. Everything unfinished moves to v0.4.0.

| Pillar | Disposition |
|---|---|
| Native temporal checking on D.I.V.E. 0.1.0 | **0.3.0 headline**, through #763. It keeps its experimental label |
| Proof health (`sail health`, MCP `proof_health`) | Ships in 0.3.0 |
| CI reports (`sail report`) and the GitHub Action | Ship in 0.3.0 |
| Neutral-contract removal, requirement-attribution retirement, delivered fixes (#563, #595, #696, #711, #719 and #683's reuse refusal) | Ship in 0.3.0 |
| Verus mutation testing (#140, #141, #664) | v0.4.0 |
| Profiling, history and stability (`sail profile`, retention, idle stability worker, fragility graduation) | v0.4.0 |
| Never slower than stock and cheaper changed proofs (#158, #232, #558, #560, #288; #162 stays a no-go study) | v0.4.0. 0.3.0 makes no new Verus performance claim; existing known issues stand |
| Installer distribution, Binstall and the editor bridge | v0.4.0 |
| Kani as an experimental backend (#135) | v0.4.0. The owner's choice of an experimental backend for hand-written harnesses carries over unchanged |
| cvc5 and the SMT-LIB service (#137) | v0.4.0. The merged backend stays an unadvertised development opt-in |

This decision does not choose v0.4.0's headline or its order. The moved
pillars keep their gates, drop rules and trackers.

Unfinished work must not ship as a half product in 0.3.0. Groundwork that is
reachable only through public library modules of published crates is hidden
from their documentation and documented there as unstable. No command, MCP
tool or JSON field is added for a moved pillar until that pillar is qualified.

The v0.3.0 milestone keeps only release blockers: release bugs in shipped
behavior, the headline's open defects and its release qualification. Other
open v0.3.0 issues move to a new v0.4.0 milestone.

## Consequences

- `docs/ROADMAP.md` describes 0.3.0 as native temporal checking plus the
  delivered items, and holds the moved pillars in a v0.4.0 section.
- The 0.3.0 release notes and changelog lead with temporal checking, in
  #763.
- The mutation brief and the runtime scope record that mutation and the
  Kani and cvc5 backends are v0.4.0 work.
- Charter appendix entry 19 records the correction.
- Publication still needs D.I.V.E. 0.1.0 on crates.io, #763's release-day
  steps and the owner's approval of the exact candidate.

## Amendment (2026-11-28): v0.4.0 pillar order

The owner decided v0.4.0's order at the Kani checkpoint
([ADR 0180](0180-kani-phase-scope.md)). Kani is the headline. Profiling and
stability, the never-slower rows, reach, and cvc5 with the SMT-LIB service
co-ship only if already qualified when Kani is. Verus mutation testing (#140,
#141, #664) moves to v0.5.0.
