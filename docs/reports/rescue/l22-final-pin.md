# Rescue lane L22: final engine pin and the M7 acceptance benchmark

- **Sail branch:** `l22-final-pin`, on `rescue/m7`. Gates and the benchmark
  ran at `d187e49` (on `rescue/m7` `380d420`, which includes L19 `b20e226`,
  patch-identical to `0aa19a5`). The branch was then rebased onto L23
  (`a078d84`) and the owner's speed acceptance (`0987471`) for landing; L23's
  code was not measured here.
- **Fork:** [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus), branch
  `l22/engine` at `13a1176c`, for the lead to move `sail` onto and tag
  `pinned/13a1176c`. It is `d2b69aea`'s fixes, then L21's VI-0001 verifier
  fix `af212563`, then the unchanged `feat: interactive repair engine`. Its
  tree differs from `d2b69aea` only in `source/rust_verify/src/rust_to_vir_impl.rs`
  and the new test `sail_vi0001_impl_accessor_order.rs`.
- **Runtime:** release `engine-runtime-13a1176c-linux`, asset `636795070`,
  archive sha256 `a28e212e07dd4fbf25dfa6f94b84faa0ec258502706327e071841778bd59d7c2`,
  variant `426d8b01e7ff…-9a788f8bd626312c`, pinned in
  [artifact-linux.json](../../../engine/verus/artifact-linux.json) (schema 2)
  and [fork.toml](../../../engine/verus/fork.toml).
- **Host:** EC2 `c7i.8xlarge` (32 vCPU), us-east-1, lane `l22`, Ubuntu 24.04,
  kernel 7.0.0-1014-aws, native x86-64. Launched with
  `scripts/perf-host.sh up l22 c7i.8xlarge` and torn down after use.
- **Raw evidence:** `~/sail-raw-evidence/l22/` on the lead's machine, cited
  by path inside its archives and SHA-256 prefix. `SHA256SUMS` covers every
  archive.

## Procedure

As L12 ([report](l12-fork-fixes.md)):

1. A CI image was built from the then-committed `d2b69aea` pin to host the
   build (`sail-ci:base-679d9641401c264c`, host-only).
2. `cargo xtask build-verus-source --interactive-engine --keep-intermediates`
   built `13a1176c` in that image (170 s).
3. The runtime was packed with `ci/engine-artifact.py pack` before any test
   ran in the build tree, uploaded as a private prerelease, and pinned.
4. The CI image was rebuilt from the committed inputs (`ci/local.sh`):
   `sail-ci:bc8748852984b813` downloaded, validated and installed the runtime.
   Its manifest sha256 `5b866414…7603` is the engine identity every engine
   arm below reports.

## Qualification of `13a1176c` (Gate 1)

Evidence: `gates-and-qual.tar.gz`.

| Check | Result | Evidence |
|---|---|---|
| Build | vstd 2,060/2,060, both vargo and release-style | `logs/step1.log` `33b280cc` |
| `test-engine.py` | 11/11 fixtures match stock | `q/test-engine.log` `0819d89b` |
| `test-cancel.py` | no seal after the kill; the restart matches stock | `q/test-cancel.log` `0f6ac145` |
| Library tests | `rust_verify` 12, `vir` 1, `air` 144 passed | `q/summary.txt` `e012c115` |
| `rust_verify_test` | 4,645 passed, 2 failed, 58 ignored; all 10 `sail_vi*` tests pass, including both VI-0001 tests | `q/rvt-full.log` `7eb60e79` |

- The two failures are the known `cargo_new_builds` and `cargo_new_verifies`:
  "no known toolchain manifest for Verus version 0.2026.10.13.13a1176". A
  source build has no release manifest; no Sail path uses `cargo verus new`.
- The fixture logs are byte-identical to L12's on `d2b69aea`.
- **Failed attempt:** the first `rust_verify_test` run stopped before
  testing. I had set `VERUS_Z3_PATH` to vargo's own copy destination
  (`q/rvt-attempt1-z3path.log`). The rerun used the pinned stock Z3, which
  has the same sha256.

