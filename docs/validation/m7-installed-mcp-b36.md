# Installed MCP live-transport smoke at b36acc7

The unchanged installed-service smoke passed on independently frozen Sail and
`sail-z3` from source `b36acc7ed234e3169f1b546d48658b2c5c99b75e`.
The [portable record](m7-installed-mcp-b36.json) preserves source/archive/binary
hashes, original controller identity, actual checks, raw stdout/stderr/message
seals, and the separate three controller shutdown controls.

The Linux amd64 Docker run was emulated on macOS arm64 with shared development
load. It exited zero in 2.70 seconds; actual MCP EOF shutdown exited zero and
cancelled-read socket cleanup took 11.32 ms. All five original transport checks
passed: direct reads without the executable or CLI children, fresh same-ID
Running-to-Cancelled replies, refusal of retained data after service loss, live
reads after actual service restart, and suppression of a late cancelled reply.
The RPC five-second, command thirty-second, cancellation one-second and normal
EOF five-second bounds were unchanged.

The private ordinary-Rust fixture and worker are real processes. The worker was
stopped and cancelled before proving anything. This is installed transport and
lifecycle evidence, not a Current proof, large-result latency census, source
allocation qualification, full gate or edit-loop performance acceptance. Older
passes, missing-program failures and prepared-reader failures remain separate.
All owned actors were cleaned up; no service was retained for another test.

See [the client contract](../result-client.md), [MCP](../mcp.md),
[known issues](../known-issues.md) and [the roadmap](../ROADMAP.md) for the
remaining independent acceptance gates. The original real-reader controller
still requires genuine Current full proofs, an independently live selected-crate
verifier, both unchanged four-reader/200-sample collectors below 200 ms p95,
and the retained CLI/MCP scenario; this smoke does not waive those requirements.
