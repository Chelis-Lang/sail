# Kani spike S4 (hit rate on real history)

Status: S4 ran on 2026-11-27 with Kani 0.68.0 built from source with the S3
determinism patch, on one native host. **S4 passes on both projects.** The
median commit invalidates 0% of harness CBMC time on s2n-quic and on
Firecracker. Projected speedup over a warm `cargo kani -j8` is
42.0× (median commit) and 3.10× (mean commit) on s2n-quic, and
15.4× and 5.24× on Firecracker. The misses that remain are mostly
whole-project invalidations: on s2n-quic, every release version bump misses
every harness. This is a measurement record: no Sail code ran and nothing here
is a product claim.

The spike, pass rule and drop rule are defined in the Kani phase design
(`docs/design/kani-phase.md`) and ADR 0180. The patch and the CBMC wrapper
method come from [S3](kani-spike-goto-determinism.md). Evidence handling
follows ADR 0038.

## Environment

| Item | Value |
|---|---|
| Host | EC2 c7i.8xlarge (32 vCPU, 61 GiB, no swap), us-east-2c, Ubuntu 24.04, kernel 7.0.0-1018-aws, lane `kani-s4`, 3 h 7 min up |
| Kani | 0.68.0 source at tag `kani-0.68.0` (`0d2328a9`), `cargo build-dev -- --release` (69 s) with `kani-0.68.0-deterministic-goto.diff` (sha256 `f969720a…3ede2`, the S3 patch) |
| CBMC, solvers | CBMC 6.11.0, `goto-cc`, `goto-instrument` and Kissat from the 0.68.0 release bundle (`cargo kani setup`) |
| Rust | nightly-2026-08-21 |
| Extra host package | libseccomp 2.6.0 from the release tarball, as in S2: Firecracker's `seccompiler` needs it and Ubuntu ships 2.5.5 |

No project source was patched. `RUSTC_WRAPPER` was empty and
`CARGO_BUILD_JOBS=16`.

| Project | Units (directory, arguments after `cargo kani -j 8 --output-format terse`) | Harnesses | Commits replayed |
|---|---|---|---|
| aws/s2n-quic | `quic/s2n-quic-core`, `common/s2n-codec`, `quic/s2n-quic-platform`, no arguments | 40, 66, 6 | base `59bb8155` (2026-03-11) then 60 commits to `641fc94a` (2026-11-13) |
| firecracker-microvm/firecracker | workspace root, `-Z unstable-options -Z stubbing -Z function-contracts -Z restrict-vtable --harness-timeout 40m --workspace` | 41 (40 at the base) | base `3f69c58d` (2026-11-20) then 60 commits to `2a8d0138` (2026-11-25) |

Firecracker builds and runs on 0.68.0, so no substitute was needed.

## Method

- **Commits.** The last 60 first-parent commits on `main` touching `*.rs` or
  `Cargo.toml` in the harness crates and their workspace path dependencies
  (normal and build edges from `cargo metadata`), or the root `Cargo.toml` or
  `Cargo.lock`. Closure: s2n-quic `common/s2n-codec`, `quic/s2n-quic-core`;
  Firecracker `src/acpi-tables`, `src/utils`, `src/vmm`. The full hash lists
  are in the archive (`res/s2n.commits`, `res/fc.commits`); the tables below
  give short hashes. The parent of the oldest commit is the base (index 0),
  which only seeds the history.
- **Checkout.** One fixed checkout path per project, `git checkout -f` of each
  commit in place, oldest first, one persistent `target/`.
- **Wrapper.** A script installed as `cbmc`, `goto-instrument` and `goto-cc`
  first on `PATH` records argv, exit code and times for every call, and the
  sha256 and size of the input file for `cbmc` and `goto-instrument`. It keeps a
  content-addressed copy of every distinct CBMC input. Except where real CBMC
  ran, `cbmc` is replaced by replaying one recorded CBMC JSON output, so the
  build and every Kani pass before CBMC run for real at every commit.
