# Watch startup and publication staging

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
