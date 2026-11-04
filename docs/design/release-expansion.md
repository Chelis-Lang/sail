# Release-expansion implementation package

Owner-selected scope, 2026-11-01; exact candidate approved and published as
0.2.0 on 2026-11-04. The release gives users a direct Verus-file workflow:
verify and repair a file, keep durable jobs/watch/MCP, prepare subsequent
verification in the background, and share CI caches while bounding local
cache storage. Cargo installation is an additional distribution channel.

## Scope and current state

| Slice | Delivered / qualified | Remaining |
|---|---|---|
| Standalone verification and repair | Native Linux/Mac CLI, jobs, watch, MCP, focus/full and negative controls; both exact stable archives pass first-use/repair/replay | Supported scope and watch-directory limits remain documented |
| Daemon and new-project preparation | Foreground priority, generated inputs, closing revisions, cancellation and queued-origin controls pass; bounded Mac cost controls complete with the intended toolchain | General background resource bounds are not established |
| Opt-in cache byte budgets | Manual and real idle-maintenance eviction, busy/newest protection and rebuild controls pass on both platforms | Soft-cap behavior remains intentional |
| CI-cache workflow and consumer skill | Four independent hosted cache jobs and installed consumer-skill controls pass; guides ship | Fixture results retain their measured scope |
| Binary compiler-input coverage | Corrected fork merged, native runtimes published/pinned; compiler-returned bytes, source suites, factories and composed controls pass | Existing documented unsupported configurations |
| Core performance changes | Complete six-round workflow and full edit catalogues captured on both platforms; all 1,944 paired steps compare Same | No global parity claim; five slower workflow medians remain explicit |
| First-unchanged/full-path overhead | Run-scoped and per-file experiments remain unselected after no general whole-call gain | Original performance goal remains open |
| Mac Anvil cold | Latest cohort is 96.87 s stock / 98.94 s Sail | Original performance goal remains open |
| Incidental edits | Exact rc.1 archives: fifteen-edit catalogue plus Anvil controls, all 114 samples/platform captured and audited | Earlier lost attempt remains indexed with its original scope |
| Crates.io distribution | All eighteen stable versions published from `3d7782bf`, independently matching the approved package bytes and clean VCS identities | Future published tags use the same source-bound publication workflow |

Preparation is automatic with `SAIL_BACKGROUND_PREPARE=0` as an opt-out.
Eviction remains disabled until an explicit budget is configured. Unselected
source/per-file digest experiments stay disabled. Delivered features do not
imply that every original performance goal is solved.

## Qualification and rollout

Both exact stable archives, the full native workspace/solver/E2E/fast/standard/
deep gates, all 316 Stock refinements and all 205 deep mutations pass.
Protected PR #661 binds final source `3d7782bf`; all eighteen packages were
regenerated and verified from that clean merge before owner approval.

The complete core measurements cover 23 paired workflow rows in six rotated
blocks plus 972 paired mutation/cold/return steps per platform. The original
strict report retains its explanation-note classification failure. Merged
ADR 0099 correction #657 independently rechecks all 1,944 steps Same, and the
separate corrected report has 26/26 valid rows without changing any timing.
Two Linux and three Mac slower medians remain known limitations.

After explicit owner approval, the exact archives were uploaded, attested,
downloaded again, matched by SHA-256 and independently verified before GitHub
publication. The release-triggered crate workflow passes from the same immutable
tag/source. The [release record](../validation/stable-candidate.md) binds
artifacts, installed consumers and their captures. See
[feature qualification](../validation/release-expansion.md),
[runtime qualification](../validation/binary-compiler-inputs.md) and
[release notes](../release-notes/0.2.0.md) for source-specific scope and limits.

Large frontend reuse, Cargo bypass and new solver-history semantics remain
outside this bounded release scope. Future release candidates still require
explicit owner approval; 0.2.0 approval is not blanket publication authority.
