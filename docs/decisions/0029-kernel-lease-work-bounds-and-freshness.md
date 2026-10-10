# 0029: Kernel lease work bounds and freshness

- **Status:** accepted
- **Date:** 2026-10-10

## Context

The M7 lease validator initially rejected a sweep after 50 ms of elapsed wall
time. Genuine final-source proof establishment completed successfully but lost
Current solely through this heuristic; actual kernel tests also refused intact
owners under concurrent work. `docs/validation/m7-1dc-fresh-reader-failure.json`
preserves the failed proof attempts. Identical-inventory parsing measurements
showed typical submillisecond work with scheduling tails. Proc reads cannot be
forcibly interrupted by checking elapsed time afterward.

This heuristic is unrelated to the cached-unknown serving threshold of 50% of
the solver resource limit. That serving policy does not change.

## Decision

Remove elapsed-time rejection from kernel lease freshness validation. Retain the
2 MiB aggregate kernel-read cap, 4,096 initial descriptor limit and every exact
owner, inventory, lease-state and namespace check. Use allocation-free row
parsing and a sorted exact inode multiset, preserving hardlink multiplicity.
Validate two independent complete inventories with owner-state checks between
and after them. Both global reads and any descriptor fallback share the same
aggregate byte cap; the second sweep never retries or revives a failed first
sweep. SIGIO may close descriptors before the helper process exits, so process
liveness alone is insufficient to establish retained lease ownership.

Each helper acquires its complete read-lease inventory before readiness and
never adds or renews leases afterward. Its immutable PID/start-time generation
can only lose ACTIVE leases. Observing every expected lease ACTIVE, even across
multiple kernel iterations or a scheduling pause, proves they were all ACTIVE
at sweep entry: a later ACTIVE observation cannot follow a break and reacquisition
within this owner. Live, non-stopped owner identity checks bracket the sweep.
The kernel inventory is certified at the final confirmation sweep's entry,
not its return or an assumed atomic proc snapshot. Caller namespace/content
barriers combine that monotonic lease evidence with the captured bytes and their
publication. The combined freshness check establishes a valid point within the
reader's validation interval; it need not coincide with the kernel sweep's entry.
A write after that point can legitimately precede response delivery; Current
describes the validated snapshot, not an atomic filesystem state at response return.
Independent namespace, interpreter, mount and inode barriers still bracket the
retained bytes. Breaking, missing, malformed, unexpected, oversized or unavailable
state permanently invalidates the owner.

This argument follows Linux's [lease transitions and ACTIVE/BREAKING status](https://github.com/torvalds/linux/blob/f155f3b3ed1af23884ffaffe8a669722b87ac9d6/fs/locks.c)
and the documented [kernel lease inventory](https://man7.org/linux/man-pages/man5/proc_locks.5.html).
In Linux v6.17, a writer sets
[`FL_UNLOCK_PENDING`](https://github.com/torvalds/linux/blob/v6.17/fs/locks.c#L1474-L1500);
the [lease timeout path](https://github.com/torvalds/linux/blob/v6.17/fs/locks.c#L1345-L1399)
unlocks and deletes that lease rather than restoring ACTIVE. Downgrading a write
lease to READ does not apply to this helper, which acquires only READ leases.
The argument depends on the bound helper's no-reacquisition policy.
Tests pause a reader after capturing kernel rows for longer than the removed
heuristic. Intact leases remain valid; an outside-hardlink write or writable
mmap mutate-and-restore during the same pause still invalidates the generation.

## Consequences

Descheduling no longer turns a valid kernel proof into Unknown. The finite
inventory caps bound work, without claiming an interruptible kernel wall-clock
deadline. Actual five-second client RPC deadlines and the 200 ms p95 concurrent
reader acceptance gate remain unchanged. Slow or unavailable reads must still
fail those gates; this decision does not establish a latency acceptance result.
