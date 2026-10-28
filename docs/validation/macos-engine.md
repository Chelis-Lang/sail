# macOS engine qualification

The current compiler-read runtime is fork `47fb3be6`, manifest `ea6853a8…`,
published as `engine-runtime-47fb3be6-macos`. Its build, engine, source-regression,
origin, 320-input three-run determinism, anonymous-download and fresh-project
checks pass. The [compiler-read origin record](compiler-read-origin.md) binds
the exact artifacts and scopes; it also records the separate Mac host and
public Sail regression checks.

## Historical f8589ed4 qualification

The macOS release archive bundles a repair engine runtime built locally from
the pinned fork commit ([releasing](../releasing.md)). There is no CI coverage
on macOS, so this record is the qualification for that runtime. It uses the
same engine checks as the Linux runtime
([engine README](../../engine/verus/README.md#validation)).

| Field | Value |
|---|---|
| Status | **Pass** for the historical runtime, fork `f8589ed4` (R3): built, qualified and published; release smoke test not yet run on it |
| Source | Sail `candidate/r3-runtime` (`af163cae`, plus this record's pin commit), 2026-10-20; fork commit `f8589ed4345f147d8fb43d8cb7992921944b91f0` (`64baccbb` plus closure-name determinism, cost-ordered dispatch and the Fx hasher) on upstream `7d82994` |
| Runtime | `scripts/build-macos-runtime.sh build`, pinned in `engine/verus/artifact-macos.json`: variant `7d829946231d6413a398f6e4160cdfe951261dad-e9b69a75ea390499`, manifest sha256 `67589de3c3f9681fb50dcd96313311e1a3d2776241d121f6c0e433be54cf7f0c`, archive sha256 `6f523cea7595b82cc6e0f1434b8ff550b6295a3897e8301733511b78065fca12` (fork prerelease `engine-runtime-f8589ed4-macos`; the anonymous download matches), Verus Rust toolchain 1.99.1 without `rust-src`, no Verus features; stock Verus `0.2026.10.18.7d82994` (arm64-macos) and its Z3 4.16.0 |
| Build roots | Under `/tmp/sail-engine-f8589ed4`. vstd.vir: 10,215 `/rustc/58cf8f9c…` spans, 0 local. The archive scan found no home-directory path, IP address, account id or host name (one false positive: `Z3 4.16.0.0`) |
| Environment | EC2 mac-m4.metal, Apple M4 (10 cores, 24 GiB), macOS 27.0 (26A428), us-east-1d |
| Scope | The script's `build` and `published` steps: build with vstd verification; `test-engine.py`; `test-cancel.py`; the fork's library tests and every `sail_vi*` test plus `basic` on a fresh clone; `scripts/verus-determinism.py --runs 3` over 320 fast-tier inputs; the fast PR conformance subset on the runtime fetched from the release; `cargo verus new` |
| Limits | Correctness only, one run of each (determinism: 3); no performance measurement. The archive scan used the earlier `strings \| grep -q` predicate, which under `pipefail` can report a large file clean after an early match ([#555](https://github.com/Chelis-Lang/sail/issues/555)); this runtime has not been rescanned with `scripts/scan-host-paths.sh` |

## Results

| Check | Result | Evidence (sha256 of the log, kept outside the repository) |
|---|---|---|
| Build, vstd verification | 2,059 verified, 0 errors; built in 164 s | `engine-build.log` `691d5777…efe3a` |
| `test-engine.py` | 11 of 11 fixtures match the plain fork run exactly and stock on everything but raw SMT, which differs in 1 session (`recursive`) | `test-engine.log` `90301274…42b6` |
| `test-cancel.py` | no seal after a kill at a blocked solver check; the restart matches stock | `test-cancel.log` `0f6ac145…5988` |
| Fork library tests | `rust_verify` 18, `vir` 13, `air` 144 passed, 0 failed | `lib-rust_verify.log` `0843e908…2e3a`, `lib-vir.log` `4d160dd2…d63d`, `lib-air.log` `dba4056f…1000` |
| Fork regression tests | all 17 `sail_vi*` targets and `basic`, 0 failed | `sail-vi.log` `e394181f…2205` |
| Determinism | solver input identical in every bucket of all 320 inputs over 3 runs; stdout, stderr and exit code identical for 319. HumanEval's `tasks/lib.rs` reported a different `rlimit-run` (260,152,849 and 260,152,832) from byte-identical solver input: Z3's own count ([z3-nla-rlimit-nondeterminism](../../corpus/repros/z3-nla-rlimit-nondeterminism/README.md)), as with `9c0ca7e7` | `determinism.log` `d9932019…c95a` |
| Conformance, fast PR subset | pass-through, cache fill and cache warm (3,442 entries): 33 units, 33 same, 0 different, 311 functions each. Engine arm: 33 same, 0 different, 32 focused units, engine `67589de3c3f9` | `r3pub.log` `20cdaadc…85b3` |
| `cargo verus new` (VI-0018) | `vstd = "=0.0.0-2026-10-18-0147"`; `cargo verus verify` 1 verified, 0 errors | `r3pub.log` |

All logs: `r3-evidence.tgz` `5166d8ea…6568`.

## Earlier runtimes

### Runtime `64baccbb`

| Field | Value |
|---|---|
| Status | **Pass** for the then pinned runtime, fork `64baccbb`: built, qualified and published |
| Source | Sail `feat/vir-span-remap` (`466da4dc`, plus this record's pin commit), 2026-10-20; fork commit `64baccbb1f4c8af3f5e015d4ac561c96058717a7` (`fix/vir-span-remap`: span-string remap, stock `dbg!` location) on upstream `7d82994` |
| Runtime | `cargo xtask build-verus-engine`, pinned in `engine/verus/artifact-macos.json`: variant `7d829946231d6413a398f6e4160cdfe951261dad-7faa35e5238b9bfc`, manifest sha256 `4392062ae1674fb473818d3bb730b4a2d2dba831dd2e9dddf98c5c57a8ed91fe`, archive sha256 `9f9381d53e1478aaae589d7d00399d01f5fee4e56daf81d9e4cde03e5dda69c2` (fork prerelease `engine-runtime-64baccbb-macos`; the anonymous download matches), Verus Rust toolchain 1.99.1 without `rust-src`, no Verus features; stock Verus `0.2026.10.18.7d82994` (arm64-macos) and its Z3 4.16.0 |
| Build roots | `RUSTUP_HOME`, `CARGO_HOME` and `SAIL_CACHE_DIR` under `/tmp/sail-engine-64baccbb`. vstd.vir names the standard library only by `/rustc/58cf8f9c…` (10,215 spans, 0 under `rustlib/src/rust`). A strings scan of every archive file found no `/Users/` or `/home/` path, IP address, account id or host name (one false positive: `Z3 4.16.0.0` in the pinned Z3) |
| Environment | EC2 mac-m4.metal, Apple M4 (10 cores, 24 GiB), macOS 27.0 (26A428), us-east-1d, a fresh instance |
| Scope | As for `9c0ca7e7` below: build with vstd verification; `test-engine.py`; `test-cancel.py`; the fork's library tests and every `sail_vi*` test plus `basic` on a fresh clone; `scripts/verus-determinism.py --runs 3` over 320 fast-tier inputs; `ci/run.sh conformance fast pr` on the runtime fetched from the release; `cargo verus new` |
| Limits | Correctness only, one run of each (determinism: 3); no performance measurement |

### Results (`64baccbb`)

| Check | Result | Evidence (sha256 of the log, kept outside the repository) |
|---|---|---|
| Build, vstd verification | 2,059 verified, 0 errors; built in 165 s | `engine-build.log` `d600e1d0…0d27` |
| `test-engine.py` | 11 of 11 fixtures match the plain fork run exactly and stock on everything but raw SMT, which differs in 1 session (`recursive`) | `test-engine.log` `90301274…42b6`, `summary.json` `6cfbb605…de43` |
| `test-cancel.py` | no seal after a kill at a blocked solver check; the restart matches stock | `test-cancel.log` `0f6ac145…5988` |
| Fork library tests | `rust_verify` 18, `vir` 11, `air` 144 passed, 0 failed | `lib-rust_verify.log` `9dc63b70…9a22`, `lib-vir.log` `f585af03…02ad`, `lib-air.log` `51e99ed4…1fa8` |
| Fork regression tests | all 15 `sail_vi*` targets (including `sail_vi0031_vir_span_remap`) and `basic`, 0 failed | `sail-vi.log` `1a9c7ec5…63a5` |
| Determinism | 320 of 320 inputs identical over 3 runs in solver input, blanked stdout, stderr and exit code | `determinism.log` `494f4f3d…d334` |
| Conformance, fast PR subset | pass-through, cache fill and cache warm (3,442 entries): 33 units, 33 same, 0 unstable, 0 different, 0 infra errors, 311 functions each. Engine arm: 33 same, 0 different, 32 focused units, engine `4392062ae167` | `conformance-pr.log` `32c68a16…524c` |
| `cargo verus new` (VI-0018) | `vstd = "=0.0.0-2026-10-18-0147"`; `cargo verus verify` 1 verified, 0 errors | `cargo-verus-new.log` `512cf1d1…da98` |

All logs: `mac64-evidence.tgz` `c6b3839c…18de`.

### Runtime `9c0ca7e7`

| Field | Value |
|---|---|
| Status | **Pass** for the then pinned runtime, fork `9c0ca7e7`: built, qualified and published |
| Source | Sail `repin/7d82994` (`362235c4`, plus this record's pin commit), 2026-10-19; fork commit `9c0ca7e75f20c8ae7120196efd760b64a61251aa` on upstream `7d82994` |
| Runtime | `cargo xtask build-verus-engine`, pinned in `engine/verus/artifact-macos.json`: variant `7d829946231d6413a398f6e4160cdfe951261dad-8f917633f244b095`, manifest sha256 `5a6114d3cbf0028e42cb182e0810b6a07ccbd10076b09be3b5b63c0e05016758`, archive sha256 `3b771f41a602de8c0d1f31c622e8bf507320c6c9343f5c0e097328361a44ff4b` (fork prerelease `engine-runtime-9c0ca7e7-macos`; the anonymous download matches), Verus Rust toolchain 1.99.1 without `rust-src` ([VI-0031](../verus-issues/VI-0031-rust-src-std-paths.md)), no Verus features; stock Verus `0.2026.10.18.7d82994` (arm64-macos) and its Z3 4.16.0 |
| Build roots | `RUSTUP_HOME`, `CARGO_HOME` and `SAIL_CACHE_DIR` under `/tmp/sail-engine-9c0ca7e7`, so no home-directory path is compiled into the runtime (panic locations and vstd spans name the build roots). vstd.vir names the standard library only by `/rustc/58cf8f9c…` (10,215 spans, as in the release's vstd.vir). A strings scan of every archive file found no `/Users/` or `/home/` path, IP address, account id or host name (one false positive: `Z3 4.16.0.0` in the pinned Z3) |
| Environment | EC2 mac-m4.metal, Apple M4 (10 cores, 24 GiB), macOS 27.0 (26A428), us-east-1d, a fresh instance with rustup, Homebrew Python 3.12 and this checkout added |
| Scope | Engine build with vstd verification; `test-engine.py` (11 fixtures, three arms); `test-cancel.py`; the fork's `rust_verify`, `vir` and `air` library tests and every `sail_vi*` test plus `basic`, on a fresh clone of the fork commit; `scripts/verus-determinism.py --runs 3` over the fast tier's direct inputs; `ci/run.sh conformance fast pr` with `SAIL_ENGINE_BUILD` set to the runtime fetched from the release (`ci/engine-artifact.py fetch`); `cargo verus new` with the runtime |
| Limits | Correctness only, one run of each (determinism: 3); no performance measurement. The fast PR subset (33 guide units) for the conformance arms |

### Results (`9c0ca7e7`)

| Check | Result | Evidence (sha256 of the log, kept outside the repository) |
|---|---|---|
| Build, vstd verification | 2,059 verified, 0 errors (as the release does: `cargo-verus build` of vstd); built in 185 s | `engine-build.log` `cc6761cd…889e` |
| `test-engine.py` | 11 of 11 fixtures match the plain fork run exactly (raw SMT included) and stock on exit, diagnostics, counters, verdicts and sessions; raw SMT differs from stock in 1 session (`recursive`, the fork's accepted SMT spellings, ADRs 0054 and 0056) | `test-engine-cancel.log` `9777ad53…d4e9`, `summary.json` `e70aac14…b34f` |
| `test-cancel.py` | no seal after a kill at a blocked solver check; the restart matches stock | `test-engine-cancel.log` |
| Fork library tests | `rust_verify` 17, `vir` 11, `air` 144 passed, 0 failed | `lib-rust_verify.log` `c20342b1…2f64`, `lib-vir.log` `49e4552e…9f7a`, `lib-air.log` `f7e2a986…b216` |
| Fork regression tests | all 14 `sail_vi*` targets (27 tests) and `basic` (62 passed, 1 ignored), 0 failed | `sail-vi.log` `7438ee71…5b8e` |
| Determinism | 320 inputs (every `examples/**/*.rs` except `integer_ring` and `scache`, and HumanEval's `tasks/*.rs`), 3 runs each: solver input identical in every bucket of every input; stdout, stderr and exit code identical for 319. `examples/imo_1988_6.rs` reported a different `rlimit-run` (82,320 and 82,318) from byte-identical solver input: Z3's own count ([z3-nla-rlimit-nondeterminism](../../corpus/repros/z3-nla-rlimit-nondeterminism/README.md), reproduced on this host: 41,644 and 41,649), and stock `verus` varies the same way (82,312 to 82,320 in 6 runs) | `determinism.log` `259df3d1…4a70` |
| Conformance, fast PR subset | pass-through, cache fill and cache warm (3,442 entries): 33 units, 33 same, 0 unstable, 0 different, 0 infra errors, 311 functions each. Engine arm: 33 same, 0 unstable, 0 different, 0 infra errors, 32 focused units, engine `5a6114d3cbf0` | `conformance-pr.log` `d92f0495…38d2` |
| `cargo verus new` (VI-0018) | with the runtime on `PATH`: project created with `vstd = "=0.0.0-2026-10-18-0147"`, `cargo verus verify` 1 verified, 0 errors | `cargo-verus-new.log` `919b560c…4d9e` |

All logs: `mac1019-engine-evidence-full.tgz` `5e6b74bc…0da2`.

The runtime pinned before `9c0ca7e7` was fork `cbbcdf44` (prerelease
`engine-runtime-cbbcdf44-macos`). The last full record before that, for
fork `73dea021`:

| Field | Value |
|---|---|
| Status | **Pass** for the then pinned runtime, fork `73dea021`: built, qualified and published |
| Source | Sail `chore/repin-73dea021` (lane L43), 2026-10-15; fork commit `73dea02112d730e7ab52f58158012d440fb89512` on upstream `426d8b0` |
| Runtime | `cargo xtask build-verus-engine` (writes Verus's toolchain manifest, VI-0018), pinned in `engine/verus/artifact-macos.json` (archive sha256 `0819e62f…1fb0`, fork prerelease `engine-runtime-73dea021-macos`), variant `426d8b01e7ffb910a36c15e1f870ff985bb61eef-c1eeaec18522f7e7`, manifest sha256 `a42a7d1914332d3e3c10f53c8fab40eb6f4da39093b71c8f39d999eaf3d0b5c8`, version `0.2026.10.15.73dea02`, Verus Rust toolchain 1.98.1, no Verus features; stock Verus `0.2026.10.04.426d8b0` (arm64-macos) and its Z3 4.16.0 |
| Environment | Developer Mac, Apple M3, macOS 26.5, shared with other lanes (load average about 10–23) |
| Scope | Engine build with vstd verification; `test-engine.py` (11 fixtures); `test-cancel.py`; `ci/run.sh conformance fast pr` on the host with `SAIL_ENGINE_BUILD` and `SAIL_VERUS_BUILD` set to the build (pass-through, cache fill and warm, engine arm); `cargo verus new` with the runtime |
| Limits | Correctness only, one run of each; no performance measurement. The fast PR subset (33 guide units) for the engine arm; the full fast tier pass-through is in the [fork-default record](fork-default.md) |

## Results

| Check | Result | Evidence (sha256 of the log, kept outside the repository) |
|---|---|---|
| Build, vstd verification | 2,060 verified, 0 errors (as the release does: `cargo-verus build` of vstd); built in 245 s | `mac-build.log` `f1bf5a8a…8294` |
| `test-engine.py` | 11 of 11 fixtures match stock (exit, stderr, counters, verdicts, raw SMT) | `mac-test-engine.log` `0819d89b…098c` |
| `test-cancel.py` | no seal after a kill at a blocked solver check; the restart matches stock | `mac-test-cancel.log` `0f6ac145…5988` |
| Conformance, fast PR subset | pass-through, cache fill and cache warm (3,442 entries): 33 units, 33 same, 0 unstable, 0 different, 0 infra errors, 311 functions each; report `sail_verus: fork:a42a7d19…`. Engine arm: 33 same, 0 unstable, 0 different, 0 infra errors, 32 focused units, engine `a42a7d191433` | `mac-conformance-pr.log` `76c3d593…28d3` |
| `cargo verus new` (VI-0018) | with the runtime on `PATH`: project created with `vstd = "=0.0.0-2026-10-04-0306"`, `cargo verus verify` 1 verified, 0 errors | `mac-cargo-verus-new.log` `9d0ad17e…bccb` |

The earlier runtime, fork `13a1176c` (archive `54a0c51f…7fa8`, manifest
`c95d5f9a…dce1`), passed the same checks on 2026-10-14 (Sail `dc9dec4`, lane
L32; logs `engine-build.log` `1dba6c5c…07fc`, `conformance.log`
`e4aeb570…9fe3`), and the two sections below.

## Re-qualification on clean hardware (runtime `13a1176c`)

The same checks on a dedicated, otherwise idle Mac, with the then pinned runtime
fetched anonymously from the fork release (`ci/engine-artifact.py fetch`,
archive digest and every runtime file checked):

| Field | Value |
|---|---|
| Status | **Pass** |
| Source | Sail `main` `4c1392a`, 2026-10-14 (lane L39); runtime `13a1176c` (manifest `c95d5f9a686d…`) |
| Environment | EC2 mac-m4.metal, Apple M4 (4 performance and 6 efficiency cores, 24 GiB), macOS 27.0 (26A428), us-east-1b, a fresh instance with only rustup, the stock Verus release and this checkout added; load average 4–7 during the checks (the checks' own work; about 1.5 from macOS services when idle) |
| Scope | Engine source build with vstd verification; `test-engine.py`; `test-cancel.py`; `ci/run.sh conformance fast pr` with `SAIL_ENGINE_BUILD` set to the fetched pinned runtime. One run of each, correctness only |

| Check | Result |
|---|---|
| Pinned runtime | fetched and validated; `sail engine status` selects it (manifest `c95d5f9a686d…`) |
| Build, vstd verification | `cargo xtask build-verus-engine`: 2,060 verified, 0 errors, 165 s. Same variant name and provenance as the pin, but not bit-identical: 5 of 14 runtime files match, and the manifest digest is `5ad93bc0…1175`. The pinned runtime, not this build, was used below |
| `test-engine.py` | 11 of 11 fixtures match stock |
| `test-cancel.py` | no seal after a kill at a blocked solver check; the restart matches stock |
| Conformance, fast PR subset | pass-through, cache fill and cache warm (3,504 entries): 33 units, 33 same, 0 different, 311 functions each. Engine arm: 33 same, 0 unstable, 0 different, 0 infra errors, 32 focused units, engine `c95d5f9a686d` |

Logs: `l39-mac-engine.tgz` `78845938…8b3d`, holding `engine-build.log`
`c26b6296…bc14`, `test-engine.log` `0819d89b…098c`, `test-cancel.log`
`0f6ac145…5988` and `conformance.log` `e4c1cfe8…81ee`.

## Release smoke test (runtime `13a1176c`)

`scripts/release-smoke.sh` on the draft `v0.0.0-preview.3` archives, built by
`scripts/release.sh` from `14a5445` (rebased on `main` `6f22621`, which
includes the macOS managed-reuse fix #99), each in a fresh directory with an
empty `SAIL_CACHE_DIR`, the stock Verus release on `PATH` and Rust 1.98.1. The
macOS archive carries the pinned runtime fetched anonymously from the fork
release:

| Archive (sha256) | Where | Result |
|---|---|---|
| `x86_64-unknown-linux-gnu` (`f303e79c…04ff`) | fresh native EC2 c7i.4xlarge, Ubuntu 24.04, us-east-1, with only rustup and the Verus release added | bundled engine selected; `sail verify` passed; `repair-check demo::max` accepted through the bundled engine (`linux-smoke3.log` `5618872c…5347`) |
| `aarch64-apple-darwin` (`bc38f47a…addb`) | the developer Mac above, new directory under `/tmp` | bundled engine selected; `sail verify` passed; `repair-check demo::max` accepted through the bundled engine (`mac-smoke3.log` `bd7c3763…aec0`) |

## Requalification

A new fork commit, Verus pin or Rust toolchain changes the variant name and the
manifest digest, so a new macOS runtime needs a new run of every check above
before it ships. Failed or interrupted attempts are listed here, one line each.

Attempts:
- `091a43f4`, 2026-10-19, fork `9c0ca7e7` build with the default `SAIL_CACHE_DIR`: vstd.vir named the build home's cache (`/Users/…/.cache/sail/…`, 511 spans); discarded unpublished (`attempt1-engine-build.log` `8e78d90b…38d2` in the evidence tarball).
- `362235c4`, 2026-10-19, rebuilt with the cache under `/tmp` but the default Rustup and Cargo homes: the archive scan found `/Users/…/.cargo` and `/Users/…/.rustup` panic locations in `verus`, `rust_verify`, `cargo-verus` and the macro libraries; refused, discarded unpublished (`attempt2-engine-build.log` `03313e3d…4cdf`). The pinned build moves all three roots under `/tmp`.
- `dc9dec4`, 2026-10-14, smoke on macOS: `SAIL0810` job endpoint did not start; the smoke directory under macOS's default `TMPDIR` made the socket path too long. The script now uses `/tmp`.
- `81de0b8`, 2026-10-14, smoke of `v0.0.0-preview.2` (before #99): Linux accepted; macOS verified through the bundled engine (exit 0, no fault) but not accepted, full evidence Historical, also with the engine installed in the cache. Passed after rebasing on #99 (above).
