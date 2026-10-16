# Performance

What Sail does and does not speed up, compared with plain
`cargo verus verify`. Each claim below comes from a run you can repeat with
the command given under it.

Sail caches the Z3 solver's answers. Rust compilation and Verus's own
frontend still run every time, so Sail helps only where the solver is a large
share of the work. On a small project or a first run, expect about the same
time as plain Verus.

## Claims

| # | Claim | Linux (EC2 c7i.4xlarge, idle) | macOS (EC2 mac-m4.metal (Apple M4), idle) |
|---|---|---|---|
| 1 | Verdicts, errors and exit codes are identical to plain Verus | 0 differences in the conformance tiers and in Verus's own 4,695-test suite | 0 differences in the fast PR subset |
| 2 | Edit loops on a solver-heavy project are faster | vest catalogue edit: 1.35–1.5× (`sail verify`, about 29 s against 39–44 s). Real projects: 1.2–2.3× for an edit confined to one module, about 1.0× for a change to a widely used spec function (see the [real-project report](reports/2026-10-15-real-project-performance.md)) | vest: 1.2–1.3× (about 16–17 s against 19–21 s) |
| 3 | Unchanged re-verification gives agents a complete report in about a second; it does not beat Cargo's no-op | `sail verify` 0.22 s (demo), 0.59 s (vest); MCP `verify` 0.35 s, 1.02 s; plain 0.12 s, 0.23 s | `sail verify` 0.36 s, 1.37 s; MCP 0.57 s, 1.75 s; plain 0.18 s, 0.24 s |
| 4 | Small projects: about 0.2 s slower than plain Verus per run | demo edits 1.08 s against 0.85 s | 0.75 s against 0.44 s |
| 5 | Cold first run: a few seconds slower than plain Verus | demo 36.6 s against 35.0 s; vest 80.5 s against 77.9 s | 15–18% slower: demo 18.7 s against 16.3 s; vest 43.5 s against 36.8 s |
| 6 | Repair loops with the engine are faster than verifying the function, then the crate | HumanEval 2.14×, Vest 1.82× (c7i.8xlarge) | not measured |
| 7 | Several agents on one daemon do not wait for each other | 4 vest agents finish 2.2× sooner (47 s against 105 s) | 4 vest agents (2 workers) finish 1.3× sooner (51 s against 67 s) |
| 8 | A new project or worktree's first run skips vstd once the machine has verified it | demo 1.4 s against plain 39.0 s; vest worktree 33.1 s against plain 89.1 s; first on the machine 40.1 s and 89.8 s | demo 3.2 s against plain 67 s (Apple M3 under load 15–23; not measured idle) |

Ratios are plain Verus time divided by Sail time (for claim 7, one job at a
time divided by concurrent jobs), from medians of three rounds.

## Where Sail does not help

