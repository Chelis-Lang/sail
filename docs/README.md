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
| Reuse complete fresh-frame decoding in a persistent Rust SDK client | [Result client](result-client.md) |
| Consume structured reports and provenance | [Output schema](output-schema.md) |
| Share signed solver replies over HTTP(S) | [HTTP shared cache](http-shared-cache.md) |
| Interpret Current, Historical and Unknown evidence | [Freshness](freshness.md) |
| Understand trust boundaries and opt-in verdict differences | [Trust](trust.md), [divergences](divergences.md) |
| Check supported verifier versions and measured performance | [Compatibility](compatibility.md), [generated benchmarks](benchmarks.md) |
| Find limitations and unresolved failures | [Known issues](known-issues.md) |

## Developing and validating Sail

[AGENTS.md](../AGENTS.md) defines the workflow, Rust conventions, documentation
requirements and gates. The [charter](CHARTER.md) and its corrections define
the correctness contract. [ADRs](decisions/README.md) explain implementation
decisions; proposed candidates are distinguished from accepted decisions.

The [roadmap](ROADMAP.md) tracks implementation and acceptance separately.
The [M7 acceptance guide](bench/m7-repair-acceptance.md) specifies frozen real
projects, unchanged deadlines, source-byte checks and performance gates.
[Fault injection](fault-injection.md) covers solver, cache and daemon failures.
The [reader acceptance guide](reader-acceptance.md) explains genuine proof setup,
owned-job archival, compatible storage layouts and failure handling.
[Whole-result profiling](result-frame-profile.md) records transfer and complete
response decoding costs without changing reader acceptance.
The [source preparation audit](../engine/verus/candidates/decode-budget/README.md)
records allocation ownership, phase bounds and outstanding combined-runtime
checks for the unselected engine candidate.
Its [closed ownership refinement](../engine/verus/candidates/decode-ownership/README.md)
records exact audited routes, retained payload probes and safe budget refusals.
Long corpus, source-build and benchmark work runs locally in the Linux container;
required CI remains short.

Evidence lives in `bench/validation/`, `docs/validation/` and
`engine/verus/qualifications/` or `engine/verus/candidates/qualifications/`.
[The retained M7 status history](validation/m7-acceptance-status-history.md)
keeps earlier failed, interrupted and setup-only scopes out of the current
roadmap. Each record applies to its stated source, runtime and environment.
A later pass does not erase an earlier failure, and a candidate build or static review does
not qualify a runtime. Reports summarize completed milestones:
[bootstrap](reports/2026-10-08-bootstrap.md),
[initial product](reports/2026-10-09-initial-product.md), and
[live workspace](reports/2026-10-10-live-workspace.md).

[Verus issues](verus-issues/README.md) contain local reproductions and workarounds.
[Survey documents](survey/README.md) retain Verus/Z3 protocol, solver behavior,
corpus and prior-art research. Upstream reports are a human decision; agents
record discoveries in this repository.
