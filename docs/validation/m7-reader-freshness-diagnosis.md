# Reader freshness diagnosis

The a6/b346 reader census established genuine complete Current reports, then
failed the all-Current admission requirement and full-result latency gate.
`m7-a6-b346-reader-census-failure.json` retains all 200 original samples. Their
admission booleans do not identify failed response labels: two early failures
and a continuous final failure tail are evidence to investigate, not proof of
an Unknown classification or its cause.

For local diagnosis, `SAIL_FRESHNESS_TRACE=1` enables static stage/reason labels
and monotonic elapsed microseconds on daemon stderr. It records namespace
barriers, runtime identity, kernel lease validation, cohort waiting and
publication. It emits no paths, environment values, report bodies or tokens.
The variable is read once per process. Tracing does not authorize any cached
result, alter freshness checks or extend deadlines. Its synchronous output can
change timing, so traced measurements are diagnostic and cannot qualify the
200 ms reader gate.

A diagnostic process using a copied completed job must establish its own native
observer and continuous kernel lease generation before retaining digests. An
old report or previously successful read cannot transfer that generation to a
new process. All establishment costs and any refusals remain part of the
record; this procedure performs no new proof and makes no new performance
acceptance claim.

The separate retained-job trace established new continuous coverage in 24.705
seconds and observed 187 Current and 13 Unknown reads. Every refusal in that
trace was a bounded validation-owner wait; it observed no namespace, lease or
runtime identity loss after establishment. This does not establish the cause
of the original live run's sustained failure tail. Full response transfer and
decode still failed the latency target. The complete classified diagnostic and
stage timing scope are in `m7-retained-proof-freshness-profile.json`.

A reader's overlap boundary now starts at `FreshnessIndex::inspect` entry,
before the first namespace barrier. A reader queued at that barrier can use a
successful checkpoint certified during its own call. A sequential reader cannot
inherit an older checkpoint. Both caller namespace barriers, the exact owner
generation and pointer, final runtime identity and nonrenewable lease validation
remain required. Publication time cannot substitute for the recorded validation
checkpoint. The existing 100 ms owner-wait budget still starts after the first
namespace barrier; neither the 5 s RPC deadline nor the 200 ms acceptance gate
changes.

The delayed-barrier regression covers a valid overlapping checkpoint, a
sequential old checkpoint, and real content writes with restored modification
times before and after validation. The former late-publication/mmap-break and
concurrent restored-write tests remain required. This change is a candidate
optimization until real workload validation demonstrates its latency and
admission behavior.
