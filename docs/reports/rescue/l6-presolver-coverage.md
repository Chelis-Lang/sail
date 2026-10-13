# L6: pre-solver coverage spike

Rescue lane L6 asked one question before any M9 work: how many of the
checks that agent edit loops actually send to Z3 could a fast in-process
"easy fragment" pre-solver refute? This report measures that, estimates the
latency effect, and drafts the M9 design. Nothing here changes the product:
the tool is an experimental `sail-bench` subcommand.

**Result:** the easy fragment covers 8.2% of edit-loop cache misses by count
and 0.1% by solver time. No edit cycle had all of its misses covered. Miss
latency comes from Z3 process and prefix setup, not from solving. Measured
as a latency feature for agent loops, M9 is a **no-go**. A narrow follow-up
is recorded below.

## Tool

The measurement tool is not merged, because the decision was no-go. It is preserved
with this report's exact inputs at the tag `archive/l6-presolver-spike`
(`fb34c8a`). Check out that tag to rerun it.

- `sail-bench presolver-capture --out DIR [--loops F] [--edits F] [--repair F] [--tier T]`
  - Runs Verus through `sail-z3` in pure pass-through with the observation
    tap on, once per edit cycle. Cycle `c00-initial` runs the unmodified
    unit.
  - `corpus/agent-loops.toml` cycles are cumulative.
    `corpus/edits.toml` edits each start from the original.
    `corpus/repair-edits.json` cycles start from the original or from a named
    earlier cycle.
  - Each cycle runs on a fresh copy of the source. The cache is never used.
- `sail-bench presolver <trace-dir>... --z3 Z3 [--jobs J] [--json F] [--save DIR] [--replay-every N]`
  - Rebuilds each check's assertion stack from the transcript, following
    push/pop and the declarations in scope.
  - Extracts a sound relaxation of the stack in three fragments.
  - Decides each relaxation with the pinned Z3 as oracle.
  - Compares the result with the in-session answer and time recorded in the
    trace.
  - Code: `crates/sail-bench/src/presolver/` at that tag.

**What counts as a miss.** A check is a default-mode cache miss if its
key-v2 (`prefix_key`, ADR 0008) did not occur in any earlier cycle of the
same sequence. Key-v1 misses are reported alongside.

**Relaxations.** Each assertion is split into top-level conjuncts. A
negated goal `(not (=> l (and …)))` yields `l` and the negated body. Two
operations relax the stack, and both preserve unsatisfiability:

- dropping a conjunct;
- abstracting a subterm outside the fragment by a fresh constant of the same
  sort. Syntactically identical closed subterms share the constant. Lets
  are expanded first, and quantifiers are abstracted whole.

The three fragments:

| fragment | contents |
|---|---|
| `lia` | Conjuncts that are entirely linear Int/Bool arithmetic over declared constants. Everything else is dropped. |
| `lia+uc` | `lia`, plus every other Int, Real or Bool subterm (uninterpreted function applications, `Poly` equalities, quantifiers) abstracted as an uninterpreted constant. |
| `lia+uc+bounds` | `lia+uc`, plus ground instances of the pinned prelude's own axioms (qid `prelude_*`: `uInv`, `iInv`, `uHi`, box/unbox, `Add`/`Sub`, `has_type`, …) and ground bit-vector operations. Instances come from two rounds of trigger matching against the stack's ground terms. An instance of an asserted axiom is entailed by the stack, so adding it is sound. |

Two further measurements run on `lia+uc+bounds`:

- **`tight-rational`.** Int variables become rationals. Each comparison atom
  becomes `p ⇒ POS`, `¬p ⇒ NEG`, with both sides tightened for integers.
  Whatever this refutes, a simplex with Farkas certificates (no
  branch-and-bound) refutes too.
- **`prototype`.** A clearly marked experimental DPLL with unit propagation
  and Fourier–Motzkin over integer coefficients with gcd tightening. It is
  used for in-process timing only (`proto.rs`).

**Self-checks:**

- The fragments are nested, so a weaker fragment being unsat while a
  stronger one is not means a tool bug. Monotonicity violations: 0.
