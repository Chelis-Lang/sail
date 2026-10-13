# L11: warm solver sessions for edit-loop cache misses

Rescue lane L11 asked whether warm solver sessions would shorten an agent's
edit-to-verdict time, and in which form. It compared two designs:

- **(A) exact-prefix warm sessions:** a pinned Z3 that has already replayed
  the bytes of a predictable prefix in a fresh process, so its history is the
  history plain Verus would give it;
- **(B) history-tolerant warm sessions:** a pinned Z3 holding the bucket's
  module-level assertions that solves only the queries whose assertion stack
  is new. It serves proofs that used well under their rlimit; failures,
  unknowns and near-limit proofs re-run in a fresh process.

Each design was measured against what an agent sees, under plain Verus and
current Sail. Design notes and the proposed decision are in
[ADR 0040](../../decisions/0040-warm-solver-sessions.md).

**Result: no-go for both designs now.**

- On the edit catalogues, the upper bound for (A) saves 9–19 ms per cycle on
  human-eval-verus (0.1–0.2% of a 5.0–5.5 s cycle) and 107 ms on vest (0.2%
  of 52 s). Prefixes that can be predicted before the edit save less: 0–6 ms
  and 98 ms.
- (B) saves nothing on human-eval-verus and 104 ms on vest. Its only large
  win was 290 ms on a 1.1 s single-file example.
- Both designs agreed with fresh runs on every verdict and every error label.
- Most agent-visible time is in Verus before and around the solver. On
  human-eval-verus the solver window is about 0.75 s of a 5 s cycle.
- The largest avoidable solver cost in these loops is VI-0001. In 22 of 63
  measured cycles, Verus's nondeterministic declaration order made a whole
  bucket miss. On human-eval-verus that costs about 2.7 s per affected cycle,
  roughly 50% of the cycle. Neither design can serve these misses.
- Priority: below the M8 warm frontend and below VI-0001 determinism work.

## Method

- **Tool.** Not merged. The tool is preserved at the tag
  `archive/l11-warm-sessions-spike` (`d37ac07`):
  - temporary shim timing, active only with `SAIL_TRACE_DIR` set in cached
    sessions;
  - `sail-bench warm-capture`, which extends the L6 capture with a cached
    arm;
  - `l11/l11.py` for offline replays and the report.
- **Sequences.** Cycle 0 verifies the unmodified unit and fills the cache.
  Every later cycle runs warm from all earlier cycles, as an agent's loop
  does, and starts from a fresh daemon per round.
  - the 2 loops in `corpus/agent-loops.toml` (cumulative);
  - 3 units from `corpus/edits.toml`;
  - human-eval-verus and vest from `corpus/repair-edits.json`.
- **Arms.** Each cycle has three arms:
  - `sail`: `sail-z3` with the daemon, plus per-session timing (spawn, raw
    write, last replayed reply, first forwarded reply, end);
  - `plain`: plain Verus;
  - `fresh`: pass-through with the observation tap, round 0 only. It
    provides per-check answers, v1 keys, rlimit and timing.
- **Rounds.** Three rounds, with the `sail` and `plain` order alternating
  each round.
- **Offline replays,** each the median of 3 runs on an idle host:
  - **(A)** time for a fresh pinned Z3 to consume a session's input up to:
    the missed command (upper bound); the last function boundary before the
    first changed byte (prediction at function granularity); the end of the
    module prefix; or nothing (a pre-spawned process).
  - **(B)** each missed session replayed in one process. The process gets
    every module-level command and only the push/pop blocks whose assertion
    stack (every state command in scope, like key v1) did not occur in an
    earlier cycle. It is timed from a process pre-warmed at the end of the
    module prefix. A "long-lived" variant also runs the previous cycle's
    version of each re-solved block first, where the module-level prefix
    allows it.
- **Cycle saving.** Each missed session's end moves earlier by its saving.
  The saving is the drop in the latest session end in the cycle. This
  ignores knock-on effects on Verus's thread scheduling, which are small when
  1–2 buckets miss.
- **History agreement.** `sail-bench history` replayed every check of the
  traced cycles (31,511) and of the fast tier (4,086) in a fresh process
  that has only the check's stack. This is (B)'s extreme case: no history at
  all.
- **Machine.**
  - Native AWS c7i.4xlarge (Intel Xeon Platinum 8488C, 16 vCPU), us-east-1,
    Ubuntu 24.04.5, kernel 7.0.0-1014-aws. Lane `l11`, torn down after use.
  - Verus 0.2026.10.04.426d8b0 with the pinned Z3 4.16.0.
  - One capture at a time; replays at 8 jobs once the capture had finished.
- **Outcomes.** Every cycle matched its catalogue's expected outcome, and the
  `sail` and `plain` outcomes agreed in every sample.

## Per catalogue

Setup is the time from spawn until a fresh Z3 has consumed the session's
input before the missed command. The offline figure isolates it. The live
figure runs from the shim's start-of-miss to the solver's last replayed
reply. "On critical path" is the sum of per-cycle (A) savings over the sum
of setups. Savings are means per cycle, with the maximum in brackets.

