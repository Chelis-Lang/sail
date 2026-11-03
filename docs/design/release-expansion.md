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
| Incidental edits | Corrected fifteen-edit catalogue and Anvil execution audits pass | Its original raw timing loss is not repaired by the different full mutation catalogues |
| Crates.io distribution | All eighteen source packages and native Linux/Mac installed controls pass; authorized rc.1 upload is progressing | All public versions and actual anonymous registry installs on both platforms |

Preparation is automatic with `SAIL_BACKGROUND_PREPARE=0` as an opt-out.
Eviction remains disabled until an explicit budget is configured. Unselected
source/per-file digest experiments stay disabled. Completed feature work does
not imply that every original performance goal is solved.

The latest full core measurements cover 23 paired workflow rows in six rotated
blocks plus 972 paired mutation/cold/return steps on each platform. All three
complete catalogues are preserved. The unchanged strict combined reporter
accepts 25 of 26 rows: CapybaraKV's row remains invalid because it counts two
different recognized explanations as a duplicate. Proposed ADR 0099 and its
qualified fix remain pending owner adoption; original failures are unchanged.
See [feature qualification](../validation/release-expansion.md),
[runtime qualification](../validation/binary-compiler-inputs.md) and
[the classification proposal](https://github.com/Chelis-Lang/sail/pull/657).

## Work up to the approval boundary

1. Complete the already-authorized rc.1 crate upload and actual anonymous
   Linux/Mac installations. Keep its immutable source/version unchanged.
2. Resolve the benchmark-classification proposal and review every slower row,
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