- **Key.** CBMC key = sha256 of the exact goto file `cbmc` is started on plus
  its argv (which carries the fixed path). A *history* miss is a key not seen
  at any earlier commit of the replay; a *previous* miss is a key different
  from the same harness's at the immediately previous commit. The
  `goto-instrument --add-library` input was hashed too: its misses equal the
  CBMC-key misses at every commit on both projects.
- **CBMC time (weights).** A harness's CBMC time is a property of its key. Real
  CBMC ran on every harness at the base, the middle (index 30) and the last
  commit (index 60) of each project, either inside `cargo kani -j8` or by
  rerunning the stored goto file with its recorded argv. Missed keys at the
  costly Firecracker and s2n-quic commits were also timed directly. Any other
  harness takes the time of the same harness at the nearest timed sample at or
  before its commit (then after). Weight sources (harness-commits):
  s2n-quic-core 488 exact and 1,952 from a sample; Firecracker 2,153 exact and 347 from a sample.
- **Build time.** Measured per commit: the wall of the incremental
  `cargo kani -j8` with replayed CBMC, which includes the compile, linking and
  every `goto-cc`/`goto-instrument` pass (a cache hit still pays those). At the
  three real-CBMC commits it is the wall minus the 8-wide CBMC makespan.
- **Projection.** direct = build + 8-wide makespan of every harness's CBMC
  time; cached = build + 8-wide makespan of missed CBMC time with 20 ms per
  hit; speedup = direct / cached. The makespan places harnesses greedily in the
  order Kani started them. For s2n-quic the three units run one after another,
  so project figures sum the units per commit. "Mean commit" is mean direct over
  mean cached.
- **Classification.** Every history miss, at every commit, was compared with
  the same harness's goto file at the previous commit by diffing
  `cbmc --show-goto-functions` per function with source-location lines removed
  (then also with `alloc_<hash>` names masked, which carry panic `Location`
  data), together with the commit's changed files: *locations only* (same
  functions once locations are stripped), *reached code changed* (functions
  differ and the commit changed Rust in the closure), *release version bump*
  or *dependency* (functions differ and the commit changed only manifests or
  the lockfile), *new harness*. This covers 27 s2n-quic-core and 12
  Firecracker commits, the whole population of commits with misses.

## Result

| | s2n-quic (3 units) | s2n-quic-core | Firecracker |
|---|---|---|---|
| Harnesses per commit | 112 | 40 | 41 |
| History misses per commit, median / mean | 0 / 32.0 | 0 / 11.6 | 0 / 3.2 |
| Previous-commit misses, median / mean | 0 / 32.0 | 0 / 11.6 | 0 / 4.35 |
| CBMC time missed (history), median / mean | 0% / 29.3% | 0% / 29.6% | 0% / 9.7% |
| CBMC time missed (previous), median / mean | 0% / 29.3% | 0% / 29.6% | 0% / 12.6% |
| Commits with no miss | 33 of 60 | 33 of 60 | 48 of 60 |
| Commits missing every harness | 17 | 17 | 2 |
| Harness hit rate (history) | 71.5% | 71.1% | 92.2% |
| Build s, median / mean | 27.6 / 29.9 | 14.7 / 15.4 | 40.5 / 42.8 |
| Direct s, median / mean | 1,402 / 1,331 | 1,032 / 935 | 644 / 650 |
| Cached s, median / mean | 31 / 429 | 17 / 306 | 41 / 124 |
| **Projected speedup, median commit** | **42.0×** | 51.8× | **15.4×** |
| **Projected speedup, mean commit** | **3.10×** | 3.05× | **5.24×** |
| Miss share if line-shift-only misses were hits, mean (information only) | 29.3% | 29.6% | 7.0% |
| Hit rate if line-shift-only misses were hits | 71.5% | 71.1% | 94.1% |

The other two s2n-quic units: `s2n-codec` (66 cheap harnesses, 28 s of CBMC per
commit) projects 1.60× median and 1.34× mean, because its build is as long as
its CBMC; `s2n-quic-platform` (6 harnesses) projects 59× and 3.36×.

Miss classes over all history misses:

