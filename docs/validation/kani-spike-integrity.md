# Kani spike S6: verdict integrity

Status: **passes.** On a 51-harness mixed set under a 32 GiB memory cap, direct
Kani 0.68.0 changed a harness's status between repetitions at both `-j16` (2
harnesses) and `-j32` (1 harness). A supervision prototype with largest-first
starts and a memory gate gave the same 51 outcomes in 5 of 5 repetitions, no
wrong success, and a median wall of 228.0 s against 244.1 s for direct `-j16`.
Of the upstream reports, #5170, #5169 and #4966 reproduce at 0.68.0. #4905's
real trigger does not, but the defect behind it is still there and an injected
fault shows it. #5043's flips reproduce as memory kills, not as timeouts. This
is spike S6 of the Kani phase design (`docs/design/kani-phase.md`, ADR 0180);
nothing here is product code.

## Environment

| | |
|---|---|
| Host | one `c7i.8xlarge` (32 vCPU, 64 GiB, no swap), us-east-2, Ubuntu 24.04, kernel 7.0.0-1018-aws, systemd 255, cgroup v2, lane `kani-s6`, 2026-11-27, 1.8 hours |
| Kani | 0.68.0 release bundle (`cargo install --locked kani-verifier --version 0.68.0`, `cargo kani setup`) |
| CBMC | 6.11.0 from the bundle, `cbmc` sha256 `01ae2ae7…eb1643a` |
| Rust | `nightly-2026-08-21` |
| `s2n-quic-core` | aws/s2n-quic at `e11382fda8a52979a2f952a789efc616db4ff4e4`, the S3 `Cargo.lock` (sha256 `5b7738d1…6cd2b3`) |

**Mixed set (51 harnesses):** the 40 `s2n-quic-core` harnesses plus 11
synthetic ones in a module added to that crate: six memory-bound (`hog_1000`
×3 at 2.6 GiB and 12 s, `hog_2000` ×2 at 5.4 GiB and 27 s, `hog_3000` at
8.8 GiB and 42 s), four SAT-bound (8 to 77 s) and one with a real
counterexample. Uncapped, one `cargo kani -j16` reports 50 successful and 1
failed. The heaviest real harness, `sync::spsc::tests::alloc_test`, needs
17.4 GiB (CBMC plus Kissat) and 220 s.

Direct arms ran the unmodified bundle. Fault injection and the prototype use a
stand-in installed as the bundle's `cbmc`, as in S3; with no `S6_*` variable
set it execs the real binary.

## Part A: direct Kani 0.68.0

Five repetitions per row; every repetition of a row gave the same result unless
stated. "Summary" is the `Complete - N successfully verified harnesses, M
failures, T total.` line.

