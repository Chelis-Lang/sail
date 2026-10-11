# Shared lookup audit after the original solver timeout

The combined `4472279` solver run timed out
`gc_and_clear_during_sessions_only_cost_misses` at its original 180-second
nextest limit (180.036 seconds reported). Its captured output contains no
intermediate phase or panic. This failure remains separate from the passing
workspace gate and the shared trust fix's focused controls.

The [audit record](m7-shared-solver-timeout-447-readonly.json) binds the exact
source objects and tested executable. The test PID was discovered while running,
but it disappeared before the bounded metadata snapshot read `/proc`. No
starttime, thread wait, descendant identity or pipe occupancy was captured;
the retained `ENOENT` record is not live evidence of a cause. The raw log copy
is an audit-time excerpt, not the complete solver suite's terminal record.

There is a concrete change in default unconfigured work: the daemon retains
its automatic shared tier, and a local/disk miss now admits a pending lookup
and starts a helper before checking current optional configuration. Previously
an absent configuration skipped before those operations. The helper checks
current presence, returns `Skipped` when absent, and performs no trust read or
shared transport in that case. This introduces a helper and response wait per
miss; it does not establish the timeout's cause.

The inspected foreground lookup waits use the same request-entry 250-ms
budget. Publication and retirement use `try_lock`; full reply cloning occurs
outside the pending mutex and is followed by a deadline check. No concrete
unbounded shared-get lock or deadlock was found. OS scheduling, thread creation
and condition-variable reacquisition are not hard real-time guarantees.

The unchanged fixture has other phase possibilities: its direct-Z3 baseline
uses `wait_with_output`; shim timeout kills and waits for the shim before
joining its reader; maintenance is stopped only after both concurrent rounds
return. These are static possibilities, not evidence that any was reached or
responsible. A phase observation would be needed to distinguish them. The cause
remains unknown. This audit launched no retry, build, proof, debugger or pipe
read, and changed no assertion, deadline or production code.
