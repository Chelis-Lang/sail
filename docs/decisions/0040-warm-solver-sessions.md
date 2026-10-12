# 0040: Warm solver sessions for edit-loop misses

- **Status:** proposed
- **Date:** 2026-10-12

## Context

ADR 0038 ranks warm, exact-prefix solver sessions second among the
latency work that follows M7. That ranking rested on L6's figure: about
59 ms of a 72 ms median miss is spent starting Z3 and replaying the prefix.
Rescue lane L11
([report](../reports/rescue/l11-warm-sessions.md)) measured what warm
sessions would remove from an agent's edit cycle. It ran on native x86-64,
with three rounds over the agent-loop, edit and repair catalogues, and
compared two designs.

**What users need (charter rule 1).**

- Per function, Sail's pass/fail and error messages match what
  `cargo verus verify` gives on the same code.
- Nothing is reported verified unless the pinned Z3 proved it.
- Where plain Verus itself varies from run to run, Sail says *unstable*.
- The owner's answer to open question 7 already applies: a cached proof
  stays sound, exhaustion and near-limit unknowns are re-solved, and rlimit
  consumption is metadata.
- Byte-identical replies (ADR 0008) are one sufficient mechanism, not the
  goal.

**The designs compared.**

- **(A) Exact-prefix sessions.** A pinned Z3 that has replayed, in a fresh
  process, a byte prefix that can be predicted before the edit. A miss is
  served only if the new stream starts with exactly those bytes. Replies are
  then byte-identical to plain Verus's, and ADR 0008 is unchanged.
- **(B) History-tolerant sessions.** A pinned Z3 holding the bucket's
  module-level assertions solves only the queries whose assertion stack is
  new. Its history differs from plain Verus's.

**Measurements** (native c7i.4xlarge, us-east-1):

- **Misses and setup.**
  - A cycle misses 1 bucket (human-eval-verus) or 2 buckets (vest) at the
    median.
  - Setup per missed bucket is 21–38 ms at p50 and 42–67 ms at p90 on
    human-eval-verus; on vest it is 82 ms and 155 ms.
  - Only 25–54% of setup time lies on the cycle's critical path.
- **(A) upper bound.**
  - It saves 9–19 ms per cycle on human-eval-verus (0.1–0.2% of 5.0–5.5 s)
    and 107 ms on vest (0.2% of 52 s).
  - Prefixes that can be predicted save 0–6 ms and 98 ms.
  - 49 of 91 missed buckets first change inside the module-level prefix,
    where Verus emits spec-function axioms and fuel.
- **(B).**
  - It saves nothing on human-eval-verus and 104 ms on vest. On one
    single-file example it saves 290 ms of 1.1 s.
  - After a miss, current Sail re-solves few unchanged queries: 171 of
    1,379.
  - 32 of 91 missed sessions must fall back to a fresh replay: 15 failing
    edits and 17 proofs above 1% of the limit.
- **History tolerance is safe for verdicts.**
  - Answers matched fresh runs on 370 of 370 re-solved checks, and on
    35,597 of 35,597 checks replayed with no history at all.
  - The error labels Verus reads matched on all 104 failing checks. Model
    bytes differed on 68 of them.
  - rlimit use moved by up to 26× (p99 1.8× on edit cycles, 7.8× on the
    fast tier). This is within the census's 37× seed spread (ADR 0009).
  - The per-function rlimit in Verus's profiling output would differ for 19%
    of (B)'s re-solved checks.
- **A long-lived process cannot pop back to an earlier function.** Verus
  asserts module-level axioms between queries, for example recursive
  function definitions. A process that has run later queries therefore
  holds a stack that is a strict superset of what an earlier query saw.
  Serving that earlier query from it could prove what plain Verus fails.
  A history-tolerant session must hold exactly the new stream's module-level
  commands up to the query. It can move forward through a bucket, and back
  only across a `(push)`/`(pop)` pair.
- **Where agent-visible time goes.**
  - On human-eval-verus the solver window is about 0.73 s of a 5 s cycle.
    The rest is Verus: the Rust front end, VIR, and SMT generation.
  - The largest solver-side cost is VI-0001. In 22 of 63 cycle samples a
    reordered `arrow_*` declaration made a whole bucket miss, costing about
    2.7 s on human-eval-verus (cycles of 7.1 s instead of 4.8 s).
  - Neither design serves those misses, because their bytes and their
    stacks both differ.

## Decision

Proposed for the owner. Neither design moves the agent loop on the current
corpus.

