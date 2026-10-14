# Performance

What Sail does and does not speed up, compared with plain
`cargo verus verify`. Each claim below comes from a run you can repeat with
the command given under it.

Sail caches the Z3 solver's answers. Rust compilation and Verus's own
frontend still run every time, so Sail helps only where the solver is a large
share of the work. On a small project or a first run, expect about the same
time as plain Verus.

## Claims

| # | Claim | Linux (EC2 c7i.4xlarge, idle) | macOS (Apple M3 developer laptop, loaded: load average 4–19) |
|---|---|---|---|
| 1 | Verdicts, errors and exit codes are identical to plain Verus | 0 differences in the conformance tiers and in Verus's own 4,695-test suite | 0 differences in the fast PR subset |
| 2 | Edit loops on a solver-heavy project are faster | vest: 1.3–1.5× (`sail verify`, about 24 s against 34–38 s) | vest: 1.8–2.3× (about 23–27 s against 43–54 s, loaded machine) |
| 3 | Unchanged re-verification gives agents a complete report in about a second; it does not beat Cargo's no-op | `--incremental` 0.29 s (demo), 1.3 s (vest); MCP `verify` 0.52 s, 1.75 s; plain 0.10 s, 0.20 s | `--incremental` 0.39 s, 1.86 s; MCP 0.57 s, 2.22 s; plain 0.18 s, 0.60 s |
| 4 | Small projects: about the same as plain Verus, not faster | demo edits 0.75 s against 0.76 s | 0.51 s against 0.52 s |
| 5 | Cold first run: about the same as plain Verus | demo 30.6 s against 30.4 s; vest 67.5 s against 66.6 s | demo 26.7 s against 24.0 s; vest 82.9 s against 72.6 s (loaded machine) |
| 6 | Repair loops with the engine are faster than verifying the function, then the crate | HumanEval 2.14×, Vest 1.82× (c7i.8xlarge) | not measured |
| 7 | Several agents on one daemon do not wait for each other | 4 vest agents finish 2.2× sooner (47 s against 105 s) | 4 demo agents: 1.3× sooner (loaded machine) |

Ratios are plain Verus time divided by Sail time (for claim 7, one job at a
time divided by concurrent jobs), from medians of three rounds.

## Where Sail does not help

- **Small projects.** When verification takes under a second, there is little
  solver work to save. `sail verify` matches plain Verus; `--incremental` adds
  about 0.25 s per run for its report and receipts.
- **Cold runs.** The first verification of a new project or worktree compiles
  dependencies and verifies vstd, all of it new to the cache. Sail matches
  plain Verus within the spread on Linux; on the busy Mac it was 11–14%
  slower. A warm [team cache](http-shared-cache.md) does not change this
  either: the compile and frontend dominate.
- **The Rust and Verus frontend.** On vest, an edit with every unchanged solver
  answer cached still takes about 24 s on Linux. That is Cargo, rustc and
  Verus's frontend, which Sail does not change. It is the floor for every mode.
- **Unchanged code from the CLI.** Plain Cargo sees nothing changed and exits
  in 0.1–0.6 s. `--incremental` takes 0.3–1.9 s, because it checks its inputs
  and builds a complete report. Its value is the report, not the time.

## Setup

| | Linux | macOS |
|---|---|---|
| Machine | EC2 c7i.4xlarge (16 vCPU, 30 GiB), us-east-1, Ubuntu 24.04, `scripts/perf-host.sh up l36`, no other work on the host | Apple M3, 8 cores, 24 GiB, macOS 26, a loaded developer laptop shared with other work: load average 4–8 during the demo runs, 7–19 during the vest runs and 13–18 during the agents run |
| Sail | `main` at `401b600`, release build of `sail-cli` and `sail-shim` | same |
| Verus | 0.2026.10.04.426d8b0 (`cargo xtask fetch-verus`), Z3 4.16.0 | same |
| Date | 2026-10-14 | 2026-10-14 |

Raw logs are kept outside the repository, in `l36-linux-perf.tgz` (sha256
`e0e5c92f1570021cdce855865dc0e0be58defc28a77d9735ea9ba5ed595a1dab`) and
`l36-mac-perf.tgz` (sha256
`5b194e7586ed0ebf5256350931c0cfa085bc4a6e621bdf53f0eee4ee0227227c`). They
hold every command's log, `results.tsv`, `header.txt` and the MCP replies.

