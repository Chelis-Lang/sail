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
