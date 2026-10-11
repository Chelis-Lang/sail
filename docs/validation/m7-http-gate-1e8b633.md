# Corrected setup workspace gate: three HTTP timeouts

The complete frozen `1e8b633` workspace gate fails: **521/524 tests pass**,
with 39 ignored solver tests left to their independent suite. Strict Clippy
passes. Nextest takes 404.526 seconds; the inclusive gate takes 1,120.192
seconds under local Linux x86-64 emulation. Later gate steps are not reached.
The [terminal record](../../bench/validation/m7-linux-gate-1e8-failure-emulated-2026-10-11.json)
binds the original archive and complete raw log.

Three original 250 ms serving checks time out:

- The credential/disconnect case fails its initial expected hit. Its later
  credential and disconnect assertions are not reached in this invocation.
- The setup-publication case times out waiting for a served reply after the
  controlled constructor is released.
- The paused-initiator case times out in the follower. The request is built
  at 174.414 ms and accepted at 397.829 ms on the fixture's phase clock;
  the server completes its unchanged 75 ms delay after the transport has
  returned a timeout.

The original eight-reader coalescing case passes. That positive result cannot
replace these three failures. No budget, server delay, trust check, fixture
concurrency or test assertion was changed. The earlier
[send/body diagnostic](m7-http-sendbody-7da9579.json) sampled only its calling
helper's CPU; it does not account for reqwest's internal runtime or the server
thread. The complete cause remains unestablished. A separate bounded owner
observation is being prepared; it is not an acceptance retry or a transport fix.

This failed gate remains distinct from the product's
[original cold managed failure](m7-sysroot-activation-1e8b633.md), the independent
solver suite and the earlier native CI pass. The branch is not eligible to push
or merge on the strength of those other scopes.