## Gates on the host (Gates 3 and 6)

Image `sail-ci:bc8748852984b813`, Sail `d187e49`, engine runtime `13a1176c`.
Evidence: `gates-and-qual.tar.gz` (`g/`) and `conformance-out.tar.gz`.

| Step | Result | Evidence |
|---|---|---|
| `gate` | pass, 535/535 tests | `g/gate.log` `0579eed9` |
| `solver` | pass, 35/35 | `g/solver.log` `abdf6c8d` |
| `e2e` | legacy, managed and repair phases pass | `g/e2e.log` `fb75a17e` |
| `conformance fast pr` | 33/33 Same in pass-through, fill, warm and engine | `g/conformance-fast-pr.log` `36086cc7` |
| `conformance fast` | 147/147 Same (3,450 functions) in pass-through, fill, warm and engine | `g/conformance-fast.log` `7a3d73d9` |
| `conformance standard` | 159/159 Same (12,726 functions) in pass-through, fill, warm and engine | `g/conformance-standard.log` `5895a426` |

- **Engine arm:** 0 Unstable and 0 Different in every tier, compared under
  ADR 0042. 146 of the 147 fast units and 158 of the 159 standard units ran
  a focused phase.
- **Unstable units:** none, in any arm.
- **rlimit drift (not a difference):** VNR
  `lib::spec::cyclicbuffer::log_entry_alive_value_wrap_around` reported
  51,945 against 51,948 in standard pass-through, warm and engine. Verdicts
  and diagnostics were identical.
- **Interrupted attempt:** a first run at `fc37e4a` (before L19) passed
  `gate` (534/534), `solver` and `e2e`. I stopped it during the PR subset to
  rebase onto L19 (`g-pre-l19/`).

## M7 acceptance benchmark (Gate 7)

`sail-bench repair`, three rounds, arm order alternating. M6 was
`26f124f` (`rescue/m6-baseline`), M7 `d187e49` with runtime `13a1176c`.
Every sample is valid: one driver, Current, 0 failing samples, 0 warm-up
infrastructure failures. The owner accepted M7's speed as measured (ADR 0038
amendment), so this run only records the figures.

| Catalogue | M6 whole loop, median (min–max) | M7 whole loop, median (min–max) | Speedup | Gate at report time |
|---|---:|---:|---:|---|
| human-eval-verus | 64.7 s (64.6–67.3) | 30.3 s (30.1–30.6) | **2.14×** | pass |
| vest | 246.6 s (246.4–249.6) | 135.1 s (135.0–138.8) | **1.82×** | `SAIL7109` |

- **Per cycle:** HumanEval cycles take 5.3–6.5 s on M7, against 11.3–15.9 s
  on M6. Vest cycles take 22.9–28.8 s, against 41.1–52.1 s.
- **First actionable:** M7 is about 3.5 s on HumanEval (M6 about 6.0 s) and
  about 13.5 s on Vest (M6 about 22.5 s).
- **Invalid samples:** none.
- **Compared with L12's `d2b69aea` run on the same instance type:**
  HumanEval 2.13× and Vest 1.80× (the L19 and L23 runs gave 2.14×/1.81× and
  2.11×/1.80×).

**Result file.** `bench/results/repair-acceptance-l22-final.json` replaces
`repair-acceptance-l23.json`, whose sha256 is
`2f97b35ebb1a2a48c54110751accf2a9fa011c1abbd487cb19e3ed5cf5f22028`. It is kept
outside the repository; [benchmarks](../../benchmarks.md) is regenerated.

**Raw evidence** (`bench/`):
- `run.log` `569eeda2`;
- `bench-out.tar.gz` `bed064a1`, every invocation's command, stdout,
  stderr and journal pages;
- `binaries.sha256` `0d89bdbf`.

