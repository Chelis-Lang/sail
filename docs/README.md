# Documentation

Start with the repository [README](../README.md) for installation and ordinary
verification. Guides describe behavior on the branch that contains them;
experimental features and validation limits are identified explicitly.

## Using Sail

| Task | Guide |
|---|---|
| Verify, reuse complete reports, watch edits, inspect and cancel jobs | [Workspace verification](workspace-verification.md) |
| Check one repair, follow proof events, explain failures and compare saved baselines | [Walkthrough](proof-repair-walkthrough.md), [repair contract](proof-repair.md) — M7 remains experimental |
| Integrate an agent through stdio MCP | [MCP tools](mcp.md) |
| Read job Status and Results from Rust | [Result client](result-client.md) |
| Consume structured reports and provenance | [Output schema](output-schema.md) |
| Share signed solver replies over HTTP(S) | [HTTP shared cache](http-shared-cache.md) |
| Interpret Current, Historical and Unknown evidence | [Freshness](freshness.md) |
| Understand trust boundaries and opt-in verdict differences | [Trust](trust.md), [divergences](divergences.md) |
| See what Sail speeds up and what it does not, with reproducible runs | [Performance](performance.md) |
| Check supported verifier versions and workload benchmarks | [Compatibility](compatibility.md), [generated benchmarks](benchmarks.md) |
| Find limitations and unresolved failures | [Known issues](known-issues.md) |
| Look up an error code | [Error codes](errors.md) |

## Developing and validating Sail

[AGENTS.md](../AGENTS.md) defines the workflow, Rust conventions, documentation
requirements and gates. The [charter](CHARTER.md) and its corrections define
the correctness contract. [ADRs](decisions/README.md) explain implementation
decisions; proposed candidates are distinguished from accepted decisions.

The [roadmap](ROADMAP.md) tracks implementation and acceptance separately.
The [M7 acceptance guide](bench/m7-repair-acceptance.md) specifies frozen real
projects, unchanged deadlines, source-byte checks and performance gates.
[Fault injection](fault-injection.md) covers solver, cache and daemon failures.
[Socket fixture retirement](validation/mcp-fixture-retirement.md) records the
MCP test-server cancellation regression and its remaining validation scope.
[Solver-continuation findings](reports/2026-10-16-solver-snapshots.md) and their
[qualification record](validation/solver-snapshots.md) preserve the blocked
exactness experiment; no continuation implementation is selected.
[Releasing](releasing.md) gives the release procedure, the archive names and
layout, and how `sail` finds the bundled engine.
[Managed runs](managed-cold-start.md) explains what the first managed run costs
and when receipts are reused.
The [reader acceptance guide](reader-acceptance.md) gives the native reader
latency gate and how to run it.
[Sail's Verus](../engine/verus/README.md) describes the fork runtime every
verification runs ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)),
its interactive engine, the prebuilt artifact and the runtime identity. Its checks are in
the [M7 validation record](validation/m7.md) (Gate 1). Long corpus, source-build and benchmark work
runs locally, and required CI stays short. Reference performance numbers come
from a native x86-64 Linux host (`scripts/perf-host.sh`).
The [CI image startup record](validation/ci-image-startup.md) describes the
shared core/MCP packaging targets and their measured acceptance status.

[M7 validation](validation/m7.md) gives each M7 acceptance gate's status, best
evidence and indexed failed or interrupted attempts, and the
[deep-tier record](validation/deep-tier.md) gives the release-time deep
conformance, `rust_verify_test` and Singular runs, the
[HTTP shared-cache record](validation/http-shared-cache.md) gives that
feature's tests and integration evidence, and the
[macOS engine record](validation/macos-engine.md) qualifies the engine runtime
bundled in the macOS release archive, and the
[clean macOS record](validation/clean-macos.md) checks the release candidate
on a fresh Mac, the [fork-default record](validation/fork-default.md)
compares Sail on its Verus with plain stock Verus, and the
[determinism record](validation/determinism.md) measures the fork's Verus for
run-to-run differences, and the
[dependency store record](validation/dependency-store.md) checks new projects
that start from a stored vstd against plain stock Verus. The
[incidental-edit record](validation/incidental-edits.md) tracks the frozen
three-project edit catalogue and the combined runtime's remaining gates.
Raw logs stay outside the
repository and are cited by SHA-256. Earlier per-run records and unselected
engine candidates are preserved in the `archive/m7-contracts-f7103bf` tag.
Each record applies to its stated source, runtime and environment. A later pass
does not erase an earlier failure, and a candidate build or static review does
not qualify a runtime. Reports summarize completed milestones:
[bootstrap](reports/2026-10-08-bootstrap.md),
[initial product](reports/2026-10-09-initial-product.md), and
[live workspace](reports/2026-10-10-live-workspace.md). ADR 0038 records the
M7 rescue's decisions; the rescue review and lane reports are kept in the tag
`archive/rescue-m7-98f6292`. The
[new-user walkthrough](reports/2026-10-13-new-user-walkthrough.md) and
[roadmap review](reports/2026-10-13-roadmap-review.md) are the current inputs
to planning. The [verification runtime scope](scope/verification-runtime.md)
records the owner's direction for releases after 0.1.0.

[Verus issues](verus-issues/README.md) contain local reproductions and workarounds.
[Survey documents](survey/README.md) retain Verus/Z3 protocol, solver behavior,
corpus and prior-art research. Upstream reports are a human decision; agents
record discoveries in this repository.

[Dependency-store stdout validation](validation/dependency-store-output.md) records the rc.5 output defect and its correction.

[Native source-sync validation](validation/host-source-sync.md) records source
content and timestamp controls for the EC2 host helpers.

The [Cargo target-context record](validation/cargo-target-context.md) covers
owned metadata selection, source-directory coverage, and declared environment
reads (#166 and #189), with native full fast/standard and separate joined
main checks.
