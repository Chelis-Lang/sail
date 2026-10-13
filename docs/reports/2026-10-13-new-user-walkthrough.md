# New-user walkthrough (L27), 2026-10-13

The question this report tests: can an agent or developer editing Verus code
get the same verdicts and errors as `cargo verus verify`, much faster on
every loop after the first, through the CLI or MCP, when starting from
nothing and reading only `README.md`, `docs/README.md` and the guides they
link to?

**Short answer.** Verdicts and errors match: in every comparison, `sail verify`
printed the same stdout and stderr as `cargo verus verify`. Speed only partly
holds. It improves edit loops on a solver-heavy real project (vest, 1.6–1.8×).
It does nothing for small projects or cold runs. On macOS the managed path
(`--incremental`, MCP, watch) takes 26–69 s on every run, including unchanged
re-verification, where plain Verus takes 0.2–0.8 s.
MCP does not work in Claude Code at all: tool listing fails. Where MCP works
(a hand-written client), a single `verify` reply is 4–16 MB. The repair
engine cannot be obtained without private repository access.

## Scope and setup

| | Linux | macOS |
|---|---|---|
| Machine | c7i.4xlarge (16 vCPU, 30 GiB), us-east-1, Ubuntu 24.04, `scripts/perf-host.sh up l27` | Apple M3, 8 cores, 24 GiB, macOS 26 |
| Sail | `main` at `212ceb4`, rsynced (the repository is private, so an outside user cannot clone it) | fresh clone of `main` at `212ceb4` |
| Toolchains | rustup installed fresh; pinned Verus via `cargo xtask fetch-verus` | existing rustup/Cargo installs reused; fresh `HOME` (`/tmp/l27h`), so the Sail cache and Verus install were fresh |
| Projects | `cargo verus new --lib demo` with five functions (spec recursion, a lemma, `max`, a loop search, a loop sum); vest `11af01dd` (`-p vest_lib`, 3,237 Verus items); human-eval-verus `bbe93f7` | same |

Each sequence ran in its own fresh copy: cold, warm (unchanged), a benign edit
(add a function), a failing edit (break a postcondition), the fix, then warm
again. Three modes ran: plain `cargo verus verify`, `sail verify` and
`sail verify --incremental` (the managed path that MCP `verify` and `watch`
use). Times are wall clock for one sample each, so they are indicative, not
benchmarks. On Linux, the perf-host image preinstalls `build-essential`,
`pkg-config`, `libssl-dev` and `python3`, which is more than a clean host has.
The Linux raw logs were lost with the host. The macOS logs are retained outside
the repository: `l27-mac-logs.tgz`, sha256
`6ee8a89cbfeb9d04efb5f8793e342035f079b4866348de2a04e96ff2979a2654`.

## What works well

- **Identical output.** For both projects on both platforms, `sail verify` and
  `cargo verus verify` produced the same stdout, stderr and exit code at every
  step (24 of 24 comparisons, after normalizing paths and `Finished … in`
  times). The failing postcondition was reported byte for byte.
- **Building Sail is quick.** Release build of `sail-cli` and `sail-shim`: 39 s
  on Linux, including toolchain install, and 112 s on macOS (warm Cargo
  registry).
- **Cold Verus install.** `cargo xtask fetch-verus` took 19 s on Linux and 43 s
  on macOS, once its undocumented prerequisites were met (see friction 4).
- **The solver cache helps on a real project.** vest edit loops:

  | vest `-p vest_lib` | cold | warm | edit | failing edit | fix | warm |
  |---|---|---|---|---|---|---|
  | Linux plain | 74.5 | 0.21 | 41.9 | 37.5 | 42.2 | 0.21 |
  | Linux `sail verify` | 73.5 | 0.25 | 38.6¹ | 20.9 | 25.6 | 0.21 |
  | Linux `--incremental` | 58.9² | 1.42 | 28.9 | 23.5 | 28.9 | 1.41 |
  | macOS plain | 99.6 | 0.80 | 53.4 | 45.6 | 42.3 | 0.69 |
  | macOS `sail verify` | 77.4 | 0.73 | 32.6 | 23.4 | 26.7 | 0.71 |
  | macOS `--incremental` | 60.9 | 58.7 | 58.4 | 47.4 | 55.2 | 68.7 |

  Times are in seconds. ¹ First edit after cold. ² That copy started with a
  solver cache already warm from the `sail verify` run.
