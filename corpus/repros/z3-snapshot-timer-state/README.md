# Snapshotting pinned Z3 changes timer state

This is a snapshot-prototype defect, not an unsoundness claim about Z3.
With pinned Z3 4.16.0 kept alive on interactive stdin, send
`idle-timer.smt2`, wait for `sat`, and inspect its threads. A completed timeout
query leaves an idle timer worker. The old snapshot preload's next marker
calls `fork`; Z3's registered prepare handler stops that worker before either
process continues. On the actual pinned macOS binary the thread counts are
1 initially, 2 after the query, and 1 after the old snapshot marker. The
continuing parent's implementation state has changed too.

The primary source is Z3 commit
[`ddb49568`](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/util/scoped_timer.cpp):
`initialize()` registers `finalize` with `pthread_atfork`; `finalize()` joins
and deletes available timer workers. A global shell timeout has an active
worker instead, which a forked child does not inherit.

The candidate now declines snapshots when it observes multiple threads. This
is conservative admission, not a proof that a process with one observed
thread has no other unsupported state. The candidate remains disabled.

The actual pinned-solver regression and ordinary-byte/side-effect comparisons
are part of:

```sh
python3 native/sail-snap/test.py --lib LIB --z3 Z3
```

The old library publishes one snapshot in each idle-query, global-timeout and
output-descriptor case; the corrected library must publish none and leave the
ordinary run's stdout, stderr, exit status and output-file bytes unchanged.
Separate tiny C fixtures cover active interval timers, finite CPU limits,
foreign descriptors and other live threads. Linux additionally checks POSIX
timers; native Linux execution remains pending.

See the [snapshot report](../../../docs/reports/2026-10-16-solver-snapshots.md)
and [qualification record](../../../docs/validation/solver-snapshots.md).
