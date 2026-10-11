# Frozen local workspace gate and solver suite

`scripts/local-combined-gates.py` runs the committed `ci/run.sh gate`, followed
by the committed `ci/run.sh solver` only after the gate exits successfully and
its ownership checks close. It uses an already running Linux CI container and
an operator-owned external Cargo target. It never creates or restarts Docker
containers, builds images, deletes targets, restores source, or runs a product
repair or performance benchmark. Required CI still uses its existing runner.

Run the helper from the exact committed revision that root intends to validate:

```sh
python3 scripts/local-combined-gates.py \
  --repo . --revision "$revision" --container "$container" \
  --native-root "$fresh_native_root" --source "$fresh_native_source" \
  --target "$existing_native_target" \
  --output "$fresh_host_output" --protected "$protected_inventory" \
  --retain-elf "$actual_daemon_test_image" \
  --retain-elf "$actual_verus_test_image"
```

The native root must be a new temporary directory with the helper's
`sail-local-gates-` prefix. Both output roots and the explicit source path must be unused. The source can
use the product grant's existing revision-based path convention, so a later
separately authorized release build and install reuse this exact tree rather than
copying another checkout. The source and external target must not overlap. The protected JSON
has `files` rows containing `path`, `bytes`, numeric permission `mode`, and
`sha256`. Supply actual installed product, engine and retained-image paths from
an existing verified inventory; the helper does not infer them or treat their
presence as qualification. Explicit retained ELF paths must come from actual
Cargo/nextest metadata or a separately checked image inventory. Copies retain
bytes and modes and are checked after the gate, before solver work can overwrite
a test image. The record does not infer execution from file presence or claim
to archive every Cargo artifact.

The helper archives an exact Git commit, binds its own committed bytes, and
checks all archived file bytes/modes, directory modes, symlinks and membership before and after
each phase. Safe tar extraction restores only the admitted ordinary Git permission bits
after the data filter; special bits, unsafe links and unsupported types refuse.
Only the intentional external-target symlink is added. Python
bytecode writes are disabled for the native controller and both phases. A
container ID, image and Docker birth check surrounds setup and execution;
the native controller also binds PID 1's birth. Protected files must match their
inventory before and after each phase.

Cargo jobs remain two and nextest independent test concurrency remains one.
The local outer caps are 3,600 seconds for the whole gate and 1,800 seconds for
the whole solver suite. They leave every original per-test deadline, ignored-test
selection, oracle and failure predicate unchanged. These generous local caps
are not CI policy changes. No later phase is launched after a failed gate.

Each native phase starts in an owned process group. The helper reuses the
birth-checked descendant-group primitives in
`scripts/m7-tspec-transcript-run.py`. Timeout or catchable interruption requests
cleanup only for birth-matched groups; uncertainty remains sticky even after the
leader is reaped. Host Docker transport is separately bounded. Transport loss,
container change, missing ownership or incomplete group observation stops the
sequence and requires a separate owner audit. Known-group scans retain Z/X rows separately as `terminated_unreaped`: these
terminated rows cannot execute or hold file descriptors. Live survivors,
unreadable/unknown rows and incomplete group observation still refuse closure.
Known-group scans do not prove
arbitrary orphan absence or discover every short-lived detached descendant.
The helper never restores source or grants cleanup credit from leader reap.

Complete raw gate/solver logs and their hashes remain in the native output root;
on a returned terminal result they are also copied and rehashed on the host.
Primary status is written before the final closure checks. A complete combined
PASS requires both original phases to exit zero with matching input identity and
closed recorded ownership. A failed, interrupted or unattempted phase receives
no skipped-gate credit; earlier failed scopes remain separate.

`python3 scripts/test-local-combined-gates.py` currently passes 26 pure controls on Python 3.12 (the actual extraction control requires
the standard-library data filter)
using fake processes and temporary files. They exercise stopping, timeout and
signal cleanup, sticky uncertainty, transport capture bounds, input substitution,
exact image retention and full two-phase PASS validation. No native gate, solver,
Docker action or compiler invocation was performed to validate this helper's
initial source. Native execution remains a separate recorded scope.

The first dfe3c47 native scope remains FAIL: its gate exited zero, but the
controller counted terminated, unreaped group members as live survivors and
never launched solver. The corrected observer does not retrospectively promote
that scope; a fresh complete gate and solver execution is required.
