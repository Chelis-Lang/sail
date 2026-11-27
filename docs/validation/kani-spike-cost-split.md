# Kani spikes S2 (cost split) and S5 (admission census)

Status: both spikes ran on 2026-11-27 with stock Kani 0.68.0 on one native
host. **S2 passes** on all three real projects: after the build, 77% to 98% of
a warm `cargo kani -j` wall is spent linking, instrumenting and checking
harnesses. **S5 passes its admission-and-running half** on both heavy projects:
today's decoder would admit 22.5% of s2n-quic-core and 73% of Firecracker
harnesses, and 100% of both once pinned SAT solvers, stubbing, should-panic and
contract harnesses are admitted as run modes; all of them run to success on
0.68.0. Conformance of a Sail executor against direct Kani is not part of this
record. This is a measurement record: no Sail code ran and nothing here is a
product claim.

The spikes, pass rules and drop rules are defined in the Kani phase design
(`docs/design/kani-phase.md`, PR #808) and ADR 0180. The decoder referred to as
"today's" is [ADR 0128](../decisions/0128-kani-bounded-observations.md)'s.
Evidence handling follows ADR 0038.

## Environment

| Item | Value |
|---|---|
| Host | EC2 c7i.8xlarge (32 vCPU Xeon Platinum 8488C, 61 GiB, no swap), us-east-2c, Ubuntu 24.04.5, kernel 7.0.0-1018-aws, lane `kani-s2` |
| Kani | 0.68.0 from `cargo install --locked kani-verifier --version 0.68.0 && cargo kani setup`; all eight `bin/` files match the sha256 pins in `engine/kani/bundle-linux.json` |
| CBMC, solvers | CBMC 6.11.0 (built-in CaDiCaL and MiniSat2), bundled Kissat 4.0.1 |
| Rust | nightly-2026-08-21, rustc 1.100.0-nightly (8925ea358 2026-08-20) |
| Extra host package | libseccomp 2.6.0 built from the release tarball (sha256 `83b60852…784dc`) because Firecracker's `seccompiler` needs `seccomp_export_bpf_mem` and Ubuntu ships 2.5.5 |

No project source was patched. `RUSTC_WRAPPER` was empty.

| Project | Commit | Directory | Arguments after `cargo kani` (the project's own CI arguments) |
|---|---|---|---|
| arrayvec 0.7.8 pilot | Sail `spike/v030-kani` at `b22a60f7`, `spikes/kani-pilot/pilot` | pilot crate | none |
| DataDog/lading | `7b3d3f78` | `lading_throttle` | `--solver cadical` |
| aws/s2n-quic | `e11382fd` | `common/s2n-codec`, `quic/s2n-quic-core`, `quic/s2n-quic-platform` | none (`package.metadata.kani` sets `tests = true`, `stubbing = true`) |
| firecracker-microvm/firecracker | `e9783a04` | workspace root | `-Z unstable-options -Z stubbing -Z function-contracts -Z restrict-vtable --harness-timeout 40m --workspace` |
| rust-osdev/x86_64 | `9fc13798` | crate root | `--all-features` |
| google/zerocopy | `610fd363` | `zerocopy` | census only, see below |

Every timed run adds `-Z unstable-options -j N --output-format terse`. Three of
the nine arrayvec pilot harnesses fail by design.

## S5: admission census

Counts come from the `kani-metadata.json` files that a cold
`cargo kani ARGS --only-codegen` writes (one record per harness: `kind`,
`should_panic`, `solver`, `stubs`, `verified_stubs`, `unwind_value`,
`has_loop_contracts`, `contract`), cross-checked against
`cargo kani list --format json` where that command worked.

| Project | Harnesses | Plain proof | should_panic | Uses stubbing | proof_for_contract | Loop contracts | Solver: default / cadical / kissat / minisat | Admitted today | Admitted with kinds widened | Share of CBMC time admitted today |
|---|---|---|---|---|---|---|---|---|---|---|
| arrayvec pilot | 9 | 9 | 0 | 0 | 0 | 0 | 9 / 0 / 0 / 0 | 9 (100%) | 9 | 100% |
| lading_throttle | 17 | 17 | 0 | 0 | 0 | 0 | 17 / 0 / 0 / 0 (CLI forces cadical) | 17 (100%) | 17 | 100% |
| s2n-codec | 66 | 66 | 0 | 0 | 0 | 0 | 58 / 8 / 0 / 0 | 66 (100%) | 66 | 100% |
| s2n-quic-core | 40 | 40 | 0 | 0 | 0 | 0 | 0 / 9 / 30 / 1 | 9 (22.5%) | 40 (100%) | 18.0% |
| s2n-quic-platform | 6 | 5 | 0 | 1 | 0 | 0 | 0 / 1 / 1 / 4 | 1 (17%) | 6 (100%) | 1.4% |
| Firecracker | 41 | 30 | 1 | 9 | 1 | 0 | 28 / 13 / 0 / 0 | 30 (73%) | 41 (100%) | 61.6% |
| x86_64 | 34 | 26 | 8 | 0 | 0 | 0 | 34 / 0 / 0 / 0 | 26 (76%) | 34 (100%) | 74.8% |
| zerocopy (default features) | 94 | 21 | 1 | 0 | 72 | 0 | 47 / 0 / 47 / 0 | 14 (15%) | 94 (100%) | not run |

"Admitted today" is a plain `#[kani::proof]` with no should-panic, stub,
contract or loop contract, whose effective solver is CaDiCaL. "Widened" also
admits should-panic, stubbing, `proof_for_contract` and the SAT solvers Kani
bundles (CaDiCaL, Kissat, MiniSat). The decoder's own field checks do not look
at stubs, so by those checks alone nine more Firecracker harnesses pass (39 of
41); the stricter count is used because stubbing needs `-Z stubbing`.

Required unstable flags: s2n-quic `-Z stubbing` (from `Cargo.toml` metadata;
one harness uses it); Firecracker `-Z stubbing -Z function-contracts
-Z restrict-vtable -Z unstable-options`; zerocopy `-Z function-contracts
-Z unstable-options`; arrayvec, lading and x86_64 none.

Other census facts:

- No project uses loop contracts. Pinned solvers are the main blocker on
  s2n-quic (36 of 46 core and platform harnesses pin Kissat or MiniSat);
  contracts are the blocker on zerocopy (72 of 94).
- `kani::solver(minisat)` makes Kani pass no `--sat-solver` flag, so CBMC uses
  its default MiniSat2. `kani::solver(kissat)` becomes
  `--external-sat-solver kissat`, a separate process per solver call (119
  Kissat processes for 30 s2n-quic-core harnesses).
- `lading_payload` is in lading's CI matrix and has no harnesses.
  `s2n-quic-dc` has two harnesses reachable only with `--tests`; its CI does
  not run them.
- `cargo kani list` in 0.68.0 accepts no Cargo arguments (`--workspace`,
  `--all-features`, `--manifest-path`, `--features` are refused) and ignores
  `package.metadata.kani` unstable flags. It reports 0 harnesses for the
  Firecracker workspace root, fails to compile x86_64 without
  `--all-features`, and needs an explicit `-Z stubbing` for s2n-quic-platform.
  Discovery through `kani list` alone would miss real harnesses; the metadata
  files from a codegen run are complete.
- zerocopy pins Kani 0.60.0. Its CI arguments are refused by 0.68.0
  (`error: unexpected argument '--memory-safety-checks' found`), and with
  those four flags dropped its CI feature set does not compile on the 0.68.0
  toolchain (`error[E0658]: use of unstable library feature 'allocator_api'`).
  The census row is a default-features codegen on 0.68.0, which agrees with
  `kani list` (22 standard, 72 contract harnesses, 27 functions under
  contract). On zerocopy's own pin (Kani 0.60.0,
  nightly-2025-03-02, a labelled secondary) the CI arguments compile: 95
  harnesses, 23 proof and 72 contract, 47 pinning Kissat, 15 admitted today.

**Against the S5 rule** (at least 60% admitted and running on two heavy
projects after widening): s2n-quic-core 40 of 40 and Firecracker 41 of 41 are
admitted after widening, and all 81 verify successfully on stock 0.68.0
(Firecracker at `-j4`; see the memory finding). Without widening the decoder
covers 18% of s2n-quic-core's CBMC time, so widening solvers first is a
precondition for any s2n-quic number.

## S2: cost split

### (a) One `cargo kani -j8`, cold and warm target

Each round removes `target/kani`, then runs `--only-codegen` twice (cold, then
warm), removes `target/kani` again and runs the full command twice (cold, then
warm). The Cargo registry stays warm. Wall, user, sys and peak RSS are GNU
`time` over the whole `cargo kani` process tree (peak RSS is the largest
single process). Values are medians with the range when there are three
rounds.

| Project | Rounds | Cold wall s | Cold user / sys s | Warm wall s | Warm user / sys s | Peak RSS GiB (cold / warm) | `--only-codegen` cold / warm s | Cargo build line cold / warm s | Post-build share of warm wall |
|---|---|---|---|---|---|---|---|---|---|
| arrayvec pilot | 3 | 5.8 (5.6–6.0) | 17 / 1 | 5.1 (5.0–5.4) | 15 / 1 | 0.61 / 0.23 | 2.1 / 1.3 | 0.8 / 0.0 | 74.2% (99.4% cargo line) |
| lading_throttle | 3 | 15.7 (15.4–15.8) | 39 / 3 | 10.3 (10.2–10.3) | 27 / 2 | 0.68 / 0.19 | 8.1 / 2.4 | 5.4 / 0.1 | 76.8% (99.1% cargo line) |
| x86_64 | 3 | 25.0 (24.9–28.6) | 151 / 4 | 24.7 (24.0–26.4) | 150 / 4 | 0.55 / 0.55 | 3.6 / 1.7 | 2.0 / 0.0 | 93.1% (100.0% cargo line) |
| s2n-codec | 3 | 21.5 (19.8–21.6) | 94 / 10 | 11.2 (11.1–11.2) | 69 / 7 | 1.02 / 0.30 | 17.7 / 7.5 | 10.3 / 0.1 | 32.9% (99.2% cargo line) |
| s2n-quic-core | 3 | 295.3 (280.2–301.6) | 1137 / 35 | 285.8 (281.4–288.8) | 1091 / 29 | 8.22 / 8.22 | 26.8 / 8.5 | 18.2 / 0.1 | 97.0% (100.0% cargo line) |
| s2n-quic-platform | 3 | 373.1 (365.0–391.7) | 617 / 23 | 358.5 (349.8–379.8) | 564 / 15 | 7.75 / 7.75 | 16.8 / 1.9 | 14.9 / 0.1 | 99.5% (100.0% cargo line) |
| Firecracker, `-j8` (invalid: OOM kill) | 1 | 580.0 | 3153 / 97 | 452.7 | 2935 / 67 | 22.18 / 22.07 | 87.8 / 22.5 | 64.7 / 2.8 | 95.0% (99.4% cargo line) |
| Firecracker, `-j4` | 1 | 1075.9 | 3418 / 92 | 998.1 | 3211 / 68 | 25.03 / 25.39 | 93.4 / 22.8 | 67.5 / 0.8 | 97.7% (99.9% cargo line) |

Post-build share is `(warm wall − warm --only-codegen wall) / warm wall`. A
warm `--only-codegen` is not a no-op: Cargo reports the build fresh in under
3 s, then kani-driver re-runs both `goto-cc` link steps for every harness,
serially. The strict figure therefore counts linking as build; the figure in
brackets subtracts only the time on Cargo's own `Finished` line.

**Against the S2 rule** (post-build share at least 50% of the `-j8` wall on two
of three real projects): lading_throttle 76.8%, s2n-quic-core 97.0% and
Firecracker 97.7% (at `-j4`; 95.0% on the two invalid `-j8` runs). Pass on
three of three. s2n-codec is the counter-example the design predicted: 66
tiny harnesses, 33% post-build, and CBMC is only 40% of its tool time.

### (b) Per-harness split

One extra cold run per project goes through a copy of the bundle
(`KANI_HOME`) in which `kani-compiler`, `goto-cc`, `goto-instrument`, `cbmc`
and `kissat` are replaced by a wrapper that records argv, start and end time
and GNU `time -v` for the real binary, and tees CBMC's stdout. The run uses a
separate `--target-dir`. Stage times are wrapper wall times summed per harness
(two `goto-cc` and three to five `goto-instrument` calls each). Symbolic execution,
SSA conversion and SAT time are the sums of CBMC's own `Runtime Symex`,
`Runtime Convert SSA` and `Runtime Solver` lines; a harness has several
solver calls. Peak RSS is the largest traced process of the harness (for
Kissat harnesses GNU `time` on `cbmc` already includes its child). Light
projects were traced serially with `--verbose`; heavy ones at the stated `-j`.
`rustc` and `kani-compiler` work per crate, never per harness, so the build
column is CPU time of all `kani-compiler` processes with the harness-bearing
crate in brackets.

| Project | Traced run | Harnesses | kani-compiler CPU s (harness crate) | goto-cc total / median s | goto-instrument total / median s | CBMC total / median / max s | CBMC share of post-build tool time | Symex / convert-SSA / SAT s | SAT share of CBMC | Slowest 10% share of CBMC | Peak RSS median / max MiB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| arrayvec pilot | -j1, 16 s | 9 | 1.8 (1.7) | 2.3 / 0.23 | 2.8 / 0.32 | 9.4 / 0.45 / 3.9 | 64.6% | 2.9 / 0.5 / 1.7 | 18.4% | 41% (n=1) | 56 / 220 |
| lading_throttle | -j1, 38 s | 17 | 13.0 (2.9) | 3.7 / 0.17 | 4.4 / 0.11 | 19.0 / 0.26 / 9.1 | 70.2% | 1.5 / 0.2 / 14.1 | 74.2% | 63% (n=2) | 51 / 189 |
| x86_64 | -j1, 141 s | 34 | 2.8 (2.2) | 2.9 / 0.08 | 4.0 / 0.11 | 131.3 / 0.65 / 14.5 | 95.0% | 26.2 / 4.3 / 76.5 | 58.3% | 30% (n=3) | 83 / 559 |
| s2n-codec | -j1, 79 s | 66 | 26.0 (8.2) | 14.2 / 0.13 | 24.9 / 0.27 | 26.6 / 0.19 / 1.9 | 40.5% | 11.2 / 1.1 / 0.3 | 1.2% | 41% (n=7) | 34 / 106 |
| s2n-quic-core | -j8, 302 s | 40 | 54.1 (18.3) | 21.2 / 0.38 | 29.3 / 0.51 | 1124.3 / 4.18 / 254.3 | 95.7% | 191.3 / 12.2 / 718.0 | 63.9% | 70% (n=4) | 90 / 8412 |
| s2n-quic-platform | -j8, 375 s | 6 | 65.1 (5.6) | 4.0 / 0.57 | 5.9 / 0.94 | 553.2 / 42.50 / 355.5 | 98.2% | 69.2 / 6.9 / 322.8 | 58.4% | 64% (n=1) | 674 / 2656 |
| Firecracker | -j4, 1129 s | 41 | 171.5 (32.3) | 43.1 / 1.27 | 103.5 / 2.46 | 3089.6 / 22.58 / 527.3 | 95.5% | 66.1 / 87.0 / 2650.4 | 85.8% | 48% (n=4) | 600 / 25937 |

- **CBMC dominates wherever harnesses are not trivial**: 95% to 98% of
  post-build tool time on x86_64, s2n-quic-core, s2n-quic-platform and
  Firecracker; 65% to 70% on arrayvec and lading; 40% on s2n-codec.
- **The SAT solver is most of CBMC on the heavy projects** (58% to 74% on
  lading, x86_64, s2n-quic-core and s2n-quic-platform, 86% on Firecracker), more than the
  1% to 61% the design quotes from Kani #5043. Symbolic execution is 2% to
  20% there.
- **CBMC time is concentrated**: the slowest tenth of harnesses holds 63% to
  70% of it on lading, s2n-quic-core and s2n-quic-platform and 48% on Firecracker (4 of 41). A reuse
  layer is worth what it does for those few harnesses.
- **Memory**: the median harness peaks below 100 MiB everywhere except
  s2n-quic-platform (674 MiB) and Firecracker (600 MiB). The largest are
  Firecracker `verify_checked_new` at 25.3 GiB, three more Firecracker
  harnesses between 10.8 and 11.4 GiB, and s2n-quic-core `alloc_test` at 8.2 GiB inside
  Kissat. Eight Firecracker jobs do not fit in 61 GiB.

### Findings a later design depends on

- **Files.** For each harness `kani-compiler` writes
  `<crate>-<hash>__<mangled harness>.symtab.out`, `.type_map.json` and
  `.pretty_name_map.json` under
  `target/kani/<triple>/debug/build/<package>/<hash>/out/`, next to one
  `<crate>-<hash>.kani-metadata.json`. `goto-cc` links the symtab with
  `kani_lib.c` into `<…>.out`, and every later step rewrites that same `.out`
  in place. CBMC reads the final `.out`. lading_throttle, 17 harnesses:
  `.symtab.out` 13.5 MB total (largest 1.6 MB), final `.out` 10.1 MB (1.1 MB),
  `.pretty_name_map.json` 5.2 MB, `.type_map.json` 0.16 MB; all of
  `target/kani` is 133 MB. s2n-quic-core, 40 harnesses: `.symtab.out` 62 MB
  (largest 3.6 MB), `.out` 62 MB, `target/kani` 543 MB. One example harness:

  | Bytes | File (lading_throttle `linear::verification::capacity_resets_on_interval_change`) |
  |---|---|
  | 78,386 | `$H.symtab.out` |
  | 106,102 | `$H.out` |
  | 29,465 | `$H.pretty_name_map.json` |
  | 2,713 | `$H.type_map.json` |

- **Exact command lines** for that harness (`$OUT` is the `out/` directory,
  `$H` the file stem, `$KANI` the bundle root):

  ```text
  goto-cc $OUT/$H.symtab.out $KANI/library/kani/kani_lib.c -o $OUT/$H.out
  goto-cc $OUT/$H.out --function _RNvNtNtCs2BWVVFqhwmo_15lading_throttle6linear12verification34capacity_resets_on_interval_change -o $OUT/$H.out
  goto-instrument --add-library --no-malloc-may-fail $OUT/$H.out $OUT/$H.out
  goto-instrument --generate-function-body-options assert-false-assume-false --generate-function-body .* --drop-unused-functions $OUT/$H.out $OUT/$H.out
  goto-instrument --ensure-one-backedge-per-target $OUT/$H.out $OUT/$H.out
  cbmc --no-malloc-may-fail --no-undefined-shift-check --no-signed-overflow-check --nan-check --no-self-loops-to-assumptions --no-pointer-primitive-check --object-bits 16 --sat-solver cadical --slice-formula $OUT/$H.out --verbosity 9 --json-ui
  ```

  A harness with `#[kani::unwind(5)]` and `kani::solver(kissat)` differs only
  in `--unwind 5 --external-sat-solver kissat`. With
  `-Z restrict-vtable` every harness gets a first
  `goto-instrument --function-pointer-restrictions-file $OUT/$H.linked-restrictions.json $OUT/$H.out $OUT/$H.out`.
  A `proof_for_contract` harness adds
  `goto-instrument --dfcc <harness symbol> --no-malloc-may-fail --enforce-contract <contract closure symbol> $OUT/$H.out $OUT/$H.out`
  before `--add-library`. Stubbing and should-panic change no tool
  arguments. Across the 121 CBMC runs of the four heavier projects the only
  varying arguments are `--unwind N` (present or absent) and the solver choice.
- **The build depends on the selection and the flags.** After a full run of
  the arrayvec pilot, `cargo kani --harness p1_push_pop_roundtrip` recompiles
  the harness crate into a second `<hash>` directory that holds only that
  harness; a following `--only-codegen`, `list` or full run with the original
  arguments recompiles nothing. The plain `--only-codegen` runs and the
  `-Z unstable-options -j 8 --output-format terse` runs of this record also
  built the harness crate separately (two `<hash>` directories). A batch
  executor that wants one shared build must keep selection and flags constant
  and filter harnesses itself.
- **CBMC's stdout is large.** Kani runs CBMC with `--verbosity 9 --json-ui`.
  Per harness the stream is 0.8 MB to 105 MB at the median by project,
  6.5 GB in total for Firecracker's 41 harnesses, and 1.33 GB at the largest (s2n-quic-platform `round_trip_test`, 26.5 million
  lines; `gzip -1` brings it to 61 MB). It names the absolute GOTO path. An
  exact-bytes store at the CBMC boundary must compress and must expect
  gigabyte streams. kani-driver holds what it parses: the whole-run peak RSS
  on s2n-quic-platform is 7.75 GiB while no CBMC or Kissat process exceeds
  2.6 GiB.