| Class | s2n-quic-core misses (commits) | share of missed CBMC time | Firecracker misses (commits) | share of missed CBMC time |
|---|---|---|---|---|
| Release version bump | 640 (16) | 89.6% | 0 | 0 |
| Dependency or lockfile | 40 (1) | 6.1% | 82 (2) | 35.0% |
| Reached code changed | 14 (10) | 4.3% | 63 (7) | 37.4% |
| Locations only (line shift) | 0 | 0 | 46 (5) | 27.6% |
| New harness | 0 | 0 | 1 (1) | 0.0% |

What the classes are:

- **Release version bump** (s2n-quic). Sixteen of the 60 commits are
  `chore: vX.Y.Z release`, which bump the version of every crate. The version
  feeds rustc's crate hash, which is in every mangled symbol, so every goto
  file changes. With crate hashes masked, the remaining difference on a sample
  of four harnesses is Kani's property IDs (`KANI_CHECK_ID_<crate>.<hash>::<crate>_<n>`),
  renumbered. These 16 commits are why the mean is near 30% while the median
  is 0. The selection rule includes them because they touch `Cargo.toml` in
  the closure, and s2n-quic changes the closure only about once every four
  days, so the 60 commits span eight months.
- **Dependency or lockfile.** s2n-quic `03f5c98c` pins `zerocopy`; Firecracker
  `e4f8db32` and `52a80a9e` change manifests. All harnesses miss.
- **Reached code changed.** Typically 1 to 3 harnesses on s2n-quic. On
  Firecracker, `14a4cb26` (dirty bitmaps per region) misses 21 harnesses and
  `c4f640d8` (VFIO PCI capabilities) 18; in the second, the reached functions
  differ in which crate a shared generic instance is named after
  (`log::LevelFilter::le` from `vfio_ioctls`), not in logic.
- **Locations only.** Firecracker `9c6d8a2c` (17 harnesses), `81f20154` (18,
  including panic-location constants), `34ccad12`, `70bac2c8`, `b6f3edf8`.
  None on s2n-quic.

Per-commit tables are in the appendix.

## Result against the pass rule

Rule: median commit invalidates at most 30% of harness CBMC time, and projected
speedup at least 2× over `cargo kani -j8`. Drop rule: below 1.3× projected.

| Project | Median commit share | Projected, median commit | Projected, mean commit | Verdict |
|---|---|---|---|---|
| s2n-quic | 0% | 42.0× | 3.10× | **pass** |
| Firecracker | 0% | 15.4× | 5.24× | **pass** |

s2n-codec on its own (1.6× median, 1.34× mean) sits between the drop rule and
the pass rule; it is a cheap unit where the cache cannot save more than CBMC
costs.

Consequences for the design:

- A cache that keeps every key it has seen and one that keeps only the
  previous commit's keys differ little: on Firecracker the history cache saves
  three more commits (`ccd28512`, `0f390853`, `2080a671`), whose keys had been
  seen before but not at the previous commit; on s2n-quic they are equal.
- Release version bumps are the dominant miss on a project that versions
  every crate in lockstep. Masking the crate hash would be normalization of
  the CBMC input and is not proposed here; it would need its own ADR and
  validation.
- Line-shift-only misses cost Firecracker 27.6% of missed time and
  s2n-quic nothing, so accepting them, as the design says, is cheap.

## Limits

- One host, one fixed checkout path per project, single samples.
- CBMC times come from three real-CBMC commits plus direct timings of missed
  keys; other harness-commits reuse the same harness's time from a sample. The
  direct timings ran several CBMC processes at once and read about 15% above
  the same keys inside `cargo kani -j8` (s2n-quic-core at `641fc94a`: 918 s
  summed inside Kani, 1,066 s timed directly).
- The projection assumes every harness finishes. Firecracker's
  `verify_checked_new` needs 24 to 27 GB and was killed by the kernel inside
  `cargo kani -j8` at the base (after 111 s) and at the last commit (after
  59 s); the projection counts it at its full time (518 to 675 s). The
  measured `cargo kani -j8` wall at the last commit was 428.6 s against a
  projected direct of 669 s, or 469 s with that harness left out. Leaving it
  out at every commit gives 10.6× median and 5.17× mean. On s2n-quic-core the
  measured wall at the last commit was 234.5 s against a projected 284.7 s.
