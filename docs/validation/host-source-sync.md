# Native source synchronization (#188, #589)

**Status: #589 local source controls pass; native Linux, platform host and
hosted qualification remain pending. Historical #188 gates are recorded below.**

## Linked-worktree metadata

The #589 candidate preserves actual working bytes and staged entries while
building standalone Git metadata from a frozen HEAD bundle and staged blobs.
It excludes local Git config/hooks/pointers, resets index optimization flags,
retains checksum/current-mtime behavior and refuses source/index/HEAD drift.
The [testing guide](../testing.md) records commands and supported index states.

All 17 local macOS real-Git/rsync controls pass for primary/linked checkouts, staged
then modified files, untracked/deleted inputs, repeated branch switching,
same-size/same-time content changes, Cargo timestamps, split/unmerged indexes,
local-only config/hooks and unsupported-state refusal. Only SSH transport is
local; the Git and rsync operations are real. An expected metadata-transfer
failure retains its nonzero result and refuses usable remote Git metadata.
Owned-lock controls cover failure after owner creation but before staging,
lost acquisition acknowledgement, and retention of foreign/missing ownership.
Native Linux whole-gate and hosted qualification remain pending. No solver,
conformance or performance result follows from these local controls.

The Linux-helper-only prefix passes all 14 controls; its raw log remains
outside Git. Both helpers then use the same transfer implementation with their
original SSH users. Actual Mac-helper transport coverage uses local SSH/rsync,
not an allocated EC2 Mac. The platform host gate remains required before push
under AGENTS.md; no new heavy local build or cloud allocation is claimed.

The complete 17-control local log has SHA-256
`b16c95bcb6283a802e9378b085abffb928cd83a49572e95b12eb6fdfedfbfbc8`.
The unchanged teardown suite passes 14/14 controls, log SHA-256
`012abdcd45ea9e86bc52555f97b9c2a7e529439b4cc3124cd796836c807d5c99`.
Both shell syntax checks, formatting and whitespace checks pass. These are
local tooling results, not the pending full native/platform gates.

## Original content and timestamp qualification

Both Linux and macOS host helpers compare source contents during sync and give
changed files current timestamps (`rsync --checksum --no-times`). This corrects
two reproducible hazards: different bytes with equal size and mtime could be
skipped, and an older checkout could retain timestamps older than a reused
Cargo artifact. Build artifacts remain on the native host.

Four local rsync controls retain the original failures and the corrections:
equal-size/equal-time old sync preserves wrong bytes; corrected sync copies the
right bytes with a current mtime; old sync preserves an older checkout's stale
mtime; corrected sync supplies current timestamps. Summary SHA-256:
`e5577708e2ce1e33d8cc78c77d7b9e30f0be0275cc849e62ed5ef84b4e44f4df`.

| Source and environment | Scope and result | Raw evidence SHA-256 |
|---|---|---|
| `317516f5`, macOS 26.5, Apple M3, Rust 1.99.0 | Full host gate: Clippy and 617 tests pass, 35 skipped; tracked inputs unchanged | Log `bd8551479b701f998a324b5990966a2bd68074ddeabedce005c031fd3d1c171a`; source manifest `843536e66ab2d77611081b94600ad35b78ab1d1a032f1ae68c0d677023adea4d` |
| `f6063c0d`, joined with main `aabf3a15`, c7i.8xlarge, us-east-1 | Full native Linux workspace gate: 631 tests pass, 35 skipped; 504 tracked source inputs match | Archive `c8ba226252696ff25b41de2ead3a8b07dc973129e9da4043d57ac57c9283d076` |

The macOS gate predates the integration with `aabf3a15`; the sync helper patch
is unchanged. These are source-sync and workspace correctness checks, without
a solver, corpus or performance change. Required PR conformance runs separately.

The earlier #183 native attempt remains rejected because its compiled source
could not be established. Its exact original sync cause is unresolved; this
reproduction does not attribute that historical failure. The accepted rerun is
bound to its complete source and executable identities in the #183 evidence.
