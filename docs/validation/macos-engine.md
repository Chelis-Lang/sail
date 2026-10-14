# macOS engine qualification

The macOS release archive bundles a repair engine runtime built locally from
the pinned fork commit ([releasing](../releasing.md)). There is no CI coverage
on macOS, so this record is the qualification for that runtime. It uses the
same engine checks as the Linux runtime
([engine README](../../engine/verus/README.md#validation)).

| Field | Value |
|---|---|
| Status | **Pass**: the macOS engine ships in the release archive, and a repair through it is accepted |
| Source | Sail `l32/release` `dc9dec4` (lane L32; qualification checks), 2026-10-14; fork commit `13a1176c02d461f166affd60591a7c3aa1a67134` on upstream `426d8b0` |
| Runtime | `cargo xtask build-verus-engine`, pinned in `engine/verus/artifact-macos.json` (archive sha256 `54a0c51f…7fa8`, fork prerelease `engine-runtime-13a1176c-macos`), variant `426d8b01e7ffb910a36c15e1f870ff985bb61eef-52c2ace7480faf24`, manifest sha256 `c95d5f9a686d1f4d101be0ec9f9ebf81809629d1f0246542d1a156041beadce1`, Verus Rust toolchain 1.98.1, no Verus features; stock Verus `0.2026.10.04.426d8b0` (arm64-macos) and its Z3 4.16.0 |
| Environment | Developer Mac, Apple M3, macOS 26.5 (25F71), shared with other lanes (load average about 16–23) |
| Scope | Engine build with vstd verification; `test-engine.py` (11 fixtures); `test-cancel.py`; `ci/run.sh conformance fast pr` on the host with `SAIL_ENGINE_BUILD` set to the build (pass-through, cache fill and warm, engine arm) |
| Limits | Correctness only, one run of each; no performance measurement. The fast PR subset (33 guide units) only; the standard and deep tiers were not run on macOS |

## Results

| Check | Result | Evidence (sha256 of the log, kept outside the repository) |
|---|---|---|
| Build, vstd verification | 2,060 verified, 0 errors (as the release does: `cargo-verus build` of vstd); built in 546 s | `engine-build.log` `1dba6c5c…07fc` |
| `test-engine.py` | 11 of 11 fixtures match stock (exit, stderr, counters, verdicts, raw SMT): `long_worker_span`, `success`, `failed_focus`, `failed_other`, `rust_frontend_error`, `late_exec`, `bitvector`, `recursive`, `macro_spans`, `missing_function`, `ambiguous_function` | `test-engine.log` `0819d89b…098c` |
| `test-cancel.py` | no seal after a kill at a blocked solver check; the restart matches stock | `test-cancel.log` `0f6ac145…5988` |
| Conformance, fast PR subset | pass-through, cache fill and cache warm (3,635 entries): 33 units, 33 same, 0 different, 311 functions. Engine arm: 33 units, 33 same, 0 unstable, 0 different, 0 infra errors, 32 focused units, engine `c95d5f9a686d` | `conformance.log` `e4aeb570…9fe3` |

## Release smoke test

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
- `dc9dec4`, 2026-10-14, smoke on macOS: `SAIL0810` job endpoint did not start; the smoke directory under macOS's default `TMPDIR` made the socket path too long. The script now uses `/tmp`.
- `81de0b8`, 2026-10-14, smoke of `v0.0.0-preview.2` (before #99): Linux accepted; macOS verified through the bundled engine (exit 0, no fault) but not accepted, full evidence Historical, also with the engine installed in the cache. Passed after rebasing on #99 (above).
