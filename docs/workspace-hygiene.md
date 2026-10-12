# Workspace hygiene

Retire a worktree after its changes are integrated or its experiment is closed.
Keep the active team roster, container mounts and running compiler paths in the
protected set. Recheck ownership immediately before each deletion: a worktree
created after the initial inventory is active until its owner says otherwise.
Use one continuing Cargo target per build environment where possible.

Before retiring a checkout, preserve its Git commit under a recovery reference,
its branch, staged and unstaged binary patches, and an archive of the actual
working files, including untracked source. Verify the archive hash and recovery
reference before removing the checkout. Keep validation output outside disposable
compiler directories. Preserve original failed outcomes, full raw invocation
logs, JSON reports and receipts, source/patch/lock identities, and the selected
required product and runtime. Keep compact evidence outside temporary storage.

Immediately retire finished run scratch and inactive compiler targets after
verified evidence transport and complete closure/holder checks. Do not archive
whole working caches or multi-gigabyte targets. Historical executables and
libraries may be retired with their hashes and source records retained; mark
them nonreexecutable without rebuilding. Preserve selected required runtime
binaries, rather than every historical test image. These retirements do not
change original validation outcomes or recorded artifact hashes. Do not clean a
target used by an active compiler, even if its source appears unchanged.

## Managed compiler targets

Managed compilation uses the selected cache's
`projects/<workspace-key>/compiler-artifacts/<family-key>/`. Ordinary and
full-engine requests retain the family's shared `target/` and ordinary
build-script export. Proof receipts and invocation records live separately
under `projects/<workspace-key>/<proof-context>/`. SMT-cache byte limits and
`sail cache stats|gc|clear` do not cover these directories. Terminal-job history
pruning also leaves compiler families and proof-context receipts intact. There
is no managed compiler-target quota, inventory command or generation-GC API.

The current source candidate isolates a focused repair's target and ordinary
build-script export under
`compiler-artifacts/<family-key>/repair-targets-v1/<proof-context>/`. Its local
source/build ledgers move to the proof context's `repair-observations-v1/`, so
old ledgers cannot attach generated inputs from a previous shared target to the
new isolated target. The same selector and configuration keep their generation
across source edits; the proof key binds arguments, runtime and environment,
while each input revision is independently checked. Different selectors can
duplicate dependency artifacts and each pays initial compilation. Ordinary and
full-engine paths stay unchanged. Native qualification of the target/observation
migration is pending; the original `387ba80` one-driver benchmark and reader
failures remain failed. This layout is neither a disk bound nor a measured
performance improvement.

For disposable validation caches, retire the complete closed run after compact
evidence transport and fresh idle/holder checks.
Keep selected products, runtimes, corpus pins and required source outside that
scratch root. Compiler bytes may then be retired with their identity records;
do not create giant target archives or retain every historical executable.
Removing an artifact makes its original receipt ineligible for reuse, and a
Historical report cannot replace it. Fresh verification must establish any new
evidence through the original path.

A future long-lived-cache maintenance API must identify exact workspace,
family and proof-context generations, retain the original nonblocking
context-then-family ownership locks, and coordinate retained freshness owners.
A terminal job or free lock alone cannot certify that a read lease or helper
has closed. Membership, birth-bound holders, descriptors, mappings and outside
hardlinks need checked treatment through teardown, with partial removal recorded
separately. This API does not exist today; SMT-cache GC is not its substitute.

## Containers and evidence

Inspect stopped containers and dangling images individually. Remove containers
without deleting their volumes. Preserve active containers and data volumes;
avoid a global prune while other projects are building.

Raw logs and archives stay outside the repository and are cited by SHA-256 from
the concise validation records (see [M7 validation](validation/m7.md)). Cleanup,
retirement and disk-space records are not kept in git.
