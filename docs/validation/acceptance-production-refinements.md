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

## Consolidated acceptance-audit bugfix candidate

The main-based bugfix extraction changes qualification tooling and fixture
isolation independently of the broader #378 production refinements. Its
current implemented scope is:

| Finding | Correction | Evidence and limits |
|---|---|---|
| #454, #508, #537 | Preserve function contracts, loop annotations, erased proof groups and all but one proof-delimited runtime interval; refuse proof-only faults and bare verification statements | All 45 local source tests pass. All 22 existing control objects and 25 captured programs remain unchanged; comparison makes zero Stock calls |
| #532 | Accept digits in reviewed proof-import identifiers | Exact import hashes, allowed prefixes and cfg/body refusals remain; covered by source tests |
| #419 | Resolve configuration homes from the same supplied environment | Supplied-home and CARGO_HOME-precedence regression passes in the native gate at `9597caad` |
| #514 | Distinguish the exact `.union(` member expression from an erased declaration | Positive Set union and all erased-declaration/alternate-macro refusals pass |
| #432 | Match lexer regexes at absolute source positions, avoiding remaining-tail copies | Six lexical span/literal/refusal regressions pass; no token/envelope memoization or new main timing claim |
| #475 | Separate core/Singular target volumes by physical checkout | Actual-wrapper fixture passes on macOS with a recording Docker substitute; separate checkouts differ, same checkout/alias stay warm. Native aggregate uses a fresh physical-checkout target volume |
| #540 | Restore generated-file scenario source and build script before later stock-oracle controls | Full local native E2E passes at `9597caad`, retaining every comparison/assertion |

Native aggregate `9597caad4e2311b642b7470764376928cbbd426c` passes:
1,161 Rust tests, solver51, model29, refinement25, full local E2E (all phases
and extras), and PR conformance. All 25 captured programs equal main byte for
byte. Host: c7i.4xlarge, x86-64 Linux, us-east-1; core image
`sha256:85ce0f274de085e47ae6bd297555fa4c43b58e18c6c3c4b392e0c608219899f2`.
Artifact `acceptance-audit-bugs-9597caad-native.tgz`, 7,023,701 bytes, SHA-256
`f77e197e5922752d69e61a00d3c50a1dc89430c41592e8ecfb856a180a333f10`;
source/image and upload/readback guards pass. This source has the same tree as
`c575471e`; later documentation commits through `08ba9bb6` leave its code
unchanged. All hosted checks pass at `08ba9bb6`.

Independent review of that head found a remaining source-admission gap: an
unchanged erased assertion could move past a runtime assignment when a separate
runtime edit was present. The original 22 registered controls do not do this.
The candidate now allows runtime edits in only one exact interval between
unchanged proof groups, with bidirectional relocation and ambiguous repeated-token
regressions. Multi-interval controls are conservatively refused. Fresh native
qualification of this correction and full fast/standard conformance are pending;
the earlier aggregate does not qualify the later checker change.

The #540 failed aggregate passes native gate (1,160 Rust tests), solver51,
model29 and production refinement25 before the full managed sequence fails;
PR conformance is unrun. Hosted phase-isolated checks pass. Artifact
`acceptance-tooling-split-d1518341-v3-native.tgz`, 6,665,267 bytes, SHA-256
`004a85a477ba1cd41c195635cc8e39265ff6adbe01032ca08444fcc7191b6d4b`;
source/image and upload/readback guards pass.

| Date | Source | Gate | Result |
|---|---|---|---|
| 2026-10-27 | `d1518341` | Full native local E2E | Failed at ghost-environment stock comparison: generated-file fixture leaked `OUT_DIR` source into an oracle lacking its build script (#540) |
| 2026-10-27 | `08ba9bb6` | Source admission review | Assertion relocation with a separate runtime edit was admitted; source-only reproduction, no Stock invocation or wrong Sail verdict demonstrated |

Earlier runtime/input fixes already merged in #436, #445, #512 and #515 are
not reintroduced by this extraction. Corrections belonging only to unselected
parser, metadata and cache components remain tracked with those components.
The independent #534 envelope cache is not a prerequisite. The only production
Rust change observes supplied configuration homes; no proof source or manifest
changes. The later source-position correction remains subject to the fresh
qualification described above.

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
