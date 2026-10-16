# Dependency-store displayed output (#184)

| Field | Value |
|---|---|
| Status | Defect reproduced on rc.5; correction passes the live macOS reproduction and host gate, native Linux gates and full fast/standard conformance. Post-#183/#186 rebase native and Mac gates and live stdout checks pass; required CI remains pending |
| Defective source/runtime | Sail `c3687990a8139b0f0453c6ef574b3787aded6e92`; Verus fork `73dea02112d730e7ab52f58158012d440fb89512`; exact draft rc.5 macOS archive SHA-256 `303e0750a8c2d7ea48d7a149f10608cd68ff00d7d67b4dd9cf2db58880b5d309` |
| Oracle | Stock Verus `0.2026.10.04.426d8b0`, Z3 4.16.0 |
| Environment | macOS arm64, Apple M3 under other work; no performance qualification claimed |
| Reproduction | Two new projects with identical manifests and lockfiles, one fresh Sail cache; second run uses the dependency store; stock uses a fresh target |
| Observed failure | Second Sail stdout omits vstd's result line; source report confirms reuse. Exit 0 and verdicts agree |
| Cause | The store saves captured output but its seeding and relay paths never restore it |
| Correction | Record exact rendered stdout, validate it and stderr before seeding and restore it once on the matching fresh Cargo artifact event. Existing targets remain silent; damaged/missing streams verify normally |
| Regression coverage | Stream damage/missing entry, ordinary attachment silence, artifact match/duplicate/rebuild filtering, nested verified roots, managed plain-copy output and unchanged silence |
| Evidence | `rc5-store-output-repro.tgz`, SHA-256 `2623e35ddd10876b5a673e2157ca11ec1741a33ae8542092ded5a10f0d363bbb`, retained outside git |
| Corrected candidate | Sail `2f7102c4`, version `0.1.0-store-output-candidate`, unchanged fork runtime `73dea021`; `ci/run.sh host`: 615 passed, 35 solver-dependent tests skipped |
| Live correction | Second project stdout equals fresh stock, including vstd's summary; diagnostic stderr is empty on both sides after excluding Cargo progress; all exit codes are 0. Next unchanged Sail run prints no stdout |
| Correction evidence | `fixed-store-output-2f7102c4.tgz`, SHA-256 `487349307a9a4e384e9381fdaead48a4bb391743658f0d42e8ba526cd9ffa3e5`, retained outside git |

The [minimal reproduction](../../corpus/repros/dependency-store-stdout/README.md)
uses only generated Sail projects. No performance claim or whole-corpus
acceptance follows from fixture tests.

## Attempts

- 2026-10-16, rc.5 `c3687990`, macOS: reproduced missing vstd stdout on a
  second new project; both Sail runs and plain stock exited 0.
- 2026-10-16, shipped-vstd candidate `c98b3828`, macOS: eight acceptance
  cases passed; the added store-copy case failed stdout identity while its
  proof origin, exit code and stderr matched. This prompted the separate fix.
- 2026-10-16, correction `2f7102c4`, macOS: the same two-project reproduction
  matches stock stdout and diagnostic stderr; the following unchanged run is
  silent. The host gate passes; native Linux gates remain pending.

## Native Linux qualification

Source `e2a3789784a269591184809d64ddb451b078811a` ran on a dedicated
EC2 c7i.8xlarge in us-east-1, Ubuntu 24.04, using the pinned fork `73dea021`
and stock `0.2026.10.04.426d8b0` oracle with Z3 4.16.0. A 503-file
source manifest matched the host apart from the excluded local agent settings.
Changed Cargo packages were cleaned and source mtimes forced before building;
the actual Sail, shim and conformance binary checksums are retained.

- Workspace gate: 626 tests; solver suite passes.
- All seven end-to-end phases and local `extra` pass, including plain
  store-copy stdout and unchanged-run silence.
- PR conformance: 33/33 Same in all four arms.
- Full fast: 147/147 Same, 3,450 functions in all four arms.
- Standard: 159/159 Same, 12,726 functions in all four arms. The pass-through
  arm records one rlimit-only drift; every other arm records zero drift.
- No Different, Unstable, infrastructure errors or skipped projects in any arm.

Raw evidence archive SHA-256:
`a8841ca77f5629ceabdc0fe4fa1f66c8a4346b21cbf7d87e5f695f3180c132ed`.
The separately retained Linux binaries archive has SHA-256
`f9b6ec0983d0bce5e01cf2ed8948076fc53396c3dcced5cc14986068b2f5d26a`.
No performance acceptance or rc.5 asset requalification follows from these
correctness checks.

The final branch is rebased onto #183 (`ba2220b9`). Its conflict resolution
keeps #183's resolved foreign-node identity and selection-free family key,
and this correction's strict refusal of nested verified roots. The complete
native checks above ran before that rebase. The exact rebased source
`6583c2c36a096786044a6e021d2da35ce3658317` then passes the native
workspace gate (629 tests, 35 solver-dependent skips), a rebuilt live donor
and second-project store hit with byte-identical stock stdout, and following
unchanged-run silence. All 503 tracked source hashes match before and after.
The matching Linux Sail binary SHA-256 is
`68d18819a75d6ee296ffc9fdb07dc2b7731c167b83134348d4e05cdcd9008b70`;
the shim is `f01ba8431c6795b9e091dec9b0acfe48270b8fb6ecdb80d24db994e8d24e1177`.
The same source passes the full macOS host gate: 618 tests, 35 skipped,
with all 503 source hashes unchanged; host log SHA-256
`1c0d8a8058c687fa5425332a37090a095eddf3ad6867e14fe07f7471f8129606`.

Final native delta evidence archive SHA-256:
`7f0f7fede16ab8762dc66c991b4bf034a9dc6d3d873063da59fd849624d4739d`;
matching Linux binaries archive SHA-256:
`43da2e8c00e6b875f1a8489d086249128006a47e99e9f46043fc217963968aa5`.
Required CI must pass before landing.

The first complete PR CI run at `4470d816` passes every required check;
its retained run summary has SHA-256
`325d2437d3eaf931f8a8c66998efef0a25f708e376bb9ca907379d03b0194402`.
After #186 merged, this correction rebased onto `5de8819e` as
`cfa4d96557b15c157c031bb986d3e9c5c0662222`. Both changelog entries remain;
#186's existing-store hold optimization, #183's resolved dependency bindings
and this correction's displayed-output/nested-root rules remain intact.

That exact combined source passes the native workspace gate (631 tests,
35 skipped) and the same rebuilt live strict store-hit, stock-stdout and
unchanged-silence probe. All 504 tracked source hashes match before and after.
Linux Sail binary SHA-256:
`13157c6b9d5c1feef585c3aa6309ca9a7b3bd4b614935ffffb926b81d02a626a`.
The native joined-source evidence archive SHA-256 is
`789e84827d7456a3e3f4583dde615832c42035e1cb718e392148d7fdbd820e01`;
the matching binaries archive is
`10bc0c6011e5b5d2530da5faebd500573c36f189922da0fc7f9fd1da463c24eb`.
The exact combined source also passes the full Mac host check (620 tests,
35 skipped), with all 504 source hashes unchanged; log SHA-256
`3019a2daa8d950fa83c91a31207753be8c86c44f93e39b35c0dfdf32a47afcaf`.
The full fast/standard corpus results above remain explicitly pre-rebase;
no full-tier repetition or performance claim follows from this delta check.
The rebased PR must pass required CI before landing.
