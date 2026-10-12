# Z3 4.16.0 behaviour on the Verus query corpus

This document records what the pinned Z3 does with the queries the pinned
Verus actually sends. It covers:
- whether replaying a session reproduces it;
- how stable answers and effort are across random seeds;
- whether a check's outcome depends on the process's history;
- how `rlimit` works;
- what all of this means for cache keys.

The literature and source review of Z3 determinism is in
[prior-art.md](prior-art.md) §5. This document holds the measurements.

- **Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
  (`Z3 version 4.16.0 - 64 bit`, binary SHA-256
  `edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb`).
- **Machine:** Apple M3 (8 cores), 24 GB, macOS 26.5 (arm64). Other work
  was running at the same time, so wall-clock figures are indicative only.
  `rlimit-count` figures do not depend on load.
- **Date:** 2026-10-08.
- **Tools:** `sail-bench profile|seeds|history` (`crates/sail-bench`).
  The summarized outputs are in `bench/results/{profile,seeds,history}-*.json`.

## Summary

| Question | Answer on this corpus |
|---|---|
| Does replaying a session reproduce it? | Answers and model bytes: yes, always. `rlimit-count`: yes, except for a few nonlinear-arithmetic queries, which vary by a few units from run to run even in plain Z3. |
| Are answers stable across seeds? | Yes. With 8 seeds, 0 of 3,645 checks changed answer. |
| Is effort stable across seeds? | No. Per-check `rlimit` deltas have a median CV of 0 but a long tail: p99 CV 0.20 to 0.26, and up to 37× between seeds. |
| Is a check's outcome a function of the v1 state (stack only)? | **Answers:** yes in 3,645 of 3,645 checks. **Model bytes:** no (64 of 97 models differ). **`rlimit` deltas:** no (2,617 of 3,644 differ, by up to 17×). Every first check of a session matches its fresh replay exactly. |
| Is `(set-option :rlimit N)` relative to the cumulative count? | Yes. Each check gets exactly N more units. |
| What does `reason-unknown` say at rlimit exhaustion? | `(:reason-unknown "canceled")` |

**Verdict for ADR 0004:** cache key v1 (stack only) is **not sufficient**
for default mode. Verdicts agreed everywhere, so answers looked like a
function of the stack. But two things are history-dependent:
- the models Verus parses for diagnostics (their bytes, though not the
  error labels in them);
- the resource effort, which decides `unknown` versus a verdict near the
  limit.

