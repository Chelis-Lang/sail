# Real-project memory instrumentation (#562)

Status: implemented; native Linux and macOS controls pass. Release-wide
memory acceptance remains open: the final candidate still needs the four
projects on both platforms and a complete sampler overhead qualification.
The [testing guide](../testing.md) defines the columns and measurement limits.

## Source and scope

The instrumentation candidate `ce72c778` is based on main `727a6a97` and
changes benchmark tooling, its CI dependencies and native source-sync
exclusions. It changes no Rust production code, corpus entry, solver input,
verdict or diagnostic check. The restack `a7eac428` is based on `39cff12d`;
its instrumentation and source-sync program bytes are unchanged.

| Environment | Executed scope | Result |
|---|---|---|
| Native Linux x86-64, c7i.8xlarge, us-east-1, `ce72c778` | Full image gate, pinned solver, full local E2E including extra, PR conformance | Gate passes; 1,225 workspace tests and 51 solver tests pass; every E2E phase passes; all four conformance arms match 33/33 units, 311 functions, zero unstable/different/infra errors |
| Native arm64 macOS 27.0, mac-m4.metal, us-east-1b, `ce72c778` | 24 real Git/rsync controls; memory 10, command 16 and report 5 controls; full `ci/run.sh host` | All controls pass; strict Clippy and 1,214 nonignored workspace tests pass, 54 skipped |
| Native Linux, same lane, `a7eac428` | Fresh full image gate, solver, full local E2E including extra and PR conformance | All pass: 1,239 workspace and 51 solver tests; all four arms match 33/33 units and 311 functions, zero unstable/different/infra; 54 workspace skips |

Both platforms use Sail Rust 1.99.0. Solver/E2E/conformance use fork
`47fb3be6` with pinned Z3 and stock Verus `0.2026.10.18.7d82994` as oracle.
Memory controls include allocation in children, concurrent tree sums,
pre-existing and detached daemons, PID reuse, signal and exit propagation,
platform unit conversion, readiness, old-row compatibility and refusal of
malformed reports. A 50 ms sample is an observation, not an exact peak;
Linux `VmHWM` is a separate lifetime observation.

## Overhead control

On a separate native c7i.8xlarge lane, the same primed sampler program runs
with and without process sampling. Its readiness gate excludes setup from
both wall intervals; the command's arguments and environment stay identical.
The staged Rust candidate is `1dc5f1a5`, and the harness is `fbf4c3f9`.
This control does not qualify product performance on the restacked runtime.

Three alternating pairs on a warm `sail new` library give:

| Phase | Disabled median, s | Sampled median, s | Absolute difference, s | Largest within-arm range, s |
|---|---:|---:|---:|---:|
| Unchanged, full path | 0.185657 | 0.183111 | 0.002546 | 0.016039 |
| Tiny edit | 0.978989 | 0.990613 | 0.011624 | 0.053335 |

The three-round IronKV matrix retains all nine phases in both stock and
Sail arms, including the expected failing edit. Sail's first unchanged
median changes from 0.221 to 0.229 s, inside the observed 0.025 s range;
its replay-disabled median changes from 0.181 to 0.187 s, inside 0.007 s.
Stock's first unchanged changes from 0.112 to 0.117 s, slightly above its
0.004 s range. That row does **not** establish overhead below noise; more
paired observations are required before release memory acceptance. All
samples, including the slow revert outliers, remain retained.

| Date | Source | Attempt | Result |
|---|---|---|---|
| 2026-10-29 | `228b205f` combined candidate | First Mac memory controls | Pre-existing daemon control fails: Darwin re-exec changes the command spelling; authoritative status PID now seeds its observed start identity |
| 2026-10-29 | Pre-priming combined candidate | Linux overhead | Rejected: Python setup adds about 40 ms; readiness now occurs before the clock |
| 2026-10-29 | `326075f6` combined candidate | Refreshed image gate | Command control fails before performance work: CI image lacks jq; image now supplies it |

Raw evidence remains outside Git, identified by SHA-256:

| Artifact | SHA-256 |
|---|---|
| `review-current.log` | `fabafd101f6242aa62ee3ad42a2014d02f801861640b24cdbbb39783310878b5` |
| `review-instrumentation.log` | `e50416686ccb294bb0ffd2e5e4a26ef46d491505076a0f81edb7ab0722e04e16` |
| `mac-instrumentation.log` | `04cff1faf2282060a9eb12af6e1e61456a7101d42620e62a5681e19cb8a4f3db` |
| `overhead-r3.log` | `4ba2310614ddd93152e92fb6fa614b50d6f21a27cc023d2837c2ea2f75c5efb6` |
| `memory-overhead.tgz` | `26ac0f3edf164ef293e4710d633a5622a9041e9ec4b3bf51a7e608030e921aca` |
| First Mac controls | `b27e4f980553b030a8c9855e8221a624b5baeb1535d2f42418de0ca146e6028f` |

## Command-clock correction candidate (2026-10-30)

A rotated three-pair IronKV rerun at `7ceb6423` retains 108 samples. Seventeen
of eighteen arm/phase strata satisfy the original within-range requirement.
Stock's first unchanged row does not: disabled median 0.142 s, sampled
0.109 s, absolute difference 0.033 s versus a 0.009 s largest arm range.
This does not establish sampler overhead within noise; it does not identify
a cause. Earlier failed controls remain above.

Source review shows the enclosing clock includes sampler finalization and
after-command harness work, and `SAIL_PERF_MEMORY` reaches the child despite
being an instrumentation switch. The correction captures a monotonic spawn
through `wait4` interval inside the sampler and removes that switch before
starting either arm. RSS sampling and every result remain intact. A missing
or invalid clock retains a refused sample and fails acceptance. Existing
19/22-column reports keep their original measurements; the correction does
not rewrite them.

New controls compare complete child-environment hashes and literal arguments
across both sampling modes, and delay sampler teardown after a real child
exits to prove that delay is outside the reported command interval. Local
Python controls pass. Native Linux `5e67761d` passes the complete image gate
(1,247 ordinary tests, 54 skipped), solver51, every local E2E phase plus extra,
and all four PR conformance arms: 33/33 Same, 311 functions, zero
unstable/different/infrastructure errors or skipped projects; five units
separately report resource-count drift. The restack onto `6d8ee2eb` keeps
these seven implementation/test file changes; its Rust bodies are the already
qualified #619 bodies. The joined local command controls and real Mac controls
pass all 19 command and 11 memory cases. No production Rust behavior changes.
The new native Linux and Mac three-pair overhead comparisons are running;
this correction does not claim that the earlier overhead failure is resolved.

Native correction gate log SHA-256: `8e53062f2236fd6a0024902c1b7c3cb0df09acc76f50f8aa47dd75e355e0611b`.
