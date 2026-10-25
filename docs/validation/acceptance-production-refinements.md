# Production acceptance refinement qualification

Status: integration candidate. Source-shared operations and production callers
are implemented. Independent final captured-source qualification, native Linux,
conformance, hosted checks and cost qualification remain pending. This record
owns the scope of the [production refinements](../design/acceptance-production-refinements.md).

## Source and scope

The candidate builds on the expanded model work and current main's pinned
runtime. It adds `sail-acceptance`, typed API factories, concrete relations and
production callers. Direct checking uses upstream stock Verus
`0.2026.10.18.7d82994`, revision
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1` and Z3 `4.16.0`.
The runner validates platform archives and all extracted runtime files from
`corpus/toolchain.toml`; it does not invoke Sail's verifier path.

Development compatibility runs establish that the three captured drivers can
check on native macOS arm64. Their counts are not final qualification: source
is still changing, particularly effective compile-only options and retained
record admission. Final source, complete inventories, actual mutation results
and receipt hashes belong here after the committed-source gate runs.

| Gate | Current evidence | Remaining |
|---|---|---|
| Native macOS workspace | Checkpoint `38a8ef34`: host gate passes, 1,066 tests; isolated Cargo configuration | Repeat on final source |
| Query regressions | Eight real solver cases pass in four bounded runs after closing failed stderr readers; original deadlines retained | Final integration and native Linux |
| Captured-source stock refinements | Component development proofs and executable guard controls pass | Full canonical committed-source runner |
| Checker failure controls | Source-capture, stencil, assembly and forwarding refusal controls implemented | Final combined gate |
| Native Linux and conformance | Not run for this candidate; AWS SSO session expired | Full native gate, fast/standard conformance |
| Added-cost and CI duration | Unmeasured for combined candidate | Native measurements before selection |
| Hosted completion | Not published | Exact-head required checks |

## Failed and interrupted attempts

| Date | Source | Gate | Result |
|---|---|---|---|
| 2026-10-25 | Query refinement before `953f74a6` | Closed-stderr solver regression | Repeated five-second timeout; captured shim stack and open reader FD while producer blocked writing stderr; fixed by closing the failed reader before joining. No false proof demonstrated |
| 2026-10-25 | Integration development source | Stock-driver compatibility | Unsupported proof-envelope/type and inventory mismatches refused; corrected without scope, resource or timeout relaxation; not qualification |
| 2026-10-25 | Checker development source | Independent assembly/erasure review | Cargo path redirection and a pointer-equality/proof-import shadow mutant exposed missing source-correspondence checks; fixed with reviewed assembly/import inventories and refusal controls |
| 2026-10-25 | `8ba92b01` input-reader source | Broad libtest diagnostics | Global Cargo wrapper caused one configuration refusal; direct-binary run had two failures of unknown cause. A Cargo-runner serial control with isolated Cargo home passes 437/437, with two solver cases ignored; final nextest gate remains separate |
| 2026-10-25 | Integration candidate | Native Linux preparation | AWS SSO expired before host launch; no native gate or measurement claimed |

The query failed/passed runs, stack/FD captures and separate primitive probes
are indexed outside the repository by `stderr-ownership-index.json`, SHA-256
`c523f56d3b1ca0e6a11fc8389e4bff7dd89f2ddc9461106b3e02949bb6876d5f`.
The shadow mutant stock-output SHA-256 is
`1efa76cd7cae16a12bf0bc19bed306fb97d80c1853e612381b04013780d24c72`;
it checks the altered proof view while native Rust violates strict order.
The final runner refuses that differing binding environment.
The isolated input diagnostic stdout SHA-256 is
`cdb9ac13efe690f436a11305b3339827f33b41c252b9fdbe8582336ff333a6d7`;
the focused global-wrapper refusal stdout SHA-256 is
`b69f7c599e619f477994a88710ee9b1214c24025bdc8425b80ef576da7438c1a`.
The earlier broad diagnostics have tool output only; no raw-file digest is
claimed for them. Final proof receipt digests belong with the frozen-source gate.
The controlled stock `--no-verify` observation is documented separately in
[VI-0035](../verus-issues/VI-0035-no-verify-success.md); intended compile-only
behavior is not classified as a verifier translation defect.

The original [model qualification](evidence-acceptance-models.md) remains
historical evidence for its stated commits, not qualification of this changed
runtime. Checked source bodies, native adapter tests and the still-unproved
service-wide theorem remain distinct.
