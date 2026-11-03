# Next-release implementation package

Owner-selected scope, 2026-11-01; readiness reviewed 2026-11-03.
The release gives users a direct Verus-file workflow: verify and repair a file,
keep durable jobs/watch/MCP, prepare subsequent verification in the background,
and share CI caches while bounding local cache storage. Cargo installation is
an additional distribution channel. Final publication is not approved.

## Scope and current state

| Slice | Implemented / qualified | Remaining |
|---|---|---|
| Standalone verification and repair | Merged; native Linux/Mac CLI, jobs, watch, MCP, focus/full and negative controls pass | Exact final archive/install qualification |
| Daemon and new-project preparation | Merged; foreground priority, generated inputs, closing revisions, cancellation and queued-origin controls pass | Complete performance interpretation and candidate packaging |
| Opt-in cache byte budgets | Merged; manual and real idle-maintenance eviction, busy/newest protection and rebuild controls pass on both platforms | Final candidate installation |
| CI-cache workflow and consumer skill | Merged; four independent hosted cache jobs and installed consumer-skill controls pass | Final release notes and walkthrough |
| Binary compiler-input coverage | Corrected fork merged, both runtimes published/pinned; compiler-returned bytes, source suites, factories and composed controls pass | Bind final candidate artifacts to this qualified runtime |
| Core performance changes | Selected core and broad native correctness controls pass; complete six-round workflow and full edit catalogues are captured on both platforms | Benchmark-note classification and review of slower cases; no global parity claim |
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
in #657. Complete retained-output rechecking and a separate corrected report
are in progress; original failures and timing bytes remain unchanged.
See [feature qualification](../validation/release-expansion.md),
[runtime qualification](../validation/binary-compiler-inputs.md) and
[the classification proposal](https://github.com/Chelis-Lang/sail/pull/657).

## Work up to the approval boundary

1. Completed: authorized rc.1 crate upload and actual anonymous Linux/Mac
   installations, retaining immutable source/version. Stable source `0.2.0`
   is prepared separately; see [candidate qualification](../validation/stable-candidate.md).
2. Complete corrected benchmark interpretation and review every slower row,
   keeping the original strict failures. First/full-path and Mac Anvil gaps
   remain engineering limitations unless an independently qualified fix lands.
3. Freeze the final candidate source and scope. Map existing results to their
   actual tested inputs; run uncovered candidate-specific checks rather than
   relabeling earlier binaries or treating static review as execution.
4. Build and stage the exact candidate archives on native Linux and Mac,
   validate shipped runtime/proof/reply indexes, exercise fresh installation
   and archive smoke, and prepare draft attestations and checksums.
5. Finish user-facing release notes and the qualification/limitation summary.
   Present the owner with one exact version, source commit, artifact set and
   explicit remaining limitations.

Stop at that boundary. Do not publish the final GitHub release or stable crate
versions without explicit owner approval of that candidate. The separately
authorized initial crate prerelease and ordinary protected PR merges may
continue. A green gate is qualification evidence, not publication permission.

Large frontend reuse, Cargo bypass and new solver-history semantics remain
outside this bounded release scope. No unqualified optimization is promoted
to manufacture a parity claim.