- Replayed CBMC output makes Kani report wrong verdicts at those commits; only
  goto files, argv and build times are used from them.
- The commit selection follows the rule literally; it includes release
  commits and, for s2n-quic, spans eight months.
- Goto files were used on the host for classification and not retained;
  per-function dump hashes, wrapper records, argv, driver logs and Kani
  metadata were.

## Attempts

| # | Commit range | Result | Cause |
|---|---|---|---|
| 1 | Firecracker base | build failed in 30 s | `seccompiler` needs libseccomp 2.6.0; Ubuntu has 2.5.5; also a relative path bug in the replay script |
| 2 | Firecracker 0 to 28 | every build failed in 4 to 8 s | libseccomp 2.6.0 build needed `gperf`, not installed |
| 3 | s2n-quic base, real CBMC beside the Firecracker base | CBMC killed by the kernel twice (19 and 22 GB), including s2n-quic-core `differential` | both projects ran real CBMC at once in 61 GiB; the other 39 s2n-quic-core timings are reused, `differential` was timed alone (829 s, 22 GB), and s2n-quic switched to replayed CBMC with timing afterwards |
| 4 | both projects, all 61 commits | tables above | |

## Evidence

`2026-11-27-kani-spike-history-replay.tar.gz`, 181,418,234 bytes, sha256
`535a6300bb6da61969173b8fb77741604cd257411977cf8d7e507b1aa49d485a`, kept
outside the repository. It holds the commit lists, every script (setup,
wrapper, replay, timing, analysis, classification), the host setup log and the
patch, per-commit wrapper records (argv, exit codes, times, input sha256 and
size for `cbmc` and `goto-instrument`), driver logs, Kani metadata, lockfile
hashes, raw CBMC JSON from every real run, direct CBMC timings with peak RSS,
the analysis and classification JSON, per-function dump hashes for every
classified goto file, and the failed attempts.

## Appendix: per-commit results

Columns: harnesses; history misses; previous-commit misses; share of CBMC time
missed (history, previous); measured build seconds; projected speedup; miss
classes.

### s2n-quic-core

