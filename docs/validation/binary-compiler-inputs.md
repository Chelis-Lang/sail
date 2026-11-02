# Exact binary compiler-input qualification

Status: the candidate consumes checksum-corrected fork `a0e11cff` on Linux and
Apple-silicon macOS. Both native runtime archives are published; anonymous
downloads match their pins. Complete source, producer/library, engine,
cancellation, factory and installed controls pass. Owner-approved ADR 0097
qualifies the one Mac consumed-counter difference separately from strict
matches. Additional composed corpus and cost qualification remains open;
no Sail release version or publication is selected.

## Source and environment

Fork `a0e11cffbdf768eac874996f35149385152f736e`, upstream
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust 1.99.1 and pinned Z3 4.16.0.
The runtime correction uses the compiler's selected SHA-256/BLAKE3 algorithm
on actual returned binary bytes. Unknown, conflicting or incomplete byte
observations withhold coverage. Sail does not reopen files to infer what
the compiler consumed. Default verdicts and compiler errors are unchanged.

Linux source/factory qualification uses native c7i.8xlarge in us-east-1;
the additional composition and final measurements use c7i.4xlarge there,
Ubuntu 24.04 and the pinned CI image. Mac uses native mac-m4.metal in
us-east-1c, macOS 26.7. Each capture retains its exact Sail source/runtime
inventory. Artifact JSONs bind the source lock, archive and every runtime file.

The earlier `c9af9534` runtime is unselected: its hardcoded SHA-256 observation
failed the actual Cargo BLAKE3 factory. Its successful source checks do not
qualify A0e; the table below records A0e's independent controls.

## Corrected A0e runtime

