# Acceptance boundary refinement qualification

Status: expanded captured-source stock checks pass all 82 production cases
and 31 model cases on macOS at `42637e0d`, with the full host gate passing
1,128/1,128 tests. Native Linux passes the earlier 71 production cases at
`0b646c48`; its end-to-end gate fails on journal reservation
reuse during a repeated same-run compiler invocation. The allocation correction
at `bcd21d9d` restores complete target evidence, but the unchanged managed test
still observes three verifier invocations instead of one; publication remains pending. The
earlier retained-transition candidate passes 78 cases and the macOS gate at `0c3dbc1d`. Draft
[PR #378](https://github.com/Chelis-Lang/sail/pull/378) targets main after #362 merged.
No service-wide theorem, complete compiler closure or authenticated physical
observation is claimed. The [obligation ledger](../design/acceptance-boundary-refinements.md)
tracks the six remaining deliverables separately.

## Source and actual scope

| Slice | Source checkpoints | Established component boundary |
|---|---|---|
| Raw query bytes and local record framing | `74f4a8c5`, `9e187938` | Exact owned transcript intervals, gap validation, contiguous feeds/replay and header/newline/payload roundtrip; actual shim/cache callers use shared bodies |
| Compiler declaration provider | `dcc3be33`, `e3b37022`, proof-only `3d78be9e` | Exact supplied producer lifecycle and required artifact-frame/read association; native producer/parser refuses missing or malformed JSON-looking observations and caps additional accounting at 64 MiB |
| Original request authority | `5bcfd678`, `1e6be959`, `d2740dfa` | Actual typed request/header/target/report relations and raw repair selection; private pre-execution capture retained through job/frame/baseline admission |

Component stock probes use the pinned original Verus
`0.2026.10.18.7d82994`, revision
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1`, pinned Z3,
`--cfg sail_verus --no-cheating --output-json --num-threads 1`, whole-crate
verification and unchanged resource settings. Local probes use different
assemblies and are not combined canonical qualification: query 163/0 with six
ordinary guard failures, inputs 69/0 with eight failures, authority 160/0 with
seven failures. Counts overlap and must not be added as a coverage metric.
The final closed source/type/import/function inventories and all original
guard controls must qualify together before selecting integration.

Focused component tests pass: query/cache/shim tests and eight actual pinned-Z3
sessions; provider source, real-child and unit-plan tests; typed authority,
retained job, engine, CLI and declared-environment controls. Independent review
retests nine real producer cases and the sticky capture-budget control from
the exact committed bodies. These tests do not prove the native adapters.

The request metadata channel is typed runner configuration carried through a
private consumed CLI argument. It does not inject new environment variables
into compilation. Original absent or user-supplied values remain consistent
with declared environment-read accounting.

## Native and hosted gates

| Gate | Current evidence | Remaining |
|---|---|---|
| Initial documentation draft | Native Linux full workspace gate at `d4bb18a3`; 1,098 tests and all workspace checks pass. Implementation bytes equal parent `c6270f75`, whose pre-push proof, solver, full e2e and PR conformance gates pass | Does not qualify the later executable changes |
| Combined production proofs | All 42 canonical cases pass on macOS and native Linux at `e7f13656`. The expanded `7cbf806e` matrix passes all 71 cases on macOS: three positive roots (216/0, 91/0, 273/0) and 68 ordinary failing controls; every prior function remains inventoried | Final expanded source requires fresh combined macOS/Linux checking |
| Combined workspace/solver/e2e | `e7f13656`: macOS strict clippy and 1,107/1,107 tests pass; native Linux workspace (1,118 tests), all 29 model cases, 42 production cases and 44 solver tests pass | Neutral-contract end-to-end phase fails; remaining end-to-end phases and PR conformance are unrun |
| Differential conformance | Corpus, comparator and deadlines unchanged | PR tier before publication; full fast and standard before merge |
| Cost | Additional byte/header scans, request validation and provider accounting are implemented; no measured delta or gain claimed | Bounded native cost measurement before selection; retain every sample |
| Hosted checks | The draft targets main; its published head is the initial documentation checkpoint | Publish qualified executable changes and require final-head checks before any merge |

## Failed and interrupted attempts

| Date | Source | Gate | Result |
|---|---|---|---|
| 2026-10-26 | Legacy updater from `c6270f75` | Missing-journal witness | Returns covered=true for missing observation journal; fixed regression passes in provider slice. Accounting defect #377, no complete false-proof exploit established |
| 2026-10-26 | Provider candidate `86567f43` | Independent real-child probe | Malformed Cargo artifact stdout was forwarded but omitted from the journal, yielding a completed empty stream. Fixed in `387f7f2e`; independent exact-body retest passes |
| 2026-10-26 | Request-capture prototype before `421a0a7b` | Environment correspondence review | Fresh transport variables could leak into compilation while excluded from context; replaced with consumed private arguments and typed configuration |
| 2026-10-26 | Combined source before `e3b37022` | macOS workspace tests | 1,104/1,105 pass; stable error registry rejects unlisted SAIL3115. Registered in the integration; no test weakened |
| 2026-10-26 | `2d3492d0` | Full captured query proof root | 171 verified/1 ordinary loop-invariant error in the strengthened input read ledger. Explicit prefix induction in `3d78be9e` makes all three full positive roots pass; runtime checks, contract, invariant and resource settings unchanged |
| 2026-10-26 | `e7f13656` | Native Linux full end-to-end | Stops at neutral-contract proof/source identity assertion because authenticated output-completion markers poisoned input accounting. Fix `946c625e`, integrated as `1551dff2`, passes the unchanged actual pinned-fork witness on macOS and native Linux. Full draft integration remains pending. No assertion or coverage check weakened |
| 2026-10-26 | `48f0e1d2` | macOS combined host and source-checker self-tests | Strict clippy passes; host tests pass 1,120/1,122. Coalescing fixture still writes legacy version 2; target collision fixture builds outside the directory scanned by Sail. Corrected fixtures retain all assertions and add legacy-record refusal; all four focused cases pass. The source-checker mutation fixture also needs the actual optional-where syntax; its refusal assertion remains required |
| 2026-10-26 | `7cbf806e` | macOS host and native Linux workspace | The source-checker self-test omits the newly protected authority module from its fixture snapshot; adding it and all four constructor-refusal mutations passes the focused test. Native workspace stops there, with later gates unrun. macOS strict clippy passes and host tests pass 1,122/1,124; both failures read the host's existing global rustc-wrapper configuration. Qualification reruns with a separate Cargo configuration directory while preserving the original cache and test assertions |
| 2026-10-26 | `0b646c48` | Native Linux full end-to-end | Workspace, all 29 model cases, all 71 production cases and solver tests pass. Managed acceptance fails its original one-invocation/reuse assertion after an edit: the fresh target is verified but has SAIL3111 and no completion authority. Actual syscall tracing confirms a second driver in the same run/unit reuses slot 0 and receives EEXIST; the earlier reservation and observation remain present. No assertion or coverage check is relaxed; remaining e2e/PR conformance are unrun |
| 2026-10-26 | `bcd21d9d` | Native Linux complete managed phase | Exclusive journal allocation restores complete evidence for both reported targets, with no report error. The original cold-edit assertion still fails: three verifier invocations, zero reused targets, expected one invocation. The 111.73-second run is a failed targeted regression check, not a full pre-push gate; the remaining cause is under investigation |

Legacy updater-body SHA-256:
`94d497cbcd16f4f6f0b94b6d4166d07f3d26abf54b9b05abc9c78eb433a65ef4`.
Independent fixed-provider stdout SHA-256:
`42888842f4de2a732f3b5389b3411120daa5c93e61e8d6b7344c1124ff1e3a05`;
source-index SHA-256:
`97840e61fad11cfb5012f48bea1fb7813ec463f6f3017fa3eda5ae75526c11a0`.
Raw receipts and logs stay outside the repository. The frozen `e7f13656`
native attempt, source bundle and component/macOS receipts are preserved at
`chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-26/acceptance-followup-e7f13656-native.tgz`.
SHA-256: `aa24c12eaa3a456234fd40b6074cdb9b0029c7fcda325a2defa7f1df67b57895`.
The archive preserves the stopped attempt; it does not qualify an end-to-end
pass or any subsequent source.

The stopped `7cbf806e` native workspace attempt is preserved at
`chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-26/acceptance-followup-7cbf806e-native.tgz`.
SHA-256: `c51ba5a60bb9c512074a42365f8705f065c590b258789d18d4f83ba3c7456825`.

The stopped `0b646c48` full native attempt includes canonical proof receipts,
source bundle, Mac receipts and actual managed stdout/stderr/events at
`chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-26/acceptance-followup-0b646c48-native-failed.tgz`.
SHA-256: `6a982da75ed339021493459bc49c5119978a82099c76e448367fb6dd66f90b07`.

The exact repeated-slot syscall trace and failed `bcd21d9d` correction check
are preserved at
`chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-26/acceptance-journal-regression-bcd21d9d.tgz`.
SHA-256: `2f1fc696c3726be9e0f7ebb04e8fb0ab281572d19fdc527d0a462530c37ca822`.

The complete `42637e0d` macOS receipt and exact source bundle are preserved at
`chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-26/acceptance-followup-42637e0d-mac.tgz`.
SHA-256: `e6faab8ffd7f6d7c5ca7645037427040bdeac67e37feae0d01472fb7fecd7874`.
All three production partitions bind identical captured source, stock archive,
runtime and Rust identities. The 82 cases contain three positive roots and 79
ordinary failing mutations; the 31 model cases contain four roots and 27
ordinary failing mutations. Every prior function remains inventoried. The
full macOS host gate, 26 production-capture tests and 12 model-checker tests
pass. The source commit's later documentation amendment changes no checked
body, assembly, manifest, runner or runtime. These results do not qualify the
native service or complete the six design obligations.

The environment and retained-readback defect probes are preserved privately at
`chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-26/acceptance-boundary-defect-probes.tgz`.
SHA-256: `4d0e17baff34cc202ae081829e6784a09fe453dcb1ddd0b6622a9b238d1c39e5`.

## Limits retained after these slices

The file-kernel/readback capture candidate at `3571ecaa` passes all three
positive stock roots (224/0, 113/0, 299/0), preserving every prior function.
Its 27 capture tests pass, including refusal of altered abstract host contracts,
erased type shadowing and unverified read-token constructors. Ten new ordinary
guard controls join the original controls. The complete 92-case production
matrix still requires qualification at the final source. The private native
file host, OS/SHA operations and compiler-domain closure retain their named
proof obligations; accepting an abstract trait declaration discharges none
of them.

The model runner now requires every inventoried guard control by name and
order. Its failure tests remove each required control in turn; all 12 tests
pass. Production capture additionally protects the private unconfigured
attempt constructor and requires the exact Cargo-name, normalized Rust-name,
target-kind and configured-slot controls. All 26 capture tests pass. Expanded
positive roots pass direct whole-crate stock checking at `f52ae9b3`: query
216/0, inputs 91/0 and authority 291/0; models 59/0, 64/0, 68/0 and composed
202/0. Every prior function remains inventoried. The complete new mutation
matrix qualifies at `42637e0d` as recorded above. The native service still
requires fresh complete checking at the final source.

SMT delimiter/classification and identity/hash encoding, complete actual
compiler inputs, Cargo/dep-info semantics, physical reads and launches,
metadata serialization, cache writer truth/persistence and all service
transitions remain open under their named contracts. Corruption that resembles
ordinary non-JSON verifier text cannot be ruled out by JSON-document refusal.
An endpoint trace is accounting evidence, not an authenticated solver or
compiler observation. Previously uncovered source inventories remain uncovered
until an owning recovery protocol can establish their replacement; a later
empty or partial observation cannot invent that fact.

## Framing milestone

Candidate `eeadb086` checks one complete supported lexical response frame and
an exact bare command head before actual shim cache interpretation. Its whole
query root passes directly invoked pinned stock Verus with 200 verified and
zero errors at unchanged settings; six new executable guard mutations each fail
with 199 verified and one ordinary verification error. All prior functions and
controls remain required in the integration inventory.

Thirteen shim unit tests and ten actual pinned-Z3 cached-session tests pass on
macOS. The late-cache-fault witness first serves one legitimate cached reply,
then refuses a self-consistent two-form payload and compares full stdout,
stderr and status with direct pinned Z3. Removing only the actual frame guard
injects an extra response and fails that same assertion ([#379](https://github.com/Chelis-Lang/sail/issues/379)).
The marker-accounting regression is separately tracked in
[#380](https://github.com/Chelis-Lang/sail/issues/380). These results qualify
component behavior; complete command semantics, launched process identity,
truthful origins and the service theorem remain open. Canonical combined
source and native integration require fresh complete checking after integration.

## Opened-object and original-submission milestones

The read factories retain the actual opened object, exact returned chunks,
complete byte count and EOF, then compare final handle/path attributes. Private
seals are checked again against both canonical and requested paths before
phase-local memo admission or reuse. The native retarget seam originally checked
only the canonical path; independent integration review found and fixed the
requested-path gap before qualification. Outer snapshot guards remain separate;
no public false-proof exploit is inferred from the inner seam witness.

Original-submission factories preserve immutable original arguments, actual
job mode, independent request and actual project borrows. Their derived
`SubmissionKind` is a request class, not a completed engine/fallback origin.
Normal fixed-scope direct-stock component checks and ordinary mutations pass;
full canonical combined source qualification is pending below. Runtime helpers
and actual dependency-domain closure remain explicit obligations.

The host-envelope prototype compiles and executes the same nonempty counter
host in the native Cargo profile and passes direct stock checking (three
obligations). Removing the actual closed-controller guard produces one ordinary
verification failure. This is syntax/control-flow qualification, not a physical
process or compiler-input provider proof. Full combined checking retains all
original roots, functions and ordinary guard controls.

## Retained-result transition candidate

The actual daemon request-enforcement, currentness classification and
freshness-application bridges now call shared API operations. The whole
authority development root passes direct pinned stock Verus with 278 verified
and zero errors at the original settings, retaining every prior function.
The new composition checks original-request authority when a Current retained
view preserves completion. A coherent nonempty request remains accepted;
refusal preserves its reports, and freshness projection preserves all targets.

The frozen implementation `0c3dbc1d` passes all 78 captured-source cases on
macOS: query 216/0, inputs 91/0, authority 278/0 and all 75 ordinary failing
guard controls, including the seven retained-transition mutations. Its full
macOS host gate passes strict clippy and 1,124/1,124 tests. The focused
API/daemon native tests also pass 202/202. Source-capture self-tests pass 26/26. The capture
names the three native bridges and pins their enclosing source/binding
environment separately from proved bodies. The native module's I/O, cache
selection and serialization are not claimed verified. The later qualification
edit changes only this record. Native Linux and combined integration gates
remain pending for this candidate. Physical compiler-input closure,
origin history and every service/presentation path remain separate obligations.

## Environment counterexamples

Candidate `4f46f822` reproduces two default-mode disagreements using the same
qualified fork `36d65c3f` and pinned Z3 on both sides. In
[#385](https://github.com/Chelis-Lang/sail/issues/385), a proof checking the
presence of internal `SAIL_MANAGED_RUNTIME` fails under plain Cargo-Verus but
passes under Sail with Current complete evidence and later reuse. In
[#386](https://github.com/Chelis-Lang/sail/issues/386), a proof checking an
ordinary variable establishes a Current complete receipt with that variable
set. Unsetting it still reuses the receipt as Current complete; forced fresh
Sail and plain Cargo-Verus reject it. Source/phase observation and original
environment restoration remain open; neither a variable exclusion nor an
environment digest establishes the missing semantic correspondence.

The observed Sail binary SHA-256 is
`3743b6a31aaf008db98a1095af04223379a032e8fca3b06b550d9cd5734dcb5e`.
The private-environment source SHA-256 is
`1c259c0cbd2784985522786fcc6e7de2dafa64d20619ae94739ddada6e525967`.
Raw private/public probe JSON SHA-256 values are
`f17e19670eeb3e6bea066f1ff388cb1ec490cbb2c47795a30310d482e1874342`
and `824d01d2a8307efd39088f4f03e404e5f7b56859ae2a81e0ad5840efc66d6201`.
These are actual execution counterexamples, separate from component proof
passes. Main `920ce606` has the same injected-environment path by source
inspection; its binary has not been qualified against these fixtures.