| # | Commit | Harnesses | Miss (history) | Miss (previous) | CBMC time missed (history) | (previous) | Build s | Speedup | Miss classes |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `a42ffcba` | 40 | 0 | 0 | 0% | 0% | 17 | 50.7 |  |
| 2 | `6932d1e6` | 40 | 0 | 0 | 0% | 0% | 14 | 60.3 |  |
| 3 | `d3addaf6` | 40 | 0 | 0 | 0% | 0% | 19 | 44.9 |  |
| 4 | `024b8048` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 5 | `19b63d8a` | 40 | 0 | 0 | 0% | 0% | 17 | 49.5 |  |
| 6 | `801f68ee` | 40 | 1 | 1 | 1% | 1% | 15 | 36.5 | 1 code |
| 7 | `222f47dd` | 40 | 1 | 1 | 0% | 0% | 14 | 52.9 | 1 code |
| 8 | `f9f2953d` | 40 | 0 | 0 | 0% | 0% | 12 | 71.2 |  |
| 9 | `08ca4dee` | 40 | 40 | 40 | 100% | 100% | 17 | 1.0 | 40 release |
| 10 | `3bdcd44d` | 40 | 0 | 0 | 0% | 0% | 15 | 57.7 |  |
| 11 | `9e7cba1b` | 40 | 40 | 40 | 100% | 100% | 18 | 1.0 | 40 release |
| 12 | `846183ca` | 40 | 0 | 0 | 0% | 0% | 12 | 70.6 |  |
| 13 | `728d1877` | 40 | 0 | 0 | 0% | 0% | 14 | 59.9 |  |
| 14 | `ce1da94e` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 15 | `172d7e5d` | 40 | 0 | 0 | 0% | 0% | 13 | 65.8 |  |
| 16 | `8986c9f5` | 40 | 40 | 40 | 100% | 100% | 18 | 1.0 | 40 release |
| 17 | `b493d2a7` | 40 | 0 | 0 | 0% | 0% | 14 | 61.4 |  |
| 18 | `4438384b` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 19 | `5c217523` | 40 | 0 | 0 | 0% | 0% | 13 | 64.5 |  |
| 20 | `547e973d` | 40 | 0 | 0 | 0% | 0% | 13 | 68.0 |  |
| 21 | `33921bd2` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 22 | `0f5a4f8a` | 40 | 0 | 0 | 0% | 0% | 16 | 54.2 |  |
| 23 | `4dbeedce` | 40 | 0 | 0 | 0% | 0% | 15 | 57.2 |  |
| 24 | `45086005` | 40 | 0 | 0 | 0% | 0% | 15 | 57.5 |  |
| 25 | `ff973813` | 40 | 0 | 0 | 0% | 0% | 13 | 68.0 |  |
| 26 | `aa867334` | 40 | 0 | 0 | 0% | 0% | 18 | 48.2 |  |
| 27 | `7860ebbf` | 40 | 40 | 40 | 100% | 100% | 18 | 1.0 | 40 release |
| 28 | `3e3e77ed` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 29 | `57432ff4` | 40 | 0 | 0 | 0% | 0% | 14 | 73.3 |  |
| 30 | `7c01b6d1` | 40 | 0 | 0 | 0% | 0% | 17 | 60.2 |  |
| 31 | `c794846f` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 32 | `57c58864` | 40 | 0 | 0 | 0% | 0% | 14 | 74.4 |  |
| 33 | `90fbbd85` | 40 | 0 | 0 | 0% | 0% | 14 | 75.2 |  |
| 34 | `68b4ed95` | 40 | 3 | 3 | 50% | 50% | 13 | 1.0 | 3 code |
| 35 | `54a7d5b0` | 40 | 1 | 1 | 0% | 0% | 18 | 39.1 | 1 code |
| 36 | `f200845b` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 37 | `a33d369b` | 40 | 40 | 40 | 100% | 100% | 18 | 1.0 | 40 release |
| 38 | `c901ebb7` | 40 | 0 | 0 | 0% | 0% | 14 | 71.1 |  |
| 39 | `f063938a` | 40 | 0 | 0 | 0% | 0% | 15 | 69.2 |  |
| 40 | `1e70d9c0` | 40 | 0 | 0 | 0% | 0% | 13 | 80.4 |  |
| 41 | `845f1cce` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 42 | `53fda77f` | 40 | 1 | 1 | 0% | 0% | 15 | 46.1 | 1 code |
| 43 | `22ef5348` | 40 | 0 | 0 | 0% | 0% | 13 | 82.2 |  |
| 44 | `90115724` | 40 | 0 | 0 | 0% | 0% | 8 | 133.6 |  |
| 45 | `1f03e209` | 40 | 1 | 1 | 0% | 0% | 17 | 41.3 | 1 code |
| 46 | `b0a09202` | 40 | 40 | 40 | 100% | 100% | 20 | 1.0 | 40 release |
| 47 | `4e0d1485` | 40 | 1 | 1 | 0% | 0% | 12 | 66.8 | 1 code |
| 48 | `03f5c98c` | 40 | 40 | 40 | 100% | 100% | 22 | 1.0 | 40 deps |
| 49 | `5ec3ba12` | 40 | 0 | 0 | 0% | 0% | 15 | 66.8 |  |
| 50 | `96b61686` | 40 | 0 | 0 | 0% | 0% | 6 | 162.4 |  |
| 51 | `4628ce01` | 40 | 1 | 1 | 0% | 0% | 15 | 45.4 | 1 code |
| 52 | `f857bddf` | 40 | 40 | 40 | 100% | 100% | 17 | 1.0 | 40 release |
| 53 | `be829a47` | 40 | 0 | 0 | 0% | 0% | 12 | 83.4 |  |
| 54 | `6a3455d5` | 40 | 0 | 0 | 0% | 0% | 14 | 75.8 |  |
| 55 | `a01e5724` | 40 | 1 | 1 | 13% | 13% | 12 | 3.8 | 1 code |
| 56 | `d7b71409` | 40 | 40 | 40 | 100% | 100% | 19 | 1.0 | 40 release |
| 57 | `022c908b` | 40 | 0 | 0 | 0% | 0% | 17 | 61.3 |  |
| 58 | `6ef4e8f1` | 40 | 0 | 0 | 0% | 0% | 13 | 78.3 |  |
| 59 | `cddae9e1` | 40 | 3 | 3 | 10% | 10% | 12 | 2.7 | 3 code |
| 60 | `641fc94a` | 40 | 0 | 0 | 0% | 0% | 14 | 20.2 |  |

