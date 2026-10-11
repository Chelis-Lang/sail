# Original managed cold stage witness

An isolated run used verified frozen `777b86a` Sail/shim bytes and the
`11e34e2` timeout evidence harness. Its original first cold invocation exceeded
240 seconds. The operation took 240.922 seconds including timeout evidence and
cleanup; the outer harness took 244.093 seconds. No complete report was received,
and later managed assertions were not reached. This run cannot replace the
[failed complete end-to-end aggregate or native CI comparison](m7-managed-cold-777-vs-f94.md).

The [portable stage record](m7-managed-cold-stage-777.json) binds the exact binary,
harness, output and snapshot hashes. Kernel process start ticks show Cargo
starting 58.56 seconds after Sail and the vstd verifier starting 140.36 seconds
after Sail, about 81.8 seconds after Cargo. Pre-Cargo setup was not separately
profiled. Compilation messages alone do not identify each dependency's duration.
The existing progress stream contains `context-ready` followed by
`target-started` for vstd; no demo target start was observed.

At the deadline, the owned compiler had an exact `--crate-name vstd` selector,
87.68 accumulated user CPU seconds and 13.97 system CPU seconds. Six shim/Z3
pairs were also captured under that verifier. These are process-wide counters;
parallel threads can accumulate more than one CPU-second per wall second. They
establish actual verification activity, not a verdict, last-interval utilization,
solver exhaustion, a deadlock, or a pure scheduling/emulation cause.

The timeout helper captured 20 owned processes and a partial 241-entry run
sidecar inventory in 0.349 seconds, reading 77,906 bytes. Limits and observation
loss remain explicit. The original owned group kill/reap and scratch cleanup
then ran. No captured process remained active; 19 zombie descendant entries
remained under container init. Zombie entries do not retain open descriptors.
No fixture preparation, copied proof, compiler argument change or deadline
extension was used. Native CI's successful cold run remains separate evidence.

The [cold-start guide](../managed-cold-start.md) describes why newly isolated
contexts compile dependencies and require complete dependency proof receipts.
This witness identifies several real cold stages, so a macro-only optimization
cannot be assumed to solve the timeout. Timing the managed preflight and
verifier stages is necessary before selecting a production optimization; the
original correctness checks remain the acceptance criteria.