## 1. Identical verdicts and errors

Sail's correctness contract is that, in default mode, every per-function
verdict and diagnostic matches plain Verus with its pinned Z3. This is
checked, not measured here:

- The fast and standard corpus tiers (159 units, 12,726 functions) and the
  deep tier, in pass-through, cache fill, cache warm and the engine arm: 0
  Different ([M7 validation, gate 6](validation/m7.md#gate-6-default-and-cache-conformance),
  [deep tier](validation/deep-tier.md)).
- Verus's own `rust_verify_test` suite, 4,695 tests, plain against Sail in
  every mode: 0 Different ([deep tier](validation/deep-tier.md)).
- On macOS, the fast PR subset (33 units) in every arm: 0 Different
  ([macOS engine](validation/macos-engine.md)); the longer tiers run on Linux.
- Every run on this page also compares exit codes: across all modes and
  rounds, only the failing edit failed.

Reproduce with `ci/local.sh conformance fast`, `ci/local.sh conformance
standard`, `ci/local.sh conformance deep` and `ci/local.sh deep-rvt`
([AGENTS.md](../AGENTS.md#gates)).

## 2. Edit loops on a solver-heavy project

vest (`secure-foundations/vest` at `11af01dd`, `-p vest_lib`, 3,237
functions). Each round starts from a fresh copy and an empty Sail cache, runs
a cold verification, then three edits in sequence, taken from the frozen
repair catalogue (`corpus/repair-edits.json`):

- **edit**: an extra assertion in `lemma_from_base128_push`;
- **fail**: a wrong postcondition in `lemma_pow128_succ` (`* 128 + 1`);
- **fix**: the postcondition restored and the lemma's two proof steps
  swapped, so the fixed function's solver query is new, not a cache replay.

Wall seconds, median (min–max) of three rounds:

| vest | Mode | cold | edit | fail | fix |
|---|---|---:|---:|---:|---:|
| Linux | `cargo verus verify` | 66.6 (66.5–67.1) | 37.3 (36.9–37.4) | 33.8 (33.2–33.8) | 37.8 (37.1–39.6) |
| Linux | `sail verify` | 67.5 (67.1–70.7) | 24.2 (24.0–24.9) | 25.1 (24.2–25.2) | 24.7 (23.4–24.9) |
| Linux | `sail verify --incremental` | 71.1 (69.7–73.3) | 26.9 (26.9–27.7) | 26.4 (26.1–26.8) | 26.8 (26.5–26.8) |
| macOS laptop (loaded) | `cargo verus verify` | 72.6 (63.6–76.0) | 48.1 (41.1–106.8) | 43.2 (40.9–62.4) | 54.4 (44.5–59.3) |
| macOS laptop (loaded) | `sail verify` | 82.9 (81.7–133.8) | 27.4 (26.2–28.9) | 22.8 (22.4–25.5) | 23.6 (20.7–24.7) |
| macOS laptop (loaded) | `sail verify --incremental` | 79.5 (77.8–128.7) | 29.3 (27.0–65.2) | 27.1 (27.0–34.1) | 30.3 (30.1–33.7) |

On Linux, `sail verify` saves 9–13 s per edit (1.3–1.5×) and `--incremental`
7–11 s (1.3–1.4×). The macOS ratios are larger (1.8–2.3×) but come from a
loaded machine, where the slower plain runs vary most. Either way, about
24–30 s per edit remains: that is the frontend floor described above.

Reproduce (each takes about 30 minutes on Linux):

```sh
scripts/perf-loops.sh vest 3
```

## 3. Unchanged re-verification and agent re-queries

After the edits above, each mode verifies the unchanged code again (`warm2`),
and one `sail mcp` session calls `verify` twice on it (`mcp1`, `mcp2`; the
second call is shown). Wall seconds, median of three rounds:

| | Linux demo | Linux vest | macOS laptop demo (loaded) | macOS laptop vest (loaded) |
|---|---:|---:|---:|---:|
| `cargo verus verify` | 0.10 | 0.20 | 0.18 | 0.60 |
| `sail verify` | 0.10 | 0.21 | 0.19 | 0.53 |
| `sail verify --incremental` | 0.29 | 1.27 | 0.39 | 1.86 |
| MCP `verify` | 0.52 | 1.75 | 0.57 | 2.22 |

Plain Cargo is fastest here: it sees nothing changed and prints nothing new.
`--incremental` and MCP `verify` check that the inputs are unchanged and
return the complete report again (every function's verdict, marked Current,
naming the run that established it). That is what an agent needs after a
restart or a context reset, and it costs one to two seconds rather than a
re-verification. It is not faster than Cargo's no-op.

Reproduce: the same runs as claims 2 and 4 (`scripts/perf-loops.sh vest 3`
and `scripts/perf-loops.sh demo 3`).

## 4. Small projects

`cargo verus new --lib demo` plus four small functions (a recursive spec
with a lemma, `max`, a loop search and a counting loop), with the same three
edit phases: add a function, break `max`'s postcondition, then restore it
with a rewritten body. Wall seconds, median (min–max) of three rounds:

| demo | Mode | cold | edit | fail | fix |
|---|---|---:|---:|---:|---:|
| Linux | `cargo verus verify` | 30.4 (30.1–30.7) | 0.76 (0.76–0.76) | 0.55 (0.55–0.56) | 0.77 (0.77–0.78) |
| Linux | `sail verify` | 30.6 (30.4–31.1) | 0.75 (0.74–0.78) | 0.57 (0.56–0.58) | 0.76 (0.75–0.78) |
| Linux | `sail verify --incremental` | 31.4 (31.3–31.5) | 1.01 (1.00–1.02) | 0.79 (0.78–0.80) | 0.99 (0.98–0.99) |
| macOS laptop (loaded) | `cargo verus verify` | 24.0 (21.6–24.2) | 0.52 (0.50–0.53) | 0.47 (0.45–0.48) | 0.53 (0.50–0.54) |
| macOS laptop (loaded) | `sail verify` | 26.7 (24.0–26.8) | 0.51 (0.51–0.56) | 0.47 (0.47–0.49) | 0.52 (0.51–0.54) |
| macOS laptop (loaded) | `sail verify --incremental` | 26.4 (25.2–26.6) | 0.82 (0.81–0.90) | 0.72 (0.71–0.74) | 0.73 (0.73–0.73) |

`sail verify` equals plain Verus. `--incremental` costs about 0.25 s more per
run. Reproduce with `scripts/perf-loops.sh demo 3` (about 5 minutes).

## 5. Cold first run

The `cold` columns above: a new copy, an empty Sail cache, and every
dependency compiled and vstd verified. Sail is within the spread of plain
Verus on Linux (demo 30.6 s against 30.4 s, vest 67.5 s against 66.6 s), with
`--incremental` 1–5 s slower. On the loaded Mac, Sail's cold runs were
11–14% slower. Every new project or worktree pays this once. The
managed path then reuses its compiled dependencies and receipts
([managed runs](managed-cold-start.md)).

## 6. Repair loops with the engine

`sail repair-check` checks a candidate repair of one function and then the
whole crate in a single run of the optional engine. The comparison, called
M6 in the [acceptance guide](bench/m7-repair-acceptance.md), is what an agent
does without the engine: `sail verify --incremental` restricted to the
function, then `sail verify --incremental` on the whole crate, two stock
Verus runs.

Over the frozen catalogue of five cycles per project (a body edit, a
specification change, a refactor, a failing attempt and its repair), the
median whole loop on an EC2 c7i.8xlarge, us-east-1:

| Catalogue | Without the engine (M6) | `repair-check` (M7) | Speedup |
|---|---:|---:|---:|
| HumanEval (`human-eval-verus`) | 64.7 s | 30.3 s | 2.14× |
| Vest (`vest_lib`) | 246.6 s | 135.1 s | 1.82× |

Every sample's full outcome matched the M6 arm. Details and spread are in
[benchmarks](benchmarks.md) (`repair-acceptance-l22-final`) and
[M7 validation](validation/m7.md#gate-7-frozen-repair-performance). This was
not measured on macOS.

Reproduce with `sail-bench repair prepare` and `sail-bench repair run`, as
given in the [acceptance guide](bench/m7-repair-acceptance.md#running) (about
two hours, and an M6 build from `rescue/m6-baseline`).

## 7. Several agents

Four agents, each with its own copy of the project (separate Cargo
workspaces), share one Sail daemon. In each round every agent makes an edit
and submits `sail verify --background` at the same moment. The time is from
submission until the last job finishes, compared with the daemon running one
job at a time (`SAIL_JOB_WORKERS=1`, how it worked before concurrent jobs).
Median (min–max) of three rounds:

| | Agents | One job at a time | Default workers | Speedup |
|---|---|---:|---:|---:|
| Linux (4 workers) | 4 × vest `-p vest_lib` | 104.5 s (104.4–104.7) | 47.1 s (46.5–47.2) | 2.2× |
| macOS laptop, loaded (2 workers) | 4 × demo | 6.7 s (6.4–6.8) | 5.2 s (4.0–5.8) | 1.3× |

The edit is a comment appended to a source file, so each job re-runs Cargo
and Verus on its crate; one such vest job alone takes about 26 s.

An earlier Linux measurement with HumanEval copies, whose jobs take about
9 s, found 2.7× (13.8 s instead of 37.7 s; L15 rescue lane report, tag
`archive/rescue-m7-98f6292`). Each job runs somewhat slower when several
share the machine, because Verus and Z3 already use several cores. Jobs in
the same workspace still run one at a time
([concurrent jobs](workspace-verification.md#concurrent-jobs)). The demo
jobs on macOS take about a second each, so that run mostly measures the
daemon's overhead, on a loaded machine with the default of two workers.

Reproduce with `scripts/perf-agents.sh vest 4 3` (about 15 minutes on Linux)
or `scripts/perf-agents.sh demo 4 3`.

## Running the scripts

`scripts/perf-loops.sh` and `scripts/perf-agents.sh` need `sail` and
`sail-z3`, `cargo verus` and `verus` on `PATH`, plus `git`, `perl` and `awk`.
They print the tables above and keep every log in the output directory
(`SAIL_PERF_OUT`, default `~/sail-perf/...`). Set `SAIL_PERF_MACHINE` to
describe the machine in the header; the header also records the CPU, core
count and load average at the start and end. vest is cloned from GitHub
unless `SAIL_PERF_VEST_SRC` names an existing clone.

Reference numbers come from a native Linux host with no other load
([AGENTS.md](../AGENTS.md)). Run the scripts directly on the machine, not in
an emulated container. The exact commands, from a Sail checkout at the commit
under test, after installing Rust with rustup (and, on Linux, `unzip`):

```sh
cargo build --release -p sail-cli -p sail-shim
cargo xtask fetch-verus      # prints the PATH line for the Verus release
export PATH="$PWD/target/release:$HOME/.cache/sail/verus/0.2026.10.04.426d8b0/verus-x86-linux:$PATH"
#   on macOS: .../verus-arm64-macos instead of .../verus-x86-linux
export SAIL_PERF_MACHINE="EC2 c7i.4xlarge (16 vCPU, 30 GiB), us-east-1, idle"
#   describe the actual machine and whether anything else is running
SAIL_PERF_OUT=$HOME/sp/demo  scripts/perf-loops.sh demo 3     # claims 3, 4, 5
SAIL_PERF_OUT=$HOME/sp/vest  scripts/perf-loops.sh vest 3     # claims 2, 3, 5
SAIL_PERF_OUT=$HOME/sp/agents scripts/perf-agents.sh vest 4 3  # claim 7
```

The macOS rows above came from the same commands with
`scripts/perf-agents.sh demo 4 3` for claim 7 (vest was too slow on the
loaded laptop), `SAIL_PERF_VEST_SRC` pointing at an existing vest clone, and
`SAIL_PERF_MACHINE="Apple M3 (8 cores, 24 GiB), macOS 26, development machine
under other load"`. Keep `SAIL_PERF_OUT` short: the daemon's socket lives
under it, and Unix socket paths are limited to about 100 bytes. Each script
prints its table on stdout and keeps it as `table.md` in the output directory;
`scripts/perf-agents.sh report OUT_DIR 4` prints an earlier run's table again.

The older workload numbers in [benchmarks](benchmarks.md) (generated by
`sail-bench`) measure the solver shim on corpus units in isolation. This page
measures what a user runs.
