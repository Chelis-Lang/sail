# Requirements after Oyster

Status: design record, nothing here is implemented. It records what the Oyster
experiments found, what Sail keeps from the work it did for Oyster, and two
smaller forms a requirements feature could take. Neither form is scheduled.
Each states the evidence that would justify building it.

Oyster was a separate, private project. It turned requirements written in
English into a formula language, asked its owner to settle ambiguities, and
sent "neutral" Rust contracts to Sail, which lowered them to Verus and proved
them. Its owner abandoned it on 2026-11-09. The figures below come from
Oyster's own assessment and experiment reports, which are not public; this
document quotes only their findings.

## What Oyster found

Oyster's assessment gives its own verdict: "not achieved, and not
demonstrated."

| Finding | Evidence | What it rules out |
|---|---|---|
| The admitted Rust subset reaches almost no real code | Across 53 public functions in four real crates, neutral contract admission accepted 0. 50 of the 53 bodies use calls, `match`, loops or other constructs outside the grammar, so relaxing signature rules alone would not change the count | A restricted source subset with its own admission rules as the way to reach ordinary Rust |
| A plain prompt notices ambiguity as well as the formal layer | One plain prompt to a strong model flagged the target ambiguity in 10 of 11 cases in each partition, with far fewer questions on clean requirements than the deterministic checks. The assessment's reading: "noticing that a requirement has two readings does not need the formal layer" | A separate requirements language justified by ambiguity detection |
| The solver did not find contradictions better than people asking | The pre-registered bar was 8 of 12 change-request chains surfacing at least 2 of 3 planted contradictions. The result was 1 of 12, at precision 0.16 against 0.46 for plain coders who asked the owner | The claim that solver checks over formalized requirements beat asking |
| Proofs certify wrong decisions as readily as right ones | In a C-to-Rust translation pilot, a proved requirement kept a behavior the owner wanted fixed in 3 of 4 builds of the Oyster arm | Reading a proof under a requirement ID as evidence that the requirement is the right one |
| The measured lever was process, not tooling | One structured bulk review of legacy behaviors raised correct decisions on never-asked quirks from 0 to about 2 per build, with Oyster (2.00) and without it (1.88). The Oyster arm cost 3.9 times as much per build and took 2.6 times as long | Paying for the formal layer to get a gain that a review step gives alone |
| Mutation could not arbitrate | Every native mutant of a neutral contract came back inconclusive, because the solver answered `unknown` (`corpus/repros/neutral-contract-unknown/README.md`) | Using rejected mutants as kills without first deciding what counts as evidence |
| The requirement ledger did not scale | The lock file that bound requirement IDs to hashes grew to about 510 MB, every command reread it, and any body edit withdrew every tier | Hash-bound requirement ledgers as the store of record |
| No user was shown to benefit | The assessment: "No human owner has done the confirmation", and "No defect has been found that direct tests, contracts or mutation would miss" | Building further on the assumption of demand |

Two things were not measured. The experiments had no narrow-trigger bugs, the
case where formal methods were predicted to win. And nobody measured whether a
proof record tied to a requirement ID has value for certification or regulated
sign-off.

### What this cost Sail

Between 0.1.0 and 2026-11-09 Sail added, for Oyster, neutral contract crates,
lowering to Verus through a generated shadow crate, source jobs, an assurance
view, bounded mutation and scalar literal witnesses. Removing them deletes
about 14,800 lines, of which about 13,200 are Rust (product code, tests,
examples and fixtures). The work also added a negotiated version for each
surface, a required end-to-end phase on every pull request, and six error
codes. The
shadow crate caused most of the operational cost: refusals for inherited Cargo
configuration (ADR 0105) and manifest drift between revisions (ADR 0107).

The pattern to avoid is the one the experiments exposed: a versioned,
negotiated protocol built for one consumer before any user had shown it changed
an outcome.

## What Sail keeps

| Kept | Why |
|---|---|
| `sail runtime` and `sail-runtime` (ADR 0108) | Supported raw SMT execution with a versioned schema. A second solver lands here |
| Evidence tiers and the ledger (`sail-ledger`, `EvidenceTier`) | Separate fields for establishment, tier, currentness and mutation, and the rule that `unknown` is never proof. The mutation report needs the same discipline |
| Function-level attribution (ADR 0014, `crates/sail-verus/src/attribution.rs`) | Ordinary Verus attribution. It was never Oyster work |
| Decoded counterexample inputs (ADR 0106, `sail_api::counterexample::decode`) | Turns a raw model into source-named values for any failed Verus proof. The supported MCP `counterexample` tool reports it as `decoded_inputs` |
| Requirement attribution on ordinary jobs (ADR 0104), experimental | Kept only until the decision described under form A. Today no row can reach a tier, because `--no-cheating` fails in `vstd` |

Everything else built for Oyster is removed: the neutral lowering and shadow
crate, neutral source jobs and their worker, neutral mutation, literal
witnesses, the neutral assurance view, the neutral specifications, the
`contracts` end-to-end phase, and the three crates `sail-contracts`,
`sail-contracts-macros` and `sail-contracts-ir`. A retained record from a
removed job reads as `SAIL3161`. The last commit that contains them is tagged
`archive/neutral-contracts-final`.

