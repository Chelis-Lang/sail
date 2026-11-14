# Compiler-owned AIR identities

Status: the combined AIR consumer and crate-namespace correction pass native
Linux workspace, solver, complete E2E, PR, fast and standard conformance,
including the owning mutation/revert sweep. Supporting runtime remains unselected.

## Source and scope

Consumer code `efff77ec3917aefd7798658bd3aef00a2b8ae919` is based on main
`0cbb2093`. Its two attribution files are byte-identical to reviewed development
`9a58aa0b`. Main's repository/embedded fork pins, shipped artifacts, qualified
manifest constants and dependencies are unchanged. This capability alone cannot
select the new compiler or authorize proof reuse from an unknown runtime.

The producer is unselected Verus `a3109bed5e02313050d5ba6a23f1bc3e1d549c9b`,
extending `1d2f3423`. Query-local metadata is emitted after lazy AIR startup;
SMT logs and actual solver input are unchanged. Normal native build verifies
2,059 vstd obligations, zero errors. Manifest SHA-256
`3e7b13848e4b96eccdadd68c9a8997643cf9f96fcc386b42120c4ef2df70b626`;
packed runtime `041a42ec649bf418e3aae0a4bb2e33cbd50002facad427998255128ebb0ea110`.
All thirteen build-evidence payloads and every packaged runtime file are
independently lead-verified; original build archive
`5d623257220ebe7a30a6bcaea5dca15ab059fb579c19ae9b38dbcf5b523e3484`.

## Native Linux consumer integration

EC2 c7i.4xlarge, us-east-2, exact consumer source above:

