# Native source synchronization (#188, #589)

**Status: #589 native Linux gates pass at `30fb48ef`, based on main
`8797ccd4`. The full Mac host gate passes at `d8c2cd21`; its exact helper,
test, wrapper and CI program bytes match the Linux-qualified source. The PR
is restacked on merged #598 (`0057eba6`); hosted CI and final merge review
remain required. Historical #188 gates are recorded below.**

## Linked-worktree metadata

The #589 candidate preserves actual working bytes and staged entries while
building standalone Git metadata from a frozen HEAD bundle and staged blobs.
It excludes local Git config/hooks/pointers, resets index optimization flags,
retains checksum/current-mtime behavior and refuses source/index/HEAD drift.
The [testing guide](../testing.md) records commands and supported index states.

All 23 local macOS real-Git/rsync controls pass (89.181 seconds). They cover
primary/linked checkouts, staged
then modified files, untracked/deleted inputs, repeated branch switching,
same-size/same-time content changes, Cargo timestamps, split/unmerged indexes,
local-only config/hooks and unsupported-state refusal. Only SSH transport is
local; the Git and rsync operations are real. An expected metadata-transfer
failure retains its nonzero result and refuses usable remote Git metadata.
Owned-lock controls cover failure after owner creation but before staging,
lost acquisition acknowledgement, and retention of foreign/missing ownership.
These source controls alone supply no solver, conformance or performance
result. The separately executed native gates are recorded below.

The first real Linux SSH sync at `f9591228` installs usable standalone Git
metadata but its source check refuses ` D .claude/settings.json`: the inherited
agent-directory exclusion drops a tracked file. No native gate runs against
that incomplete checkout. The correction transfers actual HEAD/index-tracked
files beneath exclusions, with tracked deletion and dirty/staged controls,
while retaining exclusions for unknown private siblings and worktrees. A later
review adds committed branch deletion, staged removal followed by commit, and
retry after failed metadata installation. An atomic host-side ownership ledger
preserves retired names across failed transfers; unknown or traversal-bearing
ledger state refuses without mutating source or existing Git metadata. This
failed source check remains separate from later qualification.

The Linux-helper-only prefix passes all 14 controls; its raw log remains
outside Git. Both helpers then use the same transfer implementation with their
original SSH users. Actual Mac-helper transport coverage uses local SSH/rsync,
not an allocated EC2 Mac. The separately executed full platform host gate is
recorded below; these helper controls do not substitute for that gate.

The final local source guard records unchanged helper/test/wrapper/CI bytes
before and after all 23 tests. Its log SHA-256 is
`127cef847ba24b9259a658ab7d1bdfdf0f322b0554d1694fdd9cc1ea4949f906`;
the helper SHA-256 is
`246a59d4be7ac9b76dc6ef7a412ad16aa2bce2e0d94107151867a1539a20c4b1`.
The pre-freeze 23-control attempt fails one migration control because the
allowlist omitted macOS Git's generated `core.precomposeunicode`; that failed
attempt remains outside Git and receives no qualification credit. Its log
SHA-256 is `747e2b821ee58323895d8e60d9d89beaccd08b8078649d1c7daee2d27693f470`.

The earlier 17-control local log has SHA-256
`b16c95bcb6283a802e9378b085abffb928cd83a49572e95b12eb6fdfedfbfbc8`.
The unchanged teardown suite passes 14/14 controls, log SHA-256
`012abdcd45ea9e86bc52555f97b9c2a7e529439b4cc3124cd796836c807d5c99`.
Both shell syntax checks, formatting and whitespace checks pass. These are
local tooling results, separate from the full native/platform gates.

## Native Linux qualification at `30fb48ef`

The normal helper first transfers a real linked worktree with staged then
modified settings and a new file under `.claude`, a tracked working deletion
and an untracked input. Actual remote bytes, logical index, HEAD and Git status
match. A clean resync restores the exact candidate before any gate.

Cross-platform symlink correspondence checks Git mode `120000`, actual symlink
type and the exact raw target. Both original OS permission observations remain
in evidence. Every regular-file mode is exact. All subsequent native gates
compare against the frozen Linux observation, including exact Linux symlink
permissions; no mid-gate mode change is normalized away.

The sequential normal commands pass on native x86-64 Linux, c7i.4xlarge,
`us-east-1f`:

| Command | Actual result |
|---|---|
| `ci/local.sh gate` | Complete gate passes, including strict Clippy, 1,209 tests passed and 54 skipped, doctests, rustdoc, layering and dependency checks; all 23 sync controls pass natively |
| `ci/local.sh solver` | 51 pinned-Z3 tests pass |
| `ci/local.sh e2e` | Full unchanged end-to-end gate, including local extra cases, passes |
| `ci/local.sh conformance fast pr` | Pass-through, cache-fill, cache-warm and engine each report 33/33 same, 311 functions, zero unstable/different/infra; engine has 32 focused units |

Every source/index/HEAD/mode check before and after each command and at closure
passes. The archive retains canonical Git bundles, exact input inventories,
actual image/runtime/binary identities, complete outer stdout/stderr and all
conformance reports. The unchanged successful E2E harness removes per-case
successful temporary reports; those raw reports are not claimed retained.

Private evidence key:
`experiments/2026-10-28/perf-host-sync-30fb-native.tgz`, 13,734,203 bytes,
SHA-256 `b79bd80616347417c5f011ace31282ec8782fec6d488220664fe60935529cbae`.
The complete fresh object download matches its size and SHA-256.

The earlier source omission and pre-freeze migration failure remain refused.
A later first SSH audit also refuses the Darwin `0755`/Linux `0777` symlink
permission representation mismatch; its raw observations remain separate from
the corrected audit. This is an audit portability mismatch, not a source or
product failure, and receives no pass credit.

Standard conformance and proof suites are not selected for this diagnostic
source-sync change under the scoped policy. No performance result follows.
The Mac host gate below covers the platform-specific helper changes. Hosted
CI and merge review remain separate gates.

## Mac host qualification at `d8c2cd21`

The unchanged `ci/run.sh host` passes on native arm64 macOS with
Rust 1.99.0: full workspace Clippy across all targets, then 1,214 nonignored
tests passed and 54 skipped. The total elapsed time is 251.298 seconds;
nextest reports 138.996 seconds. This host command does not run doctests.
It retains the caller's normal Cargo and kache configuration, with one Cargo
job, serial tests and `nice -n 19`. No memory-pressure pause occurs.

Canonical Git source bytes and modes, the source head, actual tool binaries
and versions, and caller/project configuration match before and after the
gate. The regenerated benchmark-test dependency record binds its binary in
the actual qualified checkout, and xtask's dependency record binds that same
checkout's manifest directory. The dynamic error-code registry test passes
against these inputs, including the new `SAIL9003` declaration and registry.
The archive retains both gate attempts, the actual path-bound dependency
records, critical benchmark/help/xtask executables, Cargo fingerprints and a
canonical source bundle. Temporary child payloads not emitted by the unchanged
test harness are not claimed retained.

| Source | Attempt | Result |
|---|---|---|
| `d8c2cd21` | First Mac host gate | Failed: 1,213 passed, one benchmark-help test failed and 54 skipped; a relocated cached artifact embedded its previous binary path. Its xtask artifact also retained the previous manifest path, so this attempt does not qualify the new dynamic registry inputs. |
| `d8c2cd21` | Rebuilt unchanged workspace inputs, full Mac host gate | Clippy and all 1,214 nonignored tests pass; 54 skipped; final source/tool/config guards pass. |

Private evidence key:
`experiments/2026-10-29/perf-host-sync-d8c2-macos.tgz`, 13,674,499 bytes,
SHA-256 `7228dda7a8cd754ddcc710bfe9f25b999cd4680a541b393d4398f9786fb51711`.
Every indexed payload is read back and checked, and a complete fresh object
download matches the archive bytes and digest.

The final restack changes documentation and ancestry relative to this Mac
execution; all non-Markdown inputs remain exact. The six helper/test/wrapper/CI
programs also remain exact relative to the earlier Linux execution. These
source comparisons preserve each execution's original attribution; they do
not describe the old Linux gate as a fresh run on the final runtime base.
No solver, corpus or performance result follows from this Mac host gate.

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

## Release proof-evidence follow-up

The factory's default `release-out-proof-evidence/` is now an explicit output
exclusion. Unknown native evidence stays untouched; explicitly tracked files
still override the exclusion. All 24 real Git/rsync fixture controls pass
locally, including an inaccessible evidence directory, private local/remote
siblings and tracked source beneath the same name. Native Linux and Mac
refresh qualification remains pending.

| Source | Date | Result | Cause |
|---|---|---|---|
| `7ad933bf` | 2026-10-29 | Linux source sync failed with SAIL9003 before the refreshed gate | Prior successful release staging left root-owned proof evidence outside the original exclusions; raw sync log SHA-256 `af6028b53db12e7bf0f86d11df068ec843a4145a12b70d662fc70fc99424845d` |
