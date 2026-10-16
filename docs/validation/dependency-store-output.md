# Dependency-store displayed output (#184)

| Field | Value |
|---|---|
| Status | Defect reproduced on rc.5; candidate correction passes focused unit tests and clippy. Live correction and native Linux gates pending |
| Defective source/runtime | Sail `c3687990a8139b0f0453c6ef574b3787aded6e92`; Verus fork `73dea02112d730e7ab52f58158012d440fb89512`; exact draft rc.5 macOS archive SHA-256 `303e0750a8c2d7ea48d7a149f10608cd68ff00d7d67b4dd9cf2db58880b5d309` |
| Oracle | Stock Verus `0.2026.10.04.426d8b0`, Z3 4.16.0 |
| Environment | macOS arm64, Apple M3 under other work; no performance qualification claimed |
| Reproduction | Two new projects with identical manifests and lockfiles, one fresh Sail cache; second run uses the dependency store; stock uses a fresh target |
| Observed failure | Second Sail stdout omits vstd's result line; source report confirms reuse. Exit 0 and verdicts agree |
| Cause | The store saves captured output but its seeding and relay paths never restore it |
| Correction | Record exact rendered stdout, validate it and stderr before seeding and restore it once on the matching fresh Cargo artifact event. Existing targets remain silent; damaged/missing streams verify normally |
| Regression coverage | Stream damage/missing entry, ordinary attachment silence, artifact match/duplicate/rebuild filtering, nested verified roots, managed plain-copy output and unchanged silence |
| Evidence | `rc5-store-output-repro.tgz`, SHA-256 `2623e35ddd10876b5a673e2157ca11ec1741a33ae8542092ded5a10f0d363bbb`, retained outside git |

The [minimal reproduction](../../corpus/repros/dependency-store-stdout/README.md)
uses only generated Sail projects. No performance claim or whole-corpus
acceptance follows from fixture tests.

## Attempts

- 2026-10-16, rc.5 `c3687990`, macOS: reproduced missing vstd stdout on a
  second new project; both Sail runs and plain stock exited 0.
- 2026-10-16, shipped-vstd candidate `c98b3828`, macOS: eight acceptance
  cases passed; the added store-copy case failed stdout identity while its
  proof origin, exit code and stderr matched. This prompted the separate fix.