| Case | Output format | Kani prints | Exit |
|---|---|---|---|
| #4905's second reproducer, unchanged | regular | `1 of 204 failed`, `VERIFICATION:- FAILED`. CBMC 6.11.0 does not run out of memory. **Does not reproduce.** | 1 |
| `hog_1000` in a scope with `MemoryMax=1G`, `MemorySwapMax=0`, `OOMPolicy=continue` | regular, terse | `CBMC failed`, `VERIFICATION:- FAILED`, "CBMC appears to have run out of memory"; summary 0/1/1 | 1 |
| same | old | no verdict line; `Verification failed for`; summary 0/1/1 | 1 |
| same three, scope left at systemd's default `OOMPolicy=stop` | all | systemd stops the whole scope after the kill: 11 of 15 runs end with no summary | 143 |
| `SIGKILL` to `cbmc` | regular, terse | as the memory row | 1 |
| `SIGTERM` to `cbmc` | regular, terse | `CBMC failed with status 15`, `VERIFICATION:- FAILED` | 1 |
| `--harness-timeout 5s` on `hog_4000` | regular, terse | `CBMC failed`, `VERIFICATION:- FAILED`, "CBMC timed out" | 1 |
| same | **old** | **`Complete - 1 successfully verified harnesses, 0 failures, 1 total.`** | **0** |
| #5170's reproduction (sleeping external solver, `--harness-timeout 3`) | **old** | **the same success, 10 of 10** | **0** |
| same | regular, terse | `VERIFICATION:- FAILED`, "CBMC timed out". Across all three formats the solver process is still running afterwards, 15 of 15 (#5169) | 1 |
| `SIGTERM` or `SIGKILL` to the Kani driver | regular | no verdict, no summary; `cbmc` is still running afterwards, 10 of 10 (#5169) | 143, 137 |
| `SIGKILL` to one `goto-instrument` in a three-harness run (#4966) | regular | `error: goto-instrument exited with status signal: 9 (SIGKILL)`; no results, no summary | 1 |
| same, `-j 4` | terse | two harnesses print `VERIFICATION:- SUCCESSFUL`, then the error; no summary; the third harness is never mentioned | 1 |
| `hog_1000` and `hog_200` in one 1 GiB scope, `-j 2` | terse | both `FAILED`, summary 0/2/2; `hog_200` needs 0.55 GiB and passes when run after `hog_1000` | 1 |

An address-space limit on `cbmc` alone (`ulimit -v` in the stand-in), swept
over 485 limits on three harnesses and three formats, never produced a
success from an incomplete run. The outcomes were `CBMC failed with status 6`,
a failed verdict with `ERROR` check statuses, a complete run, and in 15 runs a
driver panic (`cbmc_output_parser.rs:473`, "missing field `sourceLocation`",
exit 101, no summary).

`--output-format old` also reports two passing harnesses as failed, because it
takes CBMC's exit status 10 (Kani's own reachability checks) as a failure.

### Every way found for direct Kani to report success without a completed CBMC result

1. **Timeout under `--output-format old`** (#5170): real, 15 of 15. No verdict
   line, a success summary and exit 0. In `call_cbmc.rs` a timed-out run takes
   the `mock_success` branch.
2. **A `result` array followed by a failing CBMC exit** (#4905's mechanism):
   injected. `call_cbmc.rs` states "We actually ignore the CBMC exit status"
   whenever results are present. With the array cut before the failing
   assertion, CBMC's "Out of memory" error appended and exit status 6, Kani
   prints `0 of 23 failed`, `VERIFICATION:- SUCCESSFUL` and exits 0 for the
   harness whose full result is `1 of 204 failed` (10 of 10, regular and
   terse). A passing harness cut to 2 of its 6 checks is successful (10 of 10),
   and an empty array gives `0 of 0 failed`, successful (5 of 5).
3. **Complete results, then CBMC dies by a signal** (`SIGKILL`, `SIGSEGV`):
   injected. Successful, exit 0 (15 of 15).

Not success, but results lost with nothing in the failure count: one
`goto-instrument` failure ends the run without a summary (#4966); a killed
driver prints nothing and leaves `cbmc` running (#5169); the driver panic
above; and a memory-capped systemd unit with the default policy.

### The mixed set under a 32 GiB cap

`cargo kani -j N --output-format terse --harness-timeout 300s -Z
unstable-options` in a scope with `MemoryMax=32G`, `MemorySwapMax=0`,
`OOMPolicy=continue`, warm target. Exit status is 1 in every run (one harness
has a real counterexample). Kernel kills were mapped to harnesses by sampling
`cbmc` processes each second.

| Arm | Summary line per repetition (successful/failures) | Harnesses killed per repetition | Wall, s | Median |
|---|---|---|---|---|
| `-j16` | 49/2, 49/2, 48/3, 50/1, 50/1 | 1, 1, 2, 0, 0 | 237.6, 243.8, 250.4, 244.1, 248.4 | 244.1 |
| `-j32` | 48/3, 47/4, 48/3, 48/3, 48/3 | 2, 3, 2, 2, 2 | 240.9, 231.1, 233.5, 233.2, 235.8 | 233.5 |

| Harness | `-j16` | `-j32` |
|---|---|---|
| `hog_3000` | failed, failed, failed, successful, successful | failed in 5 |
| `hog_2000_b` | successful except repetition 3 | successful except repetition 2 |
| `hog_2000_a` | successful in 5 | failed in 5 |

Every killed harness printed `VERIFICATION:- FAILED` with the out-of-memory
text and was in the failure list: no wrong success and no omission in these
ten runs. All 51 harnesses were reported each time. No harness reached the
300 s timeout, so #5043's timeout-driven flips were not produced here; the
flips are memory kills, and which harness is killed depends on the start order
(random per run) and on `-j`. Repetition 1 of `-j32` includes a 10.7 s rebuild.

## Part B: supervision prototype

A Python script, outside Sail. Each harness is one `cargo kani --harness NAME
--exact --harness-timeout 300s -Z unstable-options` in its own cgroup v2
directory (`memory.max` 24 GiB, no swap) under a parent capped at the same
32 GiB. Plain cgroup directories replace transient scopes because a scope is
removed when its last process exits, before `memory.events` can be read.

**Success needs all of:** the driver's exit status observed and 0; no signal;
`oom` and `oom_kill` in `memory.events` both 0; the cgroup empty at exit (if
not, it is killed); the supervisor's 420 s deadline not fired; exactly one
`VERIFICATION:- SUCCESSFUL` line, one check summary with 0 failed and a 1/0/1
summary line; exactly one CBMC run observed by the stand-in with exit status 0
or 10, parseable output with a final status and no error message; and the
number of checks Kani prints equal to CBMC's result count less what Kani
0.68.0 hides (reachability checks, successful sanity checks, pointer
arithmetic and pointer primitive classes). A counterexample needs the same
with a failed verdict. Anything else is no-claim with its reasons.

**Scheduling:** 16 slots; harnesses ordered by prior CBMC time, longest first;
a harness starts only if its prior peak memory times 1.15 fits in the budget
after reserving, for each running harness, the larger of its prior peak and
its current use; the first harness that fits is taken. Priors (CBMC seconds
from the stand-in, `memory.peak` from the cgroup) come from one earlier
uncapped run.

| | Repetitions 1 to 5 |
|---|---|
| Outcomes | 50 success, 1 counterexample, 0 no-claim, every time |
| Harnesses whose outcome changed between repetitions | 0 |
| Wrong success | 0 |
| Memory events in the run cgroup | `max` 0, `oom` 0, `oom_kill` 0 |
| Peak memory of the run | 29.96 to 30.12 GiB |
| Wall, s | 227.5, 228.0, 227.6, 228.0, 230.0 (median 228.0) |
| Driver and Cargo overhead per harness | median 1.8 s, maximum 5.1 s |

Fault arm, one harness under the supervisor, five repetitions each, all
identical:

| Fault | Kani prints | Prototype |
|---|---|---|
| none | successful | success |
| results cut, exit 6 (way 2) | successful, exit 0 | no-claim: CBMC exit status 6, output incomplete, error message |
| results, then `SIGKILL` (way 3) | successful, exit 0 | no-claim: CBMC exit status 137 |
| `memory.max` 1 GiB | failed | no-claim: `oom_kill` 1, CBMC exit status 137 |
| supervisor deadline 8 s | nothing | no-claim: deadline fired, no exit status |
| `--harness-timeout 5s` | failed | no-claim: no CBMC exit status, 3 processes left in the cgroup |

## Result against the pass rule

Rule: zero wrong success and zero run-to-run status flips under the prototype
where direct Kani shows at least one, with wall no worse than direct `-j16`.

- Direct flips: 2 harnesses at `-j16`, 1 at `-j32`. Prototype: 0 in 5.
- Wrong success: 0 under the prototype, including the two injected ways where
  direct Kani prints success.
- Wall: prototype median 228.0 s; direct `-j16` 244.1 s; direct `-j32`
  233.5 s. Direct's wall includes harnesses it killed early.

**Pass.** Consequences for the design:

- Three downgrades to register in `docs/divergences.md`: timeout under the old
  output format, results followed by a failing CBMC exit, and results followed
  by a signal. In each, direct Kani prints success and Sail reports no-claim.
- CBMC's exit status and the completeness of its output are not visible in
  `cargo kani`'s output. The supervisor has to sit at the CBMC boundary, which
  is where the L2 shim already is.
- The count reconciliation must mirror the display filter of the pinned Kani
  version.
- Kill the cgroup after every harness: a timeout leaves the solver, and under
  a stand-in `cbmc` itself, running.
- A 12 GiB per-harness limit (design section 7) would no-claim
  `s2n-quic-core`'s `alloc_test`, which needs 17.4 GiB.
- A harness filter with `--exact` and unchanged flags does not rebuild the
  crate; changing `-Z` flags or switching between exact and substring filters
  does.

## Limits

- One host, one project plus synthetic harnesses. Every flip seen is on a
  synthetic memory-bound harness.
- Priors came from the same machine an hour earlier. A first run with no
  priors was not measured.
- Ways 2 and 3 are injected at the CBMC boundary. No real CBMC run at 6.11.0
  produced them.
- Wall figures are five samples per arm on a shared-tenancy host, not the
  quiet pool. The makespan is bounded below by the 220 s harness, so the gain
  over `-j16` is the scheduling order.
- The prototype reads Kani's regular output; direct `-j` requires terse.
- `memory.peak` includes page cache, so the gate is conservative: after the
  first 30 s at most 11 harnesses ran at once.

## Attempts

| # | What | Result | Cause |
|---|---|---|---|
| 1 | Memory harness from symbolic array writes | 0.1 s, no memory | CBMC keeps the array symbolic; replaced by a multiply chain |
| 2 | 64-bit masked multiply harness | over 5 minutes at 10 bits | replaced by 32-bit |
| 3 | Part A pass 1 | verdicts stand; surviving-process column empty | `ps` given a bad pid list; rerun as pass 2 |
| 4 | Part A capped cases, default scope policy | exit 143, no summary in 11 of 15 | systemd `OOMPolicy=stop`; rerun with `continue`, both kept |
| 5 | Prototype calibration 1 | 0 CBMC runs observed | evidence path was relative; stopped |
| 6 | Prototype calibration 2 | stopped at 50 s | reconcile rule replaced |
| 7 | Prototype calibration 3, name order, no gate, 14.9 GiB per harness | 372 s, 18 no-claim | 17 from a reconcile rule missing Kani's pointer-check filter; 1 real, Kissat killed at the limit in `alloc_test`. Rule fixed from Kani's source and checked on 5 harnesses; limit raised |
| 8 | Truncation injector, first form | driver panic, exit 101, 25 runs | injector wrote a JSON layout Kani's line parser rejects; fixed and rerun, first fault arm discarded |
| 9 | Two remote commands | killed their own shell | `pkill -f` pattern matched it; no data lost |

## Evidence

`2026-11-27-kani-spike-integrity.tar.gz`, 7,324,689 bytes, sha256
`8cc29abbdde042c36ab9f3bcadbd96144480de741e3a255a0151d4fb0f455390`, kept
outside the repository. It holds every script including the supervisor and the
stand-in, the harness sources and the `s2n-quic-core` diff, the full Kani
output and exit status of every Part A run, the ten direct logs with memory
and process samples and kernel kill lines, the prototype's per-harness records
and Kani output for the calibration, five repetitions and fault arm, the
analysis output, and tool hashes.
