# Real-project memory instrumentation (#562)

Status: implemented; functional controls pass on native Linux and macOS.
The combined candidate records all four projects on both platforms, six paired
rounds each: 864 expected exits and populated memory rows
([complete matrix](v020-performance.md)). Release-wide memory acceptance
remains unqualified because the separate below-noise sampler requirement fails.
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
Subsequent native Linux and Mac controls below remain unqualified; this
clock correction does not establish that the earlier overhead failure is resolved.

Native correction gate log SHA-256: `8e53062f2236fd6a0024902c1b7c3cb0df09acc76f50f8aa47dd75e355e0611b`.

## Equal priming and Mac observer priority candidate

The corrected-clock three-pair controls retain 108 samples per platform.
Linux still fails one of eighteen strata (stock first unchanged): absolute
median difference 35.021 ms versus a 33.235 ms largest arm range. Mac fails
twelve of eighteen, including stock body edit (80.036 ms versus 18.506 ms
range). These controls do not establish overhead within noise. The command
environment/arguments and interval are already identical; every sample stays.

Source inspection finds that disabled sampling also skips the initial,
untimed process inventory. The next candidate primes that inventory in both
arms. On Mac it additionally lowers only the observer and its later `ps`
children to nice19 after spawning the command. Both control arms receive the
same observer priority; the command retains its original caller priority.
No process inventory, RSS metric or 50 ms sampling interval is removed.
Later functional qualification passes on both platforms, as recorded below;
the complete below-noise overhead requirement remains unqualified.

The new controls require inventory before spawn in both modes, observer
priority changes only after spawn, and exact child priority/environment/argv
preservation. This is diagnostic tooling only, with no Rust or verifier change.

## Mac environment refusal (2026-10-30)

The early mac-m4.metal/macOS27 environment exposes sustained non-workload CPU:
`audiomxd` has accumulated 382 CPU minutes and `configd` 230 CPU minutes
in about 565 minutes of uptime. Earlier Mac component samples remain retained
as diagnostics; they do not establish quiet-host timing or release acceptance.
Correctness, factory and first-Serve results retain their actual executed
scope. The interrupted equal-priming/priority overhead attempt is also retained.

A console session and fresh boot clear the observed hot audio/configuration
processes on this host; newly started indexing still needs to settle before
measurement. Underlying cause is not attributed without isolation. The
candidate adds a one-second process-CPU preflight before Mac performance work:
at most 0.10 observed CPU cores across the inventory. Busy, missing, malformed
or unreadable observations refuse the run before any timing sample starts.
`host-quiet.json` retains the actual observation. This is a bounded startup
observation, not continuous proof of an idle machine; every timed sample and
harness refusal rule remains unchanged.

Pre-console raw evidence (1,885 files, including current component summaries,
all memory/control rows and installed indexes), SHA-256:
`efef3ff74f51e37dafa375041f6f34a1e834145be6937adefb8fd0f011e5f227`.
The later authenticated-console matrix uses the original bounded startup
checks. Complete sampler-overhead qualification remains open; the current
[combined record](v020-performance.md) keeps its timing and memory scope explicit.

Indexed quiet-host rerun, `6358c7a6`, 2026-10-30: two complete rotated
batches retain 36 samples; the next batch refuses before timing at 0.498
observed CPU cores. Its largest contributor is a newly respawned Bluetooth
service (0.197 cores), with smaller OS services also active. The complete
108-sample control is not established. Static wallpaper and a console session
alone do not eliminate every background burst. The refusal limit is unchanged;
no timed sample is removed. The next attempt uses Bluetooth's ordinary power
off preference and a 60-second untimed pause between batches. Neither host
configuration change itself proves a quiet measurement.

The combined `6358c7a6` native image gate passes 1,274 ordinary tests and all
51 solver tests; full E2E plus extra passes. Mac host `6358c7a6` passes strict
Clippy and 1,263 ordinary tests, with 54 skipped. All 12 memory, 5 quiet-host
and 19 command controls pass on both platforms. These correctness/tooling
checks do not qualify sampler overhead or release memory acceptance.

