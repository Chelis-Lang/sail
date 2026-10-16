# Linux host teardown (#201)

Status: the native workspace gate and 14 isolated fake-AWS controls pass for
source `e842a844c5ea9a580d27a12248da2c8dc1912bc6`. The controls cover expired
authentication, failed termination, failed waiting, a successful waiter followed
by a still-running instance, malformed/unknown/mismatched responses, exact
not-found errors and empty `Reservations`, cleanup errors, successful teardown,
and repeated teardown. Failure cases retain connection state and do not delete
resources before confirmed termination or absence.

On 2026-10-16, native x86-64 EC2 c7i.4xlarge (16 vCPU), Ubuntu 24.04,
us-east-1 ran `ci/run.sh gate`: 638 tests passed, 35 tests
skipped, and two doctests passed. The separate
`python3 scripts/test-perf-host-down.py` run passed all 14 controls. All 521
tracked source files, index and untracked-file checks matched before and after.
The core image identity was
`sha256:6f4cf197281f7b75356d4c015ab4b915a376ed1bb89203d361d83a44864d12c9`.
The compressed raw gate archive has SHA-256
`5cdbeb80f87eb186936c05749ed61a587560f4a747a8324a44a294f97b4bae9b`.

The later rebase preserves the helper and control script byte for byte; these
are source-specific native results, not a same-head gate after the rebase.
The change affects no corpus entry, verification runtime, solver byte stream,
or performance claim. This is teardown control-flow qualification using fake
AWS responses, not fault injection against a live AWS account. The required
PR checks remain separate.