- **`--export-json` under-reports solver time.** `runtime_solver_s` keeps only
  the last solver call: 304 s against 718 s summed from CBMC's lines on
  s2n-quic-core. `runtime_symex_s` agrees exactly. Its `harness_metadata`
  omits `solver`, `stubs` and `unwind_value`, which the `kani-metadata.json`
  files carry.
- **Out of memory is reported as an ordinary failure.** At `-j8` on this
  61 GiB host the kernel killed Firecracker's
  `devices::virtio::queue::verification::verify_checked_new` in both runs
  (`Out of memory: Killed process … (cbmc) … anon-rss:18605916kB` and
  `…22757016kB`). Kani printed `CBMC failed` and `VERIFICATION:- FAILED` and
  summarised "40 successfully verified harnesses, 1 failures". At `-j4` the
  same harness passes with a peak of about 25 GiB. This is the failure class
  the design's integrity supervision targets (S6); here it shows as a false
  failure, not a false success.

## Limits

- One host, one day, wall-clock on a shared-tenancy instance; no quiet-pool
  controls. Firecracker has one round per `-j`; every other project has three.
- Firecracker's `-j8` rows are from runs that lost their largest harness to
  the kernel and are not valid timings; its clean rows are `-j4`.
- Heavy projects were traced under `-j8` or `-j4`, so their per-harness wall
  times include whatever contention eight single-threaded CBMC processes cause
  on 16 cores.
