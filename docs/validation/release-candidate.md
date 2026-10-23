# Release-candidate validation

**Current status:** corrected fork `7d9d5410` passes both installed platforms'
functional qualification. All four mutation cohorts and both 31-row matrices
are complete. Performance acceptance remains open because Mac cold and
unchanged calls still have real overhead. No rc.6 draft has been created.

The [runtime record](spec-signature-runtime.md) identifies SDKs and deep checks;
the [matrix](release-matrix.md) binds each cohort to its actual consumer.
Packaging preserves the qualified stages without rebuilding.

| Corrected-candidate gate | Result |
|---|---|
| Native Linux workspace, solver, full E2E including extra, PR/full fast and standard | Pass |
| Mac host gate | Pass; 963 tests, clippy clean |
| Platform factories and exact default/100 proofs | Pass on both platforms |
| Public Vest edit/undo, both new runtime manifests | Pass; restored original proof uses zero verifier invocations |
| Four complete three-round installed mutation cohorts per platform | Pass; 651 matching comparisons on each platform, zero false verified, different, unstable or infrastructure results |
| Deep corpus | Pass; four arms each 159 Same / 13,166 functions; two optional Singular projects excluded |
| Full fork source suite | Pass-through, cache-fill and cache-warm each 4,901 Same; all 4,838 nonignored tests pass; separate stock debug-order limitation remains recorded |
| Release matrices | Both complete: 31/31 valid; slower rows remain open |
| Actual new Linux and Mac archives, README and real Claude Code MCP | Pass; first-use proof served, edited verification, repair, MCP lists 17 tools |
| Release performance decision and rc.6 draft | Open |

| Corrected archive | SHA-256 |
|---|---|
| `sail-0.1.0-rc.6-x86_64-unknown-linux-gnu.tar.gz` | `bca24a1bb7ca53256b13b4b1cad7ad8d79a0863f5a3a8e32339149e0516d8996` |
| `sail-0.1.0-rc.6-aarch64-apple-darwin.tar.gz` | `30b5e45dfa42fc9e6f65dbfafb4ecc1b433102b2575b4762c3adcc4d4e076806` |

Raw complete native consumer gates and Linux archive/MCP checks:
`experiments/2026-10-23/signature-consumer-complete.tgz`, SHA-256
`870f63fc777ee341502af2c0aafdd074b88f37a04323a94f3568e7e05b9b4755`.
Actual Mac archive, README and MCP checks:
`experiments/2026-10-23/signature-mac-smoke.tgz`, SHA-256
`f06e5604e7ff3d05077075a43e647da43286a67415bbac8e9f55cb562a99bf0a`.
These checks qualify the binaries above, not a later rebuilt consumer.

## Previous unaccepted candidate

