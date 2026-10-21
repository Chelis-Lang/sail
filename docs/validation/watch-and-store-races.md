# Watch startup and publication staging

**Status:** the latest Mac gate passes 856 tests (37 skipped, Clippy clean).
Native standard conformance of the store correction is running before merge;
the final directory/fallback watch correction awaits its fresh native gate. No solver, key or proof-admission rule changes.

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
full fast conformance (147 in all four arms). Standard remains pending.

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