### Firecracker

| # | Commit | Harnesses | Miss (history) | Miss (previous) | CBMC time missed (history) | (previous) | Build s | Speedup | Miss classes |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `14a4cb26` | 41 | 22 | 22 | 96% | 96% | 48 | 1.1 | 21 code, 1 new |
| 2 | `931f779c` | 41 | 1 | 1 | 0% | 0% | 39 | 13.3 | 1 code |
| 3 | `34ccad12` | 41 | 5 | 5 | 33% | 33% | 46 | 1.5 | 4 shift, 1 code |
| 4 | `4fdf8780` | 41 | 0 | 0 | 0% | 0% | 38 | 15.4 |  |
| 5 | `70bac2c8` | 41 | 1 | 1 | 0% | 0% | 48 | 11.8 | 1 shift |
| 6 | `6b33b14f` | 41 | 0 | 0 | 0% | 0% | 39 | 14.6 |  |
| 7 | `58cf1b2c` | 41 | 0 | 0 | 0% | 0% | 39 | 14.6 |  |
| 8 | `d1518f37` | 41 | 0 | 0 | 0% | 0% | 41 | 15.5 |  |
| 9 | `ccd28512` | 41 | 1 | 5 | 0% | 34% | 39 | 13.8 | 1 code |
| 10 | `e0bdc79c` | 41 | 0 | 0 | 0% | 0% | 48 | 12.6 |  |
| 11 | `81f20154` | 41 | 18 | 18 | 64% | 64% | 46 | 1.0 | 18 shift+panic |
| 12 | `e4f8db32` | 41 | 41 | 41 | 100% | 100% | 54 | 1.0 | 41 deps |
| 13 | `c4f640d8` | 41 | 18 | 18 | 64% | 64% | 48 | 1.0 | 18 code |
| 14 | `0ec4c322` | 41 | 0 | 0 | 0% | 0% | 51 | 13.7 |  |
| 15 | `52a80a9e` | 41 | 41 | 41 | 100% | 100% | 54 | 1.0 | 41 deps |
| 16 | `9c6d8a2c` | 41 | 17 | 17 | 62% | 62% | 42 | 1.0 | 17 shift |
| 17 | `b8e60c0e` | 41 | 0 | 0 | 0% | 0% | 40 | 15.9 |  |
| 18 | `c68983af` | 41 | 0 | 0 | 0% | 0% | 50 | 13.0 |  |
| 19 | `65c84675` | 41 | 0 | 0 | 0% | 0% | 51 | 12.5 |  |
| 20 | `3c9434ee` | 41 | 0 | 0 | 0% | 0% | 49 | 15.0 |  |
| 21 | `4fab67e9` | 41 | 0 | 0 | 0% | 0% | 39 | 16.3 |  |
| 22 | `dd0bc352` | 41 | 0 | 0 | 0% | 0% | 40 | 15.9 |  |
| 23 | `0f390853` | 41 | 17 | 41 | 63% | 100% | 50 | 1.0 | 17 code |
| 24 | `436d223c` | 41 | 0 | 0 | 0% | 0% | 50 | 11.6 |  |
| 25 | `35e407ea` | 41 | 0 | 0 | 0% | 0% | 48 | 12.0 |  |
| 26 | `2123c57b` | 41 | 0 | 0 | 0% | 0% | 48 | 12.2 |  |
| 27 | `fb9c519f` | 41 | 0 | 0 | 0% | 0% | 39 | 16.4 |  |
| 28 | `ff85e753` | 41 | 0 | 0 | 0% | 0% | 40 | 14.4 |  |
| 29 | `a5ae74ab` | 41 | 0 | 0 | 0% | 0% | 38 | 15.1 |  |
| 30 | `75d66249` | 41 | 0 | 0 | 0% | 0% | 40 | 18.0 |  |
| 31 | `985add5e` | 41 | 0 | 0 | 0% | 0% | 38 | 18.9 |  |
| 32 | `8e899058` | 41 | 0 | 0 | 0% | 0% | 41 | 13.8 |  |
| 33 | `e3b698f6` | 41 | 0 | 0 | 0% | 0% | 48 | 14.0 |  |
| 34 | `3add55c5` | 41 | 0 | 0 | 0% | 0% | 47 | 15.5 |  |
| 35 | `4a8a6f5f` | 41 | 0 | 0 | 0% | 0% | 39 | 17.3 |  |
| 36 | `2e0f2ee1` | 41 | 0 | 0 | 0% | 0% | 51 | 11.2 |  |
| 37 | `8e2aa170` | 41 | 0 | 0 | 0% | 0% | 38 | 15.1 |  |
| 38 | `2080a671` | 41 | 0 | 41 | 0% | 100% | 50 | 13.3 |  |
| 39 | `ca8059d1` | 41 | 0 | 0 | 0% | 0% | 50 | 15.9 |  |
| 40 | `3917ae86` | 41 | 0 | 0 | 0% | 0% | 41 | 15.4 |  |
| 41 | `c5691f93` | 41 | 0 | 0 | 0% | 0% | 38 | 16.7 |  |
| 42 | `6cef920f` | 41 | 0 | 0 | 0% | 0% | 40 | 15.9 |  |
| 43 | `04568305` | 41 | 0 | 0 | 0% | 0% | 40 | 16.1 |  |
| 44 | `4fa3c3c4` | 41 | 0 | 0 | 0% | 0% | 39 | 16.5 |  |
| 45 | `1e32826b` | 41 | 0 | 0 | 0% | 0% | 41 | 15.6 |  |
| 46 | `7292b81f` | 41 | 0 | 0 | 0% | 0% | 42 | 15.4 |  |
| 47 | `4053507f` | 41 | 0 | 0 | 0% | 0% | 42 | 15.6 |  |
| 48 | `c764b2d3` | 41 | 0 | 0 | 0% | 0% | 41 | 15.6 |  |
| 49 | `7812755f` | 41 | 0 | 0 | 0% | 0% | 40 | 16.1 |  |
| 50 | `38c1bee2` | 41 | 0 | 0 | 0% | 0% | 41 | 15.6 |  |
| 51 | `7c821bf4` | 41 | 0 | 0 | 0% | 0% | 40 | 16.2 |  |
| 52 | `d43a9fc0` | 41 | 0 | 0 | 0% | 0% | 40 | 17.7 |  |
| 53 | `98491607` | 41 | 0 | 0 | 0% | 0% | 40 | 17.4 |  |
| 54 | `e50c208a` | 41 | 0 | 0 | 0% | 0% | 37 | 19.9 |  |
| 55 | `e09c8a75` | 41 | 0 | 0 | 0% | 0% | 39 | 19.8 |  |
| 56 | `b6f3edf8` | 41 | 10 | 10 | 0% | 0% | 37 | 16.8 | 6 shift, 4 code |
| 57 | `287057f1` | 41 | 0 | 0 | 0% | 0% | 38 | 17.3 |  |
| 58 | `454a9e84` | 41 | 0 | 0 | 0% | 0% | 38 | 16.8 |  |
| 59 | `72fb2857` | 41 | 0 | 0 | 0% | 0% | 36 | 17.1 |  |
| 60 | `2a8d0138` | 41 | 0 | 0 | 0% | 0% | 38 | 17.4 |  |
