# 0109: Oyster retired; v0.3.0 scope

- **Status:** accepted by owner, 2026-11-09
- **Date:** 2026-11-09
- **Supersedes:** the Oyster parts of
  [ADR 0065](0065-oyster-runtime.md) and
  [ADR 0100](0100-mutation-first-roadmap.md), and the 2026-11-06 amendment to
  [open question 48](../open-questions.md) as far as it concerns Oyster
- **Amends:** ADR 0100's backend ordering and its hold on Sail's Oyster-facing
  surfaces; charter appendix entry 16
- **Unchanged:** [ADR 0101](0101-dive-behind-a-default-off-flag.md), the
  temporal extension, draft pull request #670 and every D.I.V.E. statement.
  ADR number 0102 stays reserved for that pull request.

## Context

Sail 0.2.0 was published on 2026-11-04. ADR 0100 made Verus-aware mutation
testing the next headline and put Oyster on a design-first hold. On 2026-11-06
the Oyster owner resumed Oyster for shelf completion, and on 2026-11-08
[ADR 0103](0103-scoped-neutral-widening.md) lifted the neutral-widening hold
for a named set of items.

A backlog review on 2026-11-09 found:

- The mutation headline is designed but not built. Its first step
  ([#664](https://github.com/Chelis-Lang/sail/issues/664)) is not settled.
- Sail is still slower than stock Verus in named rows: the first unchanged
  call, the full path, tiny edits and reverts, and Mac Anvil cold. A
  one-function edit in a large module re-solves its whole bucket (#158).
- The stability and fragility engine exists but is experimental and hidden.
  Users have no way to ask why a proof is slow without rerunning Verus.
- Adoption costs remain: no GitHub Action, two platforms, one Verus pin, no
  editor integration.
- Most work merged since 0.2.0 served neutral contracts for Oyster. Oyster's
  own assessment found that none of 53 real public functions taken from
  existing crates fit the neutral contract subset.
- Defects #696, #683 and #684 were found through neutral work but sit on the
  general managed path. They matter to ordinary jobs and to mutation.

## Decision

### Oyster

The owner has abandoned Oyster. Sail does no further engineering for Oyster,
plans no Oyster deliverable and has no Oyster repository to file in. The
"shelf completion" and "design-first" directions are closed. ADR 0103's lifted
items are complete as delivered; no further neutral widening is planned.

D.I.V.E. stays private. ADR 0101, the `extensions/temporal` workspace, its
local test tier and draft pull request #670 are not touched by this decision.
The roadmap's Temporal section stands as written. Any change to D.I.V.E.'s
standing is a separate owner decision.

### Release

The next release is **v0.3.0**. It removes API published in 0.2.0, so it is a
breaking release. Its pillars are:

1. **Verus mutation testing** (headline; #140, #141, #664). The feature brief
   is [`docs/design/verus-mutation-release.md`](../design/verus-mutation-release.md).
   How verifier rejections that the solver did not confirm are counted is a
   separate evidence decision for the owner. Solver answers are never
   relabelled.
2. **Proof health and profiling.** Rank functions by resource margin and
   stability, and name the quantifiers that cost a proof its time, from
   recorded sessions. Nothing here changes a verdict or the bytes of plain
   `sail verify`.
3. **Never slower than stock, and cheaper changed proofs.** Every measured row
   at or under stock on Linux and macOS, or listed as a failed gate. Lower
   cost for an edit in a large module.
4. **Reach.** A GitHub Action, simpler installation and an editor bridge.
5. **Backends.** A Kani pilot on a real pinned ordinary Rust crate (#135),
   then cvc5 in the SMT runtime, both inside this release.

### Backend ordering (amends ADR 0100)

ADR 0100 says "Kani precedes cvc5" and its roadmap text says cvc5 and the
other future backends "are not parallel commitments for the next release".
This decision keeps the order and moves cvc5 into the release:

- The Kani pilot runs first. Its go/no-go covers concrete failures,
  unsupported cases, costs and the experimental function-contract interface,
  as ADR 0100 requires.
- cvc5 work starts after the Kani decision, whichever way it goes. It lands
  only in the supported SMT runtime ([ADR 0108](0108-supported-smt-runtime.md)),
  with solver identity in every key, per-solver conformance and fault
  injection, and unchanged Z3 bytes.
- ADR 0100's instruction not to expand the raw SMT runtime is lifted for cvc5
  only. It was a hold on surfaces Oyster consumed, and that reason is gone.
- Certificates get one measurement spike with its result recorded on #138 and
  no product work. Creusot, Prusti, kernel provers, sampled-tier integration,
  cross-backend portfolios and Dafny stay future options under #146.

The same ordering is stated in these documents, which this decision updates
or supersedes:

| Document | Statement |
|---|---|
| [`docs/ROADMAP.md`](../ROADMAP.md) | "Then: Kani pilot before cvc5"; "not parallel commitments for the next release" |
| [`docs/CHARTER.md`](../CHARTER.md) | Appendix entry 16: "A real-crate Kani pilot precedes cvc5" |
| [`docs/open-questions.md`](../open-questions.md) | Question 48: "A real-crate Kani pilot precedes cvc5" |
| [`docs/scope/verification-runtime.md`](../scope/verification-runtime.md) | Status line, the current order list and conflict 9 |

### Neutral-contract surfaces

These surfaces exist and are tested today. They are frozen: maintained, not
expanded. They are removed in v0.3.0 only after all three preconditions hold:

1. The tag `archive/neutral-contracts-final` exists on the last commit that
   contains them.
2. The #664 cases are ported to plain Verus files, so mutation diagnosis no
   longer needs the neutral path.
3. Defects #683 and #696 are reproduced on ordinary managed jobs, so their
   fixes are not lost with the neutral reproduction.

**To be removed:**

- neutral lowering and shadow crates (`crates/sail-verus/src/contracts/`);
- neutral source jobs in the daemon and their worker;
- neutral mutation and the literal witness;
- the `sail-contracts-ir` crate;
- the `contracts` end-to-end phase and its checks;
- the neutral specifications (`docs/spec/neutral-contracts.md`,
  `docs/spec/neutral-contract-jobs.md`, `docs/spec/neutral-verus-lowering.md`).

The removal lists every 0.2.0 item under "Removed" in `CHANGELOG.md`. Old
durable job records of the removed kinds read as a coded error, not a panic or
a silent skip. Capabilities are bumped, and layering rows and crate edges are
dropped in the same change. ADRs for removed surfaces are marked superseded,
never deleted. The published 0.2.0 crates are not yanked.

**Kept:**

- the SMT runtime, supported under ADR 0108;
- evidence tiers and the ledger;
- counterexample decoding ([ADR 0106](0106-decoded-counterexample-inputs.md)),
  moved onto the supported counterexample tool before the assurance inputs
  it currently rides on are removed;
- general function attribution ([ADR 0014](0014-attribution-ledger-and-output-schema.md)).

**Decided at the post-spike checkpoint:**

- Requirement attribution and `sail assurance`
  ([ADR 0104](0104-ordinary-job-requirement-attribution.md)). They stay only
  as a thin experimental grouping in the mutation report, and only if the
  owner finds a hand-joined example on a real project useful once mutation
  exists. Otherwise they are removed in v0.3.0, and #684 closes with them.
- The front-end crates `sail-contracts` and `sail-contracts-macros`. They stay
  only if the Kani pilot says go, as an attribute front end over ordinary
  Rust with no neutral intermediate form or admitted subset. On no-go they
  are removed too.

### Execution

- **Spikes first.** Every track starts with spikes of a few hours, each with a
  pass/fail rule written before it runs. One owner checkpoint follows the
  spike wave, with a tractability table. Scope locks there.
- **Drop rules.** Every item has one. Work that fails its spike is closed with
  a short record (an issue comment or a line in `docs/validation/`) and is not
  carried.
- **Publication.** Passing qualification does not authorize publication. The
  owner must explicitly approve the exact final candidate before its GitHub
  release or stable crates are published.

## Consequences

- `AGENTS.md` no longer lists an Oyster repository under "Where agents may
  file". The answer to open question 21 is narrowed to Sail, the Verus fork
  and D.I.V.E.
- [Open question 49](../open-questions.md) records the owner's answer.
  Charter appendix entry 17 records the correction.
- The roadmap's "Oyster: shelf completion" section is replaced, and its
  next-release section becomes a gate table for v0.3.0.
- Issues that ask for new neutral semantics are closed as not planned when
  the removal lands. Until then the surfaces above keep their tests, and no
  conformance check, golden or baseline is weakened to make removal easier.
- The correctness contract is unchanged. This decision adds no dependency,
  wire protocol, solver policy or product behavior. The Kani crate, the cvc5
  backend, the removal and each new command need their own ADRs or pull
  requests, native qualification and green required checks.
- Requirements and contracts may return in another form. The Oyster findings
  and any later design belong in a design document written with the Kani
  pilot's result, not in this record.
