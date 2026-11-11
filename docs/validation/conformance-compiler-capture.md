# Cargo conformance compiler capture

| Field | Current state |
|---|---|
| Tracking | #632, #716; parallel scaling remains #629 |
| Design | [ADR 0113](../decisions/0113-conformance-invocation-capture.md) |
| Runtime | Existing primary stock and fork pins; no runtime/solver changes |
| Implementation | Per-invocation capture in plain, shim/cache and engine arms; explicit project and nonempty selection guards |
| Status | Implemented, qualification pending; no parallel conformance or scaling acceptance claim |

Focused native Mac development checks pass 50 library tests, five harness
command tests and three actual delegate-process controls, plus clippy for
`sail-conformance` and `sail-verus` with all targets. The process controls
check original stdout/stderr/status, a killed delegate's incomplete marker,
corrupt recorded lengths and a closed forwarding pipe. The exact candidate
identity, complete Linux/Mac gates, raw evidence hashes and real-project /
parallel-workspace results will be recorded before merge. An actual Vest
harness run is in progress; no result is claimed here.

The original strict VI-0039 failures remain in the
[reproduction](../../corpus/repros/verus-parallel-json/README.md). This
implementation does not repair Cargo's public stdout framing. A line torn
within a write, a missing/extra invocation, any unmatched probe, incomplete
capture or exhausted interleaving search remains a failure. Original
aggregate streams and every compiler record are retained. This gate is
separate from broad-corpus and scaling performance qualification.

The consumer verification skill has no command or default-verification change;
this is contributor tooling. Documentation changes are ADR 0113, the error
reference, testing guide, changelog and this qualification record.