- **Small projects.** When verification takes under a second, there is little
  solver work to save, and `sail verify` costs about 0.2 s more per edit than
  plain Verus for its receipts and complete report: reading the reused vstd
  artifacts that the receipts bind, a targeted clean of the edited package,
  the input checks before and after the run, and Verus's `--output-json
  --time` ([ADR 0047](decisions/0047-attribution-logs-on-request.md#note-2026-10-16-cheaper-checks-156)).
  The removed pass-through cost nothing here, so `sail verify` is that much
  slower than it was too.
- **Cold runs.** The first verification on a machine compiles dependencies
  and verifies vstd in Sail's own target, all of it new to the cache: 1–3 s
  slower than plain Verus on Linux, 15–18% on the Mac. Later new projects
  and worktrees that resolve the same vstd skip it (claim 8); a new set of
  Verus arguments is a new context. A warm [team cache](http-shared-cache.md)
  does not change this: the compile and frontend dominate.
- **The Rust and Verus frontend.** On vest, an edit with every unchanged solver
  answer cached still takes about 29 s on Linux. Nearly all of it is Cargo,
  rustc and Verus's frontend, which Sail does not change; the rest is the
  receipt and input checks (about 0.45 s) and Verus's `--output-json --time`
  (about 0.4 s), which every report needs. That is 0.8 s more than the
  removed pass-through took. `sail verify --output json`, jobs and MCP also
  ask Verus for its AIR and SMT logs, for per-obligation detail (about
  0.8 s more; [ADR 0047](decisions/0047-attribution-logs-on-request.md)).
- **Edits in large modules and widely used definitions.** Verus checks
  each module in one solver process. When an edit changes any query in a
  module, the solver re-runs that module's earlier queries as well, so the
  cost of an edit is the cost of its whole module, wherever the edit is.
  A change to a spec function used across the crate re-runs most modules,
  and Sail is then about as fast as plain Verus. Adding a new function or
  closure can also re-run unrelated modules that use closures. Comments
  and blank lines cost nothing
  ([real-project report](reports/2026-10-15-real-project-performance.md)).
- **Unchanged code from the CLI.** Plain Cargo sees nothing changed and exits
  in 0.1–0.25 s. `sail verify` takes 0.2–0.6 s, because it reads its inputs
  before and after Cargo runs and checks every reused receipt. Its value is
  the report, not the time.

## Setup

Every `sail verify` is managed verification
([ADR 0046](decisions/0046-managed-verification-by-default.md)); before
that change it was a thin pass-through, and managed verification was
`sail verify --incremental`.

The Linux rows of claims 2–5 below retain historical 2026-10-16 measurements for the change
that made the managed checks cheaper (#156): EC2 c7i.4xlarge, us-east-1,
Ubuntu 24.04, one timed run at a time and no other work on the host,
`scripts/perf-host.sh up m114p`, the change applied to `main` `502fc669` (before
it was rebased onto later `main`), Sail's Verus `73dea021`. The same rotation measured `main`
before the change and the removed pass-through (`SAIL_PERF_BASELINES`); the
[ADR 0047 note](decisions/0047-attribution-logs-on-request.md#note-2026-10-16-cheaper-checks-156)
has all four and cites the raw logs. This host ran about 3% slower than the
rc.5 hosts below (plain vest edit 43.2 s against 42.1 s). Those measurements
precede the rebase and final raw-path spelling correction; they do not qualify
the final code. The final-code comparison below qualifies the unchanged-run
gain and a small-project body edit, with a narrower scope.

### Final managed-check comparison

Corrected source `6563a170` against released rc.5 `c3687990`, EC2
c7i.8xlarge, us-east-1, Ubuntu 24.04, same fork `73dea021` and pinned Z3.
Three alternating, interleaved rounds use independent warmed compiler targets
and automatic Sail daemon/cache per project and mode. All 24 measured commands
exit 0. Seconds:

| Case | rc.5 median | Corrected median |
|---|---:|---:|
| Demo unchanged | 0.388 | 0.207 |
| Demo body edit | 1.154 | 0.957 |
| Vest unchanged | 1.013 | 0.539 |
| Vest body edit | 24.078 | 23.817 |

Vest edit ranges overlap (23.86–24.23 s versus 23.70–24.08 s), so this does
not establish a vest edit gain. This comparison does not qualify cold,
plain, failing/fixing, MCP or Mac performance. See the
[validation record](validation/managed-overhead.md) for checksummed raw evidence
and earlier attempts.

The Linux row of claim 8, and claims 2–5 before that change, were measured
for release candidate `v0.1.0-rc.5` on 2026-10-15, at `main` `c3687990` (managed by default, the
dependency store, and the reuse fixes of #170, #173 and #175) with Sail's
Verus `73dea021`: EC2 c7i.4xlarge (16 vCPU, 30 GiB), us-east-1, Ubuntu
24.04, one timed run at a time and no other work on the host,
`scripts/perf-host.sh up l56p` (vest; load average 2.2 at the start, from
the build before it) and `l56q` (demo, 3.4 at the start; the first-call
runs, 0.1). Raw logs: `l56-rc5.tar.gz`, cited in the
[release-candidate record](validation/release-candidate.md#rc5-evidence).
Against the previous Linux run (`l54p` and `l54q`, Sail at the ADR 0047
change, the same runtime), both plain and Sail were slower on these hosts
(plain demo cold 36.8 s against 32.7 s, plain vest edit 42.1 s against
41.8 s) and the differences between them did not change materially. That
earlier rotation also measured the removed pass-through and the managed
verification before ADR 0047 (`SAIL_PERF_BASELINES`);
[ADR 0047](decisions/0047-attribution-logs-on-request.md#measurements) has
all four.

The macOS rows were not re-measured for rc.5: they, the Linux row of
claim 7 and the table below come from the earlier run with the stock Verus
release, where the managed figures are `--incremental`'s.

| | Linux | macOS |
|---|---|---|
| Machine | EC2 c7i.4xlarge (16 vCPU, 30 GiB), us-east-1, Ubuntu 24.04, `scripts/perf-host.sh up l36`, no other work on the host | EC2 mac-m4.metal (Apple M4: 4 performance and 6 efficiency cores, 24 GiB), us-east-1b, macOS 27.0 (26A428), `scripts/perf-host-mac.sh up l39-mac`, no other work on the host, Spotlight indexing off. Load average 1.5 when each run started (macOS background services), rising to 5–14 from the runs themselves |
| Sail | `main` at `401b600`, release build of `sail-cli` and `sail-shim` | `main` at `4c1392a`, the same build |
| Verus | 0.2026.10.04.426d8b0 (`cargo xtask fetch-verus`), Z3 4.16.0 | same |
| Date | 2026-10-14 | 2026-10-14 |

Raw logs are kept outside the repository, in `l36-linux-perf.tgz` (sha256
`e0e5c92f1570021cdce855865dc0e0be58defc28a77d9735ea9ba5ed595a1dab`) and
`l39-mac-perf.tgz` (sha256
`d1aacfe887e690aca52adf25fc718768f236c34037ee2edd516ce6612d3c5229`). They
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
| Linux | `cargo verus verify` | 77.9 (77.8–78.1) | 43.2 (43.2–43.4) | 38.9 (38.8–39.0) | 43.5 (43.4–43.6) |
| Linux | `sail verify` | 80.5 (80.2–81.0) | 28.6 (28.5–28.9) | 28.8 (28.3–28.8) | 28.7 (28.6–28.8) |
| macOS M4 | `cargo verus verify` | 36.8 (36.8–37.8) | 20.7 (20.7–20.7) | 19.2 (19.2–19.3) | 20.8 (20.7–20.8) |
| macOS M4 | `sail verify` (then `--incremental`) | 43.5 (43.4–43.8) | 16.8 (16.7–16.9) | 16.0 (16.0–16.1) | 16.4 (16.4–16.4) |

On Linux, `sail verify` saves 10–15 s per edit (1.35–1.5×; 1.6× in the [real-project report](reports/2026-10-15-real-project-performance.md)'s run, with vstd patched to the pinned source); on the Mac it
saved 3–4 s (1.2–1.3×). About 29 s per edit remains on Linux and 16 s on the
Mac: the frontend floor described above, plus Sail's checks (and, on the
Mac's older run, the attribution logs plain `sail verify` no longer writes).

Reproduce (each takes about 30 minutes on Linux):

```sh
scripts/perf-loops.sh vest 3
```

## 3. Unchanged re-verification and agent re-queries

After the edits above, each mode verifies the unchanged code again (`warm2`),
and one `sail mcp` session calls `verify` twice on it (`mcp1`, `mcp2`; the
second call is shown). Wall seconds, median of three rounds:

| | Linux demo | Linux vest | macOS M4 demo | macOS M4 vest |
|---|---:|---:|---:|---:|
| `cargo verus verify` | 0.12 | 0.23 | 0.18 | 0.24 |
| `sail verify` | 0.22 | 0.59 | 0.36 | 1.37 |
| MCP `verify` | 0.35 | 1.02 | 0.57 | 1.75 |

Plain Cargo is fastest here: it sees nothing changed and prints nothing new.
`sail verify` and MCP `verify` check that the inputs are unchanged and
return the complete report again (every function's verdict, marked Current,
naming the run that established it). That is what an agent needs after a
restart or a context reset, and it costs well under a second on Linux
rather than a re-verification. It is not faster than Cargo's no-op. (The
macOS figures come from the earlier run, before the checks got cheaper.)

Reproduce: the same runs as claims 2 and 4 (`scripts/perf-loops.sh vest 3`
and `scripts/perf-loops.sh demo 3`).

## 4. Small projects

`cargo verus new --lib demo` plus four small functions (a recursive spec
with a lemma, `max`, a loop search and a counting loop), with the same three
edit phases: add a function, break `max`'s postcondition, then restore it
with a rewritten body. Wall seconds, median (min–max) of three rounds:

| demo | Mode | cold | edit | fail | fix |
|---|---|---:|---:|---:|---:|
| Linux | `cargo verus verify` | 35.0 (34.8–36.0) | 0.85 (0.83–0.86) | 0.61 (0.60–0.64) | 0.84 (0.84–0.86) |
| Linux | `sail verify` | 36.6 (36.4–36.6) | 1.08 (1.03–1.12) | 0.81 (0.78–0.86) | 1.02 (1.02–1.03) |
| macOS M4 | `cargo verus verify` | 16.3 (16.3–16.6) | 0.44 (0.43–0.45) | 0.40 (0.40–0.41) | 0.44 (0.43–0.44) |
| macOS M4 | `sail verify` (then `--incremental`) | 18.7 (18.7–18.8) | 0.75 (0.74–0.75) | 0.66 (0.66–0.66) | 0.66 (0.65–0.66) |

`sail verify` costs about 0.2 s more per run than plain Verus here; the
[ADR 0047 note](decisions/0047-attribution-logs-on-request.md#note-2026-10-16-cheaper-checks-156)
lists where it goes. Reproduce with `scripts/perf-loops.sh demo 3` (about 5
minutes).

## 5. Cold first run

The `cold` columns above: a new copy, an empty Sail cache, and every
dependency compiled and vstd verified. On Linux `sail verify` is 2–3 s slower
than plain Verus (demo 36.6 s against 35.0 s, vest 80.5 s against 77.9 s);
on the idle Mac it was 15–18% slower. Sail pays this once per machine
(claim 8), even when a project's own `target/` is built, and then reuses its compiled
dependencies and receipts
([managed runs](managed-cold-start.md)).

### New projects after the first (claim 8)

On the rc.5 build (`c3687990`, where every `sail verify` is managed), three
rounds on the native c7i.4xlarge `l56q`, one timed command at a time, load
average 0.1 at the start. Each round starts with an empty Sail cache: a
first project made with `sail new --lib` and its first `sail verify` (first
on the machine), a second new project's first `sail verify`, the same again
unchanged, and a project made with stock `cargo verus new --lib` under plain
`cargo verus verify`; then vest (`-p vest_lib`) in three fresh copies with
the same cache (vest uses another vstd, so its first copy is its first on
the machine), the second under `sail verify` and the third plain. Seconds,
median (min–max):

| Run | `sail verify` | Plain `cargo verus verify` |
|---|---:|---:|
| demo, first on the machine | 40.1 (39.9–41.0) | 39.0 (38.8–39.5) |
| demo, new project | **1.41** (1.40–1.45) | 39.0 |
| demo, new project, unchanged | 0.36 (0.36–0.36) | — |
| vest, first on the machine | 89.8 (88.8–90.6) | 89.1 (87.4–89.2) |
| vest, new worktree | **33.1** (32.9–33.2) | 89.1 |

The vest worktree's 33 s is vest_lib's own verification. These first-run
times are higher than the `cold` columns of claims 2 and 4 on both sides
(the same machine type; the cause was not investigated). The script is
`firstcall.sh` in the raw logs.

The measurement when the store was added:
[ADR 0045](decisions/0045-dependency-store.md)'s dependency store, measured
on `feat/prebuilt-vstd` (code at `93a55a0d`) against `main` at `d4ac51af`,
both on Sail's Verus `73dea021`, on a native c7i.4xlarge in the CI image,
three rounds, one timed command at a time, load average 2–5. Each round
starts with an empty Sail cache, verifies a first project (the donor), then
a new project or worktree with the donor's `Cargo.lock` (seconds):

| Run | Before | After | Plain `cargo verus verify` |
|---|---:|---:|---:|
| demo, first on the machine | 27.6 | 28.2 | 29.5 |
| demo, new project, `--incremental` | 27.5 | 1.30 | 29.5 |
| demo, new project, MCP `verify` | 27.7 | 1.51 | 29.5 |
| vest, first on the machine | 68.9 | 67.9 | 66.1 |
| vest, new worktree | 52.3 | 25.7 | 66.1 |

To reproduce on a project (vest from its repository at `11af01dd`, with
`-p vest_lib`): run `sail verify` in it with an empty
`SAIL_CACHE_DIR`, copy the project directory, and time the same command in
the copy. `scripts/dependency-store-check.py` does this for the demo and also
compares each run with plain stock Verus
([dependency store record](validation/dependency-store.md), which cites the
raw logs).

## 6. Repair loops with the engine

`sail repair-check` checks a candidate repair of one function and then the
whole crate in a single run of the optional engine. The comparison, called
M6 in the [acceptance guide](bench/m7-repair-acceptance.md), is what an agent
does without the engine: managed `sail verify` restricted to the
function, then on the whole crate, two stock Verus runs (measured with the
`--incremental` flag of the time).

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

These figures were measured on the runtime from fork `13a1176c`. A
one-round recheck on the current `73dea021` runtime (release candidate
rc.3) did not change them materially: HumanEval 64.6 s against 29.6 s
(2.18×), Vest 232.5 s against 129.4 s (1.80×)
([release-candidate record](validation/release-candidate.md#repair-benchmark)).
A one-round recheck on rc.5 (`c3687990`): HumanEval 64.8 s against 29.0 s
(2.23×), Vest 266.8 s against 146.1 s (1.83×; both arms about 8% slower on
that host). Every pair's verdicts and diagnostics matched, but the
benchmark's own outcome check currently fails on the schema v2 field
rename ([#179](https://github.com/Chelis-Lang/sail/issues/179)).

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
| macOS M4 (2 workers) | 4 × vest `-p vest_lib` | 67.3 s (65.3–67.7) | 50.6 s (49.6–51.1) | 1.3× |
| macOS M4 (2 workers) | 4 × demo | 4.3 s (3.3–4.5) | 3.0 s (2.8–3.9) | 1.4× |

The edit is a comment appended to a source file, so each job re-runs Cargo
and Verus on its crate; one such vest job alone takes about 26 s.

An earlier Linux measurement with HumanEval copies, whose jobs take about
9 s, found 2.7× (13.8 s instead of 37.7 s; L15 rescue lane report, tag
`archive/rescue-m7-98f6292`). Each job runs somewhat slower when several
share the machine, because Verus and Z3 already use several cores. Jobs in
the same workspace still run one at a time
([concurrent jobs](workspace-verification.md#concurrent-jobs)). The default
is a quarter of the cores, so the 10-core M4 runs two jobs at a time, against
four on the 16-vCPU Linux host. The demo jobs take under a second each, so
that row mostly measures the daemon's overhead.

Reproduce with `scripts/perf-agents.sh vest 4 3` (about 15 minutes on Linux)
or `scripts/perf-agents.sh demo 4 3`.

## Running the scripts

`scripts/perf-loops.sh` and `scripts/perf-agents.sh` need `sail` and
`sail-z3`, the stock `cargo verus` and `verus` for the plain baseline on
`PATH`, plus `git`, `perl` and `awk`. `sail` runs its own Verus: the one in
the Sail cache, or `SAIL_VERUS_BUILD` (the scripts keep it, and use
`SAIL_ENGINE_BUILD` when it is the only one set).
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
python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-linux.json \
  --cache ~/.cache/sail      # Sail's Verus; artifact-macos.json on macOS
cargo xtask fetch-verus      # the stock baseline; prints its PATH line
export PATH="$PWD/target/release:$HOME/.cache/sail/verus/0.2026.10.04.426d8b0/verus-x86-linux:$PATH"
#   on macOS: .../verus-arm64-macos instead of .../verus-x86-linux
export SAIL_PERF_MACHINE="EC2 c7i.4xlarge (16 vCPU, 30 GiB), us-east-1, idle"
#   describe the actual machine and whether anything else is running
SAIL_PERF_OUT=$HOME/sp/demo  scripts/perf-loops.sh demo 3     # claims 3, 4, 5
SAIL_PERF_OUT=$HOME/sp/vest  scripts/perf-loops.sh vest 3     # claims 2, 3, 5
SAIL_PERF_OUT=$HOME/sp/agents scripts/perf-agents.sh vest 4 3  # claim 7
```

