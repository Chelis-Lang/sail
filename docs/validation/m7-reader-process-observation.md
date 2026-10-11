# Owned descendant observation across Linux tasks

The [failed native-binding workflow](m7-rpc-binding-reader-terminal.md) retained
only a main-task child traversal. Its six actors do not describe the complete
Cargo process tree. The omitted verifier/solver state cannot be recovered from
that record and remains unknown.

Future scoped observations reuse `ci/managed-evidence.py::owned_processes`,
which already reads children from every task beneath the owned root. It never
scans unrelated host processes. Each process has before/after birth and parent
checks; queued children now also retain the birth ticks observed at discovery.
A reused PID is omitted with an explicit partial-observation error. An external
observer can supply its previously recorded root birth ticks through
`expected_start_ticks`; a mismatch prevents traversal. These observations
never authorize process cleanup, proof currentness or a late result.

The real Linux regression starts an owned Python parent whose worker thread
launches a child. It proves that the main task's child list omits that child,
the worker task's list includes it, and the unchanged all-task traversal finds
it with stable birth/parent identity. The fixture closes its input, waits for
the child and joins the worker normally. Additional controls refuse a changed
root birth and a reused queued child. The [native record](m7-reader-process-observation.json)
binds the exact helper/test sources and retained output: all 12 controls pass
in 0.498 s test time, 0.721860 s externally. Host checks pass with the two Linux
controls skipped; that does not substitute for the actual Linux result.

The existing gate already runs these tests. Original managed verification,
reader, RPC and cleanup deadlines are unchanged. Observation retains at most
128 processes, 128 tasks per process, 64 descriptor targets per process and
1 MiB of reads, within the existing 0.5-second scheduling budget. Kernel reads
cannot be interrupted by an elapsed check; any observation can still be partial.
Only compiler-selector facts are retained from command lines, with no complete
argv, environment, pipe consumption or diagnostic signals. Missing processes,
task limits and identity loss are not evidence that an underlying stage had
finished or that scheduling caused a timeout.

No new proof or reader run was launched for these controls. A future stock-wait
diagnostic still needs a concrete mechanism and separately frozen owned inputs;
this observer improvement alone does not justify retrying the failed workflow.
See [reader acceptance](../reader-acceptance.md),
[the timeout evidence helper](../../ci/managed-evidence.py),
[known issues](../known-issues.md) and [the roadmap](../ROADMAP.md).
