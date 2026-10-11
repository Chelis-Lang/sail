# Worker freshness fill and terminal publication review

This is a read-only design review of root source `69f9069`, with no implementation,
new actor or timing acceptance. The reported 3.17–3.47-second difference between
captured worker duration and runner `wall_ms` does not isolate freshness fill:
runner timing precedes final publication, and the daemon also waits for exit,
reads/parses output and persists status. B's separate bounded phase timings must
identify the cost before a scheduling change is selected. The original
120/30/5 fixture failure and all prior failures remain unchanged.

## Actual contract and fallback

`crates/sail-daemon/src/jobs/worker.rs` calls `execute`, then synchronously calls
`store::establish_freshness` before setting/persisting Completed. The returned
freshness snapshot is discarded. `execute` has already parsed and validated the
complete document and produced its exact stdout projection; the establishment
helper rereads and reparses that stdout to recover workspace/context fields.
Passing the already validated document by reference is a concrete duplicate-work
seam that can preserve the current publication order and all currentness rules.

The Linux independent Result path is `store::projected_currentness` through
`FreshnessIndex::inspect`. Missing or pending coverage returns no snapshot and
may start existing background revalidation. `store::classify` then returns
Unknown for an originally Current report. A changed complete revision becomes
Historical; only an independently complete, same-revision snapshot can preserve
Current. Output bytes still require their exact seal, and the guarded-output
shortcut independently brackets freshness. No optional fill authorizes result
bytes, proof receipts or Current by itself.

The repair CLI returns on the first terminal Result, recomputes acceptance and
does not wait for optional projection readiness. MCP likewise returns the first
complete result. Therefore moving fill after Completed can expose transient
Unknown and turn a successful foreground repair into a refusal. Returning the
recorded Current would violate the existing contract. Bare `refresh_context`
is not a replacement for Linux's observer/read-lease interval; it cannot silently
bypass those barriers.

## Smallest safe scheduling design, if measured useful

Publish the same validated document/projection and persist terminal status first,
then perform optional fill on the **same FIFO worker** outside the jobs mutex.
Retain its activity count until fill and cleanup close, so idle shutdown cannot
discard the owner; shutdown still joins that worker. Offer no fill when
cancellation/shutdown won the original terminal-state check or persistence
failed. Use the validated workspace/context plus the original immutable
submission environment, never the latest job or only a project-directory key.
Each read retains the unchanged revision comparison and every native/kernel
barrier. A late fill cannot rewrite any terminal status, recorded report,
cancellation decision or proof receipt.

This adds no thread, queue, retained graph or independently refundable owner.
It preserves the existing two validation permits and index policy. It improves
terminal visibility only: the same worker still cannot execute the next FIFO job
until fill returns. It is not a demonstrated solution to the complete fixture's
120-second bound, and early Unknown remains a user-visible availability risk.

A true asynchronous service needs separate review: one owned worker and fixed
slot, admission before request copies, a permit shared with existing fills,
stop-aware construction and joined shutdown. The existing detached revalidation
path has a two-operation limit, but no daemon-owned join/cancellation interface;
that alone does not establish a safe new shutdown or physical-memory argument.
An entry count is not a bound on evicted in-flight owners. No per-job detached
threads or newly copied complete reports should be introduced.

## Required sensitivity and safety controls

- Pause the real fill before native acquisition. The terminal record must become
  readable while it is held, with unchanged complete payload and exact seal.
  Cold/pending Current reads must be Unknown, never accepted. An old-order
  variant must fail the unchanged terminal-visibility assertion.
- Resume a real eligible fill and require fresh same-revision Current. While
  paused, mutate and restore input/runtime bytes or rebind a sidecar; no stale
  Current may appear. Reuse existing hardlink/mmap/owner-loss controls.
- Use equal workspace names with different context or environment and a changed
  revision. A late fill may neither certify another request nor reset a newer
  invalidated generation.
- Make cancellation win before persistence, inject persistence failure, and
  stop during held fill. No Completed rewrite or fill offer may survive the
  rejected publication; shutdown must close owned work before fixture cleanup.
- Verify FIFO order and nonzero activity during the held post-publication fill.
  This explicitly documents that the minimal design does not free worker
  throughput. Keep the original complete repair and reader gates unchanged.

Any selected scheduling change needs corresponding ADR 0024, `docs/freshness.md`
and `docs/proof-repair.md` wording: Completed means the validated process/report
finished, while live Current remains independently checked and may temporarily
be Unknown. Documentation cannot substitute for the original acceptance gates.