[Fork PR #17](https://github.com/Chelis-Lang/verus/pull/17) is merged as
`1435f4b2`; its compiler sources match `a0e11cff`. Hosted fork CI passes
[run 41372976013](https://github.com/Chelis-Lang/verus/actions/runs/41372976013).
Public build dependencies:
[Linux](https://github.com/Chelis-Lang/verus/releases/tag/engine-runtime-a0e11cff-linux),
[Apple-silicon macOS](https://github.com/Chelis-Lang/verus/releases/tag/engine-runtime-a0e11cff-macos).

| A0e control | Actual result |
|---|---|
| Linux complete exact fork, pass-through/cache-fill/cache-warm | Each 4,907 Same; 4,844 nonignored passing / 63 ignored; zero Different, unstable or infrastructure errors |
| Linux independent stock source control | 4,867 Same; 4,804 nonignored passing / 63 ignored; four approved ADR 0093 debug-order exceptions |
| Mac complete raw fork suite | 163 targets; 4,844 passing / 63 ignored; no failures |
| Producer byte/native controls and 186 library tests | Pass on both platforms, including both compiler checksum algorithms |
| Eleven source-engine fixtures and actual cancellation/restart | Pass on both platforms, retaining exact qualified recursive inventories |
| Native composed feature controls | Standalone repair, prepared replay with generated inputs, changed-script rejection and byte budgets pass on both platforms |
| Final factories | Linux and Mac complete with the corrected public runtime |
| Linux strict determinism | All 417 inputs deterministic over three runs, including exact masked stdout, stderr, exits and every solver stream |
| Mac strict determinism | 417 inputs × three: all exact solver streams stable; 416 strict stdout comparisons pass and one fails on three consumed-resource counters |

The strict Mac failure is retained as `mac-rlimit-drift/evidence.tgz`, SHA-256
`ac41746970b24c96a9b2a0de8780895202fc2e4b45a9ace92188026deaea68a7`.
The single input is `examples/imo_1988_6.rs`; requested limits, verdicts,
diagnostics and exits match. The owner subsequently approved ADR 0097; the original strict result remains
failed and the explicit recheck below records the narrower qualification.

- 2026-11-02, broader screen r1: no workloads run because the frozen commit
  argument was empty after Git refused container ownership; explicit source
  identity and the normal safe-directory setting correct setup.
- 2026-11-02, broader screen r2: all 18 commands are refused by a stale 0ad
  conformance binary; the private build requested examples without binaries.
  Building both explicit bins and examples restores the candidate identity.
- 2026-11-02, broader screen r3: the setup uses absent `rg` in a conditional,
  leaving an extra Rust target installed; stopped before accepting measurements.
  The retry uses the available `grep` and retains clean frozen source checks.
- 2026-11-02, Mac composed CPU observer: all feature and standalone controls
  pass, then the independent CPU calibration fails. Converting native counters
  with the platform clock ratio passes the unchanged child-CPU control; the
  original failed measurement is not used as product cost evidence.
- 2026-11-02, Mac A0e stage-only r1: setup omits `SAIL_ENGINE_BUILD` and stops
  before factory qualification. Retry selects the published validated runtime
  and the complete factory passes.
- 2026-11-02, Mac timing preflight r1: refuses 1.305 observed background cores
  before running any measured command. The original 0.10 ceiling is retained;
  the subsequent quiet setup passes at 0.038 cores.

Completed corrected Linux source, producer, source-engine/cancellation and
417-input strict determinism capture: `a0e-source-evidence.tgz`, SHA-256
`c285373db4c9b4a926879742a00df66fc09ece28727a66f24036cb26379ff175`.
The native source suite uses its isolated ForkTests variant and the separate
stock oracle; consumer factory/corpus and performance records retain their own
source and runtime scope.

## Current composition and outstanding qualification

The candidate is based on `000e8edd`, including the merged features, budget
readiness correction and initial bounded client repair. Frozen `039fb996`
retains identical proof admission, runtime pins, native-byte controls and corpus
inputs. Its producer/factory/features and performance evidence keep that scope;
the additional presence-probe correction changes transport only.

Private `4997927d` completes the A0e native Linux composition: Clippy and 1,322
tests, solver51, every E2E phase including `extra`, all four fast arms
(147 Same / 3,451 function records each), all four standard arms
(159 Same / 13,166 each), and all 21 mutation/cold/return comparisons pass.
Zero differences, unstable cases, infrastructure errors or skipped projects;
both returns get their own answers. Complete capture:
`runtime-connect-native-evidence.tgz`, SHA-256
`aa7bb7060286f49b1e95e1322a706a3593f60226ecd09ae2e467442c38e3394e`.

The final presence-probe composition `37af55b8` additionally passes Linux's
1,324-test workspace gate, solver51 and every E2E phase including `extra`.
All four fast arms match 147 units / 3,451 function records each; all four
standard arms match 159 units / 13,166 each. Zero differences, unstable cases,
infrastructure errors or skipped projects; all 21 mutation/cold/return
comparisons match, including both returns obtaining their own answers.
Complete native capture: `runtime-final-r3-native-evidence.tgz`, SHA-256
`25986f896a9082fd2d9653d604ee9153200585ffc451109d7ed92ec211860381`.
Native Mac Clippy, all 1,313 tests, builder
shell syntax and the four parser/actual-CLI controls pass. Its exact source and
complete host log are `mac-runtime-policy-host.tgz`, SHA-256
`e8defb4016a3e335c34176e93425fe84063c36d335c574d15805eb81f4d7a3a4`.
The [transport record](unix-connect-deadlines.md#remaining-presence-probes-654)
retains actual blocked-before/correct-after CLI, jobs and MCP controls.
Protected PR checks and release performance acceptance remain separate.

The corrected fifteen-edit catalogue and Anvil failure/fix/return sequence pass
all 114 native execution audits. Final Linux raw per-sample performance files
were lost at host expiry; their retained execution logs do not establish timing
acceptance. The old broader-screen execution log reaches all eighteen workloads
but cannot establish their individual outcomes without the lost report. A fresh
frozen `039fb996` screen, separate required-target Verismo control and six-round
paired workflow cohort run with automatic capture on native c7i.4xlarge,
us-east-1. The Mac six-round retry completes with its original 0.10-core
quietness limit, as recorded below. The complete performance gate remains open.

The original private read-only ADR 0097 experiment identifies only the three consumed-counter
changes in the original witness and rejects 30 controls covering changed exact
streams/multiplicity, requested limits, exit/stderr, verdicts/diagnostics,
unrelated fields/order/bytes and malformed, ambiguous or out-of-bound JSON,
including an unterminated escaped string. Tokenization advances from the exact
current position, avoiding repeated scans of malformed string tails. It
was diagnostic only. The owner-approved repository implementation and native
recheck are recorded below; composed runtime/corpus/cost gates remain separate.
Private diagnostic result SHA-256:
`881637b3665b9f7e2563b220a326db1fcfbdfb8ec29d20189090c3ad73036a8d`.


## Corrected Mac workflow measurements

Frozen `039fb996`, A0e runtime, native mac-m4.metal / us-east-1c / macOS 26.7:
six rotated stock/candidate blocks complete with the original CPU ceiling.
All 23 paired workflow rows pass their output comparisons; three required
realistic mutation cohorts are absent, so the report remains 23/26 valid and
is not release performance acceptance. Scripted edits are diagnostic only.

| Workflow | Stock median | Sail median |
|---|---:|---:|
| New project and first verification, timed together with fresh Cargo home | 16.72 s | 8.06 s |
| Vest cold / unchanged | 38.88 s / 0.18 s | 28.91 s / 0.03 s |
| IronKV cold / unchanged | 20.48 s / 0.13 s | 10.70 s / 0.02 s |
| CapybaraKV cold / unchanged | 31.29 s / 0.16 s | 29.64 s / 0.02 s |
| Anvil cold / unchanged | 96.87 s / 0.24 s | 98.94 s / 0.04 s |

Three slower medians remain under review: small edits add 0.11 s, small
reverts add 0.10 s, and Anvil cold adds 2.07 s. Anvil unchanged retains its
full 0.04–2.60 s range; a fast median does not establish tail parity. Shipped
vstd is served in every small, Vest and IronKV cold round, and is not offered
for CapybaraKV or the exact focused Anvil workload.

Separate no-replay full-path diagnostics retain twelve calls per arm/project
(two in each of six blocks). Stock/Sail medians are Vest 0.179/0.633 s,
IronKV 0.128/0.191 s, CapybaraKV 0.146/0.640 s and Anvil 0.195/2.347 s.
These observed gaps remain open; background replay does not eliminate them.
The seven-project Mac target-digest experiment is unselected: timed medians
are flat/slower and its separate observer/cleanup controls fail. It remains off;
[the retained study](general-performance.md#mac-per-file-digest-study) does not
establish a product gain.

Raw streams, samples, full ranges and separate cost captures:
`mac-guard-r2-evidence.tgz`, SHA-256
`8454c656b9b674bb060cdb7064c2aad70bffe923aa2085594d40512153e7eb8f`.
Complete execution log SHA-256
`9340ff2a6b3c7b69a82300729855807d528aba5c4026352d01f6e4dfcdad635f`.

- 2026-11-02, Mac broader screen r1: the configured stock home already includes
  its platform directory, so the loader appends it twice. All 18 commands refuse
  before verification; no corpus result is accepted. The retry supplies the
  documented parent directory. Failed capture SHA-256:
  `72e8933afcb14c8938e53a201f0140224338fda2948e7c7c388dc40af5215776`.
- 2026-11-02, Mac file-memo study r1: runtime and memo opt-ins pollute the ordinary
  host fixture environment (1,306 pass / three fail); the measurement never runs.
  The retry runs the ordinary gate clean, then enables the prototype only for
  its actual managed controls and controlled calls. Failed capture SHA-256:
  `c7d3fcf032204e62b949396b9958f5c71aaa8c55caaac085abd37b5a68ba44dd`.
- 2026-11-02, Mac CapybaraKV post-return cost retry: foreground calls succeed,
  but the independent observer again refuses an empty queue. Cost remains
  unqualified; the foreground exit is not substituted for the observer result.


## Corrected broader Mac corpus

Frozen `039fb996` / A0e, native mac-m4.metal / us-east-1c / macOS 26.7:
all 18 workload selections are attempted. The 17 runnable selections have
matching complete function/diagnostic comparisons: 34 units total, zero
Different, unstable or infrastructure errors. This is conformance evidence,
not another timing cohort. It includes the three semi-persistent packages,
verus-tla and Hekate in addition to the established systems and libraries.

Verismo is explicitly skipped by the existing platform gate, even with its
required Rust target installed. Default-limit Vest and RLSF have matching
failed plain baselines; neither is counted as a passing-project baseline.
The other fifteen selections have passing plain baselines. The runner retains
its nonzero aggregate result for the skipped selection; no check is changed.
Capture `mac-broader-039-r2-evidence.tgz`, SHA-256
`50206284422590f82e6c5473915d15f9c13a9c3232969d0e8827da688dc844ed`.


## ADR 0097 implementation and native qualification

The owner approved the narrow classification on 2026-11-02. The candidate's
explicit `--classify-consumed-resources` mode preserves strict comparison by
default, reports each original value/path and retains raw counter-only captures.
Only the three accepted JSON paths may differ; every other stdout byte/order,
stderr, exit and exact solver-stream multiset remains checked. The bounded
lossless parser rejects malformed/duplicate JSON and unknown differences.
The Mac runtime builder selects this explicit mode and stops before packing
on any remaining difference. Its previous blanket warning-and-continue path
is removed; original counters and classification captures remain retained.

The actual repository checker is re-applied on native mac-m4.metal / us-east-1c /
macOS 26.7 to all 417 original three-run captures. This is a complete recheck of
retained observations, not another verifier execution: 416 strict-same inputs,
one consumed-resource-only input and zero remaining qualification failures.
The original one-input strict failure is reproduced. The result binds every
capture file and both implementation files by SHA-256:
`adr0097-mac-complete-recheck.json`, SHA-256
`c5841016d5121eac28f055dff733a37452cf2d63e9d6d317cc9471b4cf61260d`.

Both native Linux and Mac run the new parser/actual-CLI controls successfully.
They require strict-default rejection, explicit-mode acceptance with unchanged
raw counters, and rejection of query-byte/requested-limit, multiplicity,
verdict, stderr and exit changes, plus the bounded malformed/ordering/type
controls. Linux's original complete 417-input strict pass remains unchanged;
no consumption exception is needed for that captured scope.

- 2026-11-02, Mac prototype controls: the first flagged host run pollutes ordinary
  fixtures. A clean retry passes all 1,309 tests. Its installed managed fixture
  then observes the release's shipped vstd proof instead of the local rebuild
  it explicitly requires; the development-binary retry passes the complete
  84-verification managed workflow without changing any assertion.
- 2026-11-02, Mac target-memo timing r3: the original queued stock-oracle job
  outlives its local watcher and overlaps setup. The unchanged 0.10-core gate
  refuses the run (8.48 observed cores); no timings from that cohort qualify.
  Raw functional/refused-cohort capture SHA-256:
  `d096a8ec872a937576cce80535f8c75605b3baa5188813254932863e162ecd30`.


## Corrected broader Linux corpus

Frozen `039fb996` / A0e, native c7i.4xlarge / us-east-1 / Ubuntu 24.04:
all seventeen main-screen selections agree (34 units / 15,549 function records,
zero Different, unstable or infrastructure errors); the complete input/source
end seal passes. Default-limit Vest retains its matching failed baseline.
RLSF also matches its failed plain baseline and gets no timing acceptance.
The remaining fifteen have passing baselines. This is a two-round screen,
not six-round release performance acceptance.

A separate required-target Verismo screen passes (one unit / 1,208 function
records, passing plain baseline, complete two-round samples and end seal).
Its additional Rust target is confined to that phase. Main-screen capture
SHA-256 `1b69223399b1ede7d6ef3947e950503203290edfdea4c10fd645e21f2699a662`;
Verismo capture SHA-256
`0629ed07ac43bdfc0f37da1810d7b4960c90dcc85b91d6e3c9e6923f665b2d06`.

## Earlier runtime attempts

All entries below are from 2026-11-01 or 2026-11-02. Original failed captures
remain retained; setup corrections do not change verdict/diagnostic checks.

- C9 source builder: missing binary-control driver stops qualification after
  compilation; uploading the driver completes the actual controls.
- C9 private receipt probe: logs in the source tree change its namespace;
  separate source/output directories complete the positive and negative witness.
- Composed `1e49b8eb`: Clippy rejects an unnecessary borrow, then invalid
  struct-variant syntax in priority fixtures; both corrected before the gate.
- `aef17eff`: 1,318 tests pass and two runtime-pin tests fail; exact new
  manifests are selected with their independently qualified runtime archives.
- `261f94ff` / C9: Linux 1,320 tests, solver51 and Mac 1,309 tests pass;
  actual preparation/factory fails. Native BLAKE3 byte observations reproduce
  the defect, repaired by A0e. C9 is never adopted on this evidence.
- A0e setup: missing public-base prerequisite, Cargo bootstrap setting and
  incorrect dep-info parser shape each refuse before acceptance; normal
  pinned setup and exact emitted syntax correct the wrappers.
- A0e AIR wrapper: omitted pinned solver fails; actual pinned retry passes
  all 146 AIR tests on both platforms.
- A0e refresh: retained rust-src refuses the release-source prerequisite;
  removing that disposable component permits the normal refresh.

Historical C9 source evidence: `c9-source-evidence.tgz`, SHA-256
`76b6d2f91aa99b86340571101ed0d05e6d7c11540d1e95fecf4c5cfce6dccd45`.
A0e native byte controls, original C9 factory failure and minimal reproduction:
`native-checksum-controls.tgz`, SHA-256
`08acc2208f9feb7cb061b71a006f240dbeb058e64350bcb51bf1402274a7ba7d`.

## Completed Mac preparation-cost controls

Frozen `039fb996` / A0e, native mac-m4.metal / us-east-1c / macOS 26.7:
Vest, IronKV and Anvil already complete the independent post-return CPU controls.
The Capybara retry now uses the matrix's explicit Rust/Cargo 1.99.1 pin and
completes genuine preparation: queued1 to ready1, zero failures/supersessions,
a Current establishing result and a Current zero-verifier result reusing both
targets. Observed daemon/reaped-descendant CPU is 2.8799 s over 2.8110 s wall,
including status polling. This is background cost, separate from caller latency.

The earlier control inherited Rust/Cargo 1.99.0, produced no managed result and
observed no request even with a bounded readiness wait. Its refusal remains;
there is no completed-preparation cost to report for that setup. The passing
control selects the same qualified Cargo as the performance matrix; no product
admission check changes. Raw inherited toolchain, complete Current results,
queue observations and CPU capture: `mac-cost-r4-evidence.tgz`, SHA-256
`524aae195c39449b5f14f97d4b97737ca619f84089406a76d90e8a70e134734c`.
The retained preceding refusal is `mac-cost-r3-evidence.tgz`, SHA-256
`2347bc85f050705bcd03a7cb6f068c87cb84b682ffc2b50374586a5c6a8ea3ad`.