- **The managed path on Linux.** Demo: unchanged re-verify 0.33 s, edit
  1.0–1.1 s. vest: unchanged 1.4 s, with both target receipts reused.
- **`sail watch` on Linux.** An edit was queued 390 ms after the write (300 ms
  debounce) and finished 1.26 s after it. The JSONL lifecycle events
  (`queued`, `started`, `target-reused`, `target-complete`, `finished`) are clear.
- **MCP from a minimal stdio client, on Linux.** On the demo, the first `verify`
  took 32 s, an unchanged one 0.54 s and a failing edit 1.15 s. Evidence tools
  answered in 20–120 ms. `compare_verification` against a saved baseline
  correctly classified `demo::max` as `recovered`. The `verify` summary text is
  good: failed functions, the counterexample obligation id and the first error
  location.
- **The HTTP team cache worked on the first try** with the documented local
  trial. `publish` sent 69,195 entries in 11.8 s. A reader with an empty cache
  got 2,437 shared hits from 2,438 lookups. `sail status` shows clear counters.
- **Some error messages are good.** `error[SAIL0101]: … cargo-verus is not on
  PATH (or set SAIL_CARGO_VERUS)`. `sail cache stats` and `sail cache verify`
  print readable output.

## Friction, ranked by user impact

1. **MCP is unusable in Claude Code.** After the documented
   `claude mcp add --transport stdio sail -- sail mcp`, `claude mcp list` (Claude
   Code 2.1.296) reports: `Connected · tools fetch failed — Invalid result for
   tools/list: … "path": ["ttlMs"] … expected number, received undefined …
   "path": ["cacheScope"] … expected one of "public"|"private"`. The captured
   traffic shows Claude Code opening with `server/discover` on revision
   `2026-07-28`. Sail accepts that revision, but its `tools/list` reply carries
   only `resultType` and lacks `ttlMs` and `cacheScope`, so the agent gets zero
   tools. The `2025-06-18` path works from a hand-written client. *Fix:* send
   valid `ttlMs` and `cacheScope` on 2026-07-28 list results, or stop
   advertising that revision. Add a Claude Code smoke test to the managed e2e
   phase.
