# Independent publication timing source review

Exact candidate `95299fd` has no static review blocker. [The identity record](m7-publication-timing-independent-review.json)
verifies all four frozen source digests. No compiler, proof or diagnostic actor
ran in this review; new controls, strict checks and installed attribution remain
pending.

Worker order remains execute, optional freshness establishment, terminal
mutation/persistence, activity decrement and pruning. Cancellation's original
terminal-state check is unchanged. The child-exit marker follows only a real
successful `try_wait` observation; cancellation and early errors may leave
fields null. Worker `write_succeeded` means this worker persisted its terminal
record, including an InfrastructureError record. It never means verification
or Current succeeded. The user diagnostic documentation states that boundary.

The expanded recorder contains 22 fixed enum/numeric/boolean fields, with no
job IDs, paths, environment or report values. An independent maximum-width JSON
calculation produces 195,329 bytes for 256 rows, within the original 256 KiB
export limit plus the source test's 1 KiB header allowance. Actual serde and
record-layout controls remain uncompiled. Ring capacity refusal, try-lock
recording, shutdown-only export and default-off behavior are preserved. Failed
observations cannot replace original operation results or authorize anything.

Managed publication spans use the existing bounded sink and RAII timers around
unchanged result, receipt/final guarded commit and compiler-generation calls.
Original error propagation and publication order are preserved. There is no
new asynchronous fill, queue, service, runtime hash or authority shortcut.
These observations do not attribute the original 120-second failure or supply
performance acceptance. The separate
[optional-fill lifecycle review](m7-worker-freshness-publication-review.md)
continues to apply to any later scheduling proposal.
