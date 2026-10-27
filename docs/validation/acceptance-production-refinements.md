# Production acceptance refinement qualification

Status: selected production bodies and model refinements pass direct stock
checking on native macOS arm64 and Linux x86-64. Native pre-push correctness
gates, hosted checks and full fast/standard conformance pass. The owner accepts
the bounded native timing result below; broader release performance and the
service-wide safety theorem remain open.
The original results apply to checkpoint `fe8be9ee`; fresh qualification of
the combined revision-only input projection is recorded below.
This record owns the scope of the
[production refinements](../design/acceptance-production-refinements.md).

## Owned query publication candidate

Whole positive discovery at `f8d61408` passes all three complete stock roots
with final source and runtime rechecks on native x86-64 EC2 `c7i.4xlarge` in
`us-east-1`. Query checks 431 obligations and 759 function entries, inputs
308 and 538, authority 498 and 878; every earlier function remains present.
Root durations are 28.168 s, 31.548 s and 21.467 s respectively. Counts overlap
across roots and do not represent a service theorem or product performance.
The positive-only report SHA-256 is
`30822d09dcc90b76c315fd3ec8fa392af478b7b20def6d9e56e7d5e60cc36876`.

The historical capture retains 115 query, 115 input and 107 authority ordinary-Verus
controls. The corrected `c44cdcfa` query run passes all four partitions:
119 full-scope calls, comprising four whole query positives and the exact
disjoint union of 115 ordinary failures. Independent archive review checks
every verifier stdout/stderr, all 759 functions, the 431-obligation scope,
direct stock commands, source/runtime/compiler identity and absence of resource
markers. Individual reports remain incomplete-component partitions; the
audited union covers query only. Native workspace/solver checks, the complete
340-case matrix and final main integration remain pending. Source-control selftests pass
43/43 at `0247a13e`, retaining all 41 original tests plus two new tests; the
final combined gate must rerun them. Development refusals below remain visible.
Native Rust mutation results are separate from ordinary-Verus negative credit.
Parser semantics, physical origin, protected writers/recovery and full service
composition remain open. This candidate precedes the lead's main reconciliation.

The closed proof archive is preserved privately at
`experiments/2026-10-27/acceptance-query-c44cdcfa-stock-controls.tar`:
64,061,440 bytes, SHA-256
`d54d73f49d80adf58600bc2d14868eaa0706fab6e0c629c622698f74de85d0ef`.
Independent S3 GET/hash verification and the full query-scope audit pass.
It includes development refusals and the final control reports; no live native
logs or native-body qualification are included.