**Failed attempt.** The first start passed the M7 binary as a relative path.
The harness resolves it from the project directory, so the M7 warm-up was an
infrastructure failure at 0 ms (`attempt1-relative-m7.log` `f5ea1c33`). I
stopped the run and restarted it with an absolute path; no measured sample
had been taken.

## L18's changed-bucket determinism count on the new engine

**Method.** L18's driver and analysis (`measure.py`, `analyze.py`), with
only their paths changed, ran in the same CI image on a separate host
checkout. The engine was a host-only build of `l22/timers` (`9cbe2c7f`): the
two `m8/timers` commits, which add per-bucket SMT digests under
`--time-expanded`, cherry-picked onto `13a1176c`. That build is not pinned.
The count covers every HumanEval repair cycle and agent-loop step, and every
Vest cycle, with a warm-up per state and three measured rounds. A bucket
varies when a measured run sends an SMT digest that is not among the digests
from the same state's warm-up.

| Catalogue | Measured samples with Verus output | Samples with a varying bucket | L18 on `89709da7` |
|---|---:|---:|---|
| human-eval-verus (12 states) | 28 | **0** | `human_eval_160` in 16 of 32 |
| vest (6 states) | 14 | **0** | `cbor::value` in 10, `asn1::integer` in 4 |

- No bucket sent more than one digest for the same state, across warm-ups
  and rounds (`count.txt`).
- The median number of changed buckets per cycle is 1 on HumanEval repair,
  HumanEval loops and Vest (L18: 1.5, 1 and 2).
- **Without Verus output:** 5 HumanEval samples and 1 Vest sample ran no
  verifier, because Cargo found the target fresh (exit 0 in 0.02 s, the
  state's text having been built earlier). They have no buckets to compare.
- Evidence: `determinism.tar.gz` (`out/runs.jsonl`, every run's output,
  `out/analysis.txt`, `count.txt`, `setup.log`).

## Evidence

`~/sail-raw-evidence/l22/` on the lead's machine; `SHA256SUMS` covers every file.

| File | Contents | sha256 |
|---|---|---|
| `gates-and-qual.tar.gz` | qualification (`q/`), gate logs (`g/`, `g-pre-l19/`), build and image logs, packed pin | `a4fcd8129b2fa600278f8782441a4cb9783f315de55edb3a7f951f294d8539dd` |
| `conformance-out.tar.gz` | every conformance report and unit output (PR, fast, standard; four arms) | `318c40c94dba897ba22418ee44112ce8da55ead9fbbf67cbeccbb0e600a51180` |
| `bench/run.log` | benchmark log | `569eeda2d7f0e5aed1a153369381ad0b56f5deab76d5d6a5322974ed1b20e270` |
| `bench/bench-out.tar.gz` | every benchmark invocation | `bed064a18f9dd1586c25bf70e661f200d6254013430e36e40045c1386b6ccdf9` |
| `bench/attempt1-relative-m7.log` | the stopped first benchmark start | `f5ea1c331448b6b4c1a8908e44572d0d834d0d4b8230991d3e7ce374a0e792cf` |
| `determinism.tar.gz` | determinism count | `801d15a6a897b49ff349550b232832c6e656e38d245ef2b5c72c30260c7cf148` |
| `l22-scripts.tar.gz` | every host script used | `c42b4497d7c5f843db7113e7c14fe4d54ce32eebabc6296a6d78f04055a4569d` |

No file contains a credential; the token reached the host only on stdin and
through BuildKit secrets.

## Remaining for the lead

- Review fork branch `l22/engine` (`13a1176c`), move `sail` to it, and tag
  `pinned/13a1176c`. `l22/timers` (`9cbe2c7f`, the `m8/timers` commits on
  `13a1176c`) is a measurement branch only and is not pinned.
- After the pin merges, delete release `engine-runtime-d2b69aea-linux`; I
  will do this when told.
