# Transcript hashing memory

The candidate streams each transcript batch, its newline and the exact DONE
echo into SHA-256. It removes the reconstructed pipe buffer from attribution;
it changes neither the hashed bytes nor their order. The existing transcript
fixture checks the digest against the reconstructed stream.

A native c7i.8xlarge in us-east-1 ran three rotated pairs against one release
test executable, hashing four 64 MiB batches. Peak resident memory was
528,644–528,892 KiB with reconstruction and 266,336–266,884 KiB with streaming.
Every run produced the same digest. This is an allocation diagnostic of the
hash operation, not a whole-CLI memory or latency claim.

The broader trial also changed source-observation scheduling and ancestor
scans. Three-round small-project and Anvil comparisons on native Linux and
Apple-silicon EC2 showed no meaningful overall latency improvement. Those
source changes are excluded. A subsequent directory-read trial was slower
and an identical-observation shortcut was inconclusive; neither is selected.
Cold and unchanged overhead remain open.

The selected change affects attribution for all corpus projects. Native Linux
workspace gate, 42 solver tests (including the privileged fault controls),
full E2E plus extra and PR conformance pass for the reduced patch. The macOS
host gate passes all 1,033 nonignored tests. Raw allocation controls and gate
logs: `s3://example-evidence-bucket/experiments/2026-10-24/sail-sprint-sprint-gates.tgz`,
sha256 `dc6116415ee79a5f7e59603c9a989ffaf8d304a97318a9d152e3fecfc59684f2`.