2. **MCP replies are too large for an agent's context.** Sizes of single
   JSON-RPC frames:

   | Reply | Demo (5–6 functions) | vest |
   |---|---|---|
   | `verify` | 4.3 MB | 15.6 MB |
   | `verification_result` | 4.3 MB | |
   | `save_baseline` | 4.4 MB | |
   | `get_baseline` | 4.4 MB | |
   | `counterexample` | 285 KB | |
   | `explain_failure` | 98 KB | |

   Most of each `verify` reply is the vstd dependency's report (1,858
   functions), sent twice: once as `structuredContent` and once as JSON text.
   Counterexample assignments are mostly `fuel%vstd!…` internals, with the
   useful `a!`/`b!` buried among them. The text summary of the evidence tools is
   fixed boilerplate ("Scoped proof evidence; provisional observations do not
   authorize a repair."). It says nothing about the explanation, the
   comparison or the baseline. *Fix:* return only requested targets by default,
   send JSON once, put source-variable models in the summary, and give each
   evidence tool a summary that states its result.
3. **On macOS, the managed path never reuses anything.** Every
   `--incremental`, MCP `verify` or watch run recompiles `proc-macro2` through
   `verus_builtin_macros` and verifies vstd again ("2 verifier runs, 0 target
   reports reused"). Demo loops take 26–32 s against 0.2–0.6 s for plain Verus
   (four MCP `verify` calls took 27–35 s). vest loops take 47–69 s against
   0.7–53 s. Results usually read `Historical` immediately after the run that
   produced them. That happened under `/tmp`. With the project under the home
   directory the first two runs read `Current`, but still reused nothing. The
   README says macOS checks "can require fresh proofs". In practice, they
   always do. *Fix:* until macOS reuse works, document that macOS users should
   use plain `sail verify` and not MCP `verify`/watch, and investigate why the
   dependency build records are withdrawn on every macOS run.
4. **There is no documented way to install Verus.** The README's "Using it"
   assumes `cargo-verus` already exists. To get from a clean Linux host to a
   working setup, I had to:
   - find `cargo xtask fetch-verus`, which is mentioned only in the repair
     walkthrough;
   - install `unzip` (otherwise `failed to run "unzip" … No such file or
     directory (os error 2)`);
   - set a rustup default (`rustup could not choose a version of cargo`);
   - install toolchain 1.98.1 (`fetch-verus` prints `rust toolchain: 1.98.1` but
     does not install it, so `cargo verus new` fails with `toolchain
     '1.98.1-x86_64-unknown-linux-gnu' is not installed`);
   - add `~/.cache/sail/verus/<release>/verus-<platform>` to `PATH`.

   `fetch-verus` prints the install path but not this step. *Fix:* add an
   "Install" section with these steps, or have `fetch-verus` install the
   toolchain and print the `PATH` line.
5. **"Much faster on every loop" holds only where the solver dominates.**
   - Small projects: `sail verify` equals plain Verus. On Linux, demo edits took
     0.84 s against 0.82 s.
   - Cold runs: no gain. A cold run with a fully warm shared cache took 34.3 s;
     plain Verus took 33.5 s.
   - vest: the 1.6–1.8× above. MCP `verify` on vest still took 24–30 s per
     edit with "0 solved fresh, 3028 from cache". With every solver reply
     cached, the Rust/Verus frontend sets the floor.
   - Unchanged code: plain Cargo already no-ops in 0.1–0.2 s. The managed
     receipt (0.3–1.4 s) is not faster from the CLI; its value is complete
     reports for agents.

   *Fix:* say this plainly in the README, with numbers, and make
   frontend/vstd reuse the headline roadmap item.
6. **Managed and MCP verdict counts disagree with Verus.** Verus reports
   `3237 verified, 1 errors` for the failing vest edit. The managed summary
   (and the MCP `verify` summary) reports `0 verified, 1 failed, 3127 unknown`:
   attribution is `partial` because of duplicate friendly names, so every
   other function becomes `unknown`. An agent reads that as 3,127 unresolved
   proofs. Even on the demo the units differ (`5 verified` against Verus's
   `7 verified`), and nothing explains why. *Fix:* when Verus's own totals show
   no other failure, report the unattributed functions separately from
   `unknown`, and explain the counting unit in the summary.
7. **A long cache path silently disables the daemon.** With `HOME` 95
   characters deep, the socket path exceeded `SUN_LEN`. Every `sail verify`
   waited about 5.5 s and then ran without any cache, printing nothing. Only
   `sail serve` showed `error[SAIL0003]: … path must be shorter than SUN_LEN`.
   Deep CI workspaces and custom `SAIL_CACHE_DIR` values can hit this. *Fix:*
   warn once on stderr when the daemon cannot start, shorten or hash the socket
   name, and document the limit.
8. **The repair engine cannot be obtained.** Results:
   - `cargo xtask build-verus-engine` stops after 2 s: `the Verus fork is
     private and gh auth token failed`.
   - The prebuilt runtime is a private release asset, and Linux-only.
   - `sail engine status` prints `error[SAIL3120]: No such file or directory
     (os error 2); run cargo xtask build-verus-engine`. It does not say which
     file is missing, and the advice cannot succeed without private access.
   - `sail repair-check … --engine interactive` without an engine prints
     `Repair validation job …` and then `error[SAIL0906]: SAIL0808: EOF while
     parsing a value at line 1 column 0`. It leaves an `infrastructure_error`
     job instead of the documented `SAIL3120`.

   The README gives this unobtainable feature its largest section. *Fix:* mark
   it "requires access to Chelis-Lang/verus" at the top, move it below
   ordinary use, and fail fast with `SAIL3120` before submitting a job.
9. **Non-Cargo Verus projects are unsupported, and the docs don't say so.**
   human-eval-verus is a set of `verus file.rs` tasks. `sail verify` there
   fails with Cargo's `could not find Cargo.toml`. The README never says that
   ordinary use, MCP and watch need a Cargo project. Only the portfolio mentions
   `--verus-source`. *Fix:* state the requirement, or support direct `verus`
   invocations.
10. **`--help` handling.** `sail watch --help` started a watcher, which ran for
    20 minutes until killed. `sail cache gc --help` gives `error[SAIL0002]:
    unknown argument --help`. `sail verify --help` shows only `cargo verus
    verify` help, with no Sail options. *Fix:* treat `--help` uniformly.
11. **Disk use grows without bound.** After one demo and three vest copies,
    managed targets used 2.7 GB on Linux (2.1 GB for one vest workspace) and
    1.7 GB on macOS. The `sail cache` commands do not cover them (this is
    documented). *Fix:* a `sail cache projects list|prune` command.
12. **`sail jobs list` shows the wrong currentness.** Every job lists as
    `Completed (Unknown)`, including jobs that `jobs status` and
    `verification_result` report as `current` or `historical`. *Fix:* show the
    same projection as `status`.
13. **Reverting does not reuse.** Under watch, restoring a revision proved two
    jobs earlier (same `input_revision`) re-verified the crate. *Fix:* keep
    receipts for more than the latest revision.
14. **vstd is verified again per project and per context.** The first MCP
    `verify` on a new copy of an already-verified demo took 32 s. This is known
    (L16). It is the main first-contact cost for agents that open many
    worktrees.
15. **The user guides read as validation records.** The README, `mcp.md`,
    `proof-repair.md` and the walkthrough mix instructions with commit ids
    (`387ba80`, `b895`, `57a319d`), gate numbers, p95 reader values and "PR #74
    is draft". The README's status line points to the M6 report, and "Getting
    started" is contributor setup (`install-hooks`, `nextest`). A new user
    cannot tell what to run. *Fix:* a short user README (install, verify, MCP,
    limits), with qualification status moved to `docs/validation/`.

Minor: macOS's first plain `cargo verus verify` took 185 s against 29 s for a
later cold copy (cause unknown; it happened without Sail). `sail status` in the
deep-`HOME` setup reported "no daemon is listening" without saying why.

## Doc errors

| Where | Says | Observed |
|---|---|---|
| `docs/workspace-verification.md` | Daemon socket `daemon-v4-workspace-v1.sock` | `daemon-v4-workspace-v2-proof-v1.sock` |
| `docs/proof-repair.md` | Job result through MCP `get_verification_result` | Tool is `verification_result` |
| `docs/proof-repair.md` | An unavailable engine returns `SAIL3120` | `repair-check` returns `SAIL0906: SAIL0808: EOF while parsing…` and an infrastructure-error job |
| `README.md` | Daemon logs to `daemon.log` in the cache root | `~/.cache/sail/results/daemon.log`. Elsewhere the docs call `~/.cache/sail` the cache. |
| `README.md` | macOS "can require fresh proofs" | Every managed macOS run re-verified everything (friction 3) |
| `docs/mcp.md` | The `claude mcp add` registration works | Claude Code lists no tools (friction 1) |
| `docs/mcp.md` | The `counterexample` example shows the model in the summary | The job-backed summary is `1 function result(s) from job …; currentness …` |
| `docs/proof-repair-walkthrough.md` | `fetch-verus` "installs artifacts" | It needs `unzip`, does not install the Rust toolchain it names, and prints no `PATH` advice |
| `docs/errors.md` | `SAIL0101`: "Put it on PATH" | No pointer to how to obtain `cargo-verus` |
| `README.md` | Usage sections, then "Getting started" | "Getting started" is contributor setup and never mentions installing Verus |

## README promises against what a user gets

| Promise | Reality |
|---|---|
| "runs your own `cargo verus verify` … Its only change is putting `sail-z3` in front" | True. Output matched in every comparison. |
| Faster loops through the solver cache | Only when the solver dominates: vest 1.6–1.8×, small projects 1.0×, cold runs 1.0× |
| Complete incremental reports and durable jobs | Linux: works, with sub-second to 1.4 s unchanged re-verify. macOS: 26–69 s per run with no reuse. |
| `sail mcp` for agents that verify after every edit | Broken in Claude Code. With other clients: 4–16 MB per `verify`, and 3,127 `unknown` after one vest failure. |
| `sail watch` | Linux: good (1.3 s from edit to result). `--help` starts a watch. |
| Team caching | Works, but cold runs gain about 0 s because compile and frontend dominate |
| Experimental proof repair: build or install the engine | Not obtainable without private GitHub access. Linux-only. Misleading errors. |
| `sail cache stats|verify|gc|clear` | Work. They do not cover the 1.7–2.7 GB of managed targets. |
| Getting from zero to a first verification | About 7 undocumented steps (friction 4). The Sail repository itself is private. |

## Documentation impact

This report is the only change. It records findings for the roadmap review
and fixes nothing.
