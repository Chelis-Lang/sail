# Compiler-read source origin qualification

Status: native Linux and macOS workspace gates pass on functional integration
`95a2312b`; its six required hosted checks also pass. The complete Linux runtime
acceptance remains attributed to `96de5097` and the exact executable variants
below. The Mac runtime completes engine11, cancellation, libraries180, the selected
19-target source suite (97 pass, one inherited ignore), installed origin4 and
all320 inputs three times (960 actual calls, zero variation). The actual95
consumer passes the bounded four-control fresh/warm/provenance-SAT smoke.
Anonymous published fetch/install, all four fast-PR arms and normal new-project verification also pass at the same source/binary scope. This record
tracks the paired Verus observation producer and Sail reuse consumers for
[#551](https://github.com/Chelis-Lang/sail/issues/551) and the distinct caller
replay gap [#552](https://github.com/Chelis-Lang/sail/issues/552). The #552
old-main reproduction returns a stale Current, confirmed verified report after
only the ghost caller input changes; fresh pinned stock rejects the same source
and changed value with a concrete bit-vector SAT counterexample. The #551
old-main compiler-child overlay defects were established from source; no
executed old-main wrong-verdict reproduction is claimed. ADR 0088's visibility exception
does not permit stale actual input reuse.

Schema2 requires explicit COMMON driver-origin metadata, separately from both
phases' tracked environment. Other driver declarations remain in those phase
maps. Genuine source COMMON reads stay raw and refuse reuse even when unset, empty or
equal to driver metadata. The same shared eligibility/marker-clear guard applies
to ten exact Sail compiler-child controls listed in `docs/freshness.md`.
Provenance and the compiler-input endpoint/binding had source-established
old-main holes alongside COMMON; the remaining seven controls are conservative policy.
Before-Cargo socket/solver/bootstrap values remain fingerprinted by Cargo.
Routine Z3/Singular declarations and inherited user inputs retain reuse.
A family-scoped write-ahead marker makes unknown/ineligible units compile
before Cargo freshness; a one-time family cleanup retires legacy origin state.
Receipt and archive versions also retire legacy reuse authority. Markers bind
the full target identity, including compiler arguments. A fresh different-unit
observation cannot clear an older marker; complete owned package cleanup may
retire those facts only after strict physical fingerprint invalidation.

The Linux descriptor selects the schema2 runtime from fork `47fb3be6`,
archive `8da9b487...`, manifest `d6096d5b...`; the immutable fork prerelease is
published and anonymous strict fetch/install passes. The macOS descriptor selects the constructed
schema2 archive `52ba48d4...`, manifest `ea6853a8...`; its native runtime
qualification passes and the immutable fork prerelease is published. Earlier Linux checkpoints correctly refused the old
macOS descriptor. The functional integration passes both native workspace gates.
See [actual runtime construction](compiler-read-origin-runtime.md) for the
normal no-rust-src build, real pack/install and independently audited receipts.
The producer source gate passes at exact `47fb3be6`; the source tests and
runtime constructor remain distinct from paired consumer acceptance.

| Gate | Result |
| --- | --- |
| Producer source gate | PASS at `47fb3be6`: libraries23/13/144, actual four-case origin matrix, all162 unfiltered fork targets (4,839 pass,63 inherited ignored) |
| Sail unit and workspace gate | PASS on `dbda2535`: 1,170 tests, 54 inherited skips; fmt/clippy/docs/layering/deny/generated benchmark page |
| Direct stock conditional models | PASS on `dbda2535`: 4 positive models and all 31 controls; normal resources |
| Production refinements, including bound native caller | PASS on `dbda2535`: 3 positive roots, all 168 controls; query282 / inputs54 / authority184 verified, whole crates |
| Current workspace gates | `95a2312b`: complete native Linux gate 1,195/1,195 PASS; native Mac host clippy and 1,184/1,184 PASS; 54 inherited skips on each platform |
| Solver and complete native e2e | `96de5097`: solver51 PASS and unchanged complete e2e/all ten phases plus extra PASS |
| Full fast and standard conformance | `96de5097`: all four arms PASS; fast147 and standard159 units per arm, zero differences, instability, infrastructure failures or skips; standard mutation controls PASS |
| Native useful reuse and lifecycle | `96de5097`: basic64 and lifecycle10 PASS on both actual normal build variants; genuine changed-input SAT failures preserved |
| Native added cost | Coherent minimal production pair: three alternating pairs, zero refusals; tiny-fixture warm medians 310.863ms baseline / 304.480ms candidate, overlapping samples; no speedup or isolated-overhead claim |
| Native macOS runtime and consumer | Construction, runtime qualification, source95 host1184 and bounded smoke4 PASS; anonymous install, four arms of fast PR (33 same units / 311 functions each) and normal new-project verification PASS |
| Runtime oracle and determinism | Linux8da9 and Mac52ba PASS at fork47: engine11, cancellation and direct320×3 on each platform |

Tests cover source-vs-driver COMMON classification, strict origin decoding,
source-only reads in either phase, legacy refusal, retained/shipped admission
and durable pre-Cargo origin state. These fixtures are not real-project
acceptance. The Linux native useful-reuse gates cover ordinary JSON/jobs/MCP,
retained restore and cached shipped dependencies, plus observation-mode/log-path
changes and source-read removal. No private provider/transport probes form part
of this qualification.

## Attempts

- 2026-10-28, `c78bae84`: native Linux workspace gate failed at strict clippy
  (a collapsible conditional and an oversized regression function). Later gate
  stages did not run; no runtime or proof qualification is credited.
- 2026-10-28, `f597d5ad`: native Linux workspace gate failed at the route
  regression's empty-string assertion lint. No later stage is credited; the
  assertion is retained with value-bearing failure output.
- 2026-10-28, `e02529b2`: native Linux strict clippy passed; workspace tests
  ran 1,169 cases, with 1,168 passed and one existing managed-child Runtime wire
  fixture missing the new required origin fields. All newly added origin,
  route-selection and legacy-family-clean tests passed. Remaining gate stages
  did not run; the fixture is updated without changing its checks.

## Local paired-runtime runner

[The local runner](compiler-read-origin-runner.md) uses a normal validated
installed variant and public CLI/jobs/MCP requests. Its corrected basic scope passes all 64 native cases against the real candidate,
including both SAT semantic controls. Source95 additionally passes the bounded Mac four-control smoke; this is separate from the full Linux64-case scope.
Actual owned interruption and eligible retained A-to-B-to-A pass all ten lifecycle
controls at `96de5097` as well as the earlier `a1600f19` checkpoint. These receipts
retain their actual source and executable identities; they do not qualify the
new integration head automatically. Neither runner scope alone closes overall acceptance. Complete compiled
proof programs retain explicitly checked correspondence to `dbda2535`; new
runtime and cleanup code remains surrounding unproved code.

## Caller-only replay requirements

The #552 repair carries names from actual target-record admission into the
private no-op facts. It does not reopen unused receipt reports or broaden any
namespace rule. Current, sibling, seeded and restored receipt requirements use
the same admission results; an unknown required receipt prevents recording.
Seeded runs already cannot record replay; their adopted receipts are ordinary
requirements on later runs. Version4 rejects earlier incomplete replay records.
Actual native prepare/replay unit controls pass, and the paired shadow semantic
witness reaches a real replay hit followed by the required changed-input SAT
failure. Complete end-to-end acceptance subsequently passes at source96; #552 is already merged independently on main.

- 2026-10-28, `4220886e`: final Linux-only whole gate correctly refused the
  old macOS descriptor under the new shared fork pin; no whole-head gate credit.
  A controller-only attempt first refused Git ownership before checks. The
  separate scoped Rust run then stopped at one diagnostic-free empty-vector
  assertion lint in the new #552 unit test; no tests were credited for that run.
- 2026-10-28, `61eda6b8`: scoped clippy passed and 1,175 unit tests ran;
  1,172 passed, including all new #552 controls. Three runtime-discovery fixtures
  failed because the controller leaked an explicit build override into tests
  of default-cache discovery. The normal source-check environment leaves that
  override unset; no production or test assertion is changed for this failure.

- 2026-10-28, `4607ccdf`: native scoped Rust clippy passed; 1,174/1,175 tests
  passed (54 inherited skips). The remaining test correctly rejects the old
  macOS artifact's variant under the shared new fork pin. Independent release
  build and all 51 solver tests pass. The first paired runner passes 53 checks
  before stopping at its incorrect expected public exit code: the changed-input
  proof actually recompiles and is confirmed failed, with Cargo exit 101 and
  nested Verus exit 1. Its pinned-solver check is UNKNOWN with incomplete
  quantifiers, also reproduced directly with stock. A bounded direct oracle
  confirms that the same assertion using `by(bit_vector)` produces SAT failure
  when present and UNSAT success when absent in stock and the candidate. This
  corrects the fixture while preserving its required SAT and failed-proof checks;
  the stopped runner remains failed and later scenarios unrun.

- 2026-10-28, `9e86ce6a` runner / `4607ccdf` built consumer: corrected basic
  scope PASS, 64/64 cases in 222.248 seconds; final source/runtime checks pass.
  Actual interruption scope stops before any probe signal on its strict live
  compiler identity guard; no lifecycle acceptance is credited.
- 2026-10-28, `4607ccdf`: full native end-to-end attempt FAILS at the unchanged
  output control after a temporary compiler-input witness is removed. An older
  argument-variant marker causes an unnecessary rebuild. The compiler-origin
  cleanup correction requires both owned layouts to be cleaned and independently
  checks physical fingerprint invalidation before retiring package facts.
  Later e2e phases and full fast/standard are unrun in that attempt. The output
  failure collector is incomplete; original stdout/stderr and producer records
  remain preserved, with no conformance credit.

- 2026-10-28, `7a1c420b` lifecycle runner / `4607ccdf` consumer: interruption,
  public cancellation, process extinction, restart and read-removal controls
  pass. Real Vest A1/B verify and returned A2 naturally restores with zero
  verifiers and A1's establishing run, but the runner refuses full attribution
  JSON equality: restored attribution is unavailable. The original failed
  assertion and complete field differences remain preserved. ADR 0050's
  verdict/diagnostic/origin contract does not promise identical attribution;
  the next control must recognize only its precise documented unavailable
  shape, retaining exact checks on every other field. No product fix or main
  bug is inferred from this harness assumption.

- 2026-10-28, `204f7322`: strict native checkpoint stops at two new test
  identities missing their Cargo-name field and one production conditional
  style lint. No tests or later runtime gates run. The identity fixtures and
  equivalent branch ordering are corrected without dropping any check.

- 2026-10-28, `aefef5ca`: strict native workspace clippy and all 10 focused
  compiler-origin/cleanup tests PASS, including seven new controls. The current
  312-path source capture includes the new test file; assembly, inventory and
  working-source checks pass. All 35 model and 171 refinement compiled programs
  remain byte-identical to the actual `dbda2535` Stock calls (no new proof calls).
  The added cleanup I/O is surrounding unproved code, qualified by Rust tests
  and runtime gates rather than that proof correspondence.

The final replay-coverage audit also refuses recording when a compiler
observation has no artifact-backed admission. Its observed report remains
available; unknown receipt inputs cannot become no-op authority. This correction
is local to the open replay fix and receives a focused actual-route regression.


- 2026-10-28, `a1600f19`: scoped clippy and release build PASS; native host
  tests pass 1,182/1,183 with 54 inherited skips, with only the known old macOS
  descriptor refusing the shared new fork pin. Basic acceptance passes 64/64,
  lifecycle passes 10/10 (actual owned cancellation, restart and profitable Vest
  A1/B/A2/A3 restoration), and solver tests pass 51/51. Full end-to-end FAILS
  at unchanged-call recording: the carrier incorrectly required every missing
  target to have a report even for human output's permitted unreported target.
  Later full fast/standard remain unrun. The correction binds the exact returned,
  admitted subset, preserves requested-coverage failure for omitted targets,
  and refuses included unknown/artifactless reports. A strict readable invocation
  inventory independently excludes any real or uncertain compiler start before
  recording; the public compiler counter is unchanged. The original end-to-end
  assertions remain unchanged for the next attempt.

- 2026-10-28, `0ae88ef9`: strict clippy and all 18 focused replay/admission
  controls PASS; host tests pass 1,184/1,185 (54 inherited skips), with only the
  known pending macOS descriptor failure. Release build and basic64/64 PASS;
  solver51/51 PASS. Full unchanged e2e progresses through ordinary replay but
  FAILS at focused `partial-record`: all three calls genuinely start demo and
  its no-verify vstd dependency (two starts each), despite known source-eligible
  observations. The marker-clear predicate mistakenly requires whole-proof
  success, which focused and no-verify reports intentionally cannot claim.
  The correction separates non-authorizing input-observation completion from
  proof success; all scoped receipt and publication rules stay unchanged.
  Full fast/standard remain unrun for this attempt; its failure collector is
  incomplete, and absent raw marker captures are not inferred.
- 2026-10-28, `0ae88ef9`: the independent public #553 counterpart PASSES:
  unchanged `--release` warm call uses zero compilers; changing only FORCE for
  one identical call genuinely starts the selected unit and returns a new
  Current confirmed report (CLI0, one compiler, no SAIL3107). The matching
  actual start, non-retained invocation, raw complete-scope verifier output and
  final source/runtime/tool guard are preserved. The main issue stays open
  until its fix merges; this is an operational rebuild check, not a false-proof
  reproduction.

## Qualified integration

PR #572 is based on main `e1365eb2`, which already contains the independent
#552 replay repair. The latest Linux receipts remain attributed to `96de5097`.
Its normal e2e/example-enabled binary (`11246f3c...`) and normal conformance /
minimal production binary (`2837f369...`) are distinct builds. Each separately
passes basic64 and lifecycle10; complete e2e uses the former, full corpus and
mutation checks the latter. The ordinary minimal release command selects the
latter bytes exactly. No binary equivalence is inferred from shared source.

Both platform descriptors and selection guards name the constructed, qualified
runtime bytes now published as immutable fork prereleases. The integration adopts the selector parser/regressions from focused PR #570
and the single runtime scanner/regressions from #557. Origin metadata and
whole-family cleanup retain manifest/config selectors while package cleanup
also preserves the actual profile and target; physical invalidation checks stay
required before retiring captured marker facts. API-only socket-key issue #571 remains
separate. All 35 model and 171 refinement compiled programs/dependencies remain exactly
identical to the actual `dbda2535` Stock calls under the current 313-path
source capture, assembly and inventory guards. This is proof-input correspondence,
not a proof of the new cleanup I/O. A final Markdown-only amendment preserves
every non-Markdown Git blob and mode from the qualified functional head; its
required hosted checks still run on that final head.

## Closed native evidence

| Packet | SHA-256 | Bytes | Scope |
| --- | --- | ---: | --- |
| `acceptance-env551-consumer-96de-native-final.tgz` | `397baf04b4994b436d70ea955c7c22f8370d271c14e1e081408b0fd1cb88cb09` | 6,575,172,107 | Source96 runtime acceptance and earlier prefixes; explicit regenerable scratch-object/IR exclusions |
| `acceptance-env551-integration-95-native-gate.tgz` | `aa36d5ebbebd626011f218f249021455ea9d1acfa19bdc32b39fc29ff10d3485` | 232,178 | Exact95 complete Linux workspace gate and 206-program correspondence |
| `acceptance-env551-macos-host-95-native.tgz` | `2d5609f53beec9946e6184af7d0a525ec5298589688a62efd37edf36af9e60e5` | 359,530 | Exact95 normal Mac host gate, unchanged-deadline scheduling comparisons and failed prefixes |
| `acceptance-env551-macos-consumer-95-native.tgz` | `541f67ee0a7b043e9f93d51be29d7007fbb8088e6831a990fe4926a4f394a081` | 43,690,633 | Actual95 frozen binaries, smoke4, published four-arm fastPR/new-project, full input/runtime/Stock guards and failed setup prefixes |
| `env551-macos-runtime-4b3-qualification-v2-ccf7edc4.tar` | `ccf7edc4f2ea8e697c6b1fe01136f19bee65fa3f67752b81f247c8f17bf67398` | 418,590,720 | Mac runtime gate: all960 raw calls/320 inputs, origin4, source19, libraries180, engine11/cancellation and original failed prefixes |
| `acceptance-env551-paired-cost-96de-B-native.tgz` | `1c34c729e77ab18e207d86fb61a03dd6dadc89bf6cdf5d1ec93a53ce8c51a4bd` | 61,410,758 | Three coherent alternating pairs, all 42 calls including six cleanup calls; no refusals |

Keys share the prefix `experiments/2026-10-28/`; each private upload is checked
by a fresh complete readback. The large96 packet uses 25-part transport after
a preserved single-PUT SSL EOF (no actual HTTP cause established). Its
transport closure is preserved separately as
`acceptance-env551-consumer-96de-upload-closure.tgz`, SHA-256
`03acd30c7a82d805eebbc1d4318f15c384271405deefa0c73f8a49ac3bcadc16`.
No raw bytes of the explicitly excluded regenerable IR are claimed re-audited.

The cost run uses c7i.4xlarge in us-east-1d: coherent old main/schema1 and
source96/minimal-B/schema2, six cold and 30 warm calls plus six cleanups.
Cold medians are 34.440437s / 34.110622s (three samples per arm); warm medians
310.863077ms / 304.479639ms (15 per arm), with overlapping ranges. This is a
tiny-fixture paired observation, not isolated validation overhead, a corpus
speedup or a measurement of the final integration head.

- `95a2312b`, Linux: initial controller refused Git ownership before checks;
  corrected command-scoped ownership then passes the complete gate.
- `95a2312b`, Mac: initial controller exits127 before Cargo due to an incorrect
  taskpolicy path. The background-QoS whole-host attempt then passes1,175 of
  1,184 and fails nine deadline controls. Controlled unchanged-source comparison
  reproduces seven failures with added background QoS and passes all nine at
  nice19 with normal host QoS. The complete normal-QoS host rerun passes all
  1,184, unchanged tests and deadlines, with source/tool hashes stable.

The Mac public fast-PR arms each compare 33 identical units and 311 functions
with zero different, unstable, infrastructure, feature-refusal or skipped
results. Five rlimit drifts per arm remain reported; the engine establishes
32 focused units. Normal `cargo verus new` and verification of its unchanged
public template succeed (raw vstd2060 and project1 verified, zero errors).
Both full Stock and fork runtime inventories/version identities stay unchanged.
The functional source95 CLI, shim and helper hashes are recorded in the packet;
no equivalence to another build configuration is inferred.

- Source95 published attempts: the first refuses missing Stock before any unit;
  a fresh-cache `xtask fetch-verus` prefix also refuses (#581), and the next
  guard points at the install parent instead of its executable subdirectory.
  The corrected attempt uses the documented `SAIL_VERUS_HOME` install parent,
  separately guards its actual Stock directory, and passes every unchanged
  public check. No conformance assertion is relaxed.
