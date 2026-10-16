# Native source synchronization (#188)

**Status: source controls and local gates pass; required PR checks are enforced separately.**

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
| `f6063c0d`, joined with main `c96fdbec`, c7i.8xlarge, us-east-1 | Full native Linux workspace gate: 631 tests pass, 35 skipped; 504 tracked source inputs match | Archive `c8ba226252696ff25b41de2ead3a8b07dc973129e9da4043d57ac57c9283d076` |

The macOS gate predates the integration with `c96fdbec`; the sync helper patch
is unchanged. These are source-sync and workspace correctness checks, without
a solver, corpus or performance change. Required PR conformance runs separately.

The earlier #183 native attempt remains rejected because its compiled source
could not be established. Its exact original sync cause is unresolved; this
reproduction does not attribute that historical failure. The accepted rerun is
bound to its complete source and executable identities in the #183 evidence.