| Gate | Actual result |
|---|---|
| Workspace gate | 1,417 passed, 58 declared skips; formatting, Clippy, documentation, layering and dependency checks pass |
| Solver | 54 passed |
| End to end | Complete local run, including Extra, all managed cases and repair |
| PR conformance | Four arms each 33 Same / 311 functions, zero different/unstable/infra/skipped; seven resource-drift units retained |
| Fast corpus | Four arms each 147 Same / 3,451 functions, zero different/unstable/infra/skipped; 25 resource-drift units retained |
| Standard corpus | Four arms each159 Same /13,166 functions, zero bad/skipped; owning18-mutant sweep fails five publication-note comparisons (#748) |

Every one of 872 full-gate originals is independently rehashed and retained
externally. Archive SHA-256
`002f8a34bcc21e150cc94e1f7555a780337c2e3444d45bda704cbbab73b6aad3`.
All32,698 original PR/fast/standard payloads are also lead-rehashed and externally
retained, archive `57c32df7db6a02a72450c67d3b35651276b205aebd9dcadf31bb5bdbafe6ba1a`.
All21 owning mutation samples remain raw; five fail solely on the newly emitted
publication note and all false-verified lists are empty. The failed run is not
called passed. The corrected combined result is recorded below. Original failed samples remain
failed; source-identical bounded review evidence is reused explicitly.

## Corrected current-source integration

Exact native source `3952fc0bf258d3a2fe5dae95fdee3ee6350c4689` composes the
reviewed consumer, namespace correction069ff3d0 and publication adapter7b68ef69.
The current-main rebase3ae8af8d retains identical crate, dependency, CI and runtime
pin bytes. Both attribution files are byte-identical to069ff3d0; the AIR identity
module is byte-identical to independently reviewed9a58aa0b. Documentation
successors do not change those executable bindings.

Native c7i.4xlarge/us-east-2 results:

| Gate | Actual result |
|---|---|
| Workspace | 1,423 passed /58 declared skips; full gate passes |
| Solver | 54 passed |
| E2E | Complete local run, including Extra, all managed cases and repair |
| PR | Four arms each33 Same /311 functions, zero bad/skipped |
| Fast | Four arms each147 Same /3,451 functions,25 resource-drift units, zero bad/skipped |
| Standard | Four arms each159 Same /13,166 functions,32 resource-drift units, zero bad/skipped |
| Owning sweep | All21 cold/mutant/revert/forced-revert samples Same; all false-verified lists empty |

Runner exits0. All32,680 original payloads,5,700,699,466raw bytes, are independently
lead-rehashed and externally retained, including original runner/log/exit, source
bundle, binary/tool bindings, E2E captures and every corpus/sample report. Archive:
`98b5694b8c602e6164efd481b8e7eddf9217f2d5547cda200bdcc2046896945a`;
index `b917dc289a5f35d11c623c8ade725989a2e5c9afc886b11459bb8b11c5f8d04a`.
The lead independently checks all12corpus summaries and21samples. No artifact,
qualified manifest constant or selected runtime is advanced. Hosted checks must
pass on the final current-main candidate before merge. No performance claim or
platform-specific implementation change is introduced by this consumer slice.

## Independent bounded reviews

Producer review approves a310: two helper tests and the persistent compiler
regression pass. The identical regression executable fails the rejected d616
producer specifically at missing adjacency. Thirteen valid comparative cases
preserve plain stdout, stderr, exits, timing verdicts/function details, SMT
files and all 204 complete bidirectional solver-session tuples. Initial/final
AIR, bit-vector/nonlinear spinoffs, recursive and assisted termination,
recommends, expansion, multiple errors and actual profile reruns are exercised.
All 1,768 raw payload hashes and three symlink targets are independently
lead-verified. Archive
`ea98bd83769a1db2812ef7d6af5358915b82f7df5589f61818900945ee458fd4`.
A Rust 2018 formatting warning in the new regression's failure message remains
visible; its asserted adjacency condition and executed counterfactual pass.

Consumer review approves 9a58's identical implementation: 15 native attribution
tests, 98 main assertions, ten additional required assertions and three actual
shipped-producer comparisons pass. Actual a310 positive control reports three
verified/complete; negative reports two verified, one failed/complete. A genuine
malicious filename creates eight apparent AIR queries versus four solver ops;
completion is refused and the failing target retains one failed/two unknown
with the VI-0003 note. Corrupted metadata/timing, provenance and receipt
controls remain strict. Actual shipped a0e11cff and unknown/legacy producers
preserve the baseline's full public outputs in the exercised cases.

An additional broad row-order assertion fails on both baseline and candidate:
only the crate display name changes. This pre-existing defect is #746; its
failed assertion and independent baseline reproduction remain retained. No
repository oracle was changed. All 345 material review files plus 29 local
review payloads are independently lead-verified and retained externally.
Archive `a1e67ae750d48f3998a06da3839609f70b8daf4898d16cb9210ca761344fb4a5`.

## Preserved failed and interrupted attempts

| Source / date | Result and cause |
|---|---|
| 48f479ed, 2026-11-14 | Build stops before compiler: repository pin changed, embedded pin did not; corrected 81e38065 normal build passes |
| Producer d616db76, 2026-11-14 | Independent review rejects first bit-vector metadata before lazy AIR prelude; original 1,298-payload archive `09c36cc8a1caf23c0cfc731f7e98f7a726ba3cf30a1a3bdc6cd790465a0b27d2` retained |
| Consumer 4e6ffa92, 2026-11-14 | Candidate image correctly refuses its source pin versus the old shipped artifact, before Rust tests |
| Consumer 4f24b0c3, 2026-11-14 | Required Clippy rejects two 101-line functions; no suppression, corrected shared ordering helper/initial-note iterator |
| Consumer 9a58aa0b, 2026-11-14 | Component Clippy and 15 tests pass; subsequent full gate still refuses two stale-artifact tests. This is not packaging qualification |
| Producer review, 2026-11-14 | Interrupted build-directory setup, invalid pre-SMT spec-profile experiment and missing rustup PATH retained; dedicated valid followup supplies the missing coverage |
| Consumer review, 2026-11-14 | Offline driver registry, root-owned isolated install and assumed optional filename failures retained; corrected controls complete. One overwritten status is separately preserved by observed-status JSON and original stderr |
| Source-suite runner 1/2, 2026-11-14 | Before compilation: exact-checkout Git ownership refusal, then overlayfs rust-src component-removal failure; clean minimal toolchain-home runner is in progress |

## Limits and remaining gates

ADR0087 classification: runtime attribution parser. Full fast and standard
corpus gates apply before merging this consumer change. Models, refined
production bodies and their checkers are unchanged. No performance or Mac pass
is claimed. The real three-function control does not qualify full IronKV/Vest
mutation campaigns, costs or concurrency.

The new producer's complete source suite passes all three4,910-entry fork arms
(63 declared ignores,4,847 nonignored), and the4,867-entry stock oracle (63 ignores,
4,804 nonignored,three approved debug-order exceptions). All29compiler libtests
pass plain and shim. Every391 original payload is independently rehashed and
externally retained, archive `51f5a155b1b1235bcb5b0142a6a8674b1ed98e6097f7418fa96d82251ff3c879`.
Its fork lock matches the embedded e047 source-lock; no closed cache snapshot is
claimed because the runner deletes its ephemeral cache after recording199,122
entries. Full new-runtime corpus, determinism, Mac,
ordinary selection and release qualification remain separate. The metadata
join does not enable focused selection of ambiguous source methods: existing
unique compiler/source-fact admission remains required. No artifact or
qualified constant is advanced, and no release is published.

Actual full IronKV/Vest campaigns with normal a310 runtime pass through full
verification but both stop at the physical-owner adapter, whose legacy source
inventory has no canonical paths. Original sources remain unchanged; no mutant
score is admitted. An explicit schema2 source-fact path join is development
work, not a relaxation of the singleton owner check. This is separate from
main #746's crate namespace correction.