After archiving results, run `scripts/perf-host.sh down LANE`. The Linux
helper confirms that the instance is terminated or absent before deleting
its security group, key pair and local connection files. Authentication,
API, waiter or cleanup errors return `SAIL9001` and retain lane state for
retry; refresh credentials or fix the named AWS error, then repeat the same
command. Already terminated/absent resources and an unrecorded lane are
idempotent. New lane state records the launch region and uses it for later commands;
keep the same AWS account. Older state without a region still requires the
original region. A successful JSON response with exactly empty `Reservations`
means absent; malformed responses, unknown states and mismatched IDs refuse.
State defaults to `~/.sail-perf`; `SAIL_PERF_STATE_DIR` selects a different
owned directory, including for isolated fake-AWS controls:
`python3 scripts/test-perf-host-down.py`. Those controls create no EC2 hosts. See the
[teardown validation record](validation/perf-host-teardown.md) for their native
workspace-gate scope and raw evidence.

The macOS rows above came from the same commands, plus
`scripts/perf-agents.sh demo 4 3`, on an EC2 Mac host
(`scripts/perf-host-mac.sh up LANE`; Apple's licence bills a Mac host for at
least 24 hours) with Spotlight indexing off (`sudo mdutil -a -i off`), each
command started once the load average was below 1.5, and
`SAIL_PERF_MACHINE="EC2 mac-m4.metal (Apple M4, 10 cores: 4P+6E, 24 GiB),
macOS 27.0 (26A428), us-east-1b, idle (Spotlight indexing off)"`. Keep `SAIL_PERF_OUT` short: the daemon's socket lives
under it, and Unix socket paths are limited to about 100 bytes. Each script
prints its table on stdout and keeps it as `table.md` in the output directory;
`scripts/perf-agents.sh report OUT_DIR 4` prints an earlier run's table again.

The older workload numbers in [benchmarks](benchmarks.md) (generated by
`sail-bench`) measure the solver shim on corpus units in isolation. This page
measures what a user runs.
