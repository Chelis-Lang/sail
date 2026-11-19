# Sail+dive against standard TLC (S7)

Status: measured on native Linux, before and after the
[#765](https://github.com/Chelis-Lang/sail/issues/765) fix. Every timed job
in every path agreed with the pinned TLC golden on the holds/refuted class
and the distinct-state count, and every Sail refutation carried D.I.V.E.'s
independent trace validation. The figures are directional measurements of
one host; they are not a performance gate on their own. The procedure is S7
of D.I.V.E.'s release-performance plan
(`docs/developer/release-performance-plan.md`). Tracking issue:
[#764](https://github.com/Chelis-Lang/sail/issues/764).

## What is compared

The standard way to check a TLA+ specification, by hand or from an agent, is
one TLC process per check: `java -jar tla2tools.jar` starts a JVM, parses the
specification with SANY and checks it. The VS Code extension and the Toolbox
also start TLC per check; there is no warm TLC service in the standard
tooling. Sail runs D.I.V.E. in its isolated worker, and a long-running
`sail serve` or `sail mcp` session can submit many jobs.

| Path | What is timed |
|---|---|
| TLC | `java -XX:+UseParallelGC -jar tla2tools.jar -workers 1 -config <cfg> <module>` in a fresh scratch copy of the model, process start to exit. TLC's documented invocation with its recommended collector, default heap. |
| dive CLI | `dive check --store exact --workers 1 --seed 0 --fp 0` with the job's `--max-states`/`--max-depth`. |
| Sail embedded | `sail_dive::client::run` from the S1 harness (the isolated worker, no `sail` process). |
| Sail execute | `sail runtime execute --file job.json`, a fresh `sail` process per job. |
| Sail serve | `sail runtime execute --file job.json --socket <s>` against a `sail serve` that has already run one job (the client process start is included). |
| Sail MCP | One `runtime_execute` call in a `sail mcp` session whose daemon has already run one job, from a JSON-RPC driver; no model or agent time is included. |

All paths use one worker. Sail's checking policy is fixed (exact store, seed
and fingerprint zero, explicit state and depth bounds: `max_states` twice the
golden's distinct states and at least 1,000, `max_depth` 100,000).

## Headline

- **Best case: 27–39× faster than TLC per check, about 13× over an agent
  loop.** On the smallest specifications (DieHard, 14 states; the
  MCRealTimeHourClock lasso, 216 states) a warm MCP call returns in 18–25 ms
  against TLC's 490–570 ms, and `sail runtime execute` in 13–19 ms. A session
  of 100 mixed small checks through one warm MCP session takes 6.7 s against
  84.5 s for 100 TLC invocations (12.6×), and two concurrent clients complete
  29.7 jobs per second against 2.2 for two concurrent TLC processes (13.3×).
  Conditions: small specifications, one host, warm `sail serve`/MCP session
  or an installed worker whose verified copy is already cached, every result
  checked against the golden.
- **Typical case: 2–17× for small and medium models, about 1.1× at
  MCLamportMutex size.** EWD840 (302 states) is 17× faster through `sail
  runtime execute`, MCLiveInternalMemory (4,408 states) 9.3×,
  MCDisruptor_MPMC_liveliness (14,365 states) 2.4×, and MCLamportMutex
  (724,274 states, 12.8 s in TLC) 1.1×. As a specification grows, start-up
  matters less and the comparison becomes D.I.V.E.'s engine against TLC's
  (see [large models](#large-models)).
- **Memory.** A Sail job peaks at 21–270 MiB against TLC's 164 MiB–3.5 GiB
  on the same models, and a warm `sail serve` holds 12 MiB.
- **Before the #765 fix Sail lost most of this.** Every job hashed and copied
  the 76 MB worker, and the socket and MCP paths hashed it again for the
  capability check, so a warm MCP call on DieHard took 376 ms (1.2× faster than
  TLC) and the 100-job loop 42.7 s (1.8×).

Multiples are TLC time divided by Sail time within each repetition (each
repetition ran every path), then the median over five repetitions. The
dive CLI is shown for reference: it is the floor that Sail's isolation
(verified worker, fresh process per job, independent evidence checks) sits on.

## Source and environment

| Item | Value |
|---|---|
| Sail before | `6592db0c292a1c5054ac681e4ac13b5222272385` (PR #763 head), worker SHA-256 `9849a0b8…c936e` |
| Sail after | `ce94df746af9332df3d4ac5a8a3693ff6c10cd39` (PR #768), installed `sail` SHA-256 `5ffa6b60…a915c`, worker `861cd558…35cfd4d` (beside it: `be3a0995`, cache only; `aca50ea3`, cache and mimalloc) |
| D.I.V.E. | backend and CLI at `3d5923a1423e67d2f64e80655f7ea53e360a0ee7`; CLI SHA-256 `fa769f14…29133` |
| TLC | pinned oracle `tlc-2026.10.06.014338`, `tla2tools.jar` SHA-256 `7beec0f0…ea78d`, OpenJDK 17.0.20.1 (the oracle's pinned JRE) |
| Host | fresh c7i.8xlarge, us-east-1; Intel Xeon Platinum 8488C, 32 vCPUs (SMT on), 61 GiB; Ubuntu 24.04.5 LTS, kernel 7.0.0-1017-aws, THP `madvise`; gp3 16000 IOPS. No CPU binding; one measured process at a time, nothing else running, page cache warmed. |
| Corpus | tlaplus/Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`, goldens `tlc-2026.10.06.014338` |
| Repetitions | five per cell, path order rotated each repetition and reversed on odd ones |

The harness is the S1 `temporal_bench` example and Python drivers kept with
D.I.V.E.'s release-performance runbook (SHA-256 `3f2c46b6…5312200a` for the
S7 driver). Raw rows are in the dive evidence archive at
`release-perf/2026-11-19/s7/s7-raw.tar.gz`, SHA-256
`e0de442aa30e7205c4934b0d97829300c9e5560acd3fc4b898a211f2b62e3c90`; the
compact record is `release-perf/2026-11-19/s7/sail-dive-vs-tlc-s7.json`,
SHA-256 `efe6a1397840fef622dbf6ef7a14fd80ec040fedb9bd16a82fcc34a093b827e7`.

## Per-check latency

Median wall time in milliseconds over five repetitions.

After the fix (`ce94df74`):

| Fixture | TLC | dive CLI | Sail embedded | Sail execute | Sail serve | Sail MCP | TLC ÷ execute | TLC ÷ MCP |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| DieHard (invariant violation, 14 states) | 491 | 6.4 | 11.0 | 12.8 | 17.6 | 18.4 | 38.9× | 26.5× |
| EWD840 (liveness, 302) | 684 | 33.5 | 36.6 | 40.3 | 45.2 | 46.1 | 17.0× | 14.8× |
| MCRealTimeHourClock (liveness violation, 216) | 568 | 10.4 | 15.3 | 18.9 | 23.3 | 24.8 | 29.6× | 22.8× |
| MCLiveInternalMemory (liveness, 4,408) | 1,500 | 144 | 156 | 160 | 164 | 164 | 9.3× | 9.1× |
| MCDisruptor_MPMC_liveliness (liveness, 14,365) | 2,648 | 1,032 | 1,083 | 1,087 | 1,099 | 1,101 | 2.4× | 2.4× |
| MCLamportMutex (safety, 724,274) | 12,765 | 11,477 | 11,674 | 11,704 | 11,699 | 11,730 | 1.1× | 1.1× |

Before the fix (`6592db0c`):

| Fixture | TLC | dive CLI | Sail embedded | Sail execute | Sail serve | Sail MCP | TLC ÷ execute | TLC ÷ MCP |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| DieHard | 459 | 6.2 | 115 | 199 | 292 | 376 | 2.3× | 1.2× |
| EWD840 | 669 | 33.5 | 155 | 236 | 329 | 414 | 2.8× | 1.6× |
| MCRealTimeHourClock | 521 | 9.5 | 121 | 204 | 297 | 384 | 2.6× | 1.4× |
| MCLiveInternalMemory | 1,341 | 136 | 252 | 339 | 429 | 516 | 4.0× | 2.6× |
| MCDisruptor_MPMC_liveliness | 2,467 | 989 | 1,147 | 1,234 | 1,324 | 1,411 | 2.0× | 1.7× |
| MCLamportMutex | 12,760 | 11,448 | 13,180 | 13,397 | 13,376 | 13,545 | 1.0× | 0.9× |

Spreads were narrow: after the fix every cell stayed within 6% of its median
on the models above 100 ms. On the millisecond fixtures single repetitions
strayed by up to 2–3 ms (14% of DieHard's 12.8 ms execute, 28% of
MCRealTimeHourClock's 10.4 ms CLI run). The first embedded DieHard
repetition took 120 ms because no reusable copy existed yet. Peak RSS after the fix
(GNU `time`, largest process of the tree; `sail serve` by its `VmHWM`):

| Fixture | TLC | dive CLI | Sail execute | `sail serve` daemon |
|---|---:|---:|---:|---:|
| DieHard | 164 MiB | 20 MiB | 21 MiB | 12 MiB |
| EWD840 | 215 MiB | 72 MiB | 75 MiB | 11 MiB |
| MCRealTimeHourClock | 171 MiB | 25 MiB | 26 MiB | 11 MiB |
| MCLiveInternalMemory | 487 MiB | 36 MiB | 39 MiB | 12 MiB |
| MCDisruptor_MPMC_liveliness | 1,269 MiB | 51 MiB | 49 MiB | 12 MiB |
| MCLamportMutex | 3,563 MiB | 266 MiB | 270 MiB | 11 MiB |

Before the fix, `sail runtime execute` peaked at 81 MiB on every small
fixture (it read the whole worker to hash it) and the daemon at 85 MiB.
TLC's figure follows the JVM's default heap sizing on a 61 GiB host.

**First job after installation.** The first job on a newly installed worker
verifies and copies it: 207 ms (206–209) for DieHard through `sail runtime
execute`, then 12.3 ms (12.1–12.4) for the next job, five times each.

## Agent loop

100 check jobs cycling over DieHard, EWD840, MCRealTimeHourClock and
MCLiveInternalMemory, run one after another. TLC is 100 invocations; MCP is
one session, its start-up and daemon autostart included.

| Path | Session total, after (min–max) | TLC ÷ path, after | Session total, before | TLC ÷ path, before |
|---|---:|---:|---:|---:|
| TLC | 84.5 s (83.6–84.9) | 1× | 74.8 s (74.7–75.1) | 1× |
| dive CLI | 5.14 s | 16.5× | 4.74 s | 15.8× |
| Sail execute | 6.14 s | 13.8× | 25.0 s | 3.0× |
| Sail serve | 6.83 s | 12.4× | 34.4 s | 2.2× |
| Sail MCP | 6.70 s (6.52–6.72) | 12.6× | 42.7 s | 1.8× |

All 5,000 jobs agreed with their goldens. The before and after loops ran
40 minutes apart and TLC's total moved by 13% between them on this shared
virtual host, which is why multiples are taken within a repetition.

**Throughput at the admission limit.** The same 100 jobs from two concurrent
clients through one warm daemon (two active executions) against two
concurrent TLC processes: 29.7 jobs/s (29.6–29.7) against 2.2 (13.3×); before
the fix 5.2 jobs/s (2.4×). With eight concurrent clients the service refuses
jobs beyond its two active executions with `SAIL6201 busy` rather than
queueing them, so that case is not a throughput figure; every job that ran
agreed with its golden.

## Large models

S7 does not rerun the engine comparison. In D.I.V.E.'s fresh R2 run for
0.1.0 (c7i.metal-24xl, SMT and turbo off, 2.4 GHz, fingerprint store) Core
took 0.19 of TLC's time on the geometric mean over 169 agreeing models; on
large models Sail admits it took 0.66 of TLC's time for MCLamportMutex, 0.44
for ElevatorLivenessMedium and 0.26 for CoffeeCan1000Beans. After the fix the
Sail worker is within 2% of the dive CLI with the same settings on
MCLamportMutex (it was 17% slower), so those ratios carry over to Sail up to
the exact-store cost. This is a composed figure, not a Sail measurement. On
this turbo-enabled c7i.8xlarge the direct MCLamportMutex comparison above is
1.1×, smaller than D1's 1.5×, so the host matters for medium models.

## Sail-only capabilities

Not multiples: TLC has no equivalent.

- **Bounded exhaustive check with a structured stop.** ElevatorSafetyLarge
  with the exact store and `max_states` 1,000,000 returns `Stopped` with no
  claim after exactly 1,000,000 distinct states, in 7.51 s (7.42–7.54) at
  345 MiB peak. TLC has no state ceiling for model checking.
- **Independent trace replay.** Validating the returned ITF counterexample
  as its own job takes 13.1 ms for DieHard (7 steps), 114 ms for
  SlidingPuzzles (117 steps) and 15.4 ms for the MCRealTimeHourClock lasso,
  all `valid`. Refutations in the check jobs above already include this
  replay.

## The #765 fix

| Variant | Source | DieHard MCP | DieHard execute | MCLamportMutex execute ÷ dive CLI |
|---|---|---:|---:|---:|
| before | `6592db0c` | 376 ms | 199 ms | 1.17 |
| verified copy reused | `be3a0995` | 24.6 ms | 12.0 ms | 1.15 |
| plus mimalloc in the worker | `aca50ea3` | 29.0 ms | 12.8 ms | 1.04 |
| plus immediate accept wake (PR #768) | `ce94df74` | 18.4 ms | 12.8 ms | 1.02 |
| experiment: plus fat LTO for the whole release | not proposed | 20.4 ms | 14.6 ms | 1.00 |

Reusing the verified worker copy ([ADR 0173](../decisions/0173-verified-worker-image-reuse.md))
removes the per-job hash and copy and the capability check's second hash.
mimalloc closes most of the in-worker gap (EWD840's worker time drops from
1.51× to 1.22× of the CLI, MCLamportMutex from 1.15× to 1.04×) and
`cargo deny` passes. Waking the service for a new connection instead of
polling every 5 ms takes 5–10 ms off each socket and MCP call. Fat LTO gained
at most 2% more and would slow every release build, so it is not proposed.
