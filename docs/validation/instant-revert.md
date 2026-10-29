# Instant revert: content-bound source checkpoint

Status: final source `83128ad0` passes the native ordinary-selection, bundled
Vest A → B → A and complete default gates with the selected `e5365c05` runtime.
The delivery composition preserves its executed Rust/Cargo bytes. PR #224
merged as `731b3052`; all required checks and the main workflow pass. Ordinary eligibility is the exact Linux/public alloc/std
profile, not every project or platform. [Paired qualification](proof-reuse-integration.md)
separates the current source from earlier component results.

## Runtime `f8589ed4` requalification

Runtime `7d829946…-81186034d2e137a2` (fork `f8589ed4`, R3, manifest
`576558fc…`, equal to `LINUX_MANIFEST`) from a release staged at `4d933e8e` (with the #271 toolchain fix)
(see the [compiler-boundary record](vstd-compile-boundary.md#verus-7d82994-bundle-on-runtime-f8589ed4)),
native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04. The protocol is
[below](#verus-7d82994-runtime-requalification): Vest public alloc/std
profile, `-- --rlimit 100`, A → B → A → unchanged, three alternating rounds.
Status: **ordinary Linux public alloc/std reuse requalified for correctness
and returned-A reuse.**

- All 24 calls exit 0. Each Sail step's stderr equals stock's after the
  arm-root and `Finished … in` normalization. Every run is `current`.
  Returned and unchanged A run zero verifiers, reuse both targets and keep
  cold A's establishing run. B reruns only `vest_lib`. No `SAIL3136`.
- Medians of three:

  | Step | Stock | Sail |
  |---|---:|---:|
  | A cold | 71.460 s | 72.709 s |
  | B edit | 39.907 s | 25.075 s |
  | A returned | 39.975 s | 1.880 s |
  | A unchanged | 0.201 s | 0.649 s |
  | Four-command cycle | 151.648 s | 100.314 s |

  The cycle is 33.9% faster. Cold A is 1.7% slower: the shipped proof
  still misses (Vest's vstd closure differs from the shipped one, #238; first attributed to `--rlimit 100`). Wall time only.
- An earlier run on a `1ae17585` stage passed the same correctness checks
  (cycle 155.260 s against 106.066 s), but diagnosis ran beside two of its
  rounds, so its times are not used.

Raw evidence: `r3l-evidence.tgz` sha256
`6594bd503f8fb56725b05628cc9cc11b4758eb1add2b7d8faa823b2b6d89137e`.

## Closure-scoped source check (#278)

Branch `perf/cold-retention` `ea5f4d66` (rebased unchanged as `0514c265`) narrows the retained source snapshot
to the selected targets' dependency closure (accepted
[ADR 0050 amendment](../decisions/0050-retained-proofs-instant-revert.md#amendment-2026-10-20-the-source-snapshot-reads-the-selected-closure-278)).
Retained keys change, so the protocol was rerun on the same runtimes as the
qualifications below: one JSON-output round per platform with this harness
(macOS mac-m4.metal; Linux c7i.8xlarge, release tree in the CI image).
Status: **correctness and returned-A reuse hold on both platforms; one round
each, so not a full requalification.**

- All eight Sail calls exit 0 with diagnostics equal to stock's and progress
  lines equal as a set. Every run is `current`. Returned and unchanged A run
  zero verifiers, reuse both targets and keep cold A's establishing run; B
  reruns only `vest_lib`. No `SAIL3136`.
- Times (stock / Sail): macOS A cold 39.98 / 43.59 s, B 22.47 / 15.08 s,
  A returned 22.32 / 1.26 s, A unchanged 0.22 / 0.77 s. Linux 72.55 / 72.23,
  40.59 / 25.61, 40.22 / 1.34, 0.21 / 0.70 s.
- Three-round human-output comparisons against main are in the
  [cold first-run record](cold-first-run.md#follow-up-closure-scoped-retained-source-check-278):
  returned A falls from 1.78 to 1.16 s (macOS) and 1.62 to 1.16 s (Linux).

## macOS qualification on runtime `f8589ed4`

macOS runtime `7d829946…-e9b69a75ea390499` (fork `f8589ed4`, R3, manifest
`67589de3…`, equal to `MACOS_MANIFEST`), staged by `scripts/release.sh stage`
from candidate/r3-macos-reuse `8afc8663`. The bundle record passed (proof
identity `ca0d4988…`, as in the
[bundle record](vstd-compile-boundary.md#verus-7d82994-bundle-on-runtime-f8589ed4);
bundle manifest `f5ba3290…`). Host: EC2 mac-m4.metal, us-east-1d, macOS 27.0,
otherwise idle; a fresh Rustup home on physical paths with default 1.99.0 and
pinned 1.99.1 minimal with `rust-src`, whose components were unchanged at the
start, after staging, at the end and after the admission witness. Protocol and
harness as [on Linux](#runtime-f8589ed4-requalification) (`bench.py`
`b612732f…`, `analyze.py` `c1578803…`). Status: **ordinary macOS public
alloc/std reuse qualified for correctness and returned-A reuse.**

- All 24 calls exit 0. Each Sail step's stderr equals stock's after the
  arm-root and `Finished … in` normalization, and the progress lines match
  as a set. Every run is `current`. Returned and unchanged A run zero
  verifiers, reuse both targets and keep cold A's establishing run; B reruns
  only `vest_lib`. No `SAIL3136`.
- Medians of three:

  | Step | Stock | Sail |
  |---|---:|---:|
  | A cold | 38.549 s | 44.175 s |
  | B edit | 22.210 s | 15.923 s |
  | A returned | 22.312 s | 2.247 s |
  | A unchanged | 0.222 s | 0.887 s |
  | Four-command cycle | 83.274 s | 63.263 s |

  The cycle is 24.0% faster (per round 83.3/63.0, 83.0/63.3 and
  83.5/65.7 s). Cold A is 14.6% slower, against 35% on `64baccbb`: the
  shipped proof still misses (Vest's vstd closure differs from the shipped one, #238; first attributed to `--rlimit 100`). Unchanged A is 0.67 s
  slower. Wall time only.
- The admission witness (`scripts/admission-witness.py`) passes all five
  cases on the same stage.

Raw evidence: `rr3-mac.tgz` sha256
`a43d0d6ca2d319e18533d376eb8781b6b3ee0e6847c225f526917aefb3223225` (rows,
every step's streams, driver, stage record, witness result and cases).

## macOS qualification on runtime `64baccbb`

macOS runtime `7d829946…-7faa35e5238b9bfc` (fork `64baccbb`, manifest
`4392062a…`, equal to `MACOS_MANIFEST`), staged by `scripts/release.sh stage`
from `eb920b8f`. The bundle record passed. Host: EC2 mac-m4.metal, us-east-1d,
macOS 27.0, idle; Rustup home with default 1.99.0 and pinned 1.99.1 minimal
with `rust-src`. Protocol and harness as [on Linux](#runtime-64baccbb-requalification):
Vest public alloc/std profile, `-- --rlimit 100`, A → B → A → unchanged,
three alternating rounds. Status: **ordinary macOS public alloc/std reuse
qualified for correctness and returned-A reuse.**

- All 24 calls exit 0. Each Sail step's stderr equals stock's after the
  arm-root and `Finished … in` normalization, and the progress lines match
  as a set. Every run is `current`. Returned and unchanged A run zero
  verifiers, reuse both targets and keep cold A's establishing run; B reruns
  only `vest_lib`. No `SAIL3136`.
- Medians of three:

  | Step | Stock | Sail |
  |---|---:|---:|
  | A cold | 39.006 s | 52.668 s |
  | B edit | 22.417 s | 18.359 s |
  | A returned | 22.479 s | 3.328 s |
  | A unchanged | 0.229 s | 0.895 s |
  | Four-command cycle | 84.171 s | 75.334 s |

  The cycle is 10.5% faster. The per-round cycle sums are 83.9/75.3,
  84.2/74.8 and 84.7/75.7 s. Cold A is 35% slower (14.5% on Linux): with
  `--rlimit 100` the shipped vstd proof misses, so Sail verifies vstd too and
  pays retained admission. Unchanged A is 0.67 s slower. Wall time only.

Raw evidence: `rr-mac64.tgz` sha256
`f5ecc2b4235f2424c53b8f9d9c6135445db171d0f6f7ba3ed0391987afc60003` (rows,
every step's streams, driver and bundle record); harness `bench.py`
`b612732f…`, `analyze.py` `c1578803…`.

## Runtime `64baccbb` requalification

Runtime `7d829946…-5fcf72e8d6cc0786` (fork `64baccbb`, manifest `cd78756b…`,
equal to `LINUX_MANIFEST`) from a release staged at `08134b6e` (see the
[compiler-boundary record](vstd-compile-boundary.md#verus-7d82994-bundle-on-runtime-64baccbb)),
native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04. The protocol is
[below](#verus-7d82994-runtime-requalification): Vest public alloc/std
profile, `-- --rlimit 100`, A → B → A → unchanged, three alternating rounds.
Status: **ordinary Linux public alloc/std reuse requalified for correctness
and returned-A reuse.**

- All 24 calls exit 0. Each Sail step's stderr equals stock's after the
  arm-root and `Finished … in` normalization. Every run is `current`.
  Returned and unchanged A run zero verifiers, reuse both targets and keep
  cold A's establishing run. B reruns only `vest_lib`. No `SAIL3136`.
- Medians of three:

  | Step | Stock | Sail |
  |---|---:|---:|
  | A cold | 71.153 s | 81.484 s |
  | B edit | 39.739 s | 27.521 s |
  | A returned | 39.672 s | 3.071 s |
  | A unchanged | 0.199 s | 0.643 s |
  | Four-command cycle | 150.749 s | 112.720 s |

  The cycle is 25.2% faster. Cold A is 14.5% slower, as before: the shipped
  proof misses (Vest's vstd closure differs from the shipped one, #238; first attributed to `--rlimit 100`). Wall time only.

Raw evidence: `rq2-evidence.tgz` sha256
`6daeee5f03f6ef2741fcc51b235091c29f010e5daabca283cee5cdb0c6f15f70`.

## Verus 7d82994 runtime requalification

Runtime `7d829946…-4ed2dee08bc877ae` (fork `9c0ca7e7`, manifest `f4568e29…`,
equal to `LINUX_MANIFEST`) from the staged release, run on native EC2
c7i.8xlarge in us-east-1 (Ubuntu 24.04). Status: **ordinary Linux public
alloc/std reuse requalified for correctness and returned-A reuse.**

- **Project.** Vest at its corpus pin, with only its vstd requirement and
  lock moved to crates.io `vstd 0.0.0-2026-10-18-0147`, the public alloc/std
  profile. `-p vest_lib` runs with `-- --rlimit 100` in both arms. Under
  7d82994, `parse_cbor_with_child` exceeds the default rlimit in stock and in
  Sail alike. Upstream verita adds the same flag for Vest.
- **Cycle.** A cold; B (`assert(true);` as the first statement of the `fn`
  at `der_ord.rs:245`); returned A; unchanged A. Three alternating rounds,
  fresh copies, `CARGO_HOME` and Sail cache each. Stock runs `cargo verus
  verify`; Sail runs `sail verify --output json`. Sail selects Cargo 1.99.1
  itself.
- **Correctness.** All 24 calls exit 0. Each Sail step's stderr equals
  stock's after removing the arm's root path and Cargo's `Finished … in`
  time; the progress lines match as a set. Every Sail run is `current`.
  Returned and unchanged A run zero verifiers, reuse both targets and keep
  cold A's establishing run. B reruns only `vest_lib`. No `SAIL3136` note.
- **Times** (medians of three):

  | Step | Stock | Sail |
  |---|---:|---:|
  | A cold | 68.746 s | 78.807 s |
  | B edit | 38.279 s | 26.533 s |
  | A returned | 38.386 s | 2.934 s |
  | A unchanged | 0.191 s | 0.609 s |
  | Four-command cycle | 145.767 s | 108.885 s |

  The cycle is 25.3% faster. Cold A is 14.6% slower: with `--rlimit 100`
  the shipped vstd proof misses (it was recorded without the flag), so Sail
  verifies vstd too, plus its own overhead. Unchanged A is 0.4 s slower.
  This round measured wall time only, not CPU or cache bytes.

Raw evidence: `rq1-bench.tgz` sha256
`75e937d0cecc509990088610f751041e8ac9f105b88aad371e636d2685785c0d`
(see the [compiler-boundary record](vstd-compile-boundary.md#verus-7d82994-bundle-and-standard-library-paths-vi-0031)
for the harness digests).

## Earlier ordinary qualification (426d8b0 runtime `e5365c05`)

Three rotated native rounds on `c7i.4xlarge`, `us-east-1`, retain all 72 calls
and 18 complete four-command cycles. Genuine stock is the pinned `426d8b01`
release; both timed Sail actors use the same normal `e5365c05` runtime override.
Every row passes the unchanged full function/verdict/diagnostic comparison,
Current/provenance requirements and final source/runtime/binary guards.

| Median complete cycle / resource | Stock | Earlier Sail `c3e64d21` | Candidate `83128ad0` |
|---|---:|---:|---:|
| Vest four-command wall sum | 148.0954 s | 130.6569 s | 113.0842 s |
| Vest observed whole-cycle time | 153.0048 s | 138.7710 s | 124.3674 s |
| Vest accounted CPU | 703.28 CPU-s | 565.28 CPU-s | 465.23 CPU-s |
| Vest final cache bytes | 316,901,084 | 560,368,829 | 1,079,701,675 |
| Small four-command wall sum | 32.5449 s | 37.5469 s | 37.5726 s |
| Small accounted CPU | 106.72 CPU-s | 123.30 CPU-s | 122.41 CPU-s |

Vest's four-command wall sum improves 13.45% against earlier Sail and 23.64%
against stock. Returned A improves from 26.540 s to 2.509 s, but cold A rises
from 76.050 s to 80.684 s, changed B from 27.324 s to 28.892 s and unchanged
A from 0.479 s to 0.810 s. The small cycle is about 0.069% slower than earlier
Sail, and Vest uses about 519 MB more cache. Eligibility follows the dependency
profile; it is not a project-size or predicted-benefit test. These results do
not establish the overall 25% M8 gate.

Accounted CPU sums the four waited CLI trees and independent daemon self and
waited-child tick deltas, with the same daemon PID/starttime bracketing each
cycle. It excludes setup, inventory/capture work, and daemon startup before the
initial sample and shutdown after the final sample. It is not a CLI-only CPU
figure or a complete operating-system accounting claim.

A separate genuinely bundled/no-override ordinary Vest control passes A → B → A
→ unchanged, preserves original A's establishing run and executes zero new
verifiers on returned/unchanged A. It is functional correctness evidence,
separate from the explicitly selected runtime in timed rows. Native final
archive SHA-256: `3f42ea4f70bbeee125e1bf4b67b7cacd3e5e26612a13fe401c5a9bd846287af2`.
The complete default gate and earlier/current source boundaries are recorded in
the paired qualification. Remaining release-bundled first-use vstd work belongs
to #161; it is not closed by instant revert.

## Earlier source checkpoints

Earlier source, controls and failures below retain their original scope.

The driver joins exact source/provider eligibility, owned complete dependency
snapshots, actual native and GHOST/EXEC compiler-read certificates, bounded
output archives and checked restoration. Publication additionally requires the
ordinary successful Current result and committed receipt. A reused native
certificate preserves its original establishing run and PID. It does not
attest a newly executed compiler. Unknown contexts take the genuine compiler
path; failed restoration cleanup refuses with `SAIL3134`.

The local macOS source checkpoint passes `cargo clippy -p sail-verus
--all-targets --offline -j 2` and 15 `managed::retained` library tests. They
exercise checksum/source ABA, policy/feature refusal, protocol/PID binding,
namespace ambiguity and literal content, late producer record arrival,
archive corruption and failed placement cleanup. These are source and owned
filesystem/wire controls, not actual Verus or real-project acceptance.
[VI-0025](../verus-issues/VI-0025-module-candidate-absence.md) records the actual
three-command rustc namespace reproduction.

The earlier unmerged filesystem-event candidate remains blocked by mapped-write
and coarse-timestamp ABA counterexamples. A source-only archive checkpoint and
its corrected algorithm-prefixed checksum / aggregate-restoration controls do
not supersede those failures. No native performance result is claimed.

Before selection: qualify the observer runtime, complete native producer and
namespace closure review, then execute real Vest A → B → A plus GHOST B / EXEC A,
private-graph mutation, one-byte/namespace/environment misses and damaged-output
controls. Require unchanged verdicts/diagnostics, original establishing run,
zero new verifier invocations only for restored A, and all ordinary final guards.

External local check logs: clippy SHA-256 `0df906bc004c45e0d41f15c6b9a52e62a19ee3f6e498746b8bfd1967070b6702`; focused tests SHA-256 `53858e75e8fe02d7c8a9537e88dce6b4fdaccbc9b2be52e33d5365ff8705fe01`.

The readiness successor imports the reviewed shared `capture_ready` algorithm:
bounded compiler scratch-directory drain, at most two complete capture
attempts, no ignored directories or special entries. The storage successor adds aggregate retained-arena admission;
its native behavior and performance remain unqualified (ADR 0050).

The readiness import also passes all-target sail-verus clippy, eight actual
filesystem graph tests and all 15 retained tests locally on macOS. It does not
qualify Linux ABI or real Vest. Test stdout SHA-256: `c22265839f2055dbabe361c01486f0d5b1bcb27a4880d0b3a11954a2abe35d0f` (graph) and `6f4e32fba5299c4579737fbc62c6f3c2a44d52a4217b7d1fe5385c87ac0ca624` (retained).

The storage successor bounds the durable retained arena at 8 GiB / 65,536
entries using serialized prospective admission for graphs and lifetime markers,
certificates and producer records, and archived proof bodies. Failed writes
remain counted; unknown live generations are preserved. Native compiler request
scratch stays in ordinary managed-run sidecars, so this is not a global bound
on all run logs or caches. Five additional owned-filesystem controls pass,
including actual graph publication/deduplication and concurrent admission;
all 21 retained library tests pass. Native qualification and performance remain
pending, including the scan/lock overhead.

The aggregate-admission successor also passes the complete macOS host check:
workspace clippy and 684 tests, with 35 solver/platform skips. This is a host
correctness gate, not Linux ABI or timing acceptance. Host stderr SHA-256:
`cb6d867932791be039e682df839ae39308c5756f2726816b03ee309097c6547e`; focused stdout SHA-256:
`ba9015c88d6ead3ab0c515a342413934ac3e74296228086663ff509d9882aea1`.

## Native ABI attempts

The first two attempts used source `b6d58f1e` and the genuine retained observer runtime
`067d0d7c`; runtime and policy checks stayed strict. Neither ran small or Vest
A → B → A, and neither qualifies retained reuse.

| Source/date | Result | Evidence and limit |
| --- | --- | --- |
| `b6d58f1e`, 2026-10-17 | Preparation FAIL; CLI/ABA unrun | Small vstd had alloc/default/std instead of the unchanged alloc/std policy. Final archive SHA-256 `c3f8da18da98a56230f117491042d8cb69e8f86dc4f34ea62e151f82c84b3f25`. |
| `b6d58f1e`, 2026-10-17 | Strict ABI FAIL; ABA unrun | Explicit small features and real Vest metadata matched all 15 vectors/718 source hashes each. Actual public CLI returned Current/0 with 1,871 vstd and one workspace function verified, two verifier calls, six native output certificates from three native documents, but no verified certificate or archive. Final archive SHA-256 `d3aa2262687b50ec9e1aea33567d7a1b6b24b5abb6c35ff35395cc41c685e547`. |
| `be2fd563`, 2026-10-17 | Strict ABI PASS; small ABA FAIL; Vest unrun | Genuine native and GHOST/EXEC official certificates and an archive were present. Public CLI A, B and returned-source A2 all returned Current/0, but A2 ran one new verifier and kept its new establishing run, so the unchanged strict ABA check refused. Final archive SHA-256 `4a89218d9ec7594d9c0751fc5578cbc5429b9e4da791b6869f8e800e25cf77b7`. Diagnostic Cargo verbosity was fixed throughout; no performance acceptance. |
| `015b2937`, 2026-10-17 | Strict ABI and small ABA PASS; Vest ABA FAIL | Genuine final `e5365c05` runtime; returned small A2 and unchanged A3 use zero new verifiers and original establishing run `7854-1792273478656931181`. Vest A1/B/A2 return Current/0, but A2 runs one verifier in 28.419 seconds and has a new establishing run. No Vest candidate or archive exists. Final archive SHA-256 `bccd90bf497ba32882b9880a1e655e39da1a9799dc0dc15ed2b56a6e03c7c4b7`. Diagnostic Cargo verbosity was fixed throughout; these times are not performance qualification. |

For the second attempt, the actual failed admission point was not logged. A separate genuine Cargo
1.98.1 argument-generation control proves that the pinned workspace manifest
produces 90 long diagnostic lint flags, including `--warn=unexpected_cfgs` and
`--allow=clippy::…`, which the previous argument policy rejects. The controlled
wrapper exits before compiling the crate; this is an ABI witness, not Verus
acceptance or proof of the original refusal's precise cause.

The source successor admits only tightly parsed long Rust/Clippy lint forms.
It preserves every argument byte, order and valid repetition in the actual
compiler target identity. Unknown forms, imports, loaders and compiler routes
still decline. All-target sail-verus clippy, 22 retained tests and the 15-test
Cargo-filter check pass on macOS (the test sets overlap), including actual
identity changes for lint bytes/order/repetitions. Those source checks preceded the strict native ABI PASS above. Real restored
A, native merge gates and default selection remain pending; no latency result
is claimed.

## Caller-environment restoration correction

The `be2fd563` A1 and A2 archive bases matched exactly. A bounded filesystem
replay of the literal retained A1 entry and blobs identifies the first refusing
check: the archived receipt records the outer caller's `RUSTC_BOOTSTRAP` as
absent, while the managed Cargo adapter receives the runner's explicit `1`.
The old restore path compared those different environment layers. The same
entry passes integrity and placement with the outer environment and refuses
before placement in the managed environment. Output/commit paths in this
local witness were relocated and resealed; its compiler documents, report,
streams and artifact bytes remained literal. This is a source/filesystem
regression, not real-project reuse qualification.

The successor captures only outer-caller value digests before Cargo starts,
with actual absence and a required current-run binding. Restore checks every
archived declared variable against that fresh caller snapshot. The existing keyed
adapter environment remains part of the exact archive basis; post-placement
source/graph/environment checks and the outer runner's ordinary Current gates
remain. Missing snapshots, stale runs and changed/present/absent declared
values refuse. No variable is ignored or special-cased. The previous strict
small ABA failure and pending Vest qualification remain visible.

The corrected source passes 25 retained tests and all-target sail-verus clippy
on macOS. An owned outer process publishes a real filesystem fixture before
its managed child receives `RUSTC_BOOTSTRAP=1`; that child calls production
archive restoration, checks literal artifact/stream bytes and original run,
and refuses changed, absent, newly present and stale-run caller snapshots
before placement. Missing snapshot fields also refuse. The same literal A1 filesystem witness now restores exact
artifact bytes under the managed child environment using the fresh outer
snapshot, preserving the original establishing run. This does not supersede
the native small ABA failure: the correction still requires a native repeat.

External regression index SHA-256: `8c223b087d1d75269b78b1f4ba745035878db8db3e83524de3133528b6547f45`; corrected retained-test stdout: `9a0d3548401d4c966631baa9fd99c1fc60f7fd51353c760b5faac9009ada9f2e`; clippy stderr: `687abac1d888da9b5c4f8526bb63a4f13410417b50f9ed026176d6c14b6b97d2`.

## Final-runtime provider compatibility

The next prospective pilot uses the genuine normal `e5365c05` runtime archive
SHA-256 `689004c074aca8883e225a2b89efebc137f3e563f24df2abd398516112b13928`,
including its original full Manifest and 168-second build record. It does not
rebuild Verus or relabel the earlier `067d0d7c` attempts. That earlier component
had a genuine build but a failed post-build source guard caused by the wrong
working directory; it was not full runtime qualification.

Final-fork source differs from the previous closed provider authority in one
of 400 official provider files: `builtin_macros/src/enum_synthesize.rs` has the
recorded VI-0001 source-order fix. The prospective policy updates that file's
exact SHA-256, compiler BLAKE3 and size; all other 717 provider files and all
15 feature vectors are unchanged. Release-base identity remains `426d8b01`;
`verus_source_fork_commit` records the actual `e5365c05` provider source.
The policy parser and every source/feature/graph check remain unchanged.
Actual macOS Cargo fetch and locked metadata checks match all 15 vectors and
718 files for both small and Vest, without compiling or measuring performance.
The later final-runtime attempt passes native ABI and small ABA; Vest ABA
still fails, as recorded below.

## Closed compile-source namespace correction

The final-runtime native attempt qualifies the public ABI and small restored A,
including its original establishment and zero new verifier calls. Real Vest
reuse still fails. The captured Vest plan is present and its selected target
is `lib`; there is no verified compiler-read document, retained candidate or
archive entry. Investigation therefore precedes archive lookup.

The plan snapshots all 216 metadata packages. Namespace admission consequently
checks uncompiled workspace siblings: an exact production lexical check rejects
26 of the pinned Vest archive's 261 Rust sources, while all 139 `vest_lib`
sources pass. The complete namespace function, with real SHA-256 reads, refuses
the whole Vest source inventory and accepts the selected-library inventory.
This is an offline source/filesystem witness, not a native corrected run or
proof that every other admission gate passes.

The correction keeps the complete snapshot, revision and content key unchanged.
It applies namespace exclusions and actual GHOST/EXEC source authority to the
same closed roots: selected package directories and all admitted provider
directories. A compiler reading any excluded sibling still fails actual
SourceMap admission. Selected-source custom path/include syntax, provider
candidate ambiguity, unknown sources, aliases and special entries retain their
existing refusal checks. The 26 retained tests include real filesystem
old-refusal/new-admission, selected-source and provider negatives; all-target
sail-verus clippy passes on macOS. Native Vest reuse and performance remain
pending.

External source-check index SHA-256: `fbebf6df6a935fddb08ec882178b473feae4dfb13f9be3f41889f5295e0c4b1b`.

The [source integration](proof-reuse-integration.md) preserves this path as a
separate default-off experiment. Enabling both research switches refuses with
`SAIL3135`; no integrated native acceptance is inferred from this record.

## Latest functional and source-candidate scope

Source `446385e9` passes native ABI, small and Vest A → B → A. Returned A2 and
unchanged A3 use zero new verifier invocations and retain the original A1
establishing run. Vest A2 took 2.420 seconds versus B at 28.512 seconds, but this
is one diagnostic-verbosity cycle, not performance acceptance. Final archive
SHA-256: `33221697927fc74c47f8b72615ae29f4d1e1caabc541cfc5db5607b6ca5d6abc`.

The first genuine-stock 72-row quiet cohort passes four stock small calls, then
refuses before Sail OFF's first command: `unknown special entry`. Its remaining
68 rows are unrun. A separate actual owned macOS daemon reproduces two socket
endpoints while the growth inventory admits only `daemon.sock`; the native
exception itself did not retain the path. This failure supplies no performance
acceptance and remains preserved.

The automatic-selection successor adds only platform/runtime selection and
existing-runtime-plan propagation. Local source checks exercise real owned
managed child transport without an explicit research flag and retain the
outer-absent/child-bootstrap environment boundary. These controls do not
qualify automatic native compiler admission, macOS reuse or default activation.

The public-provider source successor closes the ordinary-resolution gap with
two exact 15-package/662-file profiles: current-pin new library (vstd default
features) and current-pin Vest (alloc/std and its locked registry versions).
Actual offline Cargo metadata uses no provider patches. Independent checks
match every real archive member SHA-256 and compiler BLAKE3/size. These are
source authority checks; the earlier 446 functional results remain path-provider
scope. Native ordinary evidence for the integrated successor follows below.

## Ordinary public candidate and measured-scope successor

Source `aeea0587`, baseline `c3e64d21`, genuine final `e5365c05` runtime and stock
`426d8b01` completed all 72 normal-verbosity small/current-pin-Vest calls. The
two Sail actors used the same explicit `SAIL_CARGO_VERUS` runtime override,
including its exact retained warning, with research flags absent. This does not
qualify the bundled/default-discovery user entry point. The
three fresh public-enum controls have identical literal SMT input multisets.
Actual no-flag native and GHOST/EXEC ABI controls and both functional ABA chains
pass. All returned-A research rows preserve A1's establishing run and execute
zero new verifiers.

The frozen whole cohort is **FAIL**, with 64 admitted and eight refused rows.
Its function-name-only ordering comparison distinguishes same-named records
from different modules in different orders. A separately reviewed read-only
gate reconstructs all 72 projections from actual stock/producer documents and
checks complete `(function, module, mode, success)` multisets with multiplicity,
all scalar verdicts, diagnostics, stock no-op origins, Current/establishing-run
bindings, literal source equality and source/runtime/binary guards. That scoped
semantic/source/origin gate passes; it does not alter the frozen outcome.
Its 14 controls retain wrong-module verdict/mode, duplicate/missing tuple,
unknown-field and scalar refusals. Final archive SHA-256:
`2a6652c746e7a2d265ce1ed0c12bd0b2ddc09898ecc23308e97c241e1fa5d130`;
read-only gate result SHA-256:
`79637abfd06c8ebbed0bd8093cdb0809bf5f6ad5f8d87dd8f61e4d51940f1517`.

The final whole-cycle-tree inventory is absent after a separate bare assertion.
The 18 distinct A3 cycle inventories already contain 456,498 files/directories,
above the whole-root 131,072 count bound. These are pre-daemon-stop observations,
not an invented final snapshot. All 72 individual bounded growth inventories
remain. No whole-cost/default acceptance follows from the semantic reader.

Observed median seconds, retained as diagnostic measurements:

| Project/call | Stock | Ordinary Sail | Automatic candidate |
| --- | ---: | ---: | ---: |
| Small cold A1 | 32.218 | 36.578 | 40.465 |
| Small changed B | 0.704 | 0.947 | 2.246 |
| Small returned A2 | 0.709 | 0.915 | 1.496 |
| Small unchanged A3 | 0.102 | 0.194 | 0.344 |
| Small complete four-call cycle | 35.407 | 41.377 | 50.138 |
| Current-pin Vest cold A1 | 72.260 | 78.526 | 83.893 |
| Current-pin Vest changed B | 39.916 | 27.723 | 29.817 |
| Current-pin Vest returned A2 | 39.922 | 27.428 | 2.604 |
| Current-pin Vest unchanged A3 | 0.195 | 0.521 | 0.850 |
| Current-pin Vest complete four-call cycle | 157.586 | 142.664 | 128.496 |

The returned-A improvement is not the whole-workflow improvement: the measured
Vest cycle improves about 10% against ordinary Sail, with cold/B/A3 regressions.
Small cycles regress about 21%. Initial owned-tree growth is roughly
757–811 MB for the candidate small project versus 373 MB ordinary, and
916–970 MB for candidate Vest versus 492 MB ordinary. These include compiler
targets and sidecars; the 8 GiB retained-arena cap is not a total-cache bound.
Cycle times include recorded per-actor preparation and daemon setup/teardown;
they exclude common source builds, functional gates and later evidence copies.

The source successor therefore restricts ordinary routing to the already
separate public alloc/std authority, before full source hashing, runtime
validation, native certificates or graph creation. It reuses the existing
metadata authority selector; matching metadata still requires unchanged full
15-package source/feature and compiler-read admission. Default-feature public
and path-provider profiles remain available only through explicit research.
This restriction describes an exact dependency profile, not a universal proof
cost threshold. New-head native small-negative/Vest ABA, targeted cost and final
aggregate growth checks and full gates remain pending. The original 72 calls
are not rerun or relabelled as successor qualification.

The metadata-routing successor passes 32 retained-library checks and all-target
`sail-verus` clippy on macOS. These are source/owned-filesystem controls, not a
new native acceptance gate. Test stdout SHA-256:
`4969b8a5c93692bb76dfafff15cb46e3a4e5dd0c5ad76387a4077c82b6561dc8`;
clippy stderr SHA-256:
`4ebe896991329469405a3ea58b295513040a8e714010d7b34aeac4ab95574e67`.

## Unchanged-run optional-plan candidate (#156)

The optimization measured at `e0d14d75`, based on `731b3052`, omits the optional
retained-content plan when every requested target has an own-context receipt
header with the current nonempty input revision. It reuses the existing fresh
dirty-receipt scan; it adds no second source-hash pass. Cold or missing receipts,
changed revisions (including returned A against B's receipt), forced runs,
incomplete inputs, missing mutation stamps and any script/target cleanup keep
the existing path. Omitting the plan also avoids its publication snapshot.

The header is a disable hint, never proof authority. Cargo still runs normally;
an unexpected rebuild genuinely verifies. Complete receipt, artifact, native
input and final Current checks remain unchanged. A malformed matching proof
body cannot be reused on the strength of this header. At worst the optional
archive opportunity is missed, so a later invocation verifies freshly.

Two owned-filesystem Rust tests pass on macOS: cold/missing headers, changed B
and returned A revisions, wrong identity/context/version and malformed headers.
The actual native screen on `c7i.4xlarge`, `us-east-1`, passes all 36 calls with
zero unrun rows: three rotated rounds of genuine stock `426d8b01`, baseline
`83128ad0` and this candidate. Baseline executed Rust/Cargo bytes equal the
candidate's `731b3052` base. Both Sail actors use the genuine normal
`e5365c05` runtime (archive `689004c0`); no Verus rebuild is involved. The
current-pin Vest source, lock and ordinary public provider profile are identical
across actors. Sail uses ordinary default verbosity; stock's explicit JSON/time
flags supply the function/verdict/diagnostic oracle.

Wall seconds are medians with the complete three-sample min–max range:

| Call | Stock | Baseline | Candidate |
|---|---:|---:|---:|
| Cold A1 | 73.931 (73.897–74.055) | 85.879 (85.708–85.893) | 85.524 (85.409–85.582) |
| Changed B | 41.071 (40.964–41.143) | 30.591 (30.435–30.735) | 30.427 (30.330–30.610) |
| Returned A2 | 41.117 (41.020–41.130) | 2.804 (2.803–2.811) | 2.800 (2.788–2.806) |
| Unchanged A3 | 0.227 (0.202–0.232) | 0.895 (0.890–0.913) | 0.544 (0.536–0.555) |
| Four CLI calls summed | 156.431 (156.113–156.445) | 120.183 (119.859–120.314) | 119.305 (119.062–119.543) |
| Observed cycle including preparation/validation | 163.902 (163.659–164.021) | 134.849 (134.025–134.953) | 133.843 (133.691–133.945) |

Unchanged A improves 39.29% (0.895213 → 0.543504 s); every candidate sample is
below every baseline sample. The four-call sum improves 0.73%, and the broader
observed cycle improves 0.75%. Cold, edit and returned-A medians do not regress
in this screen; their overlapping ranges and three rounds do not establish a
general regression bound. Every Sail A2/A3 has Current, zero new verifiers and
the original A1 establishing run. All rows pass exact stock function/diagnostic
comparisons and source/runtime/binary guards. No general CPU saving is claimed.

The complete ending inventory covers all nine actor/round trees, targets,
sidecars, native graphs, certificates and caches: 7,263,140,779 logical bytes,
7,347,056,640 allocated bytes, 325,379 files and 2,572 directories. This is the
whole screen's footprint, not one project's cache requirement. These results
qualify the measured optimization only. The production-identical, test-lint-only
successor `e130f632` passes the full Mac host gate (744 tests, 35 skipped, plus
clippy) and native Linux workspace gate (755 tests, 35 skipped), solver tests,
all nine end-to-end phases plus extra, and PR conformance. Native full FAST
and STANDARD pass all four arms: FAST 147/147 units and 3,450 functions,
STANDARD 159/159 units and 12,726 functions. Every arm has zero differences,
instability, infrastructure errors or skipped projects. The reports retain
rlimit-only drift in 12 FAST and 18 STANDARD units per arm; those metric drifts
are not verdict or diagnostic differences. Final source/runtime/product guards
and the complete FAST/STANDARD evidence capture pass. The optimization is
qualified for the existing default profile and merged through #225 with all
required CI checks green. No Mac performance or general CPU gain is claimed.

The Linux gate commands completed successfully with unchanged source, runtime
and built-product maps. Their enclosing evidence export failed because the
unprivileged archive reader could not enter the repair fixture's root-owned
directory. The complete top-level command logs were recovered and checked
against their remote SHA-256 inventory; nested fixture files were not exported.
This is scoped command evidence, not a successful complete archive. The
original enclosing failure remains recorded below.

External evidence under
`retained-warm-skip-matched-native-2026-10-18/tight-caps-90/attempt01/`:

- `native-results/final.tgz`: SHA-256 `087ac26c877ff0af15d8c906d80fc1b032fcb295d31ed73ef27d79e5e07087bb`.
- `selected-final/evidence/performance/status.json`: SHA-256 `e83620c630706982e6f82bf309512cd6ed8ae92b2a504db7bea925332d5d8075`.
- `selected-final/summary.json`: SHA-256 `1c288f3aa8a84f924f834ce2c81023d839f118062e4dd14c02fab1f87defba63`.

Failed attempts remain indexed: `e0d14d75`, 2026-10-18, first native packet FAIL
at baseline checkout (128), all 36 calls UNRUN because the bundle omitted the
non-ancestor baseline ref. Original final archive SHA-256
`e0769e966f446114ec38d99f3266dcd0644a521f5719bffd77e3ce5901ad448c`.
The successful sibling explicitly bundles both refs and checks every file/mode
in isolated clones of both heads. It does not relabel the failed attempt or
claim earlier `7090` qualification.

Delivery-gate attempts remain separate from the completed screen: `cf1b6372`,
2026-10-18, first native qualification FAIL before staging/checks because the
watchdog timestamp was on stderr and actual launch missed the fixed cutoff;
all checks UNRUN. The Mac host gate then FAILs at clippy's existing
`assert_is_empty` lint in three new test assertions; full tests UNRUN (stderr
SHA-256 `af55f9055a0795b5157fc162c7815caa70e2195b53a631bc51eaf71580aa6181`).
The second native attempt is interrupted before checks on that known lint
failure. A test-only successor uses equality against an empty vector, preserving
the exact requirement and displaying the unexpected contents. Production code
and the measured screen are unchanged.

Delivery evidence for `e130f632`, external root
`retained-warm-skip-qualification-native-2026-10-18/`:

- `mac-host-03/result.json`: full host PASS in 124.91 s; SHA-256
  `5ef3c2a588f60c623c3092f830ad05b50aeb25ceca449a4e52e08efff07ba3fd`.
- `detached-gate-admission-attempt02/fallback-raw/status.json`: native command
  results PASS; workspace gate 62.71 s, solver 9.35 s, full end-to-end 660.09 s
  and PR conformance 57.38 s; SHA-256
  `055c809e3e88dd2d9d118efd64e28a9467bccb204a9dae3c9b06833d9bd676e2`.
  This status records both final source and runtime guards and unchanged
  product hashes. The complete fallback logs have their own checked inventory.

Additional failed or interrupted attempts, all on 2026-10-18, remain separate:

| Source | Attempt | Result and scope |
|---|---|---|
| `e130f632` | Initial post-lint run | Interrupted transport; workspace gate and solver checkpoints passed, later commands unqualified. |
| `e130f632` | Detached gate attempt 01 | Worker launch failed; product checks UNRUN. |
| `e130f632` | Detached gate attempt 02 | All declared product commands and final guards PASS; enclosing complete export FAIL on fixture directory permissions, full top-level logs recovered. |
| `e130f632` | Remaining FAST/STANDARD attempt 02 | Worker launch failed; product checks UNRUN. |
| `e130f632` | Remaining FAST/STANDARD attempt 03 | Setup budget admission failed before image build; product checks UNRUN. |

Remaining FAST/STANDARD attempt 04 passes on exact `e130f632`: FAST takes
507.23 s and STANDARD 5,387.61 s. Both complete four-arm report checks pass,
including the exact 159 STANDARD unit identities. All 640 tracked source
bodies/modes, the normal runtime and freshly established binaries remain
unchanged across the commands. The complete archive retains 777,346,531 raw
bytes and passes its checked inventory. This successful capture covers this
attempt's FAST/STANDARD scope; it does not replace the earlier gate attempt's
explicit nested-export limitation or relabel any earlier failure.

External evidence under the same delivery root:

- `remaining-fast-standard-admission-attempt04/result.tgz`: SHA-256
  `82d12e1ac4e7004245393939683dcb5025fb1fcf26c3f4025124cc6ebfe63a96`.
- Its `evidence/status.json`: SHA-256
  `344cddfbf84fbf2027b7c7e7b6ab26a185c3795ce8fed201c67d489287ce0b83`.
- Its `evidence/fast-report-check.json`: SHA-256
  `16d9204e866ef549723f87c44d83960a26e83fbe56f9d2ca3ac640d0d28d3328`.
- Its `evidence/standard-report-check.json`: SHA-256
  `5d00e5eb963b509b2edb937c159ce33df4f1d9fc558a7b031f62ab02d278143a`.

The delivery documentation amendment preserves every executed Rust/Cargo body
and mode. The measured production implementation, test-lint successor and
current qualification scopes remain separately identified above.