- Oracle errors: 0.
- "Relaxation unsat but Z3's full answer not unsat": 0 across all 35,597
  checks. This includes all 104 checks Z3 answered `unknown`, which are real
  verification failures (failing edits and the examples' expected failures).
  A wrong extraction would show up there first.
- Unit tests cover splitting, sharing, scoping, instantiation, tightening and
  the prototype.

**Obligations.** For each location label `l` in a Z3-unsat check, the tool
also decides the relaxation with `l` set and every other location label
cleared. This measures how many single proof obligations the fragment
discharges.

## Corpus and capture

| set | source | sessions | checks | Z3 unsat / unknown |
|---|---|---|---|---|
| edit sequences | 7 sequences: 2 agent loops, 3 catalogue units, 2 repair projects (human-eval-verus, vest). 7 initial + 21 edit cycles. | 4,362 | 31,511 (24,441 in edit cycles) | 31,504 / 7 |
| fast tier, cold | `--tier fast`: 147 units of verus-examples, vstd and human-eval-verus | 763 | 4,086 | 3,989 / 97 |

- **Machine.** Native AWS c7i.4xlarge (Intel Xeon Platinum 8488C, 16 vCPU),
  Ubuntu 24.04, kernel 7.0.0-1014-aws. Host lane `l6`, terminated after use.
- **Toolchain.** Verus 0.2026.10.04.426d8b0 and pinned Z3 4.16.0, sha256
  `e583c418…b765`.
- **Load.** One capture at a time, then the analysis at `--jobs 15`. Verus
  wall times are in the manifest.
- **Outcomes.** Every one of the 21 edit cycles produced its catalogue's
  expected outcome: 5 `errors`, 16 `verified`.
- **Source.** This branch (`rescue/l6-presolver-coverage`). The manifest's
  `sail_commit` reads `unknown` because the synced tree has no `.git`.
- **Raw evidence.** Kept in `~/sail-raw-evidence/l6/`, not committed:

| file | contents | sha256 |
|---|---|---|
| `l6-evidence.tar.gz` | all transcripts, both reports, saved queries | `15657700d0aedc5c7825bc290ade1a3278d099a142a33329d31a7a8d917ed5b3` |
| `edit.json` | edit-sequences report | `fd2bde359e80b037e88861855f0ae349344be70155085c0158dd5a5466ad79d8` |
| `edit.md` | edit-sequences summary | `6987ca0d332eaf0c7ab2bf8fc2c8a6e106c864234baee84975a6e5c8c6258b05` |
| `cold.json` | fast-tier report | `8d1a7908330efaca4ac293c608746490dc397f111675c734259931a3c8a47ba9` |
| `cold.md` | fast-tier summary | `fa7a65bc9e3204c7af28eea37d051f314f0d7638c190b84e92e7c3531cdc78c9` |
| `capture-manifest.json` | capture manifest | `9648984469a1df6fd151123cfdb989fb4abf44918ee91f844b70b48ccdd86bbb` |

## Coverage

"Covered" means the pinned Z3 said `unsat` *and* the relaxation is `unsat`.
Time is the recorded in-session solve time.

**Edit-loop cache misses** (key v2: 512 checks, 17.7 s solve; 505 unsat, 7
unknown):

| fragment | covered (count) | covered (solve time) |
|---|---|---|
| `lia` | 21 (4.1%) | 0.0% |
| `lia+uc` | 39 (7.6%) | 0.1% |
| `lia+uc+bounds` | 42 (8.2%) | 0.1% |
| tight-rational | 42 (8.2%) | 0.1% |
| prototype | 39 (7.6%) | 0.1% |

Key-v1 misses (455 checks) give 4.2% / 7.9% / 8.6% by count and at most 0.2%
by time.

**All checks:**

| population | checks | `lia` | `lia+uc` | `lia+uc+bounds` (count / time) | tight-rational |
|---|---|---|---|---|---|
| edit-sequence captures, all | 31,511 | 5.8% | 8.3% | 11.1% / 0.3% | 8.6% |
| fast tier, cold | 4,086 | 10.2% | 11.5% | 17.6% / 1.6% | 12.6% |
| fast tier, excluding bit-vector sessions | 3,871 | 10.8% | 12.1% | 13.4% / 0.2% | 13.3% |
| fast tier, `by(bit_vector)` sessions | 215 | 0% | 0% | 94.9% / 92.3% | 0% |

**Per edit cycle.**

- The largest misses-covered share in any cycle was 8 of 72 (vest
  `c01-base128-body`).
- The most common pattern was 1 of 30 or 0 of 25.
- **No cycle had all of its misses covered.** An agent's "verified" verdict
  always waited on Z3.
- The 7 misses that Z3 did not answer `unsat` (the actionable failures) are
  out of reach by construction.

**Obligations.**

- The relaxation discharges 14–15% of single obligations overall and 10.7%
  (293 of 2,751) on edit misses.
- 82 edit-miss checks had all but one obligation discharged.

**Certificates.** On covered checks the unsat core has 1 conjunct at the
median, 2–4 at p90 and at most 35. The relaxations themselves are large
(205–803 conjuncts at the median) because the prelude's ground facts come
along.

## Latency

Microseconds, Linux x86-64. Edit misses are fresh-replayed exactly
(options, then levels with `(push)`, then the check), which models a cache
miss served by a newly started pinned Z3.

| component | p50 | p90 | p99 |
|---|---|---|---|
| Z3 spawn and exit | 2,390 | 2,575 | 2,704 |
| edit miss: fresh setup (no check) | 59,181 | 157,922 | 182,059 |
| edit miss: fresh total | 72,456 | 205,690 | 637,361 |
| edit miss: in-session solve | 6,691 | 47,073 | 343,017 |
| covered edit miss: fresh total | 54,765 | 165,169 | 190,859 |
| spike: per-assertion translation, amortized per check | 623 | 2,767 | 8,711 |
| spike: per-check extraction (instances plus SMT text for 4 queries) | 1,264 | 4,784 | 9,709 |
| prototype decision on covered checks | 215 | 1,169 | 414,534 |

**Estimate.**

- **Per covered miss.** A covered miss costs Z3 about 55 ms (p50) when it
  needs a fresh process, against an in-process path of about 1–2 ms
  (amortized translation, instances and decision). That saves roughly 50 ms
  per covered miss.
- **Per cycle.** Covered misses are 8% of misses and are trivial for Z3
  (their in-session solve times total 24 ms across 42 checks). Every cycle
  still needed Z3 for its other misses. **The agent-visible time to verdict
  is unchanged.** The pre-solver also cannot speed up the first actionable
  failure, because it only proves `unsat`.
- **Where the time goes.** Miss latency is dominated by setup: process start
  plus replaying the prelude and module prefix (59 ms p50 against 6.7 ms
  solve). Reusing exact-prefix solver sessions (pre-replaying the same
  bytes) attacks 80–90% of every miss's latency, keeps answers identical
  under ADR 0008, and needs no new trust.
- **Prototype tail.** Fourier–Motzkin blows up (p99 0.4–0.55 s). A product
  decision procedure would need simplex.

## Surprises

1. **What gets covered.** Covered checks are mostly obligations Verus
   already reduced to `(=> %%location_label%%N true)`, or simple
   bound/label contradictions (core size 1 at the median). These are cheap
   for Z3. Real proof goals in this corpus need spec-function and sequence
   axioms beyond any ground fragment.
2. **`by(bit_vector)` sessions.** These are pure ground QF_BV and are almost
   entirely covered by `lia+uc+bounds`, but they total 1.5 s of solve time
   across the fast tier, and a bit-vector decision procedure is a different
   project (bit-blasting, not Farkas).
3. **Few misses.** Key-v2 misses were only about 2% of edit-cycle checks
   (512 of 24,441) despite ADR 0008's whole-prefix binding. The cache already
   absorbs edit loops well. What remains is the per-miss process cost.
4. **Shape of Verus's queries.** These hold, which the subset soundness
   argument relies on:
   - one check per function per `(push)` block;
   - the assertion stack at each check is exactly the base level plus that
     block;
   - follow-ups (`get-model`, `reason-unknown`, `all-statistics`) are
     separate commands after the check;
   - no `check-sat-assuming` or `let` appeared in this corpus.

   The subset argument holds per check snapshot, including the follow-up
   re-checks in error reporting.
5. **No Verus or Z3 misbehaviour was found.** There are no anomaly queries,
   so there is no repro to file locally.
6. **Branch state, not caused by this lane.** On the `rescue/m7` base,
   `cargo clippy` for the workspace fails in `sail-verus`
   (`clippy::unused_self` in `managed/build_inputs_bootstrap.rs`). L6 used
   `--no-deps` for `sail-bench`. One workspace run hit a
   `sail-api jobs::cancellable` failure under load, and it passed when
   rerun alone.

## ADR draft (unnumbered): fast in-process pre-solver (M9)

- **Status:** proposed — **not recommended now** (see "Recommendation").
- **Context:** charter roadmap item "fast in-process Rust pre-solver"; this
  report's measurements; ADRs 0004, 0008, 0009 and 0011; charter rules 3, 7
  and 8.

**Fragment and decision procedure.**

- Ground QF linear integer arithmetic with Booleans.
- Non-fragment subterms are abstracted as uninterpreted constants, shared by
  canonical text.
- Ground instances come from an allowlist of prelude axioms. Each is matched
  by qid *and* by the exact axiom bytes of the pinned Verus prelude, so a
  prelude change disables it rather than silently changing its meaning.
- The decision procedure is a DPLL(T) with a rational simplex, integer bound
  tightening and gcd normalization. It answers only `unsat` or
  "don't know". It never answers `sat`, because a relaxation's `sat` proves
  nothing.
- Exact arithmetic uses big integers, which needs a dependency ADR.
- Bit-vectors are out of scope.

**Evidence and certificate format.**

- An `unsat` carries a certificate:
  - the list of stack conjuncts used (each as byte range and SHA-256 of its
    source command);
  - the abstraction map;
  - prelude instances (axiom qid, axiom bytes hash, substitution);
  - a resolution proof over theory lemmas, each a Farkas combination (the
    multipliers and the tightening steps).
- An independent checker of a few hundred lines re-derives each lemma with
  exact arithmetic and replays the resolution. Measured cores are 1–35
  conjuncts, so certificates stay small.

**Provisional semantics.**

- In default mode the pre-solver never answers Verus. Rule 3 requires exact
  reply bytes for the check *and* its follow-ups, and Verus reports
  `rlimit-count`, which no pre-solver can produce.
- Its result goes only to the ledger, MCP and live clients as
  `tier: provisional` (`docs/output-schema.md`) with
  `provenance: presolver-certificate`. It is never `verified` (rule 7).
- The pinned Z3 always runs. Its answer confirms or contradicts the
  provisional one.
- A contradiction (pre-solver `unsat`, Z3 `sat`) is a soundness bug. It must
  be captured as a repro, disable the pre-solver for that solver identity,
  and be reported. A pre-solver `unsat` with Z3 `unknown` is recorded as an
  opt-in-mode divergence candidate.

**Opt-in accept mode** (`accept-presolver-certificate`):

- Accepts a checked certificate in place of Z3 for that check.
- Follow-up replies cannot be produced, so the mode synthesizes no bytes.
  Results are reported at the function level as "established by
  certificate", with no rlimit and no model.
- The mode is recorded in `docs/divergences.md`.
- Its cache entries use key v2 with `Mode::OptIn("accept-presolver-certificate")`,
  so they can never collide with default-mode keys (rule 8).

**Cache and ledger integration.**

- Certificates are stored beside cache entries as a distinct, stronger tier
  (rule 9). Shared-tier certificates are rechecked locally before use.
- Default-mode entries remain Z3 replies only.
- The ledger records per-obligation provisional discharges keyed by stable
  obligation identity (label to span).

**Soundness gate: differential fuzzing.** `cargo-fuzz` targets, run locally
and never in CI:

1. The `sail-smtlib` parser and the extractor on arbitrary and mutated Verus
   transcripts. They must not panic, and the extracted relaxation must parse
   in Z3.
2. The pre-solver against Z3 on random ground LIA/Boolean formulas and on
   relaxations mutated from the corpus. Any pre-solver `unsat` that Z3 calls
   `sat` fails.
3. The certificate checker must reject mutated certificates.

Also:

- The `sail-bench presolver` anomaly count must be 0 on the whole corpus
  for every pinned solver change.
- Fault injection: a broken pre-solver must not change any default-mode
  output.

**Conformance requirements.**

- `sail-conformance` default mode stays byte-identical with the pre-solver
  enabled. It is advisory-only, so it must be a no-op on Verus's byte
  stream.
- The shim must not link the pre-solver (layering: `sail-shim` stays
  minimal).
- Standard tier: run it before enabling the pre-solver.
- Opt-in mode: its own divergence suite.

**Milestones.**

- M9.0: this spike (done).
- M9.1: simplex plus certificate checker plus fuzz targets, offline over
  traces only.
- M9.2: advisory provisional results in ledger and MCP.
- M9.3: opt-in accept mode.

Each stage is gated on the measured coverage justifying it.

**Recommendation.**

- **No-go for M9 as a latency feature.** At 8% of misses by count and 0.1%
  by time, with no fully covered cycle, the agent sees no faster verdict.
- **Re-direct the latency effort** to exact-prefix warm solver sessions
  (pre-replaying identical bytes into a pooled pinned Z3). That targets the
  measured 80–90% of miss latency without new divergences.
- **Revisit the pre-solver** only as (a) a per-obligation *provisional*
  signal (10–15% of obligations, "only this assertion is still open"), or
  (b) after Verus emits more arithmetic-heavy corpora (for example
  verified-storage or page-table), where this tool should be rerun
  unchanged.

## Documentation impact

This report is new. No user guide changes: the subcommands are an
experimental benchmark tool, documented in rustdoc and in `sail-bench`
usage. AGENTS.md, ROADMAP, CHANGELOG, known-issues and the ADRs are not
modified, as the lane requires.