1. **Build neither (A) nor (B) now.** Warm solver sessions move from item 2
   of ADR 0038's direction list to after:
   - the M8 warm frontend;
   - deterministic SMT emission. Fix VI-0001 upstream, or propose an opt-in
     normalization under charter rule 5, with its own ADR and whole-corpus
     validation. On these loops this is worth 2.3–2.9 s per affected cycle,
     against at most about 0.1 s for warm sessions.
2. **Reopen when any of these holds**, and repeat the L11 measurement with
   the archived tool (`archive/l11-warm-sessions-spike`) first:
   - M8 has landed and the solver window is at least 25% of edit-cycle wall
     time;
   - a project's missed-bucket setup on the critical path is 500 ms or more
     (anvil and page-table are the candidates);
   - VI-0001 is fixed, so that misses come only from edits.
3. **Design for when it is reopened.** If reopened, build a combination that
   leads with history-tolerant serving, not with exact-prefix warmth:
   - **Sessions hold exact stacks.** A per-bucket session is fed the new
     stream's module-level commands. It skips the push/pop blocks that
     already have a cached proof of the identical stack, and solves only new
     blocks. It never serves a query whose stack differs from plain Verus's.
     Sessions move forward only, except across a push/pop pair.
   - **What may be served.** Only `unsat` with recorded use at or below 1%
     of a non-zero limit. At the worst observed history spread (26×), that
     stays at 26% of the limit. The census rule for `unknown` (ADR 0009)
     is unchanged.
   - **The rest re-runs exactly.** `sat`, `unknown`, errors and near-limit
     proofs re-run as today: the exact fresh replay of ADR 0011. That replay
     starts at the miss, in parallel, so a failure's diagnostics come from
     plain-Verus-equivalent history at no extra latency.
   - **Provenance.** A served proof is recorded as `history-tolerant`, with
     the solver identity and its effort, and is reported as verified,
     because the pinned Z3 proved that exact stack. A background exact
     replay confirms it and fills the v2 cache. If confirmation ever
     disagrees on a verdict, that is a bug: the mode switches off and an
     issue is raised.
   - **rlimit is metadata.** `:all-statistics` replies for served queries
     carry the counts of the history they ran in. Verus shows them only in
     profiling output. The report states this as a known difference.
   - **Exact-prefix sessions (A)** stay a later refinement. They are limited
     to the module prefix and to function boundaries known from the
     attribution ledger.
4. **Owner decision needed.** Point 3 serves verdicts across process
   histories in default mode. That revisits open question 8: default keys
   bind the full process prefix, and the stack-only mode was deferred. The
   data here is offered as the evidence that question asked for. Until the
   owner decides, any history-tolerant serving stays a named opt-in mode,
   recorded in `docs/divergences.md`, under keys that cannot collide with
   default keys.

**Constraints for any build:**

- **Fail safe (rule 10).** Any session fault, timeout, mismatch, daemon
  loss or unexpected reply drops the session and falls back to the exact
  replay against the pinned Z3.
- **Layering.** The shim stays small: `sail-shim` must not reach
  `sail-solver`. Sessions live in the daemon, which the shim reaches only
  through the existing `sail-api` client. The shim's forwarded bytes never
  depend on a session.
- **Bounded resources.**
  - Sessions are capped per bucket (one), per agent and per host, with a
    memory ceiling and an idle timeout.
  - Under pressure the least recently used session is evicted.
- **Fairness (ADR 0038, direction item 5).** Each agent gets a share of
  session slots. One agent's buckets cannot evict another agent's active
  session.

**Coverage required before any default use:**

- **Conformance.** The standard tier and the edit catalogues, with the mode
  on and off. Per-function verdicts and diagnostics must match plain Verus,
  with unstable units classified as in ADR 0009.
- **No-history replay.** `sail-bench history` over the standard tier.
  Verdicts and labels must agree, and the measured history spread
  recalibrates the 1% threshold.
- **Fault injection** (`docs/fault-injection.md`):
  - kill a session mid-query;
  - a hung session;
  - a session whose stack diverged;
  - daemon death while a session is held;
  - eviction while a query is in flight;
  - concurrent agents contending for slots.

  Each must end in a correct answer or the exact fallback.

## Consequences

- No product code changes now. The pre-solver remains off the list, and warm
  sessions move behind the M8 warm frontend and VI-0001 determinism.
- VI-0001's measured edit-loop cost is recorded in its issue entry and in the
  roadmap.
- If the owner later authorizes stack-only serving in default mode, Sail
  can skip re-solving unchanged queries after a miss. The same data shows
  that this matters only where buckets are large and edits fall early in
  them.
