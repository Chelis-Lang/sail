# Next-release implementation package

Owner-selected scope, 2026-11-01; readiness reviewed 2026-11-03.
The release gives users a direct Verus-file workflow: verify and repair a file,
keep durable jobs/watch/MCP, prepare subsequent verification in the background,
and share CI caches while bounding local cache storage. Cargo installation is
an additional distribution channel. Final publication is not approved.

## Scope and current state

| Slice | Implemented / qualified | Remaining |
|---|---|---|
| Standalone verification and repair | Merged; native Linux/Mac CLI, jobs, watch, MCP, focus/full and negative controls pass | Both exact stable archives pass first-use/repair/replay; final source/package assembly and owner rollout |
| Daemon and new-project preparation | Merged; foreground priority, generated inputs, closing revisions, cancellation and queued-origin controls pass | Stable native gates complete; corrected interpretation complete; Mac CapybaraKV post-return cost remains unqualified; owner review |
| Opt-in cache byte budgets | Merged; manual and real idle-maintenance eviction, busy/newest protection and rebuild controls pass on both platforms | Stable archive installation passes; owner rollout |
| CI-cache workflow and consumer skill | Merged; four independent hosted cache jobs and installed consumer-skill controls pass | Release notes and walkthrough prepared; owner rollout |
| Binary compiler-input coverage | Corrected fork merged, both runtimes published/pinned; compiler-returned bytes, source suites, factories and composed controls pass | Both stable factories and archive consumers bind this runtime; owner rollout |
| Core performance changes | Selected core and broad native correctness controls pass; complete six-round workflow and full edit catalogues are captured on both platforms | Classification corrected and all slower cases reviewed; owner acceptance of stated limits; no global parity claim |
| First-unchanged/full-path overhead | Run-scoped and per-file experiments show no general whole-call gain; remain unselected | Original performance goal remains open |
| Mac Anvil cold | Measured 96.87 s stock / 98.94 s Sail in the latest six-round core cohort | Original performance goal remains open |
| Incidental edits | Exact rc.1 archives: full fifteen-edit catalogue plus Anvil controls, 114 samples per platform, raw captures and all execution audits pass | Earlier lost attempt remains indexed; no replacement by different mutation catalogues |
| Crates.io distribution | All eighteen rc.1 versions published; actual anonymous Linux/Mac installs, setup and complete workflows pass | Final-commit stable package verification and owner approval |

Preparation is automatic with `SAIL_BACKGROUND_PREPARE=0` as an opt-out.
Eviction remains disabled until an explicit budget is configured. Unselected
source/per-file digest experiments stay disabled. Completed feature work does
not imply that every original performance goal is solved.

The latest full core measurements cover 23 paired workflow rows in six rotated
blocks plus 972 paired mutation/cold/return steps on each platform. All three
complete catalogues are preserved. The unchanged strict combined reporter
accepts 25 of 26 rows: it incorrectly counts two different recognized
explanations as a duplicate. ADR 0099's qualified per-code correction is merged
in #657. All 1,944 retained paired steps independently recheck Same. The separate
corrected report has 26/26 valid rows on both platforms; original strict failures
and timing bytes remain unchanged. Two Linux and three Mac workflow medians
still require the stated limitation review.
See [feature qualification](../validation/release-expansion.md),
[runtime qualification](../validation/binary-compiler-inputs.md) and
[the merged classification correction](https://github.com/Chelis-Lang/sail/pull/657).

## Work up to the approval boundary

1. Completed: authorized rc.1 crate upload and actual anonymous Linux/Mac
   installations, retaining immutable source/version. Stable source `0.2.0`
   is prepared separately; see [candidate qualification](../validation/stable-candidate.md).
2. Completed: corrected benchmark interpretation and review of every slower row,
   keeping the original strict failures. First/full-path and Mac Anvil gaps
   remain engineering limitations unless an independently qualified fix lands.
3. Completed native qualification: both exact stable archives, full native
   workspace/solver/E2E/fast/standard/deep, all 316 Stock refinements and all
   205 deep mutations. Source assembly uses protected PR #661; final clean
   merged-source packages must bind that merge before requesting approval.
4. Completed: native Linux/Mac archives, runtime/proof/reply indexes, fresh
   installation/repair/replay and recorded checksums. After owner approval
   only, create the exact tag/draft and obtain verified attestations before
   publishing these unchanged archive bytes.
5. Release notes and the qualification/limitation summary are prepared. Finish
   final merged-source package verification, then present the owner with the
   exact version, source, artifact set and stated limitations.

Stop at that boundary. Do not publish the final GitHub release or stable crate
versions without explicit owner approval of that candidate. The separately
authorized initial crate prerelease and ordinary protected PR merges may
continue. A green gate is qualification evidence, not publication permission.

Large frontend reuse, Cargo bypass and new solver-history semantics remain
outside this bounded release scope. No unqualified optimization is promoted
to manufacture a parity claim.
