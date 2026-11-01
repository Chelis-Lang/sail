# Background preparation

Implementation is in progress for the next release; native product qualification
is pending. Published 0.1.0 does not provide this behavior.

After a complete successful Cargo verification, Sail can prepare the next
unchanged invocation in its daemon while your command has already returned.
`sail new` can also start ordinary project verification after creating the project,
without waiting for Cargo or verification to finish. This uses the same pinned
runtime and proof checks as explicit verification.

Ordinary verification, watch, repair and jobs take priority. An immediate call
cancels unfinished preparation and verifies normally; it does not wait for a
prepared result. Later reuse still checks current inputs and keeps the original
proof's establishing run. Edits invalidate old preparation. Background progress
is never itself a verified result.

Preparation is automatic when the matching daemon supports it. To disable it:

```sh
SAIL_BACKGROUND_PREPARE=0 sail verify
SAIL_BACKGROUND_PREPARE=0 sail new demo
```

Inspect `sail status --json`'s `preparation` field for queued/running work and
completed-record, superseded and failure counts. These are lifetime activity
counters, not evidence that your current source is proved. The regular command's
diagnostics, exit status and JSON report remain authoritative.

Preparation uses at most one additional worker. Requests are bounded and newer
workspace requests replace older ones. Stopping the daemon cancels its work;
unfinished work is not resumed on restart. An unavailable or older daemon simply
leaves ordinary verification available. Explicitly selected unavailable services
are not replaced by project creation.

This uses idle CPU and disk after the command returns. Whether it reduces an
immediate call, and its total CPU/memory cost, are separate measurements. It does
not enable unselected digest optimizations or change focused verification scope.
