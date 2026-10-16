# Jobs, watch and MCP without proof reuse

| Field | Evidence |
|---|---|
| Status | Required native gates pass on the preceding source; post-#190 integration, full fast/standard conformance and final CI remain pending |
| Source | Required native checks: `b574e393`, based on main `5de8819e`; current integration is based on #190 `c96fdbec`. Earlier macOS checks cover the same feature/review patches at `b8542f03` |
| Runtime | Pinned fork `73dea021`, Z3 4.16.0; stock oracle `0.2026.10.04.426d8b0` |
| Environment | Dedicated native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, CI image; macOS non-solver host checks |
| Scope | Unsupported-setup fallback in jobs, watch and blocking/asynchronous MCP; unbound evidence comparison and repair baselines; existing default-mode corpus |
| Limits | No performance claim, release qualification or whole-corpus managed-workflow acceptance; larger conformance tiers remain required before merge |

The native workspace gate passes 634 tests, with 35 solver tests skipped;
the separate solver step passes all 35. Every end-to-end phase and local
`extra` passes. PR conformance is 33/33 Same (311 functions) in each of four
arms, with zero differences, instability, infrastructure errors or skipped
projects. These corpus checks cover the fork/shim/engine behavior, while the
end-to-end and CLI tests cover the project-result and fallback lifecycles.

The post-#190 rebase keeps both feature/review patches unchanged; its only
conflict was the changelog, where both entries were retained. Native results
below qualify `b574e393`, not this later joined source. Integration checks
remain pending before merge.

Regression controls retain exact output/exit behavior, mark unbound runs
Unknown with `no_reuse`, prevent their use as repair baselines, and keep
cross-setup comparisons incomparable even when saved context strings match.
Successful Cargo runs without verifier results remain NO RESULT in MCP.
Ordinary bound results retain the existing comparison behavior.

All 503 tracked-file hashes were verified before and after native checks;
only deliberately excluded local agent settings differ. Affected internal
Cargo packages were cleaned and Rust/Cargo source mtimes refreshed before
building, so inherited artifacts cannot qualify a different source tree.

Native evidence archive: `fallback-parity-native-evidence.tgz`, SHA-256
`d2b40f1adaf2c6113fd526ea0133b05248150f6deaf897e62f4cd3efc4f1d638`.
Retained Linux binaries archive: SHA-256
`764b5053654bf12779f5f87205c5d42f0e0a10c5e71bfff141ae83d317864c14`.
Raw files remain outside git.

The earlier macOS host run passes clippy and 618 tests, with 35 solver tests
skipped. Its feature/review patches are unchanged by integration; the only
rebase conflict was the changelog, where both entries were retained. This
does not claim a second macOS run on the joined source. Mac evidence archive:
`mac-review-evidence.tgz`, SHA-256
`b91086f26c50829be6e5c575666980dd0a94f7668c55ac7ed5d260967f520c10`.

Earlier attempt: the first macOS review check stopped on an
`assert_is_empty` lint in the new comparison regression. The empty/nonempty
assertions were rewritten without changing their expectations; the rerun
above passes. Both attempts remain in the Mac archive.
