# Fault fixture lifecycle boundary

The complete combined `4472279` workspace gate passed 540 tests, while its
original solver scope passed 38 of 39 and timed out GC/clear at 180.036 seconds.
[The retained audit](m7-shared-solver-timeout-447-readonly.md) does not establish
which fixture phase stalled or a cause for that timeout.

The fixture has a separate, concrete cleanup defect. Its maintenance thread
stops only after both concurrent rounds return. A foreground assertion or
session-thread panic skips that store; `thread::scope` then joins maintenance,
which can run forever. A `StopOnDrop` inside the scope closure now stores to the
same flag before scope joining on unwind. Normal maintenance, both rounds, all
reply/cache assertions and every original deadline remain unchanged. Fixed
stage, PID and elapsed-time markers write directly to stderr, so libtest cannot
hide them until the body returns.

The additive regression waits for maintenance to enter, injects a foreground
unwind, then requires the scope to finish with maintenance stopped. An exact
private copy with only the guard removed remains stuck after the same entered
marker and is killed by a one-second external supervisor. This proves the
cleanup defect independently of the original solver timeout; it does not
establish that the original run experienced a panic.

The [frozen controls](m7-fault-maintenance-cleanup-controls.json) retain the
actual Linux integration pass (one test, 0.01 seconds), affected all-target
strict pass, and actual Linux/host guard-removed hangs. A separate controlled
hang proves direct stderr exposes a phase before body return while libtest's
captured `eprintln!` does not. An initial negative-control copy/compile setup
failure is retained; only its working directory was corrected. No original
solver test or complete gate was rerun in this scope.

## Separate proposed session ownership correction

This proposal is not implemented by the maintenance fix. `support::direct`
currently uses `wait_with_output` without a local limit. `Shim::wait` enforces
its existing 60-second session clock, but kills only the shim and then joins
its stdout reader without another bounded predicate. A descendant that keeps a
pipe open can therefore outlive the shim and hold that join. The original
nextest limit remains the final supervisor; no such descendant was captured in
the failed run.

A subsequent test-helper change should put each launched direct solver or shim
in its own process group, retain that group's original child identity, and use
one existing `DEADLINE` clock from launch for feeding, waiting and draining.
Normal completion must still retain complete stdout/stderr, require EOF and
join every helper before applying the unchanged verdict and byte checks.
Timeout must kill the owned group and classify an infrastructure failure;
partial bytes cannot satisfy a success assertion.

A controller using nonblocking pipes and readiness polling can avoid a blocking
reader join. It would need explicit bounded buffers, complete EOF handling,
pre/post-poll deadline checks, and cleanup on unwind, including a peer that
keeps stdout open after its leader exits. A safe Unix polling/process-group API
would be a test dependency decision requiring review, rather than hidden unsafe
code or a shell command whose own wait has no bound. Merely checking
`JoinHandle::is_finished` and dropping an unfinished handle would leave detached
work and would not establish bounded ownership. Kernel kill/reaping and OS
scheduling remain outside a hard real-time promise.

Required controls include ordinary exact completion, a sleeping solver, a
blocked input consumer, large stdout/stderr, leader exit with a live pipe-owning
descendant, and a foreground unwind. The old bodies must fail the appropriate
externally supervised controls. No production shim lifecycle, source engine,
solver flags, conformance comparison or original timing limit changes belong in
this proposal.
