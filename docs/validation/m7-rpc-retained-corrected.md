# Corrected retained CLI/MCP functional pass

The one separately labelled corrected-harness functional run passed all original
120 s whole, 30 s CLI and 5 s MCP exchange limits against the genuine baseline
from the [failed reader workflow](m7-rpc-stage-workflow.md). Its complete
[portable record](m7-rpc-retained-corrected.json) preserves exact product/runtime,
corrected and original harness hashes, every operation, public Current admission,
owned actor closure and unchanged immutable postseals.

Only the reviewed wrong-stream assertion changed: it now requires the existing
explicit `SAIL0902` MCP refusal instead of a non-error gap. The
[original wrong-stream failure](m7-rpc-retained-functional.md) remains archived.
The corrected harness SHA-256 is
`207b45ef034b6c1e947dbb0efdb87368b62d5ef8a4c42ab5768413670f775a11`.
Reconnect, gaps, duplicate cursors, complete reports, exact errors and deadlines
were preserved. No reader census or preparation was repeated for this scope.

After normal service restart, independent background projection establishment
admitted the same publicly Current completed baseline in 11.919591 s. Its full
Result retained the exact input revision and 4,313,813-byte body; no copied
receipt, digest owner or prior Current classification supplied authority. The
separately bounded readiness observation issued no verification or Prepare.

The corrected harness then passed in 5.659898 s: complete actual CLI/MCP evidence,
immutable baseline save/self-comparison/overwrite refusal, exact missing job and
function errors, explicit wrong-stream refusal, cursor reconnect and duplicate
delivery, MCP comparison, and one explicitly owned forced submission followed by
real worker cancellation. Cancellation did not claim proof success or stop a
pre-existing job. The wrapper took 18.077582 s inclusive. Its owned daemon exited
normally with code 0 in 0.041473 s and all cleanup errors were empty.

This establishes functional coverage of exact tested product `0f31c1d` with the
selected b346 source runtime. It is separate from the failed Python reader
latency test: Result p95 remains 403.341333 ms, and its fail-fast Rust collector
was never attempted. No source physical-budget, 2x performance, full-corpus
conformance or overall M7 qualification follows from this pass. See
[reader scope](../reader-acceptance.md), [the repair guide](../proof-repair.md),
[known issues](../known-issues.md) and [the roadmap](../ROADMAP.md).
