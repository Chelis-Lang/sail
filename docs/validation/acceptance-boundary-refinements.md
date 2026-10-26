# Acceptance boundary refinement qualification

Status: combined captured-source stock checks pass on macOS and Linux at
`e7f13656`. Native end-to-end qualification fails; publication of the
executable changes remains pending. Draft
[PR #378](https://github.com/Chelis-Lang/sail/pull/378) is stacked on #362.
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
| Combined production proofs | All 42 canonical cases pass on macOS and native Linux at `e7f13656`: three positive roots (172/0, 69/0, 199/0) and 39 ordinary failing controls; every prior function remains inventoried | Final expanded source requires fresh checking |
| Combined workspace/solver/e2e | `e7f13656`: macOS strict clippy and 1,107/1,107 tests pass; native Linux workspace (1,118 tests), all 29 model cases, 42 production cases and 44 solver tests pass | Neutral-contract end-to-end phase fails; remaining end-to-end phases and PR conformance are unrun |
| Differential conformance | Corpus, comparator and deadlines unchanged | PR tier before publication; full fast and standard before merge |
| Cost | Additional byte/header scans, request validation and provider accounting are implemented; no measured delta or gain claimed | Bounded native cost measurement before selection; retain every sample |
| Hosted checks | Normal workflow selects main/release PR bases | Explicit hosted run for the stacked implementation, or retarget after #362 merges; final-head required checks before any merge |

## Failed and interrupted attempts

| Date | Source | Gate | Result |
|---|---|---|---|
| 2026-10-26 | Legacy updater from `c6270f75` | Missing-journal witness | Returns covered=true for missing observation journal; fixed regression passes in provider slice. Accounting defect #377, no complete false-proof exploit established |
| 2026-10-26 | Provider candidate `86567f43` | Independent real-child probe | Malformed Cargo artifact stdout was forwarded but omitted from the journal, yielding a completed empty stream. Fixed in `387f7f2e`; independent exact-body retest passes |
| 2026-10-26 | Request-capture prototype before `421a0a7b` | Environment correspondence review | Fresh transport variables could leak into compilation while excluded from context; replaced with consumed private arguments and typed configuration |
| 2026-10-26 | Combined source before `e3b37022` | macOS workspace tests | 1,104/1,105 pass; stable error registry rejects unlisted SAIL3115. Registered in the integration; no test weakened |
| 2026-10-26 | `2d3492d0` | Full captured query proof root | 171 verified/1 ordinary loop-invariant error in the strengthened input read ledger. Explicit prefix induction in `3d78be9e` makes all three full positive roots pass; runtime checks, contract, invariant and resource settings unchanged |
| 2026-10-26 | `e7f13656` | Native Linux full end-to-end | Stops at neutral-contract proof/source identity assertion because authenticated output-completion markers poisoned input accounting. Fix `946c625e`, integrated as `1551dff2`, passes the unchanged actual pinned-fork witness on macOS and native Linux. Full draft integration remains pending. No assertion or coverage check weakened |

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

## Limits retained after these slices

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
