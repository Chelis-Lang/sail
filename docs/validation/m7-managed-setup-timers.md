# Managed setup subphase timing controls

The [cold stage witness](m7-managed-cold-stage-777.md) observed 58.56 seconds
before Cargo started but did not distinguish the managed setup subcosts.
Optional setup timers now separate metadata, runtime identity, toolchain probes,
sysroot observation/hashing, source snapshots, coherent guards and invocation
preparation. The [cold-start guide](../managed-cold-start.md) documents output,
privacy, limits and interpretation. No verifier run has yet collected these
new timings, and no causal optimization or performance gain is claimed.

[The implementation record](m7-managed-setup-timers.json) binds source bytes and
validation scope. Six native macOS controls pass: fixed-field nested/error-exit
records, existing/missing destinations, no-follow creation and mode, row/byte
limits, nonwrapping file budget, and write failure preserving the original
error. Seven unchanged runner controls also pass, including corrupt engine
rejection before probing and lifecycle-only environment exclusion. Strict
all-targets Clippy and formatting pass. Initial lint/deprecation failures are
retained in the record and were corrected without exceptions.

The complete Linux gate and a real subphase timing run remain pending lead
integration. These host controls neither replace the failed 240-second cold
run nor qualify source-engine performance, conformance or live-reader latency.

The follow-up selector control treats unset and empty `SAIL_JOB_EVENTS` alike,
before file-slot allocation. It uses no process-environment mutation and prevents
a relative timing file from an empty destination. All seven diagnostic controls
and strict all-targets Clippy pass on the host after this change; the original
six-control implementation record remains preserved. No cold run was repeated.
