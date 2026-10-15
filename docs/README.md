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

[M7 validation](validation/m7.md) gives each M7 acceptance gate's status, best
evidence and indexed failed or interrupted attempts, and the
[deep-tier record](validation/deep-tier.md) gives the release-time deep
conformance, `rust_verify_test` and Singular runs, the
[HTTP shared-cache record](validation/http-shared-cache.md) gives that
feature's tests and integration evidence, and the
[macOS engine record](validation/macos-engine.md) qualifies the engine runtime
bundled in the macOS release archive, and the
[clean macOS record](validation/clean-macos.md) checks the release candidate
on a fresh Mac, and the [fork-default record](validation/fork-default.md)
compares Sail on its Verus with plain stock Verus. Raw logs stay outside the
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
to planning.

[Verus issues](verus-issues/README.md) contain local reproductions and workarounds.
[Survey documents](survey/README.md) retain Verus/Z3 protocol, solver behavior,
corpus and prior-art research. Upstream reports are a human decision; agents
record discoveries in this repository.