The next option-state candidate retains all 115 inherited query controls and
adds nineteen, alongside unchanged 115 input and 107 authority controls.
The source selftest inventory retains all 44 tests present at `c44cdcfa`
(43 at `0247a13e` plus the role-metadata regression) and adds six later tests.
The existing assembly/private-token fixture now supplies the required option
module beside its original sources, preserving every mutation and assertion
(#458); all 50 pass on native Linux at `90e2329f` in 198.139 s.
Complete whole-positive discovery at `069e91f6` passes the expanded query root
with 441 verified obligations and 776 function entries, inputs with 308/538 and
authority with 498/878, retaining every prior function. Direct stock invocations
use the same 120-second limit, one thread, default resources and complete roots;
final source/runtime rechecks pass. Root times are 33.110 s, 31.395 s and
20.976 s on the same native `c7i.4xlarge`. The closed positive report SHA-256 is
`80829df588f1cad425d1249ab606017c5551a80a9dfd68ec185bbeffa6b817cf`.
The option-only manifest freezes these actual inventories. Its 359-case matrix
(three positives plus 134/115/107 controls), workspace, solver and differential
gates remain pending; the earlier c44 result covers its exact 115 query controls.
The reviewed
scope correction retains the complete validator loops while mutating acceptance
exits (#454), and the completion fixture preserves the actual ten phases (#453).

The combined replay/presentation source passes complete positive discovery at
`e57f065f`: query 471 verified obligations and 821 function entries, inputs
308/538, authority 502/883, with every prior function retained and final
source/runtime checks. The report SHA-256 is
`92bc5c3b953207b84fe8bf37405cfc7d63fdb96dc0ec0f0eaf9bceec891230ce`.
Both complete source suites pass at the preceding `0a3a7f2c` checkpoint:
12 model and 53 production tests, retaining every earlier test. Later changes
only instantiate literal/framing/partition facts in the full constructive
witnesses, leaving executable operations, contracts and all control objects
unchanged. The manifest locks the actual combined inventories. Its 395 unique
planned cases (three whole positives plus 161/115/116 ordinary negatives) remain
pending. Qualification uses four existing partitions per component, components
sequenced: 404 actual calls include twelve repeated whole positives. Each
individual report remains incomplete; the exact all-component union requires
separate identity, function, obligation and ordinary-failure audits. This is
not a single unpartitioned run or a service theorem.

Native integration corrects the production `Duration` import (#455), renames
two payload locals and uses `AtomicUsize::try_update` with the same SeqCst
orderings and closure. The pinned Rust source implements the deprecated
`fetch_update` by calling that exact operation. The inline entry remains
unboxed with a documented lint allowance. Its recording charge now includes
the entire carrier and original logical payload/index allowance, including the
silence carrier (#456); allocator overhead and capacity remain outside this
existing logical budget. On native x86-64 the ordinary parser/delivery regression
compiles and fails with the old formula (356 versus required 476 bytes, carrier
408), then passes with the correction. The closed red/green patch digests are
`cd545f0e9387239ad312b5027b6207970142a942f33d2ee62d3499ac6599b59d`
and `543ec5e478ab7b9fef7660764229ffd0ad2a63ee3848241bbdfb1b1eda340831`.
All 28 cached-session ordinary native tests then pass, including the unchanged
Pairer/holding/annotation paths and option-state readback controls. This is
focused native test evidence, not ordinary-Verus control credit or
workspace/solver/differential/performance qualification.

The three closed native lint stops through `03725fea` are preserved privately
at `experiments/2026-10-27/acceptance-query-native-stops-through-03725fea.tar`:
92,160 bytes, SHA-256
`b57ac975d3924b2383f41c7658474ecd92a2c99120de493ec5e9a1f3ccd19894`.
Native PUT and GET/hash checks pass. Ordinary workspace tests and the solver
suite did not run in those attempts.

## Checked source and runtime

The frozen checkpoint is `3b1cb3623ef17a2c4a99bc8bd3e12e88f1ef63fb`.
The evidence/test changes through `fe8be9ee` preserve its executable, model,
manifest and runner bytes. The independent receipt binds every captured production Rust
source and manifest, distinguishing checked bodies, declaration projections,
reviewed forwarding and surrounding runtime source that remains unproved.

Direct checks use upstream stock Verus `0.2026.10.18.7d82994`, revision
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1` and Z3 `4.16.0`.
The runner validates the pinned archive and every extracted runtime file,
captures canonical Git blobs and invokes stock directly with
`--no-cheating --output-json --num-threads 1`. Sail, its fork and its caches
do not establish these proofs.

| Gate | Current evidence | Remaining |
|---|---|---|
| Captured production-source refinements | All 25 cases pass on both platforms: three positive roots, 22 ordinary failing mutations; counts query 134, inputs 54, authority 163, zero errors; hosted job passes | Service-wide refinement remains open |
| Changed component/composed models | All 29 cases pass on both platforms: four positive roots, 25 ordinary failing mutations; counts 59, 64, 64, 198; hosted job passes | Native observation and production composition obligations remain open |
| Workspace | macOS host: strict clippy and 1,087/1,087 tests. Native Linux full image gate: 1,098/1,098 tests, clippy, doctests, strict rustdoc, dependency policy, layering and generated benchmark page | Hosted combined gate |
| Pinned solver | Native Linux: all 44 tests pass, including faults and cached sessions; macOS focused cached-session tests pass at the earlier unchanged-query checkpoint | Hosted solver suite |
| Checker controls | 20 production-runner and 12 model-runner tests pass; documentation links, formatting and completion guard pass | Hosted checks |
| End to end | Native full run passes all phase entrypoints plus local extra cases, including 73 managed observations, real MCP client listing and repair | Hosted ten-phase aggregate |
| PR conformance | Pass-through, cache-fill, cache-warm and engine each match 33/33 units and compare 311 functions; zero unstable/different/infra errors and zero skipped projects; five units with resource-count drift | Full fast/standard before merge; deep before release |
| Added cost | Bounded native fixture measured below; owner accepts the observed difference and its measurement limits | Broader release performance and attribution remain separate |
| Publication | Native pre-push gates and all 20 hosted checks pass on `fe8be9ee`; PR #362 owns the combined work | Protected merge; hosted checks rerun on the final documentation head |

Linux runs on a dedicated EC2 `c7i.4xlarge` in `us-east-1`. The final full
workspace wrapper takes 108 s; model proofs 254 s, production refinements
197 s, solver 21 s, full local e2e 873 s and PR conformance 107 s. Proof steps
run sequentially with a cached core image and fit the fast-tier job budget on
this host; hosted-runner qualification remains separate. Full local e2e
combines every phase and extra cases; individual CI phases remain separate.

macOS direct checkers take 110.08 s for refinements and 145.48 s for models.
The host gate uses an isolated Cargo home with the existing registry cache.
Counts overlap across proof roots. These execution timings do not measure
Sail's product performance or the cost of the additional acceptance logic.
The 46 ordinary-test skips and 1,100 solver-step skips retain the existing
partition; two compiler-boundary witnesses require their separate frozen inputs.

Raw artifacts remain outside the repository. SHA-256:

| Artifact | Digest |
|---|---|
| macOS production refinement receipt | `0424771e79f2a4839198014034267182c49696486f364c7cf7b3cb9930223d9f` |
| macOS changed-model receipt | `64c543fe0cba032ac546b5c9edffc46210ac6e50814799896b1bcfd991731bb1` |
| Native Linux production refinement receipt | `2f6d90d3a7604765196b0fa3d7709ea8809550e20c3b484e7b70df98864d6ce9` |
| Native Linux changed-model receipt | `22971f3de43d093d7c6d0dcc76e6d3ed7f2cbe6c49bbb22c3c8cb937c0ee7efe` |
| macOS host log | `35c048c4db31d3221a65eac272266ff5b11cf5c168520f26b02ba044bcd99dfe` |
| Native Linux workspace log | `2d0ffbd5119c6206c4589b71efee63beb2ef35d911eb3672863248898a296bd6` |
| Native Linux solver log | `4e3cc6849c7b0d2d371c41c052a4c827744d71fce874cb33b208e23e361ab20c` |
| Native full e2e log | `45722e10fdbf62637d4a2f7cbf2b6feb410dc2e43ccdc938ade5f96977d53ffc` |
| Native PR conformance log | `4b8eb2143bd96700331c0a3b6357f86fab057a00bc3d4ac3012295489094c0a7` |
| Four raw conformance reports | `70c9ac8be3eb4a635a764bcce64626803e3ca09a9ff670f7a4934ca3e74cba67` |


## Failed and interrupted attempts

| Date | Source | Gate | Result |
|---|---|---|---|
| 2026-10-26 | `fdd05dcc` | Combined replay/presentation discovery | Literal framing/statistics and positive compared-stdout factory witnesses pass; only the final different-output refusal postcondition remains unproved. Instantiate the original exact receipt and sealed frame interval/bytes; no executable, predicate, control or resource change and no expanded inventory locked |
| 2026-10-26 | `0a3a7f2c` | Combined replay/presentation discovery | All 12 model and 53 production source selftests pass; whole query reports six constructive-witness diagnostics in three new replay/output factories. Instantiate exact literal name/list framing and existing transcript partition facts without changing execution, contracts, controls or limits; input/authority discovery and the planned 395-case matrix remain unrun |
| 2026-10-26 | `c87877d8` | Expanded whole-positive discovery | Ordinary and reset-assertions preservation witnesses pass. The final reset relation still needs the same explicit original-head existential instance; no expanded inventory, negative matrix or final-root qualification claimed |
| 2026-10-26 | `b74bcb06` | Expanded whole-positive discovery | Exact reset head bytes pass; the two ordinary preservation instances still need explicit original-head existential witnesses and non-setter/non-reset name facts. No contracts or scope are changed; all later roots/controls remain unrun |
| 2026-10-26 | `fe8d021f` | Expanded whole-positive discovery | Both actual setter witnesses pass; constructive ordinary/reset-assertions success exits remain unproved. Instantiate unchanged preservation/reset relations for the remaining literal history; retain all executable calls and do not weaken the full always-refuse control |
| 2026-10-26 | `52501686` | Expanded whole-positive discovery | Every literal scalar-field assertion passes, but the two successful setter calls in the constructive history lack instantiated next-state witnesses. Retain the full factory and add erased original head/keyword/value equalities and exact next-state instances; no positive inventory or control credit |
| 2026-10-26 | `7a1cf36c` | Expanded whole-positive discovery | Literal atom obligations pass, but two constructive keyword-field assertions remain unproved (440 verified, one failing function). Add explicit extensional equality between original literal head bytes and the existing UTF-8 name predicate; no executable, predicate or resource change and no expanded inventory qualified |
| 2026-10-26 | `90e2329f` | Expanded whole-positive discovery | All 50 source selftests pass, then the whole query root reports two unproved literal scalar-field assertions in its constructive option-history witness (440 verified, one failing function). No expanded inventory or later root/control qualification; add erased literal atom/span guidance without changing executable transitions or predicates |
| 2026-10-26 | `b572bc99` | Expanded capture selftests | 49/50 pass; existing assembly/private-token fixture omits newly required `query_options.rs` and raises `KeyError` before its mutations (#458). Add that source to the fixture, preserving every assertion; no whole-positive discovery or ordinary-Verus control started |
| 2026-10-26 | `03725fea` | Native workspace gate | Strict clippy next refuses two owner/payload similar-name locals, deprecated atomic alias and the enlarged inline entry variant; ordinary workspace tests and solver remain unrun. The correction preserves inline ownership and charges its complete carrier (#456) |
| 2026-10-26 | `9f6dfd52` | Native workspace gate | Shared-kernel lint corrections pass; strict clippy next refuses the native completion adapter's optional-environment guard. Use the equivalent optional borrow and retain the same raw output, with an explicitly reviewed native source binding; ordinary tests/solver remain unrun |
| 2026-10-26 | `c44cdcfa` | Native workspace gate | Source selftests complete before strict clippy refuses eight shared-kernel style diagnostics. Reasoned local annotations retain checked Option branches/lifetimes, and a local payload name changes; no executable transition or contract changes. Ordinary workspace tests and solver suite remain unrun at this checkpoint |
| 2026-10-26 | `35373419` | Initial four query-control partitions | Partition 0 completes 30 cases; partition 2 refuses an rlimit hit in `output-exact-appended-bytes`, and partitions 1/3 refuse whole-validator mutations that removed a loop VC. No aggregate qualification: retain full validator bodies/loops with terminal acceptance forced false, and add an exact-byte proof cut without changing executable append, contracts, count checks or limits |
| 2026-10-26 | `6b7b9c49` | Expanded query whole-positive discovery | Two constructive checkpoint-rank assertions remain unproved; expose the already checked frozen key/material relation directly in its factory postcondition and instantiate the original literal header/span equalities. No completed positive inventory or negative qualification claimed |
| 2026-10-26 | `dfda8f71` | Expanded query whole-positive discovery | Reports 16 unmet obligations in new capture/checkpoint/owner constructive proofs; proof guidance adds retained invariants, exact sequence equalities and owning-module preservation facts. Executable bodies, predicates, original controls, deadlines and resources remain unchanged; no positive count qualified |
| 2026-10-26 | `0247a13e` | Expanded query whole-positive discovery | Refused cross-module reveals of two closed partition specs before verification; an owning-module proved lemma supplies the same facts without widening visibility. Source selftests pass 43/43; no new positive count is qualified |
| 2026-10-26 | `e7f4dbc6` | Expanded query capture selftests | Refused three stale delivery mutation contexts and one ambiguous native-test source fragment before positive discovery; contexts are updated to preserve their original mutations, with no check or scope relaxation |
| 2026-10-26 | `fe8be9ee` | Detached merge cost guard | Stopped on a +4.09% external proof-edit median; all correctness gates pass. Retained samples expose 50 ms process-wait polling and a smaller internal timing difference; attribution remains unprofiled. Owner accepts this bounded result |
| 2026-10-25 | Query refinement before `953f74a6` | Closed-stderr solver regression | Repeated five-second timeout; captured shim stack and open reader FD while producer blocked writing stderr; fixed by closing the failed reader before joining. No false proof demonstrated |
| 2026-10-25 | Integration development source | Stock-driver compatibility | Unsupported proof-envelope/type and inventory mismatches refused; corrected without scope, resource or timeout relaxation; not qualification |
| 2026-10-25 | Checker development source | Independent assembly/erasure review | Cargo path redirection and a pointer-equality/proof-import shadow mutant exposed missing source-correspondence checks; fixed with reviewed assembly/import inventories and refusal controls. Late build-script injection now refuses after the proof loop |
| 2026-10-25 | `8ba92b01` input-reader source | Broad libtest diagnostics | Global Cargo wrapper caused one configuration refusal; direct-binary run had two failures of unknown cause. A Cargo-runner serial control with isolated Cargo home passes 437/437, two solver cases ignored; final nextest gate above passes independently |
| 2026-10-25 | `9f8085a0` | Native Linux full workspace gate | 1,098 Rust tests pass; rustdoc refuses an unqualified macro-generated API link. Fixed with an explicit public item path; no check weakened |
| 2026-10-25 | Integration candidate | Native Linux preparation | AWS SSO expired before host launch; no native gate or measurement claimed |

The query failed/passed runs, stack/FD captures and separate primitive probes
are indexed by `stderr-ownership-index.json`, SHA-256
`c523f56d3b1ca0e6a11fc8389e4bff7dd89f2ddc9461106b3e02949bb6876d5f`.
The shadow mutant's stock-output SHA-256 is
`1efa76cd7cae16a12bf0bc19bed306fb97d80c1853e612381b04013780d24c72`;
its altered proof view passed while native Rust violated strict order. The
final runner refuses that binding mismatch. A full orchestration control also
checks that a mid-run auto-build file prevents creation of a success receipt.

The isolated input diagnostic stdout SHA-256 is
`cdb9ac13efe690f436a11305b3339827f33b41c252b9fdbe8582336ff333a6d7`;
the focused global-wrapper refusal stdout SHA-256 is
`b69f7c599e619f477994a88710ee9b1214c24025bdc8425b80ef576da7438c1a`.
The earlier broad diagnostics have tool output only; no raw-file digest is
claimed for them.

The stock compile-only observation is documented in
[VI-0035](../verus-issues/VI-0035-no-verify-success.md). Intended compile-only
behavior is not a verifier translation defect. The candidate fixes Sail's
misinterpretation, including effective package options, older private records
and CLI/MCP proof wording ([#367](https://github.com/Chelis-Lang/sail/issues/367)).

The [historical model qualification](evidence-acceptance-models.md) applies to
its stated commits. Checked source bodies and native caller tests do not prove
parser framing, full compiler-input closure, physical observations, truthful
cache origins, retained-request correspondence or complete service composition.
Those remain explicit obligations in
[#368](https://github.com/Chelis-Lang/sail/issues/368).

The failed `9f8085a0` native gate log SHA-256 is
`5defc8d305c11ba90fe804be95fe3b87a11b05720fc1cc1019b3ca28762bbb0e`.

## Full merge qualification

Implementation `fe8be9ee1a3db45db9899caa5ec8706dc28ab8c4` passes full
conformance on native Linux `c7i.4xlarge` in `us-east-1`. The original corpus
selection, comparator, deadlines and baselines are unchanged:

| Suite | Each of pass-through, cache-fill, cache-warm and engine |
|---|---|
| Full fast | 147/147 Same; 3,451 functions; zero unstable/different/infrastructure errors or skipped projects; 11 units with resource-count drift |
| Standard | 159/159 Same; 13,166 functions; zero unstable/different/infrastructure errors or skipped projects; standard command includes its HumanEval mutation controls |

Full fast takes 653 seconds including preparation; standard takes 4,776
seconds. All 20 hosted checks pass on the same implementation head, including
both direct-stock proof suites and the complete end-to-end aggregate.

The same native host measures original main
`3b0bac191d0866fdd400d7dcd52e9cacc50d5cfe` against the candidate, with the
same pinned stock/fork runtime. Three rounds per version exercise one tiny
Cargo project and its vstd dependency. Each proof edit runs the verifier once
and reuses one dependency on both versions. All samples and validation remain
intact; no refusal or failed sample is discarded.

| Scenario | Baseline external median | Candidate external median | Change |
|---|---:|---:|---:|
| Unchanged | 0.364958 s | 0.364843 s | -0.03% |
| Proof edit | 1.217066 s | 1.266806 s | +4.09% |

These are observations, not an attributed cost of the new validation logic.
The runner times `Popen.wait(timeout=...)`; the measured Python 3.12.3 POSIX
implementation polls with sleeps reaching 50 ms. The external proof-edit gap
is approximately one such interval. Sail's own internal proof-edit medians
are 1.171367 s and 1.199841 s, a 28.474 ms increase over a different timing
boundary. Neither measurement profiles the contributing operations or
establishes a general slowdown. The owner accepts the bounded result on
2026-10-26; release-wide performance remains separately gated.

Raw original artifact:
`experiments/2026-10-25/proof-362-merge-fe8be9ee-durable.tgz` in the private evidence bucket.
SHA-256: `5565452090c829ef098ee139aab4e4800d89fd6c488f9b7aca54fcb31555d8c8`.
The documentation-only checkpoint update preserves all implementation, proof,
manifest and checker bytes qualified here. The later integration with main's
revision-only projection changes native adapters and requires its own fresh
qualification before the protected merge.

## Combined revision-only projection qualification

Combined implementation `c6270f75ab228efbdf30a288308dd9713b4e58f2` integrates main `3ca8264ee4f50de868a1f640598a4aa3939f10dc`.
Native x86-64 Linux validation uses a dedicated EC2 c7i.4xlarge in us-east-1.
The workspace gate, direct-stock model proofs, production refinements, pinned
solver faults, all local end-to-end phases plus extra, and PR conformance pass.
147/147 Same in all four arms; 3451 functions per arm; zero unstable/different/infra/skipped.
159/159 Same in all four arms; 13166 functions per arm; zero unstable/different/infra/skipped.
The full original corpus commands, mutation controls and deadlines are unchanged.

The merge retains the actual returned input descriptor beside each digest,
the complete checked read ledger and original-capture projection. Revision-only
callers avoid constructing an unused full snapshot. A missing capture or a
rebound observation revision refuses projection. The 26 focused input tests
and 1,087-test isolated macOS host gate pass before publication. The first
Mac host attempt refused the operator's global Cargo wrapper configuration;
that attempt is not qualification. The fresh direct-stock receipts bind the
combined source, including surrounding native adapters that remain unproved.

The bounded timing observation above remains specific to `fe8be9ee`; no new
performance claim is inferred from this integration or its proof checks.
The service-wide theorem and the named observation/origin obligations remain open.

Raw combined qualification: `experiments/2026-10-26/proof-362-integration-c6270f75-durable.tgz` in the private evidence bucket.
SHA-256: `28098974ddb3c9381ac7b7b12c39ced51d4a42a23955e87809f7a52930f23995`.
The final documentation commit changes only the four status-owning guides
and this record; required hosted checks rerun on that final head.