Refused rerun controller log SHA-256:
`76d73e41bfdd7c8a66fbaed72b48872ae6a5b8ee918bbb6833ed18d1b577f609`.
Retained raw archive SHA-256:
`2de94ca2c698be53e3bef3b70c053118d4f45ebf01626e8bcee225f84bb626fe`.

## Observer interpreter scheduling correction

A controlled child-clock regression exposes another observer delay: CPU-bound
inventory work can hold Python's interpreter lock across the child's exit.
With an intentionally one-second interpreter quantum, the prior helper reports
about 1.001 s for a child sleeping 0.1 s. The candidate gives both observer
arms a 1 ms interpreter quantum; the same regression passes. The child's
independently executed interpreter keeps its own scheduling, as an added
command control checks. The 50 ms inventory cadence, complete RSS definitions,
environment/argv, child priority and all overhead thresholds remain unchanged.
This is a reproduced clock mechanism, not attribution of every earlier Mac
difference. Local functional controls pass (13 memory, 19 command). Native Linux
`30843d0f` passes the complete image gate (1,265 ordinary tests, 54 skipped),
51 solver tests, every local E2E phase plus extra, and PR conformance: all
four arms 33/33 Same, 311 functions, zero unstable/different/infrastructure
errors or skipped projects. Native functional controls include all 13 memory,
5 quiet-host and 19 command tests. The identical sampler bodies in combined Mac `c60dc918` pass strict Clippy,
1,265 ordinary tests (54 skipped), and all 13 memory, 5 quiet-host and
19 command controls. Independent Mac before/after results below fail qualification; these functional
gates do not establish the original within-range overhead requirement.

| Raw corrected-observer evidence outside Git | SHA-256 |
|---|---|
| Native full gate log | `24343382ba05e5dc762293330cb067849f4b374426aafd3d0468b04bec4de4b6` |
| Native conformance archive | `15f1981b025dc1ab065d95ad3e9b6fc7481009ee9928c8ec48f9831875cd65b6` |
| Native E2E archive | `ae49168e1ce988a41429381dc216673901c0a2729dc693b43f35cbc5f8abd161` |

## Corrected observer native on/off controls

The 1 ms observer helper (`30843d0f`) retains all 108 IronKV samples on native
c7i.8xlarge/us-east-1. The measured release tree is main `47dd15f8`; these
controls isolate the sampler and do not qualify product performance for the
later digest-selection candidate. The four demo arm/phase comparisons pass
the original within-range rule. Seventeen of eighteen IronKV comparisons pass.
Sail's first unchanged row fails: disabled median 0.211296351 s, sampled
0.218340316 s, difference 0.007043965 s against largest arm range
0.006694430 s. The full sampler-overhead requirement remains unqualified.
No sample is removed and the threshold is unchanged.

Raw 108-sample archive SHA-256:
`48432c81aa30c54ac44b32d22f73bda2e73dcbd533119939042bf3993e3796cc`.
Mac functional checks pass on combined `c60dc918` as described above; its
independent on/off controls below remain unqualified. This clock correction
does not relabel older timings or establish release-memory acceptance.

Mac combined host/functional gate log SHA-256:
`218e990667f413ccec7775b4d4b444a6ed1c60818e5b57dad53d65a8f9bdbfff`.

Indexed Mac observer rerun, `c60dc918` / identical `30843d0f` helper,
2026-10-30: the demo plain-edit comparison fails the unchanged within-range
rule (0.401845000 s disabled, 0.410984583 s sampled, difference 0.009139583 s
against largest range 0.003362582 s). Four IronKV batches complete with 72
retained samples; two further batches refuse before timing on background CPU,
including 0.480 cores with a newly respawned Bluetooth service contributing
0.207 cores. The 108-sample qualification is not established. A normal service
pause is refused by macOS; no system protection or CPU limit is changed.
The first complete four-project matrix retains its actual source/helper scope;
a fresh corrected-helper matrix is separate, with bounded startup checks.

Retained full Mac outputs, refused observations, harnesses and staged identities
archive SHA-256:
`ce8bea751487a45a2fc476006c4a6008e5707aefb3ab74ac31845db1602c9b61`.
