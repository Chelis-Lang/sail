# Sail 0.3.1 with D.I.V.E. 0.2.0 against standard TLC (S7)

Status: S7 passes for the release candidate. Sail `b00b6de9` (branch
`feat/dive-0.2-pin`, PR #807) runs D.I.V.E. `5bb88063`, the dive 0.2.0
candidate. Every timed job on every path agreed with the pinned TLC golden on
the holds/refuted class and the distinct-state count, 0 disagreements in all.
Every Sail refutation carried D.I.V.E.'s independently validated trace. Every
Sail path was faster than a fresh TLC process on every fixture. The slowest
case was MCLamportMutex at 1.3x. The procedure is S7 of D.I.V.E.'s
release-performance plan (`docs/developer/release-performance-plan.md`).
Tracking issue: [Chelis-Lang/dive#1418](https://github.com/Chelis-Lang/dive/issues/1418).
The 0.3.0 measurement is [sail-dive-vs-tlc.md](sail-dive-vs-tlc.md).

## Headline

- **Best case: 25 to 37 times faster than TLC per check on the smallest
  specifications, and 13.8 times faster over a 100-check agent loop.**
  DieHard takes 12.3 ms through `sail runtime execute` and 18.2 ms through a
  warm MCP call, against 453 ms for TLC. A session of 100 mixed small checks
  takes 5.14 s in one MCP session against 70.8 s for 100 TLC runs, with every
  job a fresh check. Two concurrent clients on one daemon complete 39.8 jobs
  per second, against 2.7 for two concurrent TLC processes (14.7x).
- **Typical case: 11.5x on a small liveness model, 5.5x at 14,365 states and
  1.3x at 724,274 states.** These are the `sail runtime execute` figures for
  MCLiveInternalMemory, MCDisruptor_MPMC_liveliness and MCLamportMutex. For
  large models, see the composed figure under [large models](#large-models),
  which is pending D1.
- **Repeated identical checks replay.** Sail 0.3.1 replays a completed check
  from its receipt store when the identical job has already completed in the
  same cache (#777). A warm MCP call that repeats a check returns in 9.6 to
  10.1 ms on every fixture. That is 1,274 times faster than TLC on
  MCLamportMutex. The 100-check loop as S7 builds it repeats each job 25
  times. It runs 56.7x faster through MCP and 121x faster through
  `sail runtime execute`. These figures are listed separately and are not the
  headline.
- **Memory.** Sail jobs peak at 25 to 141 MiB, against 161 MiB to 3.5 GiB
  for TLC. A warm `sail serve` holds 12 MiB.

Multiples are TLC time divided by Sail time within each repetition, and then
the median over five repetitions.

## What is compared

The comparators and fixtures are the ones in the
[0.3.0 page](sail-dive-vs-tlc.md#what-is-compared). TLC runs as
`java -XX:+UseParallelGC -jar tla2tools.jar -workers 1 -config <cfg>
<module>`, a fresh process per check with the default heap. The dive CLI runs
with Sail's settings. The Sail paths are `sail_dive::client::run` (embedded),
`sail runtime execute`, `sail runtime execute --socket` against a warm
`sail serve`, and one `runtime_execute` call in a warm `sail mcp` session.
Each job uses one worker, the exact store, seed and fingerprint zero,
`max_states` set to twice the golden's distinct states (at least 1,000),
`max_depth` 100,000 and `wall_time_ms` 300,000.

**Fresh checks and replays.** A Sail receipt is keyed on the request bytes
and `wall_time_ms`. In the 0.3.0 procedure, the warm serve and MCP cells
ran the timed fixture once before timing it, and the loop repeated four jobs
25 times. Both would now measure receipt replays. The headline rows (`rc`)
therefore give each Sail check job its own `wall_time_ms`, from 300,000 ms
counting down. No timed job is then a replay, and every path does the
check. The `rcreplay` rows run the jobs exactly as S7 builds them. Each row
records `cache_hit`. The `rc` rows have 0 hits. In the `rcreplay` rows, every
warm serve and MCP latency job is a hit, and 96 of the 100 loop jobs are hits.

## Source and environment

| Item | Value |
|---|---|
| Sail | `b00b6de97ac246d7e027ab4ae81b7b70f9e276f7` (`feat/dive-0.2-pin`). Installed `sail` SHA-256 `60ffe487…c3d6`, staged worker `fb99a6f4…d7bad` |
| D.I.V.E. | `5bb880635f46c7266e015a0d812f18e4411cdc6a`. The build was checked before timing: `Cargo.lock` and `cargo tree` show `dive-api v0.2.0` at that git revision, the worker binary embeds only dive checkouts at `5bb8806`, and `sail runtime capabilities` reports `backend_revision` 0.2.0. The CLI was built from the same revision, SHA-256 `f18c0f9a…a007a` |
| TLC | Pinned oracle `tlc-2026.10.06.014338`, `tla2tools.jar` SHA-256 `7beec0f0…ea78d`, OpenJDK 17.0.20.1 (the oracle's pinned JRE) |
| Host | A fresh c7i.8xlarge in us-east-1: Intel Xeon Platinum 8488C, 32 vCPUs (SMT on, frequency not controlled), 61 GiB, Ubuntu 24.04.5 LTS, kernel 7.0.0-1014-aws, THP `madvise`. No CPU binding. One measured process ran at a time with nothing else running, and the page cache was warmed |
| Corpus | tlaplus/Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`, goldens `tlc-2026.10.06.014338` |
| Repetitions | Five per cell. The path order rotated each repetition and was reversed on odd ones |

The harness is D.I.V.E.'s `scripts/perf/release/sail/`: `s7.py` (SHA-256
`c023ef4a…2590`), `sail_temporal.py`, `temporal_bench.rs` and
`s7-chain-0.2.sh`. The compact record is
[`data/2026-11-27-dive-0.2-s7.json`](data/2026-11-27-dive-0.2-s7.json). The
raw rows are in the dive evidence archive at
`release-perf/2026-11-27-rc02/s7/s7-raw.tar.gz`, SHA-256
`92c4b073fb42a556d6324a10a4c1f012aac7dd6e65f95de0b86fcbf8da131562`.

## Per-check latency

Fresh checks (`rc`). Each cell is the median wall time in milliseconds over
five repetitions.

| Fixture | TLC | dive CLI | Sail embedded | Sail execute | Sail serve | Sail MCP | TLC ÷ execute | TLC ÷ MCP |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| DieHard (invariant violation, 14 states) | 453 | 5.5 | 10.9 | 12.3 | 17.1 | 18.2 | 36.8x | 24.9x |
| EWD840 (liveness, 302) | 634 | 31.4 | 36.1 | 39.9 | 42.4 | 43.7 | 15.9x | 14.6x |
| MCRealTimeHourClock (liveness violation, 216) | 481 | 9.3 | 15.1 | 16.5 | 20.9 | 22.4 | 28.9x | 21.6x |
| MCLiveInternalMemory (liveness, 4,408) | 1,296 | 98.4 | 109 | 113 | 116 | 116 | 11.5x | 11.1x |
| MCDisruptor_MPMC_liveliness (liveness, 14,365) | 2,386 | 410 | 433 | 436 | 440 | 441 | 5.5x | 5.4x |
| MCLamportMutex (safety, 724,274) | 12,515 | 9,184 | 9,624 | 9,587 | 9,670 | 9,649 | 1.3x | 1.3x |

Against 0.3.0, Sail is faster on every fixture above 100 ms. The gain comes
from D.I.V.E. 0.2.0's Core: MCDisruptor_MPMC_liveliness went from 1,087 ms
to 436 ms and MCLamportMutex from 11,704 ms to 9,587 ms through
`sail runtime execute`. The millisecond fixtures are unchanged within
1 ms.

Peak RSS, from GNU `time` on the largest process of the tree, with
`sail serve` given by its `VmHWM`:

| Fixture | TLC | dive CLI | Sail execute | `sail serve` daemon |
|---|---:|---:|---:|---:|
| DieHard | 161 MiB | 18 MiB | 25 MiB | 12 MiB |
| EWD840 | 213 MiB | 68 MiB | 75 MiB | 12 MiB |
| MCRealTimeHourClock | 171 MiB | 24 MiB | 28 MiB | 12 MiB |
| MCLiveInternalMemory | 480 MiB | 28 MiB | 39 MiB | 12 MiB |
| MCDisruptor_MPMC_liveliness | 1,275 MiB | 36 MiB | 57 MiB | 12 MiB |
| MCLamportMutex | 3,579 MiB | 130 MiB | 141 MiB | 12 MiB |

**Repeated jobs (`rcreplay`).** When the timed job repeats the warm-up job,
both warm paths replay its receipt. The serve path takes 8.5 to 8.9 ms and
the MCP path 9.6 to 10.1 ms on every fixture, MCLamportMutex included. That
is 45x (DieHard) to 1,274x (MCLamportMutex) faster than TLC through MCP. The
execute cells use a fresh cache for each job, so they are fresh checks and
match the `rc` row.

**First job after installation.** The first job on a newly installed worker
verifies and copies the worker. DieHard through `sail runtime execute` takes
238 ms (236–239) for that first job and 12.2 ms (12.0–12.4) for the next,
five times each. 0.3.0 took 207 ms for the first job; that regression is
[#814](https://github.com/Chelis-Lang/sail/issues/814).

## Agent loop

100 check jobs run one after another, cycling over DieHard, EWD840,
MCRealTimeHourClock and MCLiveInternalMemory. TLC is 100 separate runs. The
MCP figure is one session, including its start-up and the daemon autostart.

| Path | Fresh checks (`rc`), session total (min–max) | TLC ÷ path | As built (`rcreplay`), 96 replays | TLC ÷ path |
|---|---:|---:|---:|---:|
| TLC | 70.81 s (70.57–71.26) | 1x | 67.56 s (67.20–68.79) | 1x |
| dive CLI | 3.63 s | 19.5x | 3.64 s | 18.5x |
| Sail execute | 4.57 s (4.55–4.58) | 15.5x | 0.56 s | 121.5x |
| Sail serve | 5.27 s (5.24–5.29) | 13.4x | 1.34 s | 50.7x |
| Sail MCP | 5.14 s (5.09–5.17) | 13.8x | 1.19 s | 56.7x |

For fresh checks, the per-job median is 26.9 ms through execute and 42.3 ms
through MCP, and the p90 is 110 ms and 117 ms. All 5,000 jobs in both
variants agreed with their goldens.

**Throughput at the admission limit.** The same 100 fresh jobs came from two
concurrent clients through one warm daemon, which allows two active
executions. They completed at 39.8 jobs/s (39.5–39.8), against 2.7 for two
concurrent TLC processes (14.7x), and the daemon's `VmHWM` was 13 MiB. When
the jobs are repeated as S7 builds them, the rate is 181 jobs/s. With eight
concurrent clients, the service refuses every job beyond its two active
executions instead of queueing it, so that case is not a throughput figure.
A spot check of eight simultaneous clients gave 2 completed and 6 refused with
`SAIL6201`. Every job that ran agreed with its golden.

## Large models

S7 does not rerun the engine comparison. The composed figure is D1's Core/TLC
ratio for the model multiplied by Sail's measured factor. The factor is the
paired median of the whole isolated worker (`temporal_bench`'s `embedded_ms`,
exact store) over the dive CLI with Sail's settings, on this host:

| Model | dive CLI | Sail worker | Sail factor |
|---|---:|---:|---:|
| SlushSmall | 2,862 ms | 3,063 ms | 1.07 |
| MCDisruptor_MPMC | 834 ms | 897 ms | 1.08 |
| CoffeeCan1000Beans | 6,593 ms | 7,288 ms | 1.11 |
| ElevatorLivenessMedium | 124 ms | 138 ms | 1.12 |
| MCLamportMutex | 9,143 ms | 9,559 ms | 1.05 |

**D1's ratios for `5bb88063` were not yet recorded when S7 ran, so the
composed figure is pending.** It is D1's ratio for each model multiplied by
the factor above. On this host, the direct MCLamportMutex comparison is
1.3x faster than TLC through every Sail path. In 0.3.0 the worker was within
2% of the CLI on MCLamportMutex; it is now 4% to 5% slower (#814).

## Sail-only capabilities

Standard TLC has no equivalent for these, so they are not given as
multiples.

- **Bounded exhaustive check with a structured stop.** ElevatorSafetyLarge
  ran with the exact store and `max_states` 1,000,000. It returns `Stopped`
  with no claim after exactly 1,000,000 distinct states, in 5.92 s
  (5.89–5.93) at a 127 MiB peak (0.3.0: 7.51 s, 345 MiB).
- **Independent trace replay.** Validating a returned ITF counterexample as a
  job of its own takes 12.4 ms for DieHard, 92.6 ms for SlidingPuzzles and
  12.0 ms for the MCRealTimeHourClock lasso, and all three are `valid`.
