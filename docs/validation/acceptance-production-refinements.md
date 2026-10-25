# Production acceptance refinement qualification

Status: selected production bodies and model refinements pass direct stock
checking on native macOS arm64. The integration remains a candidate: native
Linux, conformance, hosted checks and added-cost qualification are pending.
This record owns the scope of the
[production refinements](../design/acceptance-production-refinements.md).

## Checked source and runtime

The frozen checkpoint is `20073229c44ba062fb8ff3f7fc35103fb21f1a86`.
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

| Gate | Native macOS result | Remaining |
|---|---|---|
| Captured production-source refinements | All 25 cases pass: three positive roots, 22 ordinary failing mutations; positive counts query 134, inputs 54, authority 163, zero errors | Native Linux and hosted duration/packaging |
| Changed component/composed models | All 29 cases pass: four positive roots, 25 ordinary failing mutations; counts 59, 64, 64, 198 | Changed-source native Linux and hosted checks |
| Host workspace | `ci/run.sh host`: strict clippy and 1,086/1,086 tests pass; 46 skipped | Native Linux full gate |
| Real cached-session solver tests | All eight pass with the pinned Z3 and unchanged deadlines | Full native solver suite and conformance |
| Checker and integration controls | 20 production-runner and 12 model-runner tests pass; links, formatting, completion guard and layering pass | Hosted combined gate |
| Conformance | No candidate whole-corpus result claimed | Full e2e, fast/PR and fast/standard conformance |
| Added-cost and CI duration | No clean cost measurement claimed | Native measurements before selection |
| Publication | AWS access restored; local branch, not pushed | Required native gates, then exact-head hosted checks |

Counts overlap across roots. The direct checker wall times were 123.34 s for
production refinements and 153.04 s for models; both ran alongside the host
gate. These are execution receipts, not isolated performance or CI-duration
qualification. Host validation uses an isolated Cargo home with the existing
registry cache, avoiding the owner's global compiler-wrapper configuration.

Raw artifacts remain outside the repository. SHA-256:

| Artifact | Digest |
|---|---|
| Production refinement receipt | `503141e0ec5fde1b861492b34f689e181c31f26e7d4e278edc936a4e29e8e39e` |
| Changed-model receipt | `4c25588daca56e8781c320eb261a9cb2d45bd2590d4177f107b11ced1e000714` |
| Host-gate log | `f7a546ded477901ea272ca138542a550b934195d6af130fa2e9271b7d488be2a` |
| Eight cached-session tests | `20d681c2ebf26eb6e309778f23de992e438890c667a26eaa29bf4e02df27f78f` |

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
