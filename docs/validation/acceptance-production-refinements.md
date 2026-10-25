# Production acceptance refinement qualification

Status: selected production bodies and model refinements pass direct stock
checking on native macOS arm64 and Linux x86-64. Native pre-push correctness
gates pass. Hosted checks, full fast/standard conformance and added-cost
qualification remain pending; the integration remains a candidate.
This record owns the scope of the
[production refinements](../design/acceptance-production-refinements.md).

## Checked source and runtime

The frozen checkpoint is `3b1cb3623ef17a2c4a99bc8bd3e12e88f1ef63fb`.
Subsequent evidence/test changes preserve its executable, model, manifest and
runner bytes. The independent receipt binds every captured production Rust
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
| Captured production-source refinements | All 25 cases pass on both platforms: three positive roots, 22 ordinary failing mutations; counts query 134, inputs 54, authority 163, zero errors | Hosted checks |
| Changed component/composed models | All 29 cases pass on both platforms: four positive roots, 25 ordinary failing mutations; counts 59, 64, 64, 198 | Hosted checks |
| Workspace | macOS host: strict clippy and 1,087/1,087 tests. Native Linux full image gate: 1,098/1,098 tests, clippy, doctests, strict rustdoc, dependency policy, layering and generated benchmark page | Hosted combined gate |
| Pinned solver | Native Linux: all 44 tests pass, including faults and cached sessions; macOS focused cached-session tests pass at the earlier unchanged-query checkpoint | Hosted solver suite |
| Checker controls | 20 production-runner and 12 model-runner tests pass; documentation links, formatting and completion guard pass | Hosted checks |
| End to end | Native full run passes all phase entrypoints plus local extra cases, including 73 managed observations, real MCP client listing and repair | Hosted ten-phase aggregate |
| PR conformance | Pass-through, cache-fill, cache-warm and engine each match 33/33 units and compare 311 functions; zero unstable/different/infra errors and zero skipped projects; five units with resource-count drift | Full fast/standard before merge; deep before release |
| Added cost | No product performance gain or added-cost result claimed | Native measurements before selection |
| Publication | Required native pre-push gates pass; draft PR #362 owns the combined work | Exact-head hosted checks; owner review |

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
