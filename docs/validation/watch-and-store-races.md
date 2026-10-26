# Watch startup and publication staging

## Late notifications, dropped notifications and the backstop (#403, #416, 2026-10-26)

**Defect 1 (#403).** After the first real edit, every relevant native notification
cancelled the run in flight. FSEvents under load delivers an edit's
notifications late and repeats cumulative flags. A notification for the
`src/target/mod.rs` edit arriving after the 300 ms debounce had submitted
the run that already included that edit cancelled it and queued a fourth
run, which matches #403's 143 ms cancellation on live events. A second
defect affected content-scan mode. Refreshing roots after a completed run
recaptured the scan baseline, so an edit made before the next scan was
adopted as already verified and never re-run.

**Correction.** Just before each submission, watch records a snapshot of
the input bytes and file identities, including change times. A notification
batch restarts the run only if the inputs now differ from that snapshot.
Removals and renames of inputs, unreadable state and non-Unix hosts still
restart unconditionally. Root refreshes recapture only when coverage changes,
and they first check the old coverage against the baseline. Writes to Cargo's
output directory never restarted watch in any instrumented run, and they
remain excluded.

**Defect 2 (#416).** Under load, fseventsd can stop delivering a watcher's
events for an input while unrelated events still arrive, and send its "user
dropped" notice minutes late or never. Event tracing showed the watch loop
alive and the edit unseen for 120 s, on main and with the correction above.
Watch reacted correctly only once the notice arrived.

**Correction.** Every two seconds, in every mode, a backstop compares a
metadata stamp of the inputs (path list, identity, size, and modification and
change times; no file is read) with the last state seen. It rehashes and
restarts only when the stamp changes. Native events, polls and scans share
that last-seen state, so one edit is never reported twice. The stamp walks
the inputs with one `lstat` per entry, and a test pins its coverage to the
content capture's. On Vest's workspace (3,458 entries) one pass costs about
19 ms of CPU, about 18 ms wall; this was measured at load average 60–110, so
it is an upper bound. A full rehash costs about 120 ms and runs only on a
stamp change.

**Evidence** (Apple silicon, 8 cores, Darwin 25.5, base `b5420f24`):

- On the base, the delayed-duplicate and refresh-swallow regressions fail.
  With the corrections, they pass, along with deterministic tests for Cargo
  output writes, degradation to scans mid-run, a silently dropped edit
  notification caught by the backstop within one interval, and stamp
  coverage. That is 14 watch unit tests and 104 `sail-cli` tests, and Clippy
  reports no warnings.
- Under load (a `cargo build --workspace` loop, 2–4 concurrent instances,
  other sessions' suites, load average 30–120), the original integration test
  never failed the "Cargo output must not trigger a watch loop" assertion.
  Before the backstop, 41 of 330 runs stalled on dropped events (#416); the
  base stalled under the same conditions. With the backstop, 80 of 80 runs
  pass under the setup that previously stalled 24 of 120.
- #403's extra restart did not reproduce on the base in 150 runs (it was
  about 1 in 100 at its report). The deterministic tests are the evidence for
  that defect.
- Linux (inotify) uses the same code, and CI covers it.

## Current startup correction (2026-10-25)

The candidate based on `237c7d8d` corrects notification-batch classification.
Ancestor subscriptions can report unrelated sibling removals and renames.
Previously these disabled startup replay suppression for the whole batch,
including delayed Cargo.lock creation events, even when every tracked byte
and the complete filesystem epoch were unchanged. Only a relevant input
removal or rename now disables suppression. Actual input changes, including
edit-and-restore, keep their existing invalidation checks.

The deterministic regression fails on the baseline and passes after the fix,
covering unrelated and relevant removals and renames. Clippy and the complete
Mac host gate pass 1,056 tests. Twenty repetitions of both existing watch
integration tests pass while unrelated temporary files are continuously
created and removed. These checks preserve startup-count, cancellation and
restart assertions. Native Linux c7i.4xlarge in us-east-1 passes the workspace
gate (1,067 tests), solver (42), full E2E including local extra checks, and
all four PR conformance arms (33 Same each, 311 functions).

A tracing run on the baseline reproduced the extra restart. The same run
also missed a separate proof-workflow mock-worker deadline; cause unknown.
The subsequent complete fixed Mac gate passes without changing timeouts.
The older missing-file RPC symptom in #306 was not reproduced and remains
separate; this fix does not establish its cause.

Mac logs and the red/green regression witness are retained in the evidence
bucket as `experiments/2026-10-25/sail-watch-next-mac.tgz`, SHA-256
`b8df3c6031cb8482c024853de41e9e867b0ab8dd36dd7f14dd4e6537f612d0bf`.

Linux gate logs and conformance reports are retained as
`experiments/2026-10-25/sail-watch-next-linux.tgz`, SHA-256
`cc3feddc4dd8bfcf85532be0f66e0b85d0194d5f5075d1cc0404dc97ff3633ef`.
The code patch SHA-256 is
`3f7d7d0f56e472474366d61f1929f0a4be22eac18f8587fb56d656155d415b91`.

## Earlier qualification

**Status:** the final composed Mac gate passes 905 tests (37 skipped,
Clippy clean), including the directory-symlink regression. Native Linux
workspace gate, solver, full e2e plus extra and PR conformance pass on the
final watch correction; the symlink-only addition also passes its fresh
workspace gate. The exact store correction passes full fast and standard
conformance: 147 and 159 Same respectively in all four arms, zero Different
or infrastructure errors. No solver, key or proof-admission rule changes.

The watch correction distinguishes replayed startup notifications from real
edits using the complete initial file bytes and filesystem identity. An actual
change disables that filter; edit-and-restore with restored mtime still
cancels verification. Publication reserves a staging directory exclusively
and leaves a colliding directory untouched.

On Apple silicon macOS, the staging ownership witness fails before the fix.
The corrected witness and concurrent publisher/seeder test pass 100 stress
iterations; startup and edit-and-restore checks pass 20. Source `d7a975da`
passes the full Mac host gate: 854 passed, 37 skipped, Clippy clean.
Native Linux c7i.8xlarge, us-east-1, passes the workspace gate (865 tests),
solver (35), full e2e plus extra, PR conformance (33 in all four arms), and
full fast conformance (147 in all four arms). The store correction subsequently passed standard conformance in all four arms.

Mac raw logs: `experiments/2026-10-21/watch-store-mac.tgz` in the S3 evidence
bucket, SHA-256
`e78a826306e48966af73ab6528a52e06cc025124e4c72f13b302031e107951b9`.
No release performance claim is made from these checks.

Failed attempts remain failed:

- 2026-10-21, before the startup-epoch correction: 17/20 bounded startup
  repetitions passed; the three failures showed late creation notifications.
- 2026-10-21, `0c88bb9a`: the full Mac gate passed 853 tests and failed
  watch cancellation with an unclassified missing-file RPC error. Cause is
  unknown (#306). Operation context was added; 20 isolated repetitions and
  the subsequent full gate pass without changing cancellation assertions.
- An earlier concurrent-publication test failed intermittently. The staging
  witness establishes its own ownership defect, not that failure's cause.

The final watch correction also records empty input directory entries and uses
polling alone after native coverage degrades. Twenty repetitions of all four
monitor regressions pass. An earlier full Mac run after directory capture
passed 853/855: it exposed duplicate native/poll invalidations, and a repair
fixture independently missed its mock-worker deadline (cause unknown). Neither
timeout nor cancellation assertions were changed.
