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
compiler directories. Do not discard failed attempts, pinned runtimes, executable
identities or libraries needed to reproduce retained results.

Compiler `.rlib`, `.rmeta`, dependency and object files and Cargo `incremental`
directories can be removed from closed builds. Keep executable and loadable
library files when validation records identify them. Do not clean a target used
by an active compiler, even if its source checkout appears unchanged.

Inspect stopped containers and dangling images individually. Remove containers
without deleting their volumes. Preserve active containers and data volumes;
avoid a global prune while other projects are building.

The [M7 cleanup record](validation/m7-workspace-cleanup.json) records retired
checkouts, compiler files, preservation checks and observed free space. Recovery
archives live under the local Sail cache in
`retired-worktrees-2026-10-10/<worktree>/`. Each directory contains
`source.tar.gz`, `metadata.json` and staged/unstaged patches, plus `target/` when
raw artifacts were retained. Its recorded `refs/cleanup/worktrees/<worktree>`
reference restores the commit; unpack the source archive into that checkout to
restore the actual working files. The patches also retain staging information.

One newly active diagnostic checkout was mistakenly included in that cleanup.
Its archived edits and commit survived; the agent recovered them into a protected
checkout and restarted from committed native Linux source. The interrupted
compile remains a setup failure. No tests ran in that interrupted attempt.