- "Running" means stock Kani reports success. No Sail executor ran, so the
  conformance half of S5 is open.
- zerocopy and x86_64 were not part of the pass rules; zerocopy has census
  data only.

## Attempts

| Date | Attempt | Result |
|---|---|---|
| 2026-11-27 | Census pass 1 | `kani list` refused Cargo arguments on x86_64, zerocopy and Firecracker and failed on s2n-quic-platform without `-Z stubbing`; Firecracker's build script failed to link (`undefined symbol: seccomp_export_bpf_mem`, host libseccomp 2.5.5); zerocopy's CI arguments refused. Reran with libseccomp 2.6.0 and per-project list arguments. |
| 2026-11-27 | zerocopy codegen on 0.68.0 with its CI features | Compile error E0658 (`allocator_api`). Default-features census used instead. |
| 2026-11-27 | First arrayvec rounds | Discarded: the warm `--only-codegen` sample followed a full run and rebuilt the harness crate. Round order changed as described above. |
| 2026-11-27 | Firecracker `-j8`, cold and warm | 40 of 41, one harness killed by the kernel OOM killer in each run. Kept in the table, marked invalid. |
| 2026-11-27 | Firecracker `-j4`, cold and warm | 41 of 41. |
| 2026-11-27 | zerocopy codegen on its own Kani 0.60.0 pin (secondary) | Builds with the project's CI arguments; census only, harnesses not run. |

## Evidence

Raw logs, traces, per-harness tables, the census JSON and the scripts that
produced them: `2026-11-27-kani-spike-cost-split.tar.gz`, 283,088,800 bytes, sha256
`08fa6be1707445acd5894baf043e507ac8e680fb82a05a0890cc50f2f7b5f086`. The host was terminated after the copy.

Documentation changed: this record only. No command, MCP tool or released
behaviour changes, so the user guides and the consumer skill are unchanged.