The remainder records the preceding fork-f8589ed4 attempt. Its archive hashes,
smoke checks and completed deep results do **not** qualify the corrected
candidate above. Those rc.6 archives are superseded; the Mac HumanEval failure
remains a failed attempt ([#337](https://github.com/Chelis-Lang/sail/issues/337)).

**Status:** rc.6 archives are built and pass native installation smoke tests.
Final deep checks pass. Mac mutation qualification has a functional failure
tracked in [#337](https://github.com/Chelis-Lang/sail/issues/337); performance
qualification also remains open.
No rc.6 GitHub draft has been created and release acceptance remains open.

The selected consumer combines the retention-budget correction (#334) and
exact default/limit-100 proof support (#335), using fork f8589ed4 on Verus
7d82994 and Rust/Cargo 1.99.1. Packaging (#336) preserves the exact qualified
CLI, shim, runtime, proof and signed reply pack; it does not rebuild them.
See the [combined matrix](release-matrix.md) for binary identities and the
[raised-profile record](rlimit-subsumption.md) for source and producer checks.

| Gate | Current result |
|---|---|
| Native Linux workspace, solver and full E2E including extra | Pass |
| Full fast and standard conformance | Pass; each standard arm: 159 Same, 13,166 functions, zero different, unstable or infrastructure errors |
| Mac host gate | Pass; clippy clean, 963 tests |
| Exact default/100 Mac proof recording and successful public Vest undo | Pass; original proof restored with zero verifier invocations |
| Actual Linux archive, fresh Ubuntu 24.04 host | Pass: bundled runtime, first-use proof served, edited verification and repair accepted |
| Actual Mac archive, fresh extraction and empty cache | Pass: bundled runtime, first-use proof served, edited verification and repair accepted |
| Linux README and real Claude Code MCP | Pass; 17 tools |
| Mac README and real Claude Code MCP on selected artifact | Pass; 17 tools |
| Final deep corpus and full source suite | Pass; all four deep corpus arms and full source-suite pass-through, cache-fill and cache-warm |
| Final Linux/Mac matrices and all four mutation cohorts per platform | Both scripted matrices complete; Mac HumanEval fails with three functional disagreements (#337); remaining cohorts pending |
| Release performance decision and rc.6 draft | Pending |

The Mac archive smoke uses the native paid host with the documented Rust
prerequisites already installed. Linux installs those prerequisites on a
fresh native c7i.4xlarge, us-east-1, Ubuntu 24.04 host. Neither smoke uses a
separate Verus installation. These are functional checks, not benchmarks.

| Asset | SHA-256 |
|---|---|
| `sail-0.1.0-rc.6-x86_64-unknown-linux-gnu.tar.gz` | `fd1875e6ed2869d45577b06b747f4aeda85ba317d84a31e6b41a971747b48c40` |
| `sail-0.1.0-rc.6-aarch64-apple-darwin.tar.gz` | `37018e5090925ff41aef3095f7f236651f25e51aec888fbbd5e3725eb60bce34` |

Raw packaging and Mac smoke logs:
`experiments/2026-10-23/rc6-packaging-smoke.tgz`, SHA-256
`6f7720da12c2fb51e92c4ff2b4d27508dfb221be62c3f202cae8afe79df6948d`.
Raw Linux smoke log, prerequisites and result documents:
`experiments/2026-10-23/rc6-linux-smoke.tgz`, SHA-256
`f7854d643affa53dd2321c8ff33af7265312d346332ec189f9e42476b817259f`.

Final deep logs and reports: `experiments/2026-10-23/final-combined-deep.tgz`,
SHA-256 `60f7131b23c2f5ed9a6519392e77e059f0ba7438254e3619b7b507f220fd49cd`.
The selected fork has 4,901 matching tests in each source-suite arm, with
4,838 nonignored tests passing. The stock control retains its separately
recorded existing failures; no new shim disagreement is accepted.

Historical candidate checks follow; rc.5 remains an unaccepted draft.

## Qualified-stage packaging control, 2026-10-22

Source: `release/package-qualified-stages` on `ccc01205`, runtime f8589ed4 /
Verus 7d82994. Native EC2 mac-m4.metal, us-east-1c, macOS 27.0.
The new packaging-only command produces both platform archives with the
original CLI, shim and release-index bytes, the expected single top directory,
root-owned entries and correct checksum files, including stages reached through
directory aliases. A swapped platform is refused
before any output; existing assets are refused without changing their bytes.
Both source stages remain unchanged. This qualifies packaging, not release
performance or publication. The input Linux CLI is `06cf3abe…32945f5` and Mac
CLI `8fb73ac6…15e33b`; complete digests and control results are in
`experiments/2026-10-22/qualified-package-alias-control.tgz`, SHA-256
`96c432daf9e2b1dfeef0fe8b98446b5a471be4a738e3a9374455f61b053aa86f`.
The Mac host gate passes: clippy clean, 963 tests passed. Its first attempt
stopped before tests because cargo-nextest was missing; the pinned tool was
installed before the successful rerun. Native c7i.8xlarge workspace gate,
solver, full E2E including extra and PR conformance also pass. Their raw logs:
`experiments/2026-10-22/package-native-gates.tgz`, SHA-256
`1a4e31493729d3a8e1d350d3751c43b6a1e9814e5dfd719539ce3ebe0ee09200`.

## v0.1.0-rc.5

**Post-validation correction (#184).** The exact draft rc.5 macOS asset
omits vstd's stdout summary when a second project's fresh target is seeded
from the dependency store. Its original fresh-install check exercised the
second project but did not compare stdout for that case. The conformance
tier results below remain results for their recorded scope; they did not
cover this output regression. The [minimal reproduction and correction
record](dependency-store-output.md) identifies the asset, source, runtime
and raw evidence. The correction is a separate post-rc.5 candidate; the
existing draft asset has not been rebuilt or revalidated and remains a
draft. No release acceptance follows from the candidate fix's checks.

**Additional unresolved input-coverage limit (#187).** A later-source
macOS reproduction replaced private `cc` bytes at unchanged `PATH`: fresh
stock failed with a linker error while dependency-store-seeded Sail succeeded
without it. The [known-issues entry](../known-issues.md) identifies the actual
source/runtime and raw checksum. Source inspection finds v1 native-artifact
seeding still on main; today's exact main and this draft asset were not rerun
for that case. The historical tables do not qualify this mutation. The
unmerged replacement has its own APFS mmap guard blocker. The #166/#189
metadata correction fixes neither, and the draft remains unchanged and
unaccepted.

The 0.1.0 candidate. `main` changed materially after rc.4: every
`sail verify` is managed verification (#155, ADR 0046); new projects and
worktrees start from a machine-local store of verified vstd (#154, ADR
0045) and `sail new` creates a ready-to-verify project (#152); reports use
output schema v2 (#153); attribution logs are written on request (#172,
ADR 0047); and managed reuse was fixed for member directories (#170, issue
#157), build-script edits, generated includes and unbuilt scripts (#173,
issues #119–#121) and store publication (#175). The Verus pin, both
runtimes and the CI image are unchanged. Everything below ran on
`c3687990`, 2026-10-15/16, lane L56, on native c7i.8xlarge hosts (32 vCPU,
us-east-1), one suite or shard per host, unless stated.

| Check | Environment | Result |
|---|---|---|
| `scripts/release.sh 0.1.0-rc.5` | Linux half on a native c7i.4xlarge (lane `l56-rel`, torn down by the script); macOS half on the developer Mac | pass: draft `v0.1.0-rc.5` targeting `c3687990`; both self-checks selected and validated the bundled runtime (manifests `4f0d0d3f…ccdd` and `a42a7d19…b5c8`); `sail --version` prints `sail 0.1.0-rc.5` from each archive |
| `ci/local.sh gate` | `l56`, image `sail-ci:3c02e445e25df766` | pass: 620 passed, 35 skipped; fmt, clippy, doctests, rustdoc, links, layering, deny, benchmarks page |
| `ci/local.sh solver` | `l56` | pass: 35 passed |
| `ci/local.sh e2e` (legacy, managed, repair) | `l56b` | pass: `sail new` identical to `cargo verus new`; `sail verify` identical to `cargo verus verify` (exit 101; 20,247 cache entries); combined output, compile error and partial verification identical; `LD_LIBRARY_PATH`, `--no-verify`, `--config`, `--tests`, `--target-dir`, `--output-json --time` and a `-p`/`--features` workspace as plain; managed acceptance (failing first runs keep vstd, dependency store seeding, CLI/MCP jobs, prune, build-script reuse and more); `claude-code: listed 15 sail tools on protocol 2026-07-28`; real repair CLI/MCP acceptance |
| `ci/local.sh conformance fast pr` | `l56` | 33 / 33 / 0 / 0 / 0 without the daemon, fill and warm (311 functions, 3,442 entries); engine 33 same, 32 focused (`4f0d0d3f4315`) |
| `ci/local.sh conformance fast` | `l56b` | 147 / 147 / 0 / 0 / 0 in every arm (3,450 functions; 40,361 entries); engine 147 same, 146 focused |
| standard tier, without the daemon and engine | `l56c` | 159 / 159 / 0 / 0 / 0 in both; 12,726 functions; 158 focused; 0 units with rlimit drift |
| standard tier, fill and warm | `l56d`, its own daemon on a fresh cache | 159 / 159 / 0 / 0 / 0 in both; 144,933 entries |
| deep tier, every arm | `l56e` (group A, all arms), `l56f` and `l56g` (group B) | 159 / 159 / 0 / 0 / 0 in every arm (150 + 9 units; 9,404 + 3,322 = 12,726 functions; 149 + 9 focused). Group A's engine arm showed the usual rlimit drift with no difference (VNR `log_entry_alive_value_wrap_around`, 51,948 against 51,945) |
| `ci/local.sh deep-rvt`, one round (no cache rounds) | `l56` | 4,695 tests (58 ignored): 4,691 same, 4 Unstable, **0 Different**; plain 4,635 ok and the 2 source-build `cargo::cargo_new_*` FAILED, as before. All 4 Unstable are `output_json` tests (finding 11) |
| repair benchmark, one round per catalogue | `l56` (HumanEval), `l56b` (Vest) | no regression; the outcome check fails on a harness schema mismatch (below; finding 13) |
| `ci/run.sh host` | developer Mac (Apple M3, macOS 26.5, shared; load average 35–45) | pass: clippy clean; 609 passed, 35 skipped |
| `ci/run.sh e2e managed` | developer Mac, the pinned macOS runtime, stock Verus as the oracle | pass: managed acceptance as on Linux; `claude-code: listed 15 sail tools`. The first attempt failed before any check: the fixture's daemon socket under macOS's default `TMPDIR` is longer than a Unix socket allows; rerun with `TMPDIR=/tmp` |
| `scripts/release-smoke.sh`, Linux archive | fresh c7i.4xlarge `l56s`, no Sail checkout and no Verus; rustup and Rust 1.98.1 only | pass: bundled runtime selected; `sail new` and `sail verify` passed; `repair-check demo::max` accepted |
| `scripts/release-smoke.sh`, macOS archive | developer Mac, fresh directory, empty `SAIL_CACHE_DIR`, no `cargo-verus` on `PATH` (functional check only, not clean-macOS evidence) | pass, same checks |
| README install and first use, with the checks below | Linux: fresh `ubuntu:24.04` container (24.04.5) on `l56s`; macOS: the developer Mac, the macOS archive in a fresh directory, empty cache | pass |

| Asset | Size | sha256 |
|---|---:|---|
| `sail-0.1.0-rc.5-x86_64-unknown-linux-gnu.tar.gz` | 42.3 MB (42,268,534 B) | `ea4cfb787c79ce24496d61c7a8944ab5861184a9641e0ac810369d5fd27f2f5e` |
| `sail-0.1.0-rc.5-aarch64-apple-darwin.tar.gz` | 35.2 MB (35,243,180 B) | `303e0750a8c2d7ea48d7a149f10608cd68ff00d7d67b4dd9cf2db58880b5d309` |

### README install and first use

Both platforms followed README Install and First verification verbatim
(on Linux as a new non-root user `walker` with only `sudo` added; the
draft's assets, which cannot be downloaded anonymously, were copied in
instead of fetched with `curl`). `shasum -a 256 -c`: `OK`; rustup and Rust
1.98.1 in 19 s; `sail engine status` selected the bundled runtime; no
`cargo-verus` on `PATH`. Then, in the same cache: a second new project, a
workspace member verified from its own directory (#157), a new project
whose first run fails, MCP `verify` on it, the fix, `repair-check`, a
loader variable and `--verify-function`. Seconds, one sample each; the
macOS times are on the shared developer Mac at load average 33–47 and are
indicative only:

| Step | Linux | macOS | Result |
|---|---:|---:|---|
| `sail new --lib demo` | 0.1 | 1.3 | `Created Verus library package demo … (vstd 0.0.0-2026-10-04-0306, matching Sail's Verus)` |
| `sail verify` (README; first on the machine, vstd verified) | 33.0 | 140.5 | 2,060 vstd and 1 demo verified |
| the same, unchanged | 0.4 | 0.7 | exit 0, no output, as plain Cargo |
| second new project, first `sail verify` | **1.3** | **3.0** | `1 verified`; vstd from the dependency store |
| workspace member from its directory (no `-p`), first / second / after an edit | 1.3 / **0.4** / 1.0 | 2.9 / **0.8** / 1.8 | the second run reuses everything, the edit reuses vstd (#157 fixed); `--output json`: Current, `reused: true` |
| new project whose first run fails, first / second | 1.1 / 0.7 | 2.9 / 1.4 | exit 101, `postcondition not satisfied`, `0 verified, 1 errors` |
| MCP `verify` on it (stdio client, protocol `2025-06-18`, 15 tools) | 0.95 | 1.5 | 3.9 KB: `FAILED: 1 of 1 function(s) in demo3 failed: demo3::max`; vstd 1,871 verified, not shown |
| fixed / unchanged | 1.0 / 0.4 | 1.3 / 0.6 | `1 verified`; `--output json` Current |
| `repair-check --function demo3::max --engine interactive` | 30.2 | 136.8 | accepted, full evidence Current |

- **Loader variable.** A new project verified with `LD_LIBRARY_PATH` set,
  compared with plain `cargo verus verify` (the bundled `cargo-verus` by
  path, in a fresh `target/`) under the same variable: exit codes equal,
  stdout byte-identical (`2060 verified`, then `1 verified`), stderr
  identical once Cargo's progress lines and Sail's one
  `note[SAIL3112]: LD_LIBRARY_PATH is set, so this run reuses no proof
  receipts and records none …` are removed. Sail 29.2 s against plain
  31.4 s on Linux (vstd verified, as plain does; the solver cache stays on).
- **`--verify-function`.** `sail verify -- --verify-function max` and plain
  `cargo verus verify -- --verify-function max` both stop with
  `Error: Partial verification must use cargo verus focus`, exit 1, on
  passing and failing code: identical stdout and stderr.
- **Issues #119–#121.** Their reproductions, run as `sail verify --output
  json` (now the default managed path) on both platforms; every run
  Current. Linux seconds (macOS in brackets):

  | Reproduction | Runs | Result |
  |---|---|---|
  | #119, `build.rs` without `rerun-if-changed`: cold, unchanged, two comment-only edits | 1.4, 0.4, 1.1, 1.1 (5.0, 1.0, 3.7, 4.9) | each edit 1 verifier run with vstd reused (was 17 s and 0 reused) |
  | #120, a `serde = "=1.0.228"` dependency: three unchanged runs | 2.7, 0.4, 0.4 (20.3, 1.8, 1.5) | runs 2 and 3: 0 verifier runs, both targets reused (was Historical every run) |
  | #121, workspace `-p a`, `b` enabling `foo/serde`: three unchanged runs | 1.3, 0.4, 0.4 (8.3, 1.4, 1.6) | runs 2 and 3: 0 verifier runs, both targets reused (was Unknown every run) |

  The first time this workspace was verified on each machine (an earlier
  pass of the same script), its first run did not use the dependency store
  and verified vstd (29 s Linux, 111 s macOS) and stored another entry;
  the second pass above reused it (finding 15).

### Repair benchmark

`sail-bench repair` as for rc.3: M6 from `rescue/m6-baseline` (`26f124f`),
M7 at `c3687990` with the `73dea021` runtime, one round per catalogue on
its own c7i.8xlarge, against the frozen figures measured on `13a1176c`.

| Catalogue | M6 whole loop | M7 whole loop | Speedup | Recorded (M6 / M7 / speedup) | M7 first actionable |
|---|---:|---:|---:|---|---|
| HumanEval | 64.8 s | 29.0 s | 2.23× | 64.7 s / 30.3 s / 2.14× | 3.47 s (recorded 3.46–3.57 s) |
| Vest | 266.8 s | 146.1 s | 1.83× | 246.6 s / 135.1 s / 1.82× | 14.6–15.1 s (recorded 13.4–13.7 s) |

No M7 figure is more than 15% slower than recorded, so no three-round run
followed. On Vest both arms ran 8% slower than recorded, the unchanged M6
binary included, so the speedup held; the cause of the slower host was not
investigated. Every sample was Current and accepted as the catalogue
expects, but the judge reported `SAIL7107` (differing M6/M7 full outcomes)
for all ten pairs. A comparison of the raw documents found equal exit
codes, per-function verdicts (331 and 3,127 functions) and diagnostics in
every pair: the outcome hash reads the per-function `tier`, which output
schema v2 renamed `evidence_tier`, so the v1 (M6) and v2 (M7) hashes always
differ (finding 13).

### rc.5 findings

13. **The repair benchmark's outcome check fails on schema v2.** Above.
    Harness only; no verdict differs. Medium: the benchmark's acceptance
    gate cannot pass until it compares outcomes across schema versions.
    Issue #179.
14. **Reports omit the release suffix.** Every report's `sail_version` is
    `0.1.0`, where `sail --version` prints `0.1.0-rc.5`
    (`CARGO_PKG_VERSION` rather than `SAIL_VERSION`). Low. Issue #180.
15. **Some workspaces get their own dependency-store entry.** The #121
    workspace's first run on each machine verified vstd although the store
    held an entry for the same vstd; it then stored and reused its own.
    Correct but slower once per such workspace. The store's key includes
    the versions of vstd's dependencies in `Cargo.lock` (README Limits),
    and this workspace's lock differs from a new project's; which input
    differed was not checked (see also #177). Low.

16. **Fresh dependency-store seeding omits vstd's stdout summary.** Later
    reproduced on the exact draft rc.5 macOS asset; issue #184. The original
    second-project install check did not compare stdout. A separate
    post-rc.5 correction is being qualified; the draft asset remains affected.
    See the [correction record](dependency-store-output.md).

Finding 11 (`deep-rvt` output order, issue #124) is unchanged: this round
had 0 Different and 4 Unstable `output_json` tests. Open from earlier
candidates: 5 and 9 (low).

### rc.5 evidence

Raw logs outside the repository, in `~/sail-raw-evidence/l56/` on the
lead's machine: `l56-rc5.tar.gz` (sha256
`390340d5f2a7f0b2fc6bc9adbaba4fe029cf75708d81ecaa76dc4a359715378a`) holds
the release log, every host's logs and conformance reports, both repair
benchmark journals and raw outputs, the performance runs for
[performance](../performance.md) (`l56p`, `l56q`), the Linux smoke and
walkthrough logs and files, the macOS host, managed e2e, smoke and
walkthrough logs and files, the asset checksum files and the harness
scripts. For the vest edit loops it holds the script's output (every
sample's time and exit code, and the table), not the per-command logs,
which a packing error left on the host before it was terminated.

Attempts:
- `c3687990`, 2026-10-16, `ci/run.sh e2e managed` on macOS: failed before
  any check (daemon socket path too long under the default `TMPDIR`; test
  environment); passed with `TMPDIR=/tmp`.
- `c3687990`, 2026-10-16, Linux walkthrough: the container has no
  `python3`, so the MCP client and the reproduction summaries did not run
  in the first pass; rerun in the same container after installing
  `python3` (outside the README steps).

## v0.1.0-rc.4

A short revalidation of the change since rc.3: #125 (issue #123, rc.3's
finding 12), which keeps vstd's report after a failing first managed run
(`crates/sail-verus/src/managed/runner.rs`, with a new case in
`ci/test-managed.py`), plus documentation (#126 and the README and
known-issues entries of #125). The Verus pin, both runtimes and the CI image
are unchanged. Everything below ran on `5d99671e`, 2026-10-15, lane L50.

| Check | Environment | Result |
|---|---|---|
| `scripts/release.sh 0.1.0-rc.4` | Linux half on a native host (lane `l50-rel`, c7i.4xlarge, torn down by the script); macOS half on the developer Mac | pass: draft `v0.1.0-rc.4` targeting `5d99671e`; both self-checks selected and validated the bundled runtime (macOS manifest `a42a7d19…b5c8`); `sail --version` prints `sail 0.1.0-rc.4` from each archive |
| `ci/local.sh gate` | c7i.8xlarge `l50`, image `sail-ci:3c02e445e25df766` | pass: 567 passed, 35 skipped; fmt, clippy, doctests, rustdoc, links, layering, deny, benchmarks page |
| `ci/local.sh solver` | `l50` | pass: 35 passed |
| `ci/local.sh e2e` (legacy, managed, repair) | c7i.8xlarge `l50b` | pass: identical to `cargo verus verify` (exit 101; 20,236 cache entries); managed acceptance, now including `failing first runs keep vstd`; `claude-code: listed 15 sail tools on protocol 2026-07-28`; real repair CLI/MCP acceptance |
| `ci/local.sh conformance fast pr` | c7i.8xlarge `l50c` | 33 / 33 / 0 / 0 / 0 in pass-through, fill and warm (311 functions, 3,442 entries); engine 33 same, 32 focused (`4f0d0d3f4315`) |
| `ci/run.sh host` | developer Mac (Apple M3, macOS 26.5, shared) | pass: clippy clean; 556 passed, 35 skipped |
| `scripts/release-smoke.sh`, Linux archive | fresh c7i.8xlarge `l50d`, no Sail checkout and no Verus; rustup and Rust 1.98.1 only | pass: bundled runtime selected; `sail verify` passed (2,060 vstd and 2 demo verified); `repair-check demo::max` accepted |
| `scripts/release-smoke.sh`, macOS archive | developer Mac, fresh directory, empty `SAIL_CACHE_DIR`, no `cargo-verus` on `PATH` (functional check only, not clean-macOS evidence) | pass, same checks |
| README install, first use and a failing first managed run, Linux | fresh `ubuntu:24.04` container (24.04.5) on `l50d` (below) | pass |
| The same, macOS | developer Mac, the macOS archive in a fresh directory, empty cache | pass |

Carried over from rc.3 unchanged, because #125 changes only how a managed
run records a failing first run's vstd report, which those runs do not
exercise beyond the managed acceptance above: the standard and deep tiers
(0 Different in every arm), `deep-rvt` (not clean, finding 11; issue #124),
the repair benchmark and the clean-macOS record (rc.1).

| Asset | Size | sha256 |
|---|---:|---|
| `sail-0.1.0-rc.4-x86_64-unknown-linux-gnu.tar.gz` | 42.0 MB (42,022,382 B) | `eaa95fd796c48672ed3fe05f2485072587e79c70f008a38e34be83504b8257e9` |
| `sail-0.1.0-rc.4-aarch64-apple-darwin.tar.gz` | 35.0 MB (35,015,792 B) | `a6f800671635c12b3cebd03659cfd7586a7c286d1022c1ac41f040c20cf953e9` |

### Failing first managed run (finding 12)

Both platforms followed README Install and First verification verbatim (on
Linux in a new container as a new non-root user `walker` with only `sudo`
added; the draft's assets, which cannot be downloaded anonymously, were
copied in instead of fetched with `curl`). `shasum -a 256 -c`: `OK`; rustup
and Rust 1.98.1 in 20 s; `sail engine status` selected the bundled runtime;
no `cargo-verus` on `PATH`. Then the proof was broken (`max` returns the
smaller value), and the project's first and second managed runs, one MCP
`verify` (a stdio client, protocol `2025-06-18`, 15 tools), the fix, and
plain `sail verify` followed. Seconds, one sample each:

| Step | Linux | macOS | Summary line |
|---|---:|---:|---|
| `sail verify` (README, passes; vstd verified) | 33.0 | 54.6 | 2,060 vstd and 1 demo verified |
| broken, `sail verify --incremental`, first | 31.0 | 61.3 | `0 verified, 1 failed`; `2 verifier runs, 0 target reports reused` |
| broken, `sail verify --incremental`, second | 0.7 | 1.4 | `0 verified, 1 failed`; `1 verifier runs, 1 target reports reused` |
| broken, MCP `verify` | 0.9 | 1.2 | `demo::max` failed; vstd 1,871 verified, not shown |
| fixed, `sail verify --incremental` | 0.9 | 1.8 | `1 verified`; `1 verifier runs, 1 target reports reused` |
| unchanged, `sail verify --incremental` | 0.3 | 0.7 | `0 verifier runs, 2 target reports reused` |
| fixed, `sail verify` | 0.1 | 0.4 | exit 0 |

The first managed run verifies vstd, as before; every later run reuses it,
where rc.3 verified it again on each run until one passed (21–27 s). The
macOS times are on the shared developer Mac and are indicative only.

### rc.4 findings

None new. Finding 12 is fixed (above). Open from earlier candidates: 5 and
9 (low), and 11 (`deep-rvt`, issue #124), unchanged.

### rc.4 evidence

Raw logs outside the repository, in `~/sail-raw-evidence/l50/` on the
lead's machine: `l50-rc4.tar.gz` (sha256
`8c8acc402c0a40bc164c4a5cdf11fc9ec7956a7343de3a8f844f7e3c6eb6b944`) holds
the release log, the gate, solver, e2e and conformance logs and reports,
the Linux smoke and walkthrough logs, the macOS host and smoke logs, the
asset checksum files and the harness scripts.

No rc.4 attempt failed or was interrupted.

## v0.1.0-rc.3

`main` changed materially after rc.2: Sail runs its own Verus, built from
the fork, for every verification (#117, ADR 0043), pinned to fork `73dea021`
(#122), which both archives bundle; the README no longer installs stock
Verus. Everything below ran on `8bd4aed2`, 2026-10-15, lane L45. The owner
approved sharding the long runs across hosts.

| Check | Environment | Result |
|---|---|---|
| `scripts/release.sh 0.1.0-rc.3` | Linux half on a native host (lane `l45-rel`, c7i.4xlarge, torn down by the script); macOS half on the developer Mac | pass: draft `v0.1.0-rc.3` targeting `8bd4aed2`; both self-checks selected and validated the bundled runtime; `sail --version` prints `sail 0.1.0-rc.3` from each archive |
| `ci/local.sh gate` | `l45` | pass: 567 passed, 35 skipped; fmt, clippy, doctests, rustdoc, links, layering, deny, benchmarks page |
| `ci/local.sh solver` | `l45` | pass: 35 passed |
| `ci/local.sh e2e` (legacy, managed, repair) | `l45` | pass: `sail verify` on Sail's Verus identical to stock `cargo verus verify` (exit 101; 20,236 cache entries); managed acceptance; `claude-code: listed 15 sail tools on protocol 2026-07-28`; real repair CLI/MCP acceptance |
| `ci/local.sh conformance fast pr` | `l45` | 33 / 33 / 0 / 0 / 0 in pass-through, fill and warm (311 functions, 3,442 entries); engine 33 same, 32 focused (`4f0d0d3f4315`) |
| standard tier, pass-through and engine | `l45b` | 159 / 159 / 0 / 0 / 0 in both; 12,726 functions; 158 focused; 0 units with rlimit drift |
| standard tier, fill and warm | `l45c`, its own daemon on a fresh cache | 159 / 159 / 0 / 0 / 0 in both; 144,933 entries |
| deep tier, every arm | four hosts (below) | 159 / 159 / 0 / 0 / 0 in pass-through, fill, warm and engine (150 + 9 units; 9,404 + 3,322 = 12,726 functions; 149 + 9 = 158 focused) |
| `deep-rvt cache` | four full runs (below) | **not clean**: 4,695 tests (58 ignored); both sides 4,635 ok and the 2 source-build `cargo::cargo_new_*` FAILED in every round. Pass-through: 0 Different in two runs, 1 in two. Cache fill: 1 Different in each of three runs. Cache warm (one run): 1 Different. Every Different and Unstable is an `output_json` test (finding 11) |
| repair benchmark, one round per catalogue | `l45g` (HumanEval), `l45n` (Vest) | no regression (below) |
| `ci/run.sh host` | developer Mac | pass: clippy clean; 556 passed, 35 skipped |
| `scripts/release-smoke.sh`, Linux archive | fresh c7i.8xlarge `l45h`, no Sail checkout and no Verus; rustup and Rust 1.98.1 only | pass: bundled runtime selected; `sail verify` passed (2,060 vstd and 2 demo verified); `repair-check demo::max` accepted |
| `scripts/release-smoke.sh`, macOS archive | developer Mac, fresh directory, empty `SAIL_CACHE_DIR`, no `cargo-verus` on `PATH` (functional check only, not clean-macOS evidence) | pass, same checks |
| README install and first use, Linux | fresh `ubuntu:24.04` container on `l45h` (below) | pass with the fixed README; failed as written (finding 10) |
| README install and first use, macOS | developer Mac, the macOS archive in a fresh directory, empty cache | pass with the fixed README: `sail verify`, `--incremental` (28 s first, 0 s unchanged), MCP `verify` (15 tools; 0.6–1.1 s replies), `repair-check` accepted |

The fork-versus-stock arm of `deep-rvt` was not built: `deep-rvt` compares
the stock source build with and without `sail-z3`, as ADR 0043 keeps it.
The fork's own full `rust_verify_test` pass (4,649 passed, 0 failed, 58
ignored; [determinism record](determinism.md#fixes)) is the evidence for the
fork commit. Clean macOS was not rerun.

| Asset | Size | sha256 |
|---|---:|---|
| `sail-0.1.0-rc.3-x86_64-unknown-linux-gnu.tar.gz` | 42.0 MB (42,021,230 B) | `4e8a6af164f6c473691d3c81177e04105d66e17a75463fb659b946554eaaeab1` |
| `sail-0.1.0-rc.3-aarch64-apple-darwin.tar.gz` | 35.0 MB (35,015,281 B) | `051652e09d21fd77124a3100e1aa707675933068d10df3c97d326b3f05e320a0` |

The archives hold no README, so the README fix (finding 10) needs no
rebuild.

### Shard layout

Each host had its own cache, daemon and output directory and ran the same
commands as `ci/run.sh` with its `--jobs` value. The deep tier was split
into two project groups, each run in every arm:
A (`verified-node-replication`, `verified-ironkv`, `vest`,
`vest-generated`, `verus-examples`, `vstd`; 150 units) on `l45d`
(pass-through, engine) and `l45e` (fill, warm), and B (`human-eval-verus`,
`verified-storage`, `verismo`, `verismo-main`, `page-table`, `anvil`; 9
units) on `l45i` and `l45j`. Together they are the deep tier's 159 units;
its two Singular projects are not runnable in the image, as for rc.1.
Group A's pass-through and fill arms showed the usual rlimit drift with no
difference (VNR `log_entry_alive_value_wrap_around`, 51,948 against 51,945).

`deep-rvt` was meant to run as four module shards, but a harness error
dropped the shard filters, so `l45`, `l45f`, `l45k` and `l45l` each ran
`ci/local.sh deep-rvt cache` over the full suite. `ci/run.sh` stops after a
round with a Different, so the warm round ran only as a separate fill-then-warm
pass on `l45f` (`rvt-cache.sh`, the same commands).

| Host | Pass-through | Cache fill | Cache warm |
|---|---|---|---|
| `l45f` | 4,693 same, 1 Unstable, 1 Different | 4,693 same, 1 Unstable, 1 Different | 4,691 same, 3 Unstable, 1 Different |
| `l45` | 4,691 same, 4 Unstable, 0 Different | 4,693 same, 1 Unstable, 1 Different | not run (stopped) |
| `l45k` | 4,692 same, 2 Unstable, 1 Different | not run (stopped) | not run |
| `l45l` | 4,691 same, 4 Unstable, 0 Different | 4,692 same, 2 Unstable, 1 Different | not run (stopped) |

### Repair benchmark

`sail-bench repair` per the [acceptance guide](../bench/m7-repair-acceptance.md),
M6 from `rescue/m6-baseline` (`26f124f`), M7 at `8bd4aed2` with the
`73dea021` runtime, one round per catalogue on its own c7i.8xlarge, as a
regression check against the frozen figures measured on `13a1176c`
(`repair-acceptance-l22-final`). Every sample was valid, Current and
matched the catalogue; no sample was more than 15% slower than recorded, so
no three-round run followed.

| Catalogue | M6 whole loop | M7 whole loop | Speedup | Recorded (M6 / M7 / speedup) | M7 first actionable |
|---|---:|---:|---:|---|---|
| HumanEval | 64.6 s | 29.6 s | 2.18× | 64.7 s / 30.3 s / 2.14× | 3.47 s (recorded 3.46–3.57 s) |
| Vest | 232.5 s | 129.4 s | 1.80× | 246.6 s / 135.1 s / 1.82× | 12.6–12.9 s (recorded 13.4–13.7 s) |

Every M7 cycle was 1–7% faster than its recorded median: VI-0007's fix, which holds a
module's diagnostics until lower-numbered modules finish, did not slow the
repair loop or its first actionable evidence.

### README install and first use (Linux)

A fresh `ubuntu:24.04` container with a new user `walker` and only `sudo`
added, following README Install verbatim (Prerequisites, 1. Sail, 2. Rust).
The draft's assets, which cannot be downloaded anonymously, were copied in
instead of fetched with `curl`. No Sail checkout and no `cargo-verus`.

- `apt-get update` and `install build-essential curl`; `shasum -a 256 -c`:
  `OK` (`shasum` came with the prerequisites' perl); `sail --version`:
  `sail 0.1.0-rc.3`; rustup and Rust 1.98.1 in 18 s; `sail engine status`
  selected the bundled runtime.
- **First verification as written failed** (finding 10): `cargo new --lib`'s
  `src/lib.rs` has no `use vstd::prelude::*;`, so `sail verify` stopped with
  `The verus_builtin crate was not imported` (exit 101). The same on macOS.
- With the fixed README, verbatim: `sail verify` passed in 29 s (2,060 vstd,
  1 demo verified), 0 s again. A failing edit: `sail verify` exit 101 in 1 s;
  `sail verify --incremental` reported `0 verified, 1 failed` in 27 s; the
  fix, `--incremental`, `1 verified` in 27 s (finding 12); unchanged,
  `--incremental --output json` in 1 s; `sail repair-check --package demo
  --lib --function demo::max --engine interactive`: `repair accepted; full
  evidence Current` in 28 s.

### rc.3 findings

10. **README First verification fails as written.** *Fixed* (#126).
    `cargo new --lib` plus the two `Cargo.toml` entries and `sail verify`
    fails to compile, because the generated `src/lib.rs` is not Verus code.
    The README now writes a minimal `src/lib.rs`. Before, every new user's
    first command failed.
11. **`deep-rvt` reports VI-0008 variants as Different.** In the `output_json`
    tests, stock Verus prints `func-details` in hash order. Eleven plain runs
    gave 3 to 6 distinct stderr orders (VI-0008 recorded 4), so a Sail-side
    run often shows an order none of them showed. In every Different, stdout
    is byte-identical, both sides are `ok`, and the 12 stderr files sort to
    the same lines. This arm runs the stock source build, which Sail no
    longer runs for users (ADR 0043); Sail's Verus fixes VI-0008
    (`bf59e1f0`). rc.1's three rounds had 0 Different. The check was not
    changed; the harness follow-up (set comparison of stderr as in ADR 0042,
    or `deep-rvt` on the fork) is issue #124. Not a blocker.
12. **Managed runs reverify vstd until a project's first passing run.** If a
    project's first `sail verify --incremental` fails verification, each
    later managed run verifies vstd again (21–27 s, "2 verifier runs, 0
    target reports reused") until one passes; after that, failing runs reuse
    it (0 s). Reproduced on macOS. Plain `sail verify` is unaffected. Medium:
    agents often start from failing code. Fixed after rc.3 by #125 (issue
    #123): the next run after a failing first run reuses vstd (28.6 s → 0.7 s).

Open from earlier candidates: 5 and 9 (low), unchanged.

### rc.3 evidence

Raw logs outside the repository, in `~/sail-raw-evidence/l45/` on the
lead's machine: `l45-rc3.tar.gz` (sha256 `0653bd797c7a266ce2f0dc0c04fdf8fc62460e437b1bb51cd73622ce98cf8c35`) holds the release
log, every host's logs and conformance reports, the macOS smoke, README and
host logs, and the harness scripts. The benchmark journals are separate:
`l45g.tgz` (HumanEval, `0cc2eb5eaf62fc61192219ff13a02add6b19290f613bf594eae62b857fe35a3d`) and `l45n.tgz` (Vest, `5e63b83e0c690f898db8c2602aa817282e046ddc2fc62b9a70ee22bf769c66ca`).

Attempts:
- `8bd4aed2`, 2026-10-15, `deep-rvt`, deep tier and repair benchmark,
  unsharded: stopped after about 15 minutes to reshard them for wall time
  (owner direction).
- `8bd4aed2`, 2026-10-15, `deep-rvt` module shards: the shard filter files
  never reached the hosts, so each shard ran the full suite (harness error;
  the runs are kept and counted above).
- `8bd4aed2`, 2026-10-15, README walkthrough (Linux) and the README part of
  the macOS smoke: failed at First verification (finding 10); rerun with the
  fixed README.

## v0.1.0-rc.2

A short revalidation after the fixes for rc.1's findings: #108 (findings 1,
4, 6 and 7), #109 (finding 2) and #110 (finding 3). Everything below ran on
`8f427e81`, 2026-10-14, lane L43. History: rc.2 ran stock Verus in default
mode; rc.3 superseded it and its draft is deleted.

| Check | Environment | Result |
|---|---|---|
| `scripts/release.sh 0.1.0-rc.2` | Linux half on a native host (lane `l43-rel`, c7i.4xlarge, torn down by the script); macOS half on the developer Mac | pass: draft `v0.1.0-rc.2` targeting `8f427e81`; both self-checks passed; `sail --version` prints `sail 0.1.0-rc.2` from each archive |
| `ci/local.sh gate` | c7i.8xlarge `l43`, image `sail-ci:14df87b176833bea` | pass: 565 passed, 35 skipped; fmt, clippy, doctests, rustdoc, layering, deny, benchmarks page |
| `ci/local.sh solver` | `l43` | pass: 35 passed |
| `ci/local.sh e2e` (legacy, managed, repair) | `l43` | pass, including `claude-code: listed 15 sail tools on protocol 2026-07-28` and the real repair CLI/MCP acceptance |
| `ci/local.sh conformance fast pr` | `l43` | 33 / 33 / 0 / 0 / 0 in pass-through, fill, warm and engine (32 focused); 311 functions |
| standard tier, fill and warm | `l43`, its own daemon on a fresh cache | 159 / 159 / 0 / 0 / 0 in both; 12,726 functions; 150,363 entries |
| standard tier, pass-through and engine | c7i.8xlarge `l43-eng`, its own cache | 159 / 159 / 0 / 0 / 0 in both; engine `5b866414…7603`, 158 focused |
| `ci/run.sh host` | developer Mac (Apple M3, macOS 26.5, shared) | pass: clippy clean; 554 passed, 35 skipped |
| `scripts/release-smoke.sh`, Linux archive | fresh c7i.8xlarge `l43-walk`; README Install step 2 only (rustup, Rust 1.98.1, the Verus release zip, no Sail checkout) | pass: bundled engine selected; `sail verify` passed (2,060 vstd and 2 demo verified); `repair-check demo::max` accepted |
| `scripts/release-smoke.sh`, macOS archive | developer Mac, fresh directory, empty `SAIL_CACHE_DIR`, stock Verus `426d8b0` (functional check only, not clean-macOS evidence) | pass, same checks |

The standard tier ran as two shards with the commands of `ci/run.sh
conformance standard` (`--jobs 3`), split by arm, each arm over the full unit
list. Its default arms show the same rlimit drift as rc.1 and no
difference: VNR `log_entry_alive_value_wrap_around`, 51,948 against 51,945.
Clean macOS was not rerun: the [rc.1 record](clean-macos.md) stands, since
the changes since rc.1 are not macOS-specific.

| Asset | Size | sha256 |
|---|---:|---|
| `sail-0.1.0-rc.2-x86_64-unknown-linux-gnu.tar.gz` | 42.0 MB (41,999,108 B) | `0b343f6f063eb47e92d1c01b360dd39d09b9f893dfa64cfb75082c37fdaa933e` |
| `sail-0.1.0-rc.2-aarch64-apple-darwin.tar.gz` | 35.0 MB (34,989,456 B) | `b6ccb41981c7969ad43d6cb5dd69c870d78fe9a81e1263547a3f0cd5ec4b7ff8` |

The Linux archive is 15.5 MB smaller than rc.1's, because the release
binaries no longer carry line tables (#108).

### Walkthrough recheck

A fresh `ubuntu:24.04` container (24.04.5) on `l43-walk`, with a new
non-root user and only `sudo` added. The image has no `curl`, `git`,
`python3`, C compiler or `shasum`. The commands are the README's, except
that the draft's assets, which cannot be downloaded anonymously, were
copied in instead of fetched with `curl`.

- **(a) Install from the archive: works end to end, no Sail checkout.**
  `shasum` is absent, so `sha256sum -c` per the README note: `OK`. Unpack,
  `bin/` on `PATH`, `sail --version` prints `sail 0.1.0-rc.2`. Step 2: apt
  prerequisites, rustup and Rust 1.98.1 (40 s), the Verus release zip, whose
  sha256 matches the README (11 s), `cargo verus --help`. `sail engine
  status` selected the bundled engine. A new `cargo verus new --lib` project
  then verified with `sail verify` in 29.9 s (vstd included). Two small gaps
  in a minimal image (finding 8).
- **(b) Compile error in vest: fixed.** vest `11af01dd`, `-p vest_lib`,
  with its vstd pin set to the published `0.0.0-2026-10-04-0306` for the
  installed Verus. Seconds, one sample each; the compile error is a type
  error in an appended function, and the fix replaces it with a verified
  function:

  | vest | cold | warm | edit | compile error | again | fix | warm |
  |---|---:|---:|---:|---:|---:|---:|---:|
  | `cargo verus verify` | 64.9 | 0.20 | 34.5 | 5.6 | 5.6 | 34.6 | 0.20 |
  | `sail verify --incremental` | 65.4 | 1.64 | 36.3 | 7.1 | 7.0 | 24.9 | 1.61 |
  | rc.1, `--incremental` | | | | 37.9 | 37.9 | 58.3 | 1.5 |

  Each compile-error run reports `1 verifier runs, 1 target reports reused`
  (vstd is reused, not verified again), and so does the fixed run.
- **(c) MCP `compare_verification`: fixed.** A scripted stdio client
  (`2025-06-18`) in the vest workspace: `verify` unchanged (2.1 s, reused),
  a failing edit to the appended function (20.6 s, `0 verified, 3128 passed
  by Verus's totals`), `save_baseline`, the fix (25.8 s), then
  `compare_verification` against the baseline: `Compatible comparison: 1
  recovered, 3016 unchanged.` in a 2,233-byte reply (rc.1: `95
  incomparable, 2921 recovered`, 1.26 MB). Its notes name the 3,128
  functions counted by Verus's totals and the 95 repeated names compared as
  groups. `detail: "full"`, which lists every function, is 1.36 MB
  ([comparisons](../mcp.md#comparisons)).
- **(d) `sail --version` and `sail bench`: as documented.** `sail
  --version` prints `sail 0.1.0-rc.2`. `sail --help` no longer lists
  `bench`. `sail bench`, `sail bench --help` and `sail bench cold` exit 2
  with `SAIL0003`, saying it is a developer tool not in the release
  archives and how to build it ([errors](../errors.md)). `--help` exits 0
  for `verify`, `watch`, `jobs`, `mcp`, `cache`, `engine` and
  `repair-check`.

### rc.1 findings

| # | Finding | Status in rc.2 |
|---|---|---|
| 1 | No archive install path in the README | **Fixed** (#108): (a) installs Sail and Verus from the README with no checkout |
| 2 | A compile error makes each managed run verify vstd again | **Fixed** (#109): 7.1 s against plain's 5.6 s (was 37.9 s); the fixed run 24.9 s (was 58.3 s) |
| 3 | `compare_verification` misleading after incomplete attribution | **Fixed** (#110): `1 recovered, 3016 unchanged`, 2.2 KB (was 1.26 MB) |
| 4 | Stale release table in the roadmap | **Fixed** (#108) |
| 5 | `0 verified, … passed by Verus's totals` reads as zero first | Open, low; unchanged |
| 6 | `sail bench` listed but not shipped | **Fixed** (#108): unlisted; a clear `SAIL0003` |
| 7 | `sail --version` cannot identify an RC; debug info shipped | **Fixed** (#108): `sail 0.1.0-rc.2`; line tables stripped |

New, low:

8. **README Install on a minimal image.** Step 1 downloads with `curl`,
   which only step 2 installs. Docker's `ubuntu:24.04` also ships without
   package lists, so step 2's `apt-get install` needs `apt-get update`
   first, which the README does not say. Desktop and cloud Ubuntu installs
   have both curl and package lists.
9. **Managed summary of a compile error.** `sail verify --incremental`
   prints `Current evidence: 0 verified, 0 failed, 0 unknown` after Cargo's
   `could not compile`. The exit code and the Cargo error match plain Verus;
   the count line says nothing about the crate that did not compile.

### rc.2 evidence

Raw logs outside the repository, in `~/sail-raw-evidence/l43/` on the
lead's machine (sha256):

| File | Contents | sha256 |
|---|---|---|
| `release-rc2.log` | `scripts/release.sh 0.1.0-rc.2` | `219b4748f70b7ee59c076c9b5dc4ff8329286efa4cee9a63e0626cd8113ce6db` |
| `l43-main.tgz` | gate, solver, e2e, fast PR, standard fill and warm: logs and reports | `c1773c5c7fb0639a7f6740feaf9d66d6dfd2d4ef96da8e547f2065f54516b605` |
| `l43-eng.tgz` | standard pass-through and engine: log and reports | `4b8fb27c8c0523a4040de986dd8ad69cf2647fd76b543ae3926e7e5806ab3a49` |
| `mac-host.log` | `ci/run.sh host` | `48803c52dd716d5d5b9df9b31e73767c82ad676ecc7fe8f31d663856c5cfb075` |
| `linux-smoke-rc2.log` | Linux smoke | `a3fe57084caf03bf6592cf8bb4cca20de55456b211e2bb7300b2493db2d72249` |
| `mac-smoke-rc2.log` | macOS smoke on the developer Mac | `b25f0ce78e175b0bd0c5b5c292c54541f7cc8491ead51a5ffba463424da79188` |
| `walk-0-setup.log`, `walk-a-install.log` | container setup; (a) | `13b02cdad3461331439b515959ccf7fc453bd80770a129cd34f5508346d61702`, `e951032755538874418750c4be767355a5dc05f6a54793b94f449af362201554` |
| `walk-b-vest.log` | (b) | `061f503020a9aef9e1a9c9317313ef78ceb754aa574652dc6e0f6a5f83c17473` |
| `walk-c-mcp.log` | (c) | `c101208f5f168957e0046737b09986d4efe3970396b4eafade301af47cd9b73f` |
| `walk-d-version-bench.log` | (d) | `e21c92a59cba944de911381f182e26c22e184d88770c5a05393247fca9113c55` |
| `walk-outputs.tgz` | per-step stdout and stderr for (b); MCP frames for (c) | `e71edef82f57bada9036e2fc9e7bbbe9e64c9f2ea81622657bdec6e55860cae9` |
| `l43-harness.tgz` | the scripts that ran the shards and the walkthrough | `f7c7c6c32a37d4e33d8cbeeaca6ffee9e574003b90e0e40a223072af5840b37a` |

No rc.2 attempt failed or was interrupted.

## v0.1.0-rc.1

The rest of this record is rc.1's validation.

## Suites

All suites ran on `7782b136`. The commits between it and the release
candidate are listed under [release build](#release-build); they change
versions, docs, benchmark scripts and a CI marker check, not code that the
suites exercise.

| Suite | Result |
|---|---|
| `ci/local.sh gate` | pass: 559 tests passed, 35 skipped; fmt, clippy, doctests, rustdoc, layering, deny, benchmarks page |
| `ci/local.sh solver` | pass: 35 passed |
| `ci/local.sh e2e` (legacy, managed, repair) | pass: identical to `cargo verus verify`; managed acceptance passed; `claude-code: listed 15 sail tools on protocol 2026-07-28`; real repair CLI/MCP acceptance passed |
| `ci/run.sh host` (Mac) | pass: clippy clean; 548 tests passed, 35 skipped |

Conformance summary lines (units / Same / Unstable / Different / Infra; functions):

| Tier | Pass-through | Cache fill | Cache warm | Engine |
|---|---|---|---|---|
| fast PR subset | 33 / 33 / 0 / 0 / 0; 311 | 33 / 33 / 0 / 0 / 0 | 33 / 33 / 0 / 0 / 0 | 33 / 33 / 0 / 0 / 0; 32 focused |
| fast | 147 / 147 / 0 / 0 / 0; 3,450 | 147 / 147 / 0 / 0 / 0 | 147 / 147 / 0 / 0 / 0 | 147 / 147 / 0 / 0 / 0; 146 focused |
| standard | 159 / 159 / 0 / 0 / 0; 12,726 | 159 / 159 / 0 / 0 / 0 | 159 / 159 / 0 / 0 / 0 (151,393 entries) | 159 / 159 / 0 / 0 / 0; 158 focused |
| deep (Singular projects skipped, no Singular in the image) | 159 / 159 / 0 / 0 / 0; 12,726 | 159 / 159 / 0 / 0 / 0 | 159 / 159 / 0 / 0 / 0 (150,368 entries) | 159 / 159 / 0 / 0 / 0; 158 focused |
| Singular projects (`ci/local-singular.sh`, exit 0) | 3 / 3 / 0 / 0 / 0; 606 | 3 / 3 / 0 / 0 / 0 | 3 / 3 / 0 / 0 / 0 (8,080 entries) | 3 units `engine-lacks-feature` (`singular`), 0 Same, 0 Different |

The standard tier's default arms had one unit with rlimit drift and no
difference, as in earlier runs: VNR `log_entry_alive_value_wrap_around`,
51,948 against 51,945.

`ci/local.sh deep-rvt cache` (4,695 tests, 58 ignored; plain 4,635 ok and
the 2 source-build `cargo::cargo_new_*` FAILED in every mode, ADR 0019):

| Mode | Same | Unstable | Different | Infra | Wall s |
|---|---:|---:|---:|---:|---:|
| Pass-through | 4,690 | 5 | 0 | 0 | 1,131 |
| Cache fill | 4,693 | 2 | 0 | 0 | 1,113 |
| Cache warm | 4,694 | 1 | 0 | 0 | 1,082 |

Every Unstable is an `output_json` test (`test_json_custom_err_on_ensures`,
`…_custom_err_on_requires`, `…_proof_note_on_assume_and_req_with_no_cheating`,
`…_proof_note_on_requires`, `…_proof_note_on_ensures`). For each one the
report records `plain_varies: true` and `matches_a_plain_run: true`: the plain
runs disagreed among themselves on the order of `func-details` keys
([VI-0008](../verus-issues/VI-0008-func-details-hash-order.md)), and Sail's
output matched one of them (ADR 0009). Both sides `ok`.

### Shard layout

The owner approved splitting the long runs across hosts. Each shard had its
own host, cache, daemon and output directory, ran `7782b136` in
`sail-ci:14df87b176833bea` with the `--jobs` value `ci/run.sh` uses, and
covered whole arms, so every arm ran its tier's full unit list on one host.
The unit IDs of the four standard arms are identical (159), and so are those
of the four deep arms (159), with no duplicates.

| Host | Scope | Result |
|---|---|---|
| `l38` | gate, solver, e2e, conformance fast PR and fast (all arms), standard pass-through, fill and warm | pass. Its serial standard engine arm was stopped once warm finished (exit 137 by design), because `l38-sing` ran that arm |
| `l38-deep` | deep pass-through, fill and warm | pass. Its serial engine arm was stopped the same way |
| `l38-rvt` | `deep-rvt cache` | pass |
| `l38-sing` | `ci/local-singular.sh`, then the standard engine arm (`sail-conformance engine --tier standard --jobs 3`) | pass |
| `l38-s1` | deep engine arm (`sail-conformance engine --tier deep --jobs 3`) | pass |

## Release build

`scripts/release.sh 0.1.0-rc.1` built the **draft** release `v0.1.0-rc.1`
from `958dee95`. The Linux half ran on a native host (lane `l38-rel`, which
the script tore down). Both staged trees passed the script's self-check.
`sail --version` prints `sail 0.1.0`. The archives carry debug info
(`optimized + debuginfo`):

| Asset | Size | sha256 |
|---|---:|---|
| `sail-0.1.0-rc.1-x86_64-unknown-linux-gnu.tar.gz` | 57.5 MB | `fbc547bc300aa8ce35c79bc98acf81221dfbe8eb973f8eb2cbbfec8b5bc80fe3` |
| `sail-0.1.0-rc.1-aarch64-apple-darwin.tar.gz` | 35.3 MB | `d6fe42d4ce0e21650a74769672eea3e01bce9ad6a36059afe77d09bc1497fd66` |

Three commits separate the validated `7782b136` from `958dee95`:
- #104, `ci:`: `scripts/check-ai-markers.sh` rejects hostname-fallback commit
  identities;
- #105, `docs:`: `docs/performance.md`, the README's speed wording, and the
  `scripts/perf-loops.sh` and `scripts/perf-agents.sh` benchmark scripts;
- #106, `chore:`: the workspace version `0.0.0` to `0.1.0` (`Cargo.toml`,
  `Cargo.lock`, `CHANGELOG.md`), and a test-only change in
  `crates/sail-cli/src/report.rs`, so the golden report no longer depends on
  the version.

The diff between the two commits changes no product code, so the suites
above were not rerun. The owner sets the release version at publish time.

An earlier build, the draft `v0.0.0-rc.1` from `7782b136`, was made before
the version bump. Its Linux tarball (`27e4077b…18b1`) passed the same smoke
test on a fresh host (`l38-smoke`) and is the tarball the walkthrough
installed. That draft and the older `v0.0.0-preview.3` have been deleted.

## Smoke tests

`scripts/release-smoke.sh` with an empty `SAIL_CACHE_DIR`:

| Archive | Where | Result |
|---|---|---|
| `v0.1.0-rc.1` Linux (`fbc547bc…0fe3`) | fresh c7i.8xlarge (`l38-smoke2`), README steps 1, 2 and 4 only (apt prerequisites, rustup, `cargo xtask fetch-verus`) | pass: `sail 0.1.0`; bundled engine selected; `sail verify` passed; `repair-check demo::max` accepted through the bundled engine |
| `v0.0.0-rc.1` Linux (`27e4077b…18b1`, deleted draft) | fresh c7i.8xlarge (`l38-smoke`), README steps 1, 2 and 4 only (apt prerequisites, rustup, `cargo xtask fetch-verus`) | pass: bundled engine selected; `sail verify` passed (2,060 vstd and 2 demo verified); `repair-check demo::max` accepted through the bundled engine |

The macOS tarball's smoke test belongs to L39 (below).

## Linux walkthrough

The previous walkthrough repeated on a fresh c7i.8xlarge (`l38-walk`),
following only `README.md` and the guides it links to, with `main` at
`e1547b0` for the docs (it includes the [performance page](../performance.md),
#105). Install used the Linux tarball from the `v0.0.0-rc.1` draft, because
rc.1 had not been built yet. Its binaries and layout match rc.1, except that it
prints version `0.0.0`.

**Install.** apt prerequisites, rustup, then the tarball: `sha256sum -c`,
unpack under `~/.local/opt`, symlink `bin/sail` and `bin/sail-z3` into
`~/.local/bin`. Symlinks work: `sail engine status` resolves them and selects
the bundled engine. The README does not document this path (finding 1).
`cargo xtask fetch-verus` needs a Sail checkout. It downloaded the pinned
Rust 1.99 for xtask, installed Rust 1.98.1 for Verus, and printed the `PATH`
line, in 49 s.

**Demo** (`cargo verus new --lib`, five functions; one appended function
for the edits). Seconds, one sample each:

| demo | cold | warm | edit | fail | fix | warm |
|---|---:|---:|---:|---:|---:|---:|
| `cargo verus verify` | 33.8 | 0.14 | 0.85 | 0.63 | 0.86 | 0.13 |
| `sail verify` | 34.2 | 0.13 | 0.88 | 0.63 | 0.88 | 0.13 |
| `sail verify --incremental` | 31.9 | 0.35 | 1.16 | 0.87 | 1.09 | 0.36 |

**vest** (`11af01dd`, `-p vest_lib`; the edits append a function to
`vest_lib/src/lib.rs`), starting with an empty Sail cache and no managed
targets:

| vest | cold | warm | edit | fail | fix | warm |
|---|---:|---:|---:|---:|---:|---:|
| `cargo verus verify` | 72.3 | 0.23 | 39.7 | 35.3 | 39.8 | 0.25 |
| `sail verify` | 73.1 | 0.25 | 37.6 | 20.2 | 24.5 | 0.23 |
| `sail verify --incremental` | 58.4¹ | 1.48 | 28.1 | 22.3 | 27.7 | 1.47 |

¹ Run after `sail verify`, so its solver cache was warm.

`sail verify` matched `cargo verus verify` in stdout, stderr and exit code
at all 12 steps (after normalizing the directory name, `Finished … in` times
and the order of parallel `Compiling` lines). The numbers agree with the
README and the [performance page](../performance.md): cold and small runs are
the same as plain Verus. The first edit after cold gains little (37.6 s
against 39.7 s). Later vest edits gain 1.6× (24.5 s against 39.8 s).

The managed summary of the failing vest edit now reads `0 verified, 1
failed, 3127 passed by Verus's totals (not attributed one by one), 0 unknown
(functions; Verus counts verification units: 3237 verified, 1 errors)`.

**MCP.** `claude mcp add --transport stdio sail -- sail mcp` in the project,
then `claude mcp list` (Claude Code 2.1.299 on the host): `sail: sail mcp - ✔
Connected`. Then the real Claude Code CLI (2.1.299, `claude -p`, stream-JSON
transcript), with `sail mcp` on the Linux host over ssh stdio, ran a fresh
demo with a bug in `max`. It verified (FAILED, counterexample `a = 1, b = 0`),
fixed the swapped branches without touching the specification, and verified
again (VERIFIED), in 6 turns and 79 s. It reported both summary first lines
correctly. Tool replies seen by the model were 3.7 KB and 2.4 KB.

Reply sizes from a minimal stdio client (single JSON-RPC frames):

| Reply | demo | vest |
|---|---:|---:|
| `tools/list` | 17.8 KB | 17.8 KB |
| `verify`, unchanged (reused) | 2.9 KB, 0.56 s | 3.0 KB, 2.1 s |
| `verify`, failing edit | 3.8 KB, 1.2 s | 4.4 KB, 22.8 s |
| `counterexample` | 2.2 KB | 2.4 KB |
| `verification_result` | 4.6 KB | 5.5 KB |
| `explain_failure` | 3.1 KB | |
| `save_baseline` / `get_baseline` | 3.2 KB | 3.6 KB |
| `cached_result` | 2.3 KB | |
| `compare_verification` after the fix | 1.0 KB: `1 recovered, 4 unchanged` | **1.26 MB**: `95 incomparable, 2921 recovered` (finding 3) |

Counterexamples now show source variables only (`a = 1, b = 0, r = 1`;
`r = 0, x = 0`).

**`sail watch`.** An edit was queued about 0.45 s after the write and
finished 1.35 s after it; the fix finished 1.53 s after its write. The
lifecycle events (`queued`, `started`, `target-reused`, `target-started`,
`target-complete`, `finished`) carry the job's currentness and exit code.

**`sail cache projects`.** `list` showed 2 workspaces, 2.8 GiB (vest
2.5 GiB). `prune --all` and `prune --missing` worked.

**Repair engine from the tarball.** `sail engine status` selected the
bundled engine; `sail repair-check --package mcpdemo --lib --function
mcpdemo::max --engine interactive` was accepted with full evidence Current.
With the binaries copied away from `engine/`, `engine status` and
`repair-check` both fail fast with `SAIL3120` and say what to do.

**`--help`.** Every subcommand prints help and exits 0, including `watch`,
`cache gc`, `cache projects` and `verify` (Sail options first). Exception:
`sail bench --help` gives `error[SAIL0003]: cannot run sail-bench` (finding 6).

## Previous walkthrough: frictions

| # | Friction (2026-10-13) | Now |
|---|---|---|
| 1 | MCP unusable in Claude Code | **Fixed.** Connected, 15 tools; a real Claude Code loop verified, fixed and re-verified |
| 2 | MCP replies of 4–16 MB | **Mostly fixed.** 1–6 KB per reply, except `compare_verification` against a baseline with incomplete attribution: 1.26 MB on vest (finding 3) |
| 3 | No managed reuse on macOS | Not retested here (L39) |
| 4 | No documented Verus install | **Fixed for a source checkout.** `fetch-verus` installs the toolchain and prints the `PATH` line, and the README lists the prerequisites. Tarball users still need a checkout (finding 1) |
| 5 | "Much faster" holds only where the solver dominates | **Fixed in the docs.** The README and performance page say so, with numbers that match this run |
| 6 | Managed counts disagree with Verus | **Fixed** for `verify` (`passed by Verus's totals`, counting unit explained). Not for comparisons (finding 3) |
| 7 | A long cache path silently disables the daemon | **Fixed.** A 133-character `SAIL_CACHE_DIR` works; the socket moves under `/tmp/sail-<hash>/` |
| 8 | The repair engine cannot be obtained | **Fixed.** It is bundled in the tarball and accepted a repair; a missing engine fails fast with `SAIL3120` |
| 9 | Non-Cargo projects unsupported, undocumented | **Fixed in the docs** (README, Limits) |
| 10 | `--help` handling | **Fixed**, except `sail bench` in the release (finding 6) |
| 11 | Unbounded disk use | **Fixed.** `sail cache projects list|prune` |
| 12 | `jobs list` shows `Unknown` | **Fixed.** It shows `Current` and `Historical` |
| 13 | Reverting does not reuse | Not fixed; out of the release (B4) |
| 14 | vstd verified per project | Not fixed; out of the release (B1); documented in the README (about 30 s per new project) |
| 15 | User guides read as validation records | **Mostly fixed.** The README is a user guide. `docs/mcp.md` still mixes in validation wording (for example "does not establish a live-reader latency gain") |

| Doc error (2026-10-13) | Now |
|---|---|
| `workspace-verification.md` socket name | Fixed |
| `proof-repair.md` `get_verification_result` | Fixed |
| `proof-repair.md` `SAIL3120` | Fixed (observed) |
| README `daemon.log` location | Fixed (`results/daemon.log`) |
| README macOS "can require fresh proofs" | Reworded; macOS behavior is L39's to check |
| `mcp.md` Claude Code registration | Fixed (observed) |
| `mcp.md` `counterexample` summary example | Fixed (matches) |
| walkthrough `fetch-verus` "installs artifacts" | Fixed |
| `errors.md` `SAIL0101` | Fixed (points to `fetch-verus`) |
| README "Getting started" is contributor setup | Fixed |

## New findings, by user impact

As found on rc.1. Lanes L40 (findings 1, 4, 6 and 7), L41 (finding 2) and
L42 (finding 3) fixed them; their current status is in
[rc.1 findings](#rc1-findings) under rc.2.

1. **The release tarball has no install path in the README.** *Open, L40.*
   - README Install still says release binaries "will be published" and
     documents only the source build.
   - Step 4 installs Verus with `cargo xtask fetch-verus` "from the Sail
     checkout", which a tarball user doesn't have. `errors.md` `SAIL0101` and
     the `SAIL3120` message also point to a checkout. Following it means
     cloning Sail and building xtask, which also downloads Rust 1.99.
   - The README's repair section tells Linux users to fetch the engine with
     `python3 ci/engine-artifact.py` from a checkout. It also says the macOS
     runtime is not pinned yet. Both are stale: the tarballs bundle the
     engine, and `engine/verus/artifact-macos.json` pins the macOS runtime.

   *Fix:* a tarball install section (checksum, unpack, keep `bin/` and
   `engine/` together, symlink or `PATH`). Add a Verus install that needs no
   checkout, such as the Verus release zip plus `rustup toolchain install
   1.98.1`, or a `sail` subcommand. Say that the engine is bundled.
2. **After a Rust compile error, each managed run re-verifies vstd.** *Open, L41;
   agents hit compile errors often.* On vest, `sail verify --incremental`
   on a crate that fails to compile took 37.9 s on each of two runs ("2
   verifier runs, 0 target reports reused"), where plain Verus fails in 6.2 s.
   The first successful run after the fix took 58.3 s, against about 28 s for
   an ordinary edit (`walk-linux-repro-compile-error.log`). MCP `verify` and
   `watch` use the same path.
3. **`compare_verification` is misleading after a failing run with incomplete
   attribution.** *Open, L42. It undoes A2 and A3 for the compare-after-fix step
   that agents use.* After fixing the one failing
   vest function, comparing with the failing run's baseline reported `95
   incomparable, 2921 recovered` in a 1.26 MB reply. The baseline's 3,127
   "passed by Verus's totals" functions are `unknown`, so each one counts as
   `recovered`, and the totals do not add up to the 3,128 functions.
   `docs/mcp.md` says a vest comparison is 33 KB. On the demo, with complete
   attribution, the same step correctly gives `1 recovered, 4 unchanged`.
4. **`docs/ROADMAP.md`'s release table is stale.** Every merged item (A1–A6,
   release builds, public engine runtimes, the performance page, disk
   cleanup) still reads "Not started". *Open, L40.*
5. **Headline counts on a failing vest edit.** `0 verified, 1 failed, 3127
   passed by Verus's totals` is accurate and explained, but "0 verified"
   first is easy to misread. Low.
6. **`sail bench` is listed in `sail --help` but not shipped.** The release
   has no `sail-bench`, so `sail bench --help` fails with `SAIL0003`. Low;
   *open, L40.*
7. **`sail --version` prints only the workspace version** (`sail 0.1.0` for
   `v0.1.0-rc.1`), so it cannot identify an RC. The archives also ship debug
   info. Low; *open, L40.*

## Clean macOS

Lane L39 checked rc.1's macOS archive on a clean EC2 Mac (M4): the smoke
test, the walkthrough and the Mac performance numbers. All passed; see
[clean macOS](clean-macos.md).

Before the scope change, `scripts/release-smoke.sh` on the deleted `v0.0.0-rc.1`
macOS tarball passed on the shared developer Mac, in a fresh directory with an
empty cache (`mac-smoke.log`). That run is not clean-macOS evidence.

## Evidence

Raw logs and reports are kept outside the repository, in
`~/sail-raw-evidence/l38/` on the lead's machine (sha256):

| File | Contents | sha256 |
|---|---|---|
| `l38-main.tgz` | gate, solver, e2e, fast PR, fast and standard (default arms) logs and reports | `38648d04f07a0102b805b097f5d1ed1052e84e130065f8cba678b7277fe2efac` |
| `l38-deep.tgz` | deep tier default arms | `f12c36769f8be1ee0b9bff258443ba01e13a0c9e91853cdbe732094aa25d7090` |
| `l38-s1.tgz` | deep engine arm | `7a8a05cf917bd0c35d1242caf826091061a009ebc2866b3c3e333e412a94d3b0` |
| `l38-sing.tgz` | Singular projects; standard engine arm | `c464246df64eacdc47514425d4009d0107252c2575ea8617276e4f135768cfdc` |
| `l38-rvt.tgz` | `deep-rvt cache` log and reports | `b74baf1e67eaeca77065197f5f2a356e6531a3b699845eae0f6136e0b4267491` |
| `mac-host.log` | `ci/run.sh host` | `d937944c29e476b4d9793e8cd2c42864484886544e041e6d0b7a51cb2239ab17` |
| `release-rc1.log` | `scripts/release.sh 0.1.0-rc.1` | `ca60584b2c551387eb1a21f231e63d10afd868cbacbff759bd9ccc0df3db5ce4` |
| `linux-smoke-rc1.log` | Linux smoke, `v0.1.0-rc.1` | `4a2ab590f83c107652be98437f0ad3fb3325a23d633fcd04ec81c9944c29f1ec` |
| `release.log` | `scripts/release.sh 0.0.0-rc.1` (deleted draft) | `c88e190609ea9d0c4fa0e1b9816fd491652c04e7965110dbff41bedc92d2f700` |
| `linux-smoke.log` | Linux smoke, `v0.0.0-rc.1` | `0a865278011ddff42bdce51eb94a55e0626d2274638e1542bb20bbb0b1fd19b3` |
| `mac-smoke.log` | macOS smoke on the shared Mac (not clean-macOS evidence) | `331f7fff4639c742669e67be88544cebde26d4c0ded1863c6270316b50278690` |
| `walk-linux-1-install.log` | install | `94a69de6867d185f51ed5e54d0c6d56e8684361e7378e51779e529517f097a3e` |
| `walk-linux-2-demo.log` | demo sequences | `7d357cd5d8e40724c9dce289c555d0dfa78a6f44054cc3064231fc33c7fc8813` |
| `walk-linux-3-vest.log` | vest sequences | `f3dd7b29c5ac5a8e6a2220adf724bae59e7d924381b355d5399a8de70d3d5a7c` |
| `walk-linux-repro-compile-error.log` | finding 2 | `3fd9a91b566888012442a1468d89a27b842faca8a53d0158819786cb6c3caf25` |
| `walk-linux-4a-mcp.log`, `walk-linux-4b-mcp.log` | MCP registration, listing, reply sizes | `38c164a4190ba69c4db6a57146c988393ee81da99a20a170550762cb272434de`, `d7e384633ea882722d6cabdb46744c95e7e010d88e88b45f7bbcc4b5effcc697` |
| `claude-code-loop.jsonl` | Claude Code transcript | `0183e3844915662dbed22cf9ce069e37f4fb488882e360b287d06167ef65bdf2` |
| `walk-linux-outputs.tgz` | per-step stdout, stderr, exit codes and times; MCP reply records; watch events | `e4574c1cec30cf0791f07f79681226d7430a177ea750731657d6af2cb4d03678` |
| `walk-linux-5-misc.log` | watch, cache projects, engine, `--help` | `fbc69ed0cfaa4d92608fa9ef971f3323243884f79f1f5c4da0a1cf140de059bf` |
| `walk-linux-7-papercuts.log` | jobs list, long cache path, missing engine | `4a5dccb79b22f8a6902e9a406f2de1eb9b6af160ca70ae0393e7ee669dbd9498` |

## Failed or interrupted attempts

- `7782b136`, 2026-10-14, Linux smoke (`l38-smoke`): the harness failed to
  read `fetch-verus`'s indented `PATH` line, so `cargo verus` was missing.
  The rerun on the same host passed. Cause: a harness error.
- `7782b136`, 2026-10-14, vest walkthrough, first attempt: the appended test
  function lacked `use vstd::prelude::*`, so the fail and fix edits did not
  compile. The rerun from an empty cache is the one above. Cause: a harness
  error. This attempt is what exposed finding 2.
