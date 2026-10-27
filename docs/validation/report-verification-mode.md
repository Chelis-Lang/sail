# Report-mode construction qualification

Status: the selected report constructor passes its complete native qualification,
including full fast and standard conformance. The restacked combined source has
passed native coexistence, all three whole positive roots and its complete
26-case authority component. No service-wide or compiler-origin
theorem is claimed.

The adapter delegates reported-argument construction to the source-shared checked
body. Physical compiler arguments and environment are unchanged. This replaces a
native iterator/String-comparison loop with an indexed/byte-comparison loop, so
ADR 0087 requires runtime conformance despite intended identical behavior.
No corpus entry, check, scope, resource limit or baseline is weakened. The change
introduces no raw-metadata/DOM or inherited-trait macro implementation, provider
change, or verifier runtime pin change.

## Native qualification of the selected implementation

Initial frozen `f4cf66d0` passes all 46 source tests, strict API/Verus/CLI clippy,
both actual mode fixtures and three whole Stock positives in 116.524 seconds.
Observed obligations/function entries are query 134/263, inputs 54/91 and
authority 184/314; the manifest locks the actual authority inventory. Independent
review reconstructs all 24 compiled driver/dependency files from canonical Git
and audits the raw results. This focused run supplies discovery credit only.

The implementation at frozen `9cb1ada6` is byte-identical; its complete native
qualification passes on dedicated c7i.4xlarge, native Linux x86-64, us-east-1:

| Gate | Result |
|---|---|
| Workspace gate | 1,162 tests pass; 46 refinement source tests pass |
| Pinned-Z3 solver suite | 51 tests pass |
| Stock mathematical models | 35 cases: 4 positive, 31 ordinary negative controls |
| Stock production refinements | 39 cases: 3 positive, 36 ordinary negative controls |
| Full local E2E | Every phase, including local extras, passes |
| PR conformance | Each of four arms: 33 Same, 311 functions |
| Full fast conformance | Each of four arms: 147 Same, 3,451 functions |
| Full standard conformance | Each of four arms: 159 Same, 13,166 functions |
| Standard mutation controls | All 18 HumanEval mutations Same; all 21 comparisons Same |

Every conformance arm has zero different, unstable, infrastructure-error or
skipped projects. PR/fast/standard have respectively 5/11/17 units with recorded
resource-limit drift; this is not a verdict disagreement. Standard conformance
takes 4,757.650 seconds; the complete qualification takes 7,415.043 seconds,
including image construction. No performance claim is made.

The actual core/MCP image IDs are
`sha256:3b2dee0bce7c260371ede81012d2ad977f54842ebe5ea3dd83eda91fe178fb7b`
and `sha256:a2c9b523d49868813174461f6ffd2a4cbcff2cc01db437897424d948ded092dd`.
All 74 Stock calls use pinned stock Verus, whole-root scope, default resources,
a 120-second deadline, one thread and `--no-cheating`. Independent audit
reconstructs 519 compiled driver/dependency files and checks all raw documents,
including all 67 ordinary negative results. Final source/runtime/Rust/image and
physical-target guards pass. Both native runs use immutable source checkouts.

## Combined assembly after the cache and model deliveries

The combined candidate `fefe3c73`, based on merged main `e1dedb64`, retains all
139 query controls, four input controls and 25 authority controls. Its complete
registry contains 171 unique production cases: query 140, inputs five and
authority 26. All 35 mathematical-model cases remain mandatory. Fresh whole
positives observe query 282/464, inputs 54/91 and authority 184/314
obligations/function entries. Fresh combined native coexistence and 57 source
tests pass. All 26 complete authority cases pass, including all 25 ordinary
negative controls. The fresh combined qualifier takes 492.977 seconds; authority
controls take 269.021 seconds. Independent audit reconstructs and compares the
actual compiled files and raw results for all 29 fresh whole-root calls.

Canonical Git reconstruction compares every complete query/input program and
dependency byte with the actual compiled programs in the earlier `c0957b92`
cache qualification. All 145 unique cases match, and their raw whole-scope
results, including 143 ordinary negatives, are independently audited. All 35
model programs and recipes also match the actual original calls and raw results.
The original query positive was executed four times across control partitions;
those repeated historical calls do not increase the unique-case total. This is
exact compiled-input reuse under ADR 0087, not 171 newly executed Stock calls.
Fresh authority 26 supplies the changed compiled inputs.

The combined source adds unselected cache kernels and syntax support. The
selected report/native bodies, compiler configuration, harness, pins and corpus
are unchanged from `9cb1ada6`; the old macro token expansion is identical when
its new optional bound is absent. The full runtime/conformance evidence remains
at its original source. The separate fresh native gate establishes combined
compilation and coexistence; no identical-whole-binary claim is made.

## Retained evidence and limits

Raw archives remain outside Git under `experiments/2026-10-27/` or
`experiments/2026-10-28/` in native evidence storage; uploads have independent
SHA-256 read-back verification:

| Archive | Bytes | SHA-256 |
|---|---:|---|
| `acceptance-mode-f4cf66d0-initial-native.tgz` | 6,173,813 | `8c490d2c7f2909692ad09819e755999bb4f71a3cf548fb6b4641857ae978b320` |
| `acceptance-mode-9cb1ada6-full-native.tgz` | 53,962,828 | `e6bcde0d9684b1b9c2091c1ceaec5752cdbaa5069481b944b17905fb08917ee6` |
| `acceptance-mode-fefe3c73-combined-native.tgz` | 7,809,131 | `d8f262dc0351a66721944fff5fa17bdc3a7368df802c471de12f17691aa9c0b9` |
| `acceptance-cache-core-c0957b92-native.tgz` | 39,507,851 | `bc75e9542cbb38db0dfb64ee0577213b83d9b5cd1b861a60e66936df0608f26b` |

Attempt index: `e1dedb64` with unresolved local rebase, 2026-10-28: source
preflight refused the uncommitted mode file; no Stock or native execution and
no qualification credit. The conflict was resolved before the frozen combined
candidate was committed and qualified.

The packed-option decoder, original compiler environment/arguments and physical
launch/input completeness remain native/integration boundaries. The separate
main-relative compiler driver-argument reuse gap is tracked in
[#551](https://github.com/Chelis-Lang/sail/issues/551). The constructor theorem
does not close that gap or establish compiler-origin/service-wide correctness.
