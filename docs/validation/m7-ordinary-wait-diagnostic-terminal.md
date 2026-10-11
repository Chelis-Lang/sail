# Focused ordinary verification completed; historical wait remains unresolved

The one [frozen focused diagnostic](m7-ordinary-wait-diagnostic-freeze.json)
completed successfully under the original 600-second bound. It used the
preserved baseline and ordinary context, product 46, selected b346 runtime and
`SAIL_MANAGED_FORCE=1`, with no new repair preparation, manual Prepare, reader
census or retry. The [terminal record](m7-ordinary-wait-diagnostic-terminal.json)
binds the full result, actual actors, raw archive and unchanged post-run seals.

External launch-to-closure time was 87.054 seconds; the wrapper's inclusive time
was 84.987 seconds and the product reported 82.051 seconds of execution. These
are different measurement boundaries. The report was Current, with exit code 0,
no error and no notes. The ordinary context and input revision exactly matched
the frozen historical values. One HumanEval verifier invocation ran; vstd was
reused through the normal product receipt path. No receipt was copied into the
context by the diagnostic.

Verus reported 2,060 verified vstd obligations and 441 HumanEval obligations,
with zero errors. The complete public function summaries contain 1,871 vstd
functions and 331 HumanEval functions, all verified. These are distinct counting
units, not evidence of missing functions or a changed target. The full report
remains retained, including both complete target graphs.

The persisted `run-sidecars/result.json` is retained verbatim at 2,158,274 bytes.
Raw CLI `stdout` is independently retained at 2,158,275 bytes, including its
trailing LF. The JSON record names both archive members and seals; neither file
was normalized to derive its seal.

## What the observations establish

The three snapshots occurred at context-ready (21.716 seconds), target-started
(29.342 seconds) and the 60-second boundary (60.020 seconds). The later two
scheduled snapshots were not due before completion. All three were explicitly
limited: two lease helpers reached the existing 64-FD observation cap. The
observer retained the original five-snapshot, 0.5-second, 1 MiB, 128-process and
128-task bounds. No process environment or stream was consumed.

The 60-second snapshot captured the birth-bound `rust_verify` and five shim
children. The verifier was sleeping in `futex_do_wait`; three shims were sleeping
in `anon_pipe_read` and two were running. These are instantaneous states during a
subsequently successful proof, not deadlock evidence. With the actual clock tick
rate of 100, the verifier had accumulated 30.94 seconds of user CPU and 6.82
seconds of system CPU. The observations cannot certify unobserved descendants
absent. Guest `/proc/<pid>/exe` links point to Rosetta; executable file seals,
not those links, bind the application bytes.

A bounded post-closure audit examined the actual invocation's `sidecars`
directories: 228 log entries and 113 completed shim provenance entries, within
256 entries per directory. Reading 36,272,964 bytes stayed within the 40 MiB
content cap. It found 2,205 QUERY and 2,205 RESPONSE blocks. QUERY is logged before
request enqueue and establishes intent only. RESPONSE is logged after delivery
of the reader's DONE result. All 113 provenance entries report
`solver_started=false`, with 4,410 replies served. Thus this successful invocation
did exercise the daemon-cached path; the earlier forwarding-only component did
not. An initial audit of absent direct-run log directories is also preserved;
it is not evidence that invocation-sidecar logs were absent.

The owned daemon closed normally, and original cleanup reported no errors.
The subsequent read-only closure scan found both recorded leaders absent and
no matching group members among 512 examined process entries. It reached its
entry cap, so it does not certify every escaped, reparented or previously
uncaptured descendant absent. No discovered process was signalled.

## Scope and remaining issue

The [original reader prerequisite failure](m7-rpc-binding-reader-terminal.md)
remains a failed 600-second invocation. This focused invocation did not reproduce
that wait and cannot explain it retrospectively. It supplies no reader latency,
M7 qualification, source-candidate or differential correctness credit. There was
no second reader workflow, census or retained CLI/MCP run.

The local raw archive is sealed at 1,413,100 bytes with SHA-256
`4fc764ee73cce88cbbe038e2020bf2fec1cb7cbf427afee1a14b9fd3aa3e6db8`.
It retains private configuration locally; this public record exposes only its
hash, not environment values. See the [reader guide](../reader-acceptance.md),
[known issues](../known-issues.md) and [roadmap](../ROADMAP.md) for the broader
acceptance state.
