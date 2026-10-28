# Cache ownership core qualification

Status: the staged library passes its scoped native Linux qualification.
Production cache adoption remains separate. The delivery stacks the audited
35-case authority-model baseline and preserves its observed production
174-obligation/303-function inventory. PR review, current-main reconciliation
and hosted required checks remain merge requirements.

| Gate | Current evidence | Remaining requirement |
|---|---|---|
| Source/capture | All 53 source tests pass on the Mac and native Linux, retaining the former 45 plus eight private-owner/macro regressions; all 132 cache descriptors and seven module envelopes are bound | Keep exact source/API/control inventories on integration |
| Direct production refinements | Frozen `c0957b92`: 160 full-root calls, six positives and 154 distinct ordinary negatives; 157 unique cases. Four disjoint query partitions cover all 139 query negatives; complete inputs/authority cover four/eleven. Whole positives are query 282/464, inputs 54/91, authority 174/303 obligations/functions | No partition alone or single unpartitioned 157-case run is claimed |
| Models | All 35 cases pass: four positives and 31 ordinary negatives | Retain the independently delivered model scope |
| Native fast tier | Strict workspace gate, 1,161/1,161 nextest tests, examples/doctests/rustdoc/layering/deny; 51/51 solver tests; full E2E including extra, managed copies/native/scripts/MCP and repair; PR conformance passes | Hosted checks and exact-head review remain |
| Product adoption | No native cache, daemon, disk or solver-path implementation changes | Separate PR with caller/lock refinement, native/solver/E2E, applicable conformance and measured carrier/cost behavior before representation selection |

The four PR-conformance arms each report 33 same units and 311 function
comparisons, zero unstable/different/infrastructure failures and no skipped
projects. Each records five units with rlimit drift; the engine arm also checks
32 focused units. These are conformance results, not performance measurements.
Full fast/standard corpus acceptance is not required for this unselected library
cut under [ADR 0087](../decisions/0087-scoped-merge-conformance.md).

Qualification uses native x86-64 Linux on c7i.4xlarge in us-east-1, the current
`c4710b7fd3ca282a` CI inputs, Rust 1.99.0 and the pinned stock Verus
`0.2026.10.18.7d82994` with its Rust 1.99.1. Each refinement call retains the
full function/VC inventory, default 120-second/one-thread settings and
`--no-cheating`, with final source/runtime/archive/Rust guards. The native gate
uses actual `ci/local.sh`, its checkout-specific target volume and no global
`CARGO_TARGET_DIR`. Core image ID starts `5a6b6db857d0`; MCP starts
`9dd02bdf2422`, with the complete core layer prefix preserved.

Closed artifact:
`experiments/2026-10-27/acceptance-cache-core-c0957b92-native.tgz`,
39,507,851 bytes, SHA-256
`bc75e9542cbb38db0dfb64ee0577213b83d9b5cd1b861a60e66936df0608f26b`.
PUT/GET and exact readback pass. The lead independently checks all 4,238 indexed
files, reconstructs all 160 production candidates/drivers and 35 model calls,
and audits raw whole-root function/count/resource results plus native receipts.
The archive contains full source bundles, compiled inputs, exact commands and
raw reports. The successful ordinary temporary contract-project directories
are not retained after the normal E2E container exits; its complete log remains.

| Date | Source | Attempt | Result |
|---|---|---|---|
| 2026-10-27 | Uncommitted extraction | Initial source suite | 43/45 pass; the same-body native-macro selector needed its Copy-bound spelling, and a final-build-script fixture initially saw only committed files. Indexed observation only; raw local output was not separately retained; zero proof/control credit |
| 2026-10-27 | `16afedff` | First whole discovery | E0433 for missing cache_bucket/cache_counted/cache_slots imports in relocated witnesses; zero verified VCs and zero credit; later roots and original final runtime guard unrun. Raw commands/compiled files remain; a separate final source audit supplies no runtime credit |
| 2026-10-27 | `978096fe` | Corrected whole discovery | Import-only correction; all three complete roots pass with actual query 282/464, inputs 54/91 and authority 174/303 inventories; all prior main and retained cache functions remain; final identities pass |
| 2026-10-27 | `c0957b92` | First native aggregate | Workspace/solver/models pass. Nondefault contract evidence path beneath the checkout triggers the fixture's correct SAIL3140 Cargo-configuration refusal; full E2E fails and PR conformance is unrun. This aggregate remains incomplete |
| 2026-10-27 | `c0957b92` | Normal-path continuation | Only unfinished full E2E and PR conformance run, both pass; normal `ci/local.sh` temporary contract path restored. Earlier completed gates are reused by exact source and target identity; the first aggregate is not renamed PASS |

The inventory lock retains the former query function/count set as required
minima, then records the actually observed complete enlarged root. No count is
inferred from the earlier, larger input context. Restacking the existing feature
commits onto the current model delivery changes parent policy/documentation;
compiled Rust, proof inputs, checker and manifest bytes remain identical to
frozen `c0957b92`. The current parent also inherits the three CI failure-evidence
observer scripts from merged PR 545; that change has its own parent review/CI
receipt. All other 736 tracked non-documentation files remain byte-identical;
the older native receipt does not claim to execute the newer observer scripts.

Historical indexed evidence remains separate: source
`f14ca0117188206112694331288315f5a3e8c227` has four 225/0 positives and 72
ordinary 224/1 negatives, all 355 actual functions/default resources. Its
110 selected native tests and eleven separately compiled native assertion
faults are not introduced by this extraction or substituted for this delivery.
Artifact `experiments/2026-10-27/acceptance-cache-indexed-f14ca011-native.tgz`
contains 29,488,376 bytes, SHA-256
`877dc31e4db5b22a485cf5a98291d19989d575994ccca4af257bd8b8e1c6ab72`.

The later Resident API remains excluded. No cache-origin, physical
allocation/RSS, Mutex/global-owner, full-service or performance acceptance is
established by this staged core.
