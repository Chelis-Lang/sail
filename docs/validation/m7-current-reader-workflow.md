# Original current-product reader workflow

The one reviewed attempt **failed the original Python full-Result latency
check**. It completed all 200 samples with the original admission boolean true
and the same selected-crate verifier spanning each RPC. Status p95 was
148.430 ms; Result p95 was 394.713 ms against the unchanged requirement of
strictly less than 200 ms. The original failfast controller did not attempt the
Rust census or retained CLI/MCP workflow. No second attempt was made.

The [terminal record](m7-current-reader-workflow.json) preserves every original
sample, both operation summaries, full proof-phase summaries and cleanup seals.
The [prepared record](m7-current-reader-workflow-prepared.json),
[reviewed plan](m7-reader-current-original-plan.md) and
[external ownership wrapper](m7-current-reader-workflow.py) bind the exact
current product, original controllers, selected b346 runtime, new source-only
template and unchanged collectors. No previous target, cache, receipt or digest
owner was copied. The frozen source is `4472279`; this is Linux amd64 Docker
emulation on arm64 macOS, with concurrent development recorded without assigning
it a causal role.

## Genuine preparation and original bounded checks

The separately authorized unmeasured preparation completed in both independent
workspaces. It cost 413.612 seconds for baseline and 466.987 seconds for active,
880.599 seconds total. Each ordinary and exact repair preparation returned a
Current, successful full report: vstd 1,871 functions and HumanEval 331 functions,
all verified, with no failed, unknown, provisional or unstable full verdicts.
Each cold report recorded two actual invocations and no reuse. Declaration
establishment notes were retained, and no declaration retry was needed.

The subsequent checks retained the original 600-second bounds:

| Bounded check | Inclusive seconds | Result |
|---|---:|---|
| Baseline ordinary full | 64.069 | Current, full success |
| Baseline exact repair | 87.958 | Current, accepted, full success |
| Active ordinary full | 51.077 | Current, full success |
| Active exact repair | 84.788 | Current, accepted, full success |

These bounded reports had no notes, one actual verifier invocation and one
legitimately reused vstd target from this same fresh run. Both complete reports
remain present. These are setup checks, not edit-loop performance samples or a
qualification of the selected source engine's physical allocation budget.

## Original live census and cleanup

The Python collector issued 100 Status and 100 Result RPCs through four readers.
Every sample retained the same non-stopped, non-zombie `human_eval_verus` verifier
PID, parent and birth ticks before and after its RPC. The exact original combined
admission predicate was true for all 200 samples; individual raw currentness
labels were not separately retained by that collector.

| Operation | Median ms | p95 ms | Maximum ms | Body bytes |
|---|---:|---:|---:|---:|
| Status | 82.500 | 148.430 | 255.044 | 452 |
| Result | 249.417 | 394.713 | 532.663 | 4,312,245 |

The five-second RPC bounds, four-reader workload, 200 samples, complete result
graph and original collector source were unchanged. Full Result timing includes
fresh RPC transfer and decode; this run does not separate those costs or prove a
specific cause. Status p95 passes; Result p95 fails. The previous failed
collectors and offline profiles remain separate observations.

The workflow closed in 1,218.356 seconds externally, including preparation and
cleanup; the wrapper recorded 1,218.095 seconds internally. It archived and
cancelled its independent active job, then the owned daemon exited normally with
code 0 in 0.122 seconds. Cleanup, archival and finalization errors were empty.
All immutable product, selected-artifact, template and controller seals matched
afterward. A bounded check of exact tracked actor identities found no surviving
non-zombie actor; the captured verifier and its parent remained orphan zombies,
which is recorded explicitly rather than described as complete PID disappearance.

The private archive contains 147 regular files, 149,438,353 raw bytes, with final
closed-worker streams separate from the original controller's before-cancel
copies. Its 4,794,530-byte compressed SHA-256 is
`8b9338eac63cd93e8bda4edffcee1ac05ab475390ef895839caca29ab6f1469e`.
All archived file hashes and the compressed hash were verified after host copy.
Verification workspaces, targets, caches and environment-bearing job records
are excluded. The initial host archive utility lacked `hashlib.file_digest`;
bounded streaming verification corrected that utility failure without changing
any evidence or workflow.

This establishes successful setup and Current admission under the original live
Python workload, with an unmet full-result latency gate. It does not establish
Rust reader performance, retained CLI/MCP acceptance, source physical-budget
qualification, 2× repair performance, full conformance or overall M7 acceptance.
See [reader acceptance](../reader-acceptance.md),
[freshness](../freshness.md), [the repair guide](../proof-repair.md) and
[the roadmap](../ROADMAP.md).
