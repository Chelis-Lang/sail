# Optional retained-content publication declines

Status: native Linux gate/integration and bounded independent review pass at
`0ed29577`. Current-main rebase `4be19f53` has identical selected runtime bodies;
its workspace/solver gate also passes. Tracks #712 C/#595 and ADR0139.

Publication returns sixteen fixed typed stages while preserving admission,
I/O order and continue/abort policy. Counters bound the explanation inventory;
there is no operation journal. SAIL3146 notes reach returned CLI/JSON/MCP and
unchanged-call replay. Human stderr shows them only with SAIL_EXPLAIN=1.
Verification, receipts, currentness, exits, keys and ordinary fallback stay.

The internal managed `result.json` snapshot precedes committed receipts and
optional publication, so it does not contain late publication notes. Supported
CLI, jobs/MCP and replay use the final returned result or sealed stdout. That
internal snapshot remains unchanged; publication stays after Current/commit.

## Actual qualification

Native c7i.4xlarge/us-east-2, Rust1.99.0, verifier1.99.1, production core
`6c19f5c8d397b61c-core`. Full source `0ed29577dca8de0f49caa143652971b764397134`:

| Scope | Actual result |
|---|---|
| Complete workspace gate | 1,405 passed / 57 declared skips; fmt, Clippy, doctests, rustdoc, layering, resources, dependency policy and generated-page checks pass |
| Solver tier at `0ed29577` | 53 passed |
| Current-main `4be19f53` workspace / solver | 1,410 workspace passed / 58 declared skips; 54 solver passed |
| Full local E2E | Pass, including Extra, all managed cases, retained reuse, native replacement and repair CLI/MCP |
| Four PR conformance arms | Each 33 Same / 311 functions; zero different/unstable/infra/skipped; seven resource-drift units retained |
| Independent actual retention group | 52 passed, zero failed/ignored, including all eight tests in the archive group |
| Independent actual archive differential | Nine cases per original/candidate arm; identical outcomes and complete resulting file trees, with actual typed stages checked |
| Independent exact-method controls | Nine Plan publication traces; 24 default plus 24 explained note-branch combinations; all sixteen stages, aggregation and counter saturation |

The archive differential covers success, identity, compiler-read/header, original
output, budget/symlink, blob/target directory and entry-write cases. Old outer
abort and per-entry continue remain. Rare encoding/overflow/eviction-race
branches are inspected rather than forced in the filesystem differential; the
counter test exercises every enum stage. Controlled branch collaborators do
not qualify fresh compiler eligibility or real-project fault injection.

The lead independently rehashes all 878 full-gate payloads; archive SHA-256
`36144f8084e5978d7f5140cc6862f9319285283f90e880290eba92952602feb6`.
Both independent native review archives are fully verified, including every
regular file, directory mode and symlink metadata without following links:

| Review archive | Exact SHA-256 / checked entries |
|---|---|
| Final native originals | `9620b4e42f0a98c162148ea289f8e78195a7b90022e7c92f94e4f35e5712a6f6`; 1,844 entries / 1,521 regular files |
| Original collector-warning originals | `5337d932aa5443708e3eddb69a299182d80b135750250dcab76ee333900ace8f`; 1,839 entries / 1,516 regular files |

All 183 local review payloads are also independently rehashed and externally
retained. The reviewer approves the bounded diagnostic change, separately from
the lead's full merge qualification. Native review image is
`sha256:c6a93d780290e09c4d049f44ab272ddd147e5aa097c4cbf47ff02458875be7fe`.

## Current-main source reuse and limits

The rebase adds #744's diagnostic prefix module outside selected verification
paths. All sail-verus, API, CLI, daemon, MCP, solver sources and dependency/runtime
pins remain byte-identical to `0ed29577`. Its full E2E/PR evidence above is reused
for those identical behaviors. The current source's workspace/solver gate completes separately as recorded
above. All six required hosted checks remain before merge; no unrun combined
E2E/PR or long corpus is labelled passed.

ADR0087 diagnostic/refactor classification follows independent executable
comparison of admission predicates, operation order and resulting artifacts.
No model/refinement body, selected runtime, dependency graph or platform-specific
interface changes. No long corpus, benchmark, formal, Mac, complete recovery or
release acceptance is claimed. Missing-export explanation is already in #708;
recovery, typed restore reasons and cost-policy decisions remain open.

## Retained failures

All attempts are 2026-11-14; no check or baseline is weakened.

| Source / attempt | Result and established cause |
|---|---|
| `2f24d6e7`, independent review | Reject: new test assumes blobs absent although its fixture creates an empty directory. Original assertion independently fails 101 on the compilable successor scaffold; this is not a full original-build claim. |
| `c8306494`, native1 | Compile E0596: late notes require a mutable result binding. Successor changes only that binding and failure record. |
| `0ed29577`, native2 | Invalid quick-check wrapper: target defaults inside a read-only source mount; gate not reached. Explicit own target fixes wrapper; native3 passes. |
| Independent reviewer followup/collection | Host could not overwrite Docker-owned disposable files; collector observed its own changing directory; retry lacked executable permission. Corrected inside-image followup and excluded-output-directory collection pass, with all original logs retained. |

Documentation: ADR0139, ADR0136 correction, this record, instant-revert guide,
errors, changelog and roadmap. Consumer skill commands and availability do not
change. Final qualification amendments change documentation only.
