# Proof health

Available in v0.3.0 development builds; absent from the published 0.2.0 release.

`sail health` reads observed solver effort from a completed verification job
or a saved `sail verify --output json` document. It runs no verification and
does not change verdicts, evidence tiers or provisional establishment.

```sh
sail verify --background --output json
sail health JOB
sail health JOB --output json
sail verify --output json > result.json
sail health --file result.json --limit 50
```

Without an explicit job or file, `sail health` selects the newest completed
job submitted for the current directory. A running, cancelled or unavailable
job has no completed health view; inspect it with `sail jobs status JOB`.
Setup and read errors use `SAIL0906` and exit 2. A written view exits 0 even
when the original verification failed.

The view includes requested targets, keeping each function's package identity,
Cargo target name and kinds. A same-named library and binary remain separate.
Dependency-only targets are outside this view. The default is 20 functions;
`--limit N` accepts 1–1000. `total_functions` and `omitted` state the complete
requested inventory and the number outside the displayed bound.

Entries rank resource exhaustion first, then the number of checks near their
limits, then the highest exact finite consumption fraction. Ties use the
complete target/function identity. The comparison uses integer arithmetic,
including at the u64 boundary.

| Measurement | Meaning |
|---|---|
| `attributed` | An obligation inventory was recorded; false differs from an explicitly empty inventory. |
| `complete` | Every recorded check has an answer, answer source, requested limit and actual effort. It is not a proof-success or repeated-run stability claim. |
| `checks` | Original checks, including advisory ones. |
| `measured_checks` | Checks with actual effort and a finite positive limit. |
| `near_limit_checks` | Effort exceeds half the check's requested finite limit, using ADR 0009's strict threshold. Exactly half is not near the limit. |
| `exhausted_checks` | The original reason-unknown names resource exhaustion. |
| `unlimited_checks` | The requested rlimit was explicitly zero; no finite fraction is invented. |
| `unrecorded_checks` | At least one of answer, source, limit or effort is absent. |
| `peak` | Highest measured `{used, limit}` pair, or null without a finite measurement. Whole-function timing never substitutes for check effort. |

Function verdicts, establishment, provenance and any explicitly measured
`fragility` remain as recorded. A single run does not establish stability.
Entries retain the target's original `establishing_run` and `reused` flag;
the view also records the original project exit status. Reused target effort
belongs to that establishing run rather than new solver work in this job.
No fragility score is inferred when replay was not measured. Missing
attribution remains unavailable rather than becoming zero effort. Cached
answers retain their original solver effort and cache provenance; they do not
describe new solver work in the current invocation.

Each new schema-v2 function report may include the same `health` measurements.
The command recomputes its view from original checks, so older v2 reports
without that optional field remain usable. Health adds no answer attribution
that was missing from the original report.

## Compare retained evidence

```sh
sail baseline save before-edit --job BEFORE_JOB
sail health AFTER_JOB --baseline before-edit --output json
sail health AFTER_JOB --before BEFORE_JOB
```

The reference is shown separately with its own job/run, input revision,
configuration, verdicts and coverage. Immutable baselines stay historical.
Malformed named baselines claiming Current or Unknown are refused at both
the CLI and MCP response boundaries; a selected reference job keeps its own
service-reported currentness.
Different contexts or projects do not acquire shared proof authority by being
displayed together. These commands do not rerun an earlier proof or calculate
a synthetic combined fragility score.

For a result file, `currentness` is the value recorded in that file. Reading a
file does not inspect today's workspace. Retained job views also respect a
service's historical or unknown currentness; they never upgrade it.

## MCP

`proof_health` returns the same `sail-proof-health`, version-one JSON view:

```json
{"job_id":"JOB", "limit":20, "baseline":"before-edit"}
```

Only `job_id` is required. An optional `before_job` replaces `baseline`; the
two cannot be combined. The tool is read-only and refuses malformed bounds,
unsupported results and mismatched response identities. Use `explain_failure`
or `verification_result` for the original diagnostics and obligation details.

Session retention, background stability measurements and quantifier profiling
are separate development work. This view uses evidence already retained by
verification jobs; it does not recover discarded solver sessions.