ADR 0004's criterion (verdict, model *and* `rlimit-count` delta must match)
fails. Under that ADR, the default-mode key must bind the process's full
command prefix, and the follow-up ADR should record that. See
[Implications for cache keys](#implications-for-cache-keys).

## The query corpus

The corpus was captured with the pass-through shim (`sail-z3`, ADR 0005) and
`SAIL_TRACE_DIR`:
- **Examples:** every runnable file in the pinned Verus's `examples/`, with
  the same selection as [corpus.md](corpus.md) §1.2. That is 146 files:
  `ignore`-tagged files and `integer_ring/*` were skipped, and
  `--expand-errors` was added for the `expand-errors` tag.
- **vstd:** verified as in [corpus.md](corpus.md) §1.1.

| Set | Verus invocations | Solver processes | Checks | Answers |
|---|---|---|---|---|
| examples | 146 (6 fail by design) | 285 | 1,585 | 1,488 `unsat`, 97 `unknown` (all `(incomplete quantifiers)`) |
| vstd | 1 | 365 | 2,060 | 2,060 `unsat` |

- No check in either set answered `sat`.
- No check exhausted its rlimit. The largest single-check delta was 15.3 M
  units, against Verus's limit of 30 M.
- Every check was paired with its reply, and every session model was exact
  (no `inexact` snapshot).

Capture commands (paths relative to the scratch directory, `$V` the pinned
Verus directory, `$EX` the pinned Verus `examples/`):

```sh
cargo build --release -p sail-shim
# per example file f (with --expand-errors for the expand-errors tag)
VERUS_Z3_PATH=bin/sail-z3 SAIL_Z3_PATH=$V/z3 SAIL_TRACE_DIR=traces/<name> \
  $V/verus --crate-type=lib --no-report-long-running --num-threads 4 $EX/<f>
# vstd
VSTD_KIND=IsVstd RUST_MIN_STACK=10485760 VERUS_Z3_PATH=bin/sail-z3 SAIL_Z3_PATH=$V/z3 \
  SAIL_TRACE_DIR=traces-vstd $V/verus --crate-type=lib --is-vstd --export vstd.vir \
  --out-dir . --no-report-long-running --num-threads 4 \
  --cfg 'feature="std"' --cfg 'feature="alloc"' --cfg 'feature="nonzero_internals"' \
  <verus>/source/vstd/vstd.rs
```

Analysis commands:

```sh
sail-bench profile traces      --json profile-examples.json
sail-bench profile traces-vstd --json profile-vstd.json
sail-bench seeds   traces      --z3 $V/z3 --seeds 8 --jobs 4 --json seeds-examples.json
sail-bench seeds   traces-vstd --z3 $V/z3 --seeds 8 --jobs 4 --json seeds-vstd.json
sail-bench history traces      --z3 $V/z3 --jobs 4 --json history-examples.json
sail-bench history traces-vstd --z3 $V/z3 --jobs 4 --json history-vstd.json
```

The `seeds` and `history` subcommands were removed later (rescue lane L17).
They remain in the repository's history, for example at tag
`archive/l11-warm-sessions-spike`; the results are in `bench/results/`.

`sail-bench history` with `--all-checks` gives the per-check data used for
the ratio figures below. Each analysis took 20 to 45 s of wall time with 4
jobs. None of the solver runs hit the tool's safety-net timeout.

**A trace defect, found and fixed.** The first capture lost pairing in 2
of 650 sessions. The shim's tap received a solver reply before the input
chunk that caused it. The forwarding thread wrote the chunk to the solver
first, and only then handed it to the tap, so the other pump could win the
race. The tap now gets each chunk before it is forwarded
(`crates/sail-shim/src/lib.rs`, `pump`). The send never blocks, so
forwarding is unaffected. The recapture paired 650 of 650 sessions. The
analyses above re-pair from the raw transcripts by `echo` batch, so they did
not depend on the tap's pairing.

## Profile: where solver time goes

Solve time is the wall-clock time from the `check-sat` batch reaching the
shim to its answer coming back, as recorded in the trace.

| Set | Checks | Total | p50 | p90 | p99 | Max | Top 1% share | Top 10% share |
|---|---|---|---|---|---|---|---|---|
| examples | 1,585 | 7.2 s | 0.90 ms | 7.8 ms | 51 ms | 752 ms | 34.7% | 72.4% |
| vstd | 2,060 | 14.8 s | 2.3 ms | 12.6 ms | 88 ms | 1,109 ms | 31.5% | 63.8% |

- Solver time is heavily concentrated. The slowest 10% of checks take 64% to
  72% of all check time, and the slowest 1% take about a third.
- In the examples, two files (`cuckoo_hash_table/cuckoo.rs` and `main.rs`)
  account for 39% of all check time, mostly in one query each.
- The median check is around a millisecond. For most queries, per-query
  overhead in sail (hashing, IPC) must therefore stay well below a
  millisecond to pay off.
- Check time is a small part of Verus's wall time. All of vstd's checks sum
  to 14.8 s of solver time, against roughly 47 s of wall time with
  4 threads. Much of the rest is Verus's own pipeline and Z3 parsing the
  prelude and context. The cold-workload benchmark (ADR 0007) measures that
  side.

## Replay determinism (seed 0 versus the recording)

`sail-bench seeds` replays each whole recorded session, unmodified, into a
fresh Z3 (seed 0), and compares it with the recorded replies.

| Set | Answers differing | Follow-up replies differing | `rlimit` deltas differing |
|---|---|---|---|
| examples | 0 / 1,585 | 0 / 194 (models and reasons, byte for byte) | 1 / 1,584 |
| vstd | 0 / 2,060 | 0 / 0 | 0 / 2,060 |

The one `rlimit` difference is real solver nondeterminism, not a replay
artefact:
- **Where.** It is the `by(nonlinear_arith)` spinoff processes of
  `examples/imo_1988_6.rs` (`smt.arith.solver 6`).
- **Direct Z3.** Feeding one of these inputs to the pinned Z3 30 times gave
  two different final `rlimit-count`s, 11 times and 19 times. The answers
  were the same each time.
- **Through the tool.** Repeated seed-0 replays of that example flagged 0 to
  3 of its 14 nonlinear processes, each differing by 2 to 6 units.

This is the behaviour already captured in
`corpus/repros/z3-nla-rlimit-nondeterminism/`. The measurement here adds
one point: across the whole corpus, it is confined to nonlinear spinoffs.
No `smt.arith.solver 2` session (every main bucket process, and all of vstd)
varied.

## Seed stability

For seeds 1 to 7, `sail-bench seeds` inserts
`(set-option :smt.random_seed S)` and `(set-option :sat.random_seed S)`
immediately before the first command that is not a `set-option`, which is
right after Verus's option prelude. Seed 0 is the unmodified session; Z3's
default seed is 0.

**Z3 accepts this placement.** No replay produced an `(error …)` reply or an
unpaired batch. The options take effect, since effort changes with the seed
(below).

| Set | Checks (all 8 seeds answered) | Answer-unstable | Verdict-unstable (unsat versus not) | `rlimit` CV p50 / p90 / p99 / max | Zero CV | max/min p90 / p99 / max |
|---|---|---|---|---|---|---|
| examples | 1,585 | 0 (0.00%) | 0 | 0.000 / 0.030 / 0.204 / 1.57 | 1,023 / 1,584 | 1.08 / 2.01 / 37.1 |
| vstd | 2,060 | 0 (0.00%) | 0 | 0.000 / 0.040 / 0.255 / 1.02 | 1,278 / 2,060 | 1.11 / 2.19 / 6.7 |

- **No answer changed under any seed,** including the 97 `unknown`
  (`incomplete quantifiers`) answers, which stayed `unknown` with the same
  reason.
- **Effort is seed-sensitive in the tail.** Most checks (65%, and 62% in
  vstd) used exactly the same `rlimit` under every seed. Others varied a
  lot:
  - `guide/iterators.rs`, check 3, ranged from 151 k to 5.6 M units across
    seeds;
  - `guide/recursion.rs`, check 12, from 29 k to 87 k.
- **Implication.** The corpus has no check near its rlimit, so this
  variation never turned into a verdict change here. On a corpus with
  queries closer to the limit, the same spread (up to 37×) would turn into
  `unknown` versus `unsat` flips. That is the Mariposa instability, and the
  fragility mode would measure it.

## History dependence (ADR 0004 experiment)

`sail-bench history` builds a fresh-process input for every check, from
exact bytes, as follows:
1. every `set-option` sent before the check, in the original order;
2. the snapshot's assertion stack, as `sail-session` records it: each
   level's commands in order, with `(push)` between levels;
3. `(get-info :all-statistics)`, the check, and `(get-info :all-statistics)`;
4. the commands the session sent after the check, up to its next statistics
   request: the `(set-option :rlimit 0)`, `get-info :reason-unknown` and
   `get-model` follow-ups, and anything else in between.

It runs this input in a fresh Z3 and compares the result with the in-session
replies. The `rlimit` delta is measured over the same span on both sides:
from the last statistics before the check to the first after its follow-ups.

| Set | Checks | Answer equal | Follow-ups: equal / differ / none | True error labels in models equal | `rlimit` equal / differ / n.a. |
|---|---|---|---|---|---|
| examples | 1,585 | 1,585 | 33 / 64 / 1,488 | 97 / 97 | 506 / 1,078 / 1 |
| vstd | 2,060 | 2,060 | 0 / 0 / 2,060 | (no models) | 521 / 1,539 / 0 |

**The method is sound.**
- The first check of each session runs in a state identical to its fresh
  replay, apart from the extra option commands. Its `rlimit` matched in
  648 of 650 sessions. The 2 exceptions are the nondeterministic nonlinear
  processes above; a second full run had only 1 exception.
- Later checks (`prior_checks > 0`) differ in 2,615 of 2,994 cases.
- Every v1 key the tool computed equals the shim's trace key.

**What differs, and by how much.**
- **Answers:** never.
- **Models:** 64 of the 97 models differ in their bytes. These are all
  `unknown` (`incomplete quantifiers`) checks followed by `get-model`.
  - The fresh-name numbering is history-dependent (`distinct-elems!4!val!39`
    in session, `distinct-elems!6!val!18` fresh).
  - So is the set of functions the model defines. One fresh model
    contained a `define-fun I` that the in-session model lacked.
  - The `%%location_label%%`/`%%global_location_label%%` values that Verus
    reads to choose diagnostics agreed in all 97.
  - The `reason-unknown` replies agreed in all cases.
- **Effort:** for later checks, the fresh `rlimit` divided by the in-session
  one was as follows.

  | Set | Fresh lower | Fresh higher | p10 | p50 | p90 | Range |
  |---|---|---|---|---|---|---|
  | examples | 81 | 995 | 1.00 | 1.04 | 1.18 | 0.25 to 17.4 |
  | vstd | 104 | 1,435 | 1.00 | 1.06 | 1.12 | 0.40 to 7.4 |

  One vstd check used 619 k units in session and 337 k fresh.

**Same key, other process.** Across the corpus, 13 (examples) and 26 (vstd)
v1 keys were shared by two or more checks in different processes. In every
such group the recorded answers, follow-ups and `rlimit` deltas agreed.
This is a natural experiment, but a weak one. The groups are small, and
they were not checked for having the same history. Spinoffs that replay the
same bucket prefix would produce exactly such groups.

## rlimit semantics

`crates/sail-bench/tests/z3_experiments.rs`
(`rlimit_is_relative_and_exhaustion_reads_canceled`, run with
`SAIL_TEST_Z3`) is a direct test: a pigeonhole problem (12 pigeons, 11
holes) checked three times in one process, with `:rlimit` 200,000, 200,000
and 300,000.

- **Relative.** The `rlimit-count` deltas are exactly 200,000, 200,000 and
  300,000. A first manual run continued from 25,515 to 225,515, 425,515
  and 925,515. Each check's budget is counted from the cumulative count at
  the start of that check, as `scoped_rlimit` in Z3's source suggests
  (prior-art §5.2) and as Verus's per-query accounting assumes. Survey
  item 5 (`verus-z3.md`, "Must be confirmed empirically") is confirmed.
- **Exhaustion.** `check-sat` answers `unknown`, and
  `(get-info :reason-unknown)` answers `(:reason-unknown "canceled")`. It is
  not `max. resource limit exceeded`, and no timer was armed. Verus maps
  `canceled` to "Resource limit (rlimit) exceeded"
  (`verus-z3.md` §4). Survey item 4 is confirmed. Consequences for
  charter rule 4:
  - In 4.16.0's CLI, `canceled` is how an rlimit exhaustion is reported, so
    the string alone cannot tell rlimit exhaustion from a cancellation.
  - A cache can still separate them, by knowing whether it armed a timer
    (sail must not arm one in default mode), and by checking that the
    `rlimit-count` delta reached the limit.
- **Corpus.** The corpus contained no rlimit exhaustion, so this string
  comes only from the synthetic test. The only `reason-unknown` the corpus
  produced was `(incomplete quantifiers)`.

## Implications for cache keys

1. **Key v1 must not answer default-mode queries.**
   - Verdicts matched in every check measured. But a cached reply includes
     the model bytes, and those depend on history.
   - Labels agreed here, but a model with different bytes can in principle
     also differ in label values, and Verus's diagnostics depend on those.
   - More importantly, effort depends on history by up to 17×. For any
     query near its rlimit, a different history can turn `unsat` into
     `unknown`, or back.
   - ADR 0004's rule therefore applies: the default-mode key must also bind
     the full command prefix of the process. This should be recorded in the
     follow-up ADR.
2. **Prefix keys are faithful as far as this corpus shows.**
   - A first check replayed in a fresh process with the same prefix
     reproduces answers, models and `rlimit` exactly.
   - The exceptions are the few-unit `rlimit-count` jitter of nonlinear
     spinoffs, which affects plain Verus equally. The repro and
     `docs/open-questions.md` already track it.
   - A key over the exact command prefix, with the pinned binary and
     platform, therefore identifies a reply that the pinned Z3 really gives
     for that input.
3. **A stack-only key is still useful, but only for hints or opt-in modes.**
   - It is a good *candidate* index: equal stacks gave equal verdicts in
     3,645 of 3,645 cases.
   - Uses that do not change what is returned are fine, such as
     prioritising, prefetching, or predicting fragility.
   - So is a named opt-in mode that accepts a verdict from another history
     and recomputes models when diagnostics are needed.
4. **Re-synthesize `:all-statistics`.** Statistics replies are
   history-dependent (cumulative `rlimit-count`, time, memory). A replayed
   or pooled session must re-synthesize them, or pass them through from a
   real solver, as the Verus survey already recommends.
5. **Seeds are not a lever on this corpus.**
   - With no verdict changes in 8 seeds, a seed portfolio would find no
     extra proofs here.
   - Fragility reporting should use `rlimit` spread (CV, max/min) rather
     than answer flips. About 10% of checks have a seed-to-seed CV above
     0.03 to 0.04, and about 1% above 0.2.