The contract crates went with the rest because the Kani pilot selected Kani as
an experimental backend for hand-written harnesses only: Kani function
contracts are not adopted as a front end (ADR 0109), so form B's attribute
surface had no consumer. `sail-contracts-macros` also expanded each attribute
by calling the admitted-subset validator in `sail-contracts-ir`, so keeping the
macros would have kept that validator.

## Two candidate next forms

Both forms accept Oyster's deepest finding instead of working around it: Sail
can show how well a specification constrains code, and it cannot show that the
specification says what the owner meant. Neither form adds a requirements
language, an admitted subset, a lowering step or a lock store.

### Form A: requirement tags as a grouping over `sail mutate` and proof health

**User.** A developer or agent with a Verus project that already verifies, who
also tracks requirements somewhere else: issue numbers, a specification
document, a safety case.

**Problem.** "Verified" does not say which requirements have a specification
that actually constrains the code. An agent can reach green by weakening an
`ensures`, and nothing in the result shows which requirement lost its teeth.

**One session.**

1. Tag functions with the existing `--requirement ID=FUNCTION`, or a doc
   comment `/// sail:req REQ-7` that needs no Verus attribute support.
2. Run `sail mutate run -p CRATE --budget 10m`.
3. Read the same report grouped by tag: "REQ-7: `parse_header` proved; 9 of 12
   mutants rejected, 3 survived; rlimit margin 4%."
4. Open the three surviving diffs, strengthen the `ensures`, and run
   `sail mutate recheck`. The survivors are rejected.
5. CI reads the same JSON and fails when a tagged requirement gains a survivor.

**Why.** It is a grouping key over evidence that `sail mutate` and `sail health`
already produce per function. It adds no proof obligation and never raises a
tier. It gives the `mutation: not-established` field that ADR 0104 hard-codes a
value with a source.

**What it does not do.** The link from a tag to intent stays an unchecked
submitter claim, and the report says so. It repeats Oyster's "ID join" in a
smaller form, which is the reason for the bar below.

**Evidence that would justify building it.**

- `sail mutate` exists and reports survivors on a real project. Without it
  there is nothing to group.
- On IronKV or Vest, a hand-joined table (five tagged functions, the existing
  catalogue mutants, recorded rlimit margins) leads the owner or an outside
  user to a specification change that the per-function report alone did not
  prompt. An agent's own judgement that the table "looked useful" does not
  count.
- Someone asks for the grouped view in CI. One private consumer is not demand.

If the hand-joined table does not pass that bar, ADR 0104 requirement
attribution and `sail assurance` are removed in the same release instead of
left without a purpose.

### Form B: one contract attribute, checked by Kani and proved by Verus

**User.** A Rust developer who does not write Verus, with ordinary code: loops,
calls, `match`, standard collections. This is the population where Oyster's
subset admitted 0 of 53 functions.

**Problem.** There is no route from "I can state a precondition and a
postcondition" to machine-checked evidence that does not start with rewriting
the function in a proof language. Bounded checking would be useful evidence on
its own, and a later proof should not need the contract restated.

**One session.**

1. Write `#[requires(n <= 64)]` and `#[ensures(|r| *r == popcount_spec(n))]` on
   an ordinary function.
2. Run `sail verify --backend kani`. The attribute expands to `kani::requires`
   and `kani::ensures`, Kani checks the function within stated bounds, and the
   result carries tier `bounded` with the bounds named.
3. Later, for the functions that matter, add loop invariants and run plain
   `sail verify`. The same attribute expands to Verus specifications and the
   result carries tier `smt-verified`.
4. The report lists both tiers per function and never merges them.

**Why.** The reusable part of the Oyster work is the attribute surface
(`requires`, `ensures`, and an optional requirement tag), not the neutral
intermediate form. Kani accepts loops and calls with bounds, so real code is
reachable at the bounded tier without an admitted subset. Each backend reads
the source it already understands; Sail validates nothing against a grammar of
its own.

**What it does not do.** It does not make Verus accept code that needs
invariants, and a bounded result is not a proof. The expansion must reject what
a backend cannot express instead of silently dropping it.

**Evidence that would justify building it.**

- The Kani pilot reports go on function contracts: passing and failing
  harnesses on a pinned ordinary crate, the same verdict on two direct Kani
  runs, and a measured cost.
- At least half of a sample of real public functions, such as the 53 in
  Oyster's bindability study, accept a contract and reach the bounded tier
  with no source change beyond the attribute. Oyster's subset scored 0 on the
  same measure.
- For at least one function, the unchanged attribute also proves under Verus
  once invariants are added.
- The macro expansion works without `sail-contracts-ir`. If it needs a
  validator of its own, the form has drifted back to an admitted subset.

The Kani pilot did not adopt function contracts, so `sail-contracts`,
`sail-contracts-macros` and `sail-contracts-ir` were removed and this section is
the closing record. Form B would start again from a new attribute crate, with
its own ADR, only if a later pilot produces the evidence above.

## Order

Form A depends on `sail mutate`. Form B is closed unless a later pilot reopens
it. Neither blocked the removal, and neither ships a schema before its evidence exists.
