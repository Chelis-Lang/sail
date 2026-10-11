# Closed host evidence retirement

Two closed host copies, `target/retained-prior-worktree-binaries` and
`target/preparation-namespace`, were archived together and retired to reduce
backing-space pressure. Every one of 2,458 regular files retains its exact bytes,
size and mode in the verified archive. All members were independently streamed
and hashed, then original files were rehashed before deletion. No symlink members
were present. The [record](m7-host-tested-evidence-retirement.json) binds the
archive and complete original inventory.

The archive is 386,830,878 bytes, SHA-256
`c4d94cf9baf5f11ee249c82cb3ede386852e0791da6da3d855a4a959c7bce9d0`.
Source-directory allocated inventories summed to 1,268,195,328 bytes. Observed
backing free rose from 9,440,309,248 to 10,561,523,712 bytes during concurrent
activity; that difference is not an exact attribution of physical reclamation.

An earlier native census failed with Docker API 500 during Desktop interruption
and did not authorize deletion. After fresh container births and write checks,
both native cmd/cwd/FD/maps censuses found zero relevant holders and zero
unreadable entries. Fresh host open-file checks also found none. Only these two
closed host copies were removed. Native targets, current runtime/source seals,
active scopes and all archived tested images remain preserved.
