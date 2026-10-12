# Completed M7 scratch retirement

The owner requested aggressive retirement of completed work on 12 October.
The [retirement record](m7-scratch-retirement-20261012.json) records the actual
deletions and compact archive identities. This is a storage operation, not new
verification or benchmark evidence.

Both Sail containers were idle before their completed scratch was captured.
All 966 completed Sail temporary directories were removed after verifying
197,820 retained text and JSON files. Their two compressed evidence archives
total about 580 MiB; compiler targets and historical executable copies were
retired. Receipt data, raw admitted logs, original outcomes and reproduction
scripts remain in those local archives. Their manifests identify retained
members; these archives are not complete binary working-tree backups.
The archives and deletion journals live under
`evidence/2026-10-12-scratch-retirement/` in the local Sail cache, outside
disposable temporary storage.

Cleanup also removed 63 obsolete product installations, 20 archived host
worktree targets, two closed end-to-end workspaces and obsolete build-tree
archives. Source archives, recovery metadata and Git history remain. The
selected engine runtime, stock toolchains, M6 baseline, current product and
continuing Cargo target were preserved. Other projects were not cleaned.

Observed host free space rose from 414 MiB to about 112 GiB. Other projects
were active, so this is an observed total rather than exclusive attribution
of every reclaimed byte. Historical execution records keep their original
results and executable hashes; retired historical images are no longer
available for direct replay from the former scratch paths.

New validation uses the continuing Cargo target. Completed runs should retain
compact evidence outside their scratch and retire the scratch immediately.
Automatic terminal-safe cleanup is separate work and is not claimed complete
by this record.