| catalogue | cycles (samples) | missed buckets / cycle, median (max) | setup ms / bucket, p50 / p90 (offline; live) | setup on critical path | (A) upper bound, ms (% of wall) | (A) predicted, ms | (B), ms (% of wall) | Sail wall, ms |
|---|---|---|---|---|---|---|---|---|
| agent loops (human-eval-verus) | 6 (18) | 1 (2) | 21 / 49; 23 / 40 | 54% | 15, max 50 (0.22%) | 1 | 0 (0%) | 5,534 |
| edits (human-eval-verus, vectors, vstd) | 5 (15) | 1 (4) | 24 / 42; 33 / 40 | 44% | 19, max 68 (1.1%) | 6 | 58, max 291 (5.3%) | 8,130 |
| repair human-eval-verus | 5 (15) | 1 (2) | 38 / 67; 22 / 41 | 25% | 9, max 91 (0.12%) | 0 | 0 (0%) | 5,138 |
| repair vest | 5 (15) | 2 (5) | 82 / 155; 73 / 153 | 36% | 107, max 245 (0.20%) | 98 | 104, max 282 (0.20%) | 52,311 |

- **Where setups fall.** Setups rarely overlap: 2 overlapping pairs in all
  63 samples. Only 25–54% of setup time lies on the critical path, because
  the missed bucket is usually not the last to finish.
- **What can be predicted.** 49 of 91 missed buckets first differ inside the
  module-level prefix, before the first query. Verus emits spec-function
  axioms and fuel at module level. For those buckets a warm session can hold
  at most part of the prefix. Only 33 of 91 had a function boundary before
  the change, and the median predicted saving per bucket is just the
  1.5 ms spawn.
- **Why (B) saves little.** A missed bucket re-solves few unchanged queries
  after the miss: 171 of 1,379 post-miss checks. In 32 of 91 missed sessions
  (B) must fall back anyway:
  - 15 are failing edits;
  - 17 are proofs above 1% of the limit, mostly in reordered buckets (next
    section). Raising the threshold to 4% changed nothing.
- **The single-file example.** On `examples/vectors.rs`, the edited bucket
  re-solves 7 unchanged queries after the miss, and (B) saves 290 ms of
  1.1 s.

## Agent-level metrics

These are means per cycle. First failure is the median over failing
samples; it is measured from the start of Verus to the solver's first
`sat`/`unknown` reply. Unchanged checks are re-solved checks whose stack
occurred in an earlier cycle. For (A), first failure assumes the predicted
prefix. For (B) it equals Sail's, because a failure's diagnostics come from
a fresh, exact replay. That replay can start when the miss happens, so it
costs nothing extra.

| catalogue | edit-to-verdict, ms: plain / Sail / A / B | first failure, ms: plain / Sail / A / B | unchanged checks re-solved per cycle: plain / Sail / A / B |
|---|---|---|---|
| agent loops | 32,479 / 5,534 / 5,534 / 5,534 | 3,332 / 3,155 / 3,144 / 3,155 | 436 / 1.0 / 1.0 / 0.5 |
| edits | 25,014 / 8,130 / 8,124 / 8,072 | 3,389 / 3,114 / 3,113 / 3,114 | 644 / 2.0 / 2.0 / 0 |
| repair human-eval-verus | 32,909 / 5,138 / 5,138 / 5,138 | 3,363 / 3,163 / 3,161 / 3,163 | 435 / 0.6 / 0.6 / 0 |
| repair vest | 66,836 / 52,311 / 52,214 / 52,207 | 52,532 / 46,081 / 46,024 / 46,081 | 3,212 / 7.6 / 7.6 / 0 |

Current Sail already removes almost all re-solving of unchanged checks.
What remains of the cycle is Verus:

- **human-eval-verus.** The solver window is a median 0.73 s of 5.0 s
  cycles that have no reordering miss.
- **vest.** The solver window is 6.9 s of 52 s.

## VI-0001 dominates the solver side of edit loops

A missed bucket counts as reordered when its input has the same byte
histogram as a session from an earlier cycle. VI-0001 reorders the `arrow_*`
accessors and renumbers their location labels. These were checked by hand
on human-eval-verus `lib.rs`: `arrow_1`/`arrow_2` swapped, at byte 31,029 of
1.1 MB.

| catalogue | samples with a reordering miss | re-solve after the miss, median ms | Sail wall mean, with / without |
|---|---|---|---|
| agent loops | 6 of 18 | 2,711 | 7,094 / 4,754 |
| edits | 6 of 15 | 997 | 15,068 / 3,504 (mostly vstd vs the rest) |
| repair human-eval-verus | 2 of 15 | 2,710 | 7,621 / 4,756 |
| repair vest | 8 of 15 | 173 | 52,838 / 51,710 |

Neither design serves these misses:

- the bytes differ from the first reordered declaration, so (A) holds at
  most a few kilobytes of prefix;
- the assertion stacks differ, so (B) re-solves every query in the bucket:
  25 on human-eval-verus `lib.rs`, at about 2.7 s.

