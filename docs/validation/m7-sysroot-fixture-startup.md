# Sysroot handoff fixture startup coordination

The complete frozen `37e5342` workspace run failed its single
`assert!(index.prepare(scope))` in the independent handoff fixture: 540 of 541
tests passed. The [original failed scope](../../bench/validation/m7-linux-core-gate-37e5342-failure-emulated-2026-10-11.json)
records no acquisition outcome that establishes its cause. The independently
executed original solver suite passed 39 of 39; the earlier `4472279` GC/clear
timeout remains separately retained.

Production `prepare` deliberately returns false if the maintenance state is
busy. `Index::new` starts maintenance immediately, and the old fixture asserted
its first hint without coordinating that thread's initial mutex acquisition.
The test fixture now pauses maintenance before that acquisition. It retains the
original cold lookup and single preparation assertion, then releases the worker
before the original construction wait. No retry, ready snapshot, content hash or
proof is inserted. Later preparations in the eviction/replacement tests retain
their original behavior. Production preparation and lookup remain unchanged.

A test-only owned channel releases on fixture drop before the index joins its
worker, including assertion unwind. A controlled maintenance thread holding the
actual state mutex demonstrates that `lookup` returns None and `prepare`
returns false within the unchanged 100 ms control bound. The old single
preparation assertion fails in that controlled state. This proves a possible
fixture race, not the historical failed run's cause. A separate controlled
unwind verifies release and worker join.

The private Linux run passes all eight controls in 17.16 seconds and strict
affected-crate all-target Clippy in 28.20 seconds. The tested executable, source
overlay, lockfile and raw logs are retained by exact hashes in the
[control record](m7-sysroot-fixture-startup.json). These controls are neither a full workspace gate nor proof, live reader, source engine
or M7 performance acceptance. Original production deadlines, independent
inventory/guard checks and every original handoff assertion are retained.