The remedy is deterministic emission: an upstream fix, or an opt-in
normalization under charter rule 5. Normalization would need its own ADR
and whole-corpus validation.

## History tolerance: agreement with fresh runs

| population | checks | answers equal | failing checks: error labels equal | failing checks: model bytes equal | rlimit differs | rlimit spread (max/min), p50 / p99 / max |
|---|---|---|---|---|---|---|
| (B) re-solved checks, edit cycles | 370 | 370 | 7 of 7 | 3 of 7 | 71 (19%) | 1.0 / 1.32 / 3.06 |
| (B) long-lived variant | 370 | 370 | 7 of 7 | - | 73 | 1.0 / 1.32 / 3.06 |
| no history, edit cycles | 31,511 | 31,511 | 7 of 7 | 3 of 7 | 24,641 (78%) | 1.06 / 1.77 / 26.0 |
| no history, fast tier | 4,086 | 4,086 | 97 of 97 | 33 of 97 | 2,944 (72%) | 1.04 / 7.8 / 17.4 |

- **Verdicts and labels.** Verdicts and the error labels Verus reads never
  changed with history. Model bytes did change.
- **Per-function rlimit.** The rlimit Verus prints in profiling output would
  differ for 19% of the re-solved checks under (B), and for up to 78% with
  no history.
- **rlimit use.** In-session use as a fraction of the limit has p50 0.04%,
  p99 5.9% and a maximum of 82%. 6.3% of edit-cycle checks use more than 1%
  (2.9% on the fast tier).
- **Threshold.** History moved a check's rlimit use by up to 26×. That fits
  inside the 37× seed spread of the fragility census (ADR 0009). Serving
  only proofs at or below 1% of the limit therefore keeps the worst observed
  history at 26% of the limit. 1/26 ≈ 3.8% is the break-even. ADR 0040
  proposes 1%.

## Contrast with the 2026-10-08 measurement

The [2026-10-08 measurement](../../ROADMAP.md) found that warm pools could
only shorten a small solver-side remainder of a whole warm run. This
report agrees, and adds the edit-loop view:

- **Misses per cycle.** A cycle misses 1–2 buckets, not every check.
- **Setup per bucket.** Setup is paid once per missed bucket, at a median of
  21–38 ms on human-eval-verus and 82 ms on vest.
- **L6's 59 ms.** That figure is per check, as if each check started a fresh
  process. The shim starts one process per missed bucket (ADR 0011).
- **Critical path.** Even the per-bucket setups are mostly off the critical
  path.

## Limits

- **Corpus.** Two real projects, one vstd edit and one example. Projects
  with large buckets edited near their end (anvil, page-table) could make
  setups 10× larger. Re-measure there before closing the question for good.
- **(B) timing is emulated.** The emulation is an offline replay, not a
  prototype inside the shim. It assumes a pre-warmed module prefix and
  ignores the cost of keeping such processes.
- **Scheduling model.** The critical-path model ignores Verus thread
  scheduling.
- **Plain first failure** comes from the traced pass-through arm (round 0).

## Raw evidence

The files are in `~/sail-raw-evidence/l11/` and are not committed.

| file | contents | sha256 |
|---|---|---|
| `traces-r0-fast.tar.zst` | round-0 sessions (all arms) and fast-tier traces | `0497a726befb8e9d69438ac11673a79b3afd4c3fbf8b97fe1a9aa991c400419b` |
| `missed-inputs-r1-r2.tar.zst` | inputs of missed sessions, rounds 1–2 | `b55638ae3d051e4825f0e081ddeb23a15a8b11d9b036db9d4d9c7b1b64ff7160` |
| `timings.tar.gz` | every cached session's timing record | `4dd0c73889e88f2b216eb02b26ff0401fda11d4f42bf2cb5f48bd1ef555433de` |
| `edit-manifest.json` | capture manifest: arms, walls, outcomes | `1c57dcb18d046f9062315a6da5d8a9c107ba8215fc23445f4103d9743762ebee` |
| `fast-manifest.json` | fast-tier capture manifest | `e5a2e72851f96b70ff4bdf062247d4f68e8de84c801091508825a27ca8d8aa0b` |
| `measure.json` | offline (A) and (B) replays | `f50a94b0d6597d7aa15e8e711dacc22d8b80d66694006194d37bb759083235c1` |
| `report.json` | per-sample rows and summaries (1% threshold) | `2b96718140866da916c315c4cc6266408ab2a4ed581988db59e0a28da9c9c5c6` |
| `report-near004.json` | the same at a 4% threshold | `f292732d6713ccb30311f17e7a4636a459750f273b2a5b062b4a5ef20210e823` |
| `history-edit.json` | no-history replay, edit cycles | `b3cb4d947cb21798819ef1e8176dbf432bf12489cec727078a4b41e3066b6f6d` |
| `history-fast.json` | no-history replay, fast tier | `1ce47842880ed65345e4b543d42a7cb2cd2b718c3db7961b5ef754ee5ca89479` |
| `tables.md` | the tables above, as generated | `f427b753f5faca6c71c164fcbde2280dd9a3f787a8f167be5f1e4696730f005d` |
