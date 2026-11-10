# Documentation

Read the [Sail book](https://cproof.ai/docs/sail/) or start with the
[quickstart](quickstart.md). The [reuse guide](reuse.md) explains solver caching,
proof receipts, and replay.

## Using Sail

| Topic | Guide |
|---|---|
| Install a release, check provenance, Rust toolchain requirements | [Installation](install.md) |
| Verify, get complete reports, run background jobs, watch for edits | [Workspace verification](workspace-verification.md) |
| What Current, Historical and Unknown mean | [Freshness](freshness.md) |
| Restore an earlier proof after undoing an edit | [Instant revert](instant-revert.md) |
| Check a proof repair, explain failures, compare saved baselines | [Repair walkthrough](proof-repair-walkthrough.md), [repair reference](proof-repair.md) |
| Connect an agent over MCP | [MCP](mcp.md) |
| Run SMT-LIB transcripts through the pinned Z3 (`sail runtime`) | [SMT runtime](runtime.md), [protocol](spec/runtime.md) |
| Install the agent skill and follow verification/SMT workflows | [Sail skill](../skills/sail-verification/SKILL.md) |
| Read the JSON report | [Output schema](output-schema.md) |
| Read job status and results from Rust | [Result client](result-client.md) |
| Cache Sail in CI | [CI cache](ci-cache.md) |
| Annotate pull requests and write a job summary or SARIF | [CI reports](ci-reports.md) |
| Verify in GitHub Actions with one step | [GitHub Action](github-action.md) |
| Prepare the next verification in the background | [Background preparation](background-preparation.md) |
| Manage disk use | [Disk use and cleanup](workspace-hygiene.md) |
| Where Sail is faster and where it isn't | [Performance](performance.md), [generated benchmarks](benchmarks.md), [memory instrumentation](validation/benchmark-memory.md), [v0.2.0 candidate qualification](validation/v020-performance.md) |
| Evaluate broader projects, dependency scaling and native work counts | [Performance evaluation](performance-evaluation.md), [algorithm review](design/general-performance.md), [compiler boundary audit](design/frontend-lowering-boundary.md) |
| Supported Verus and project setups | [Compatibility](compatibility.md) |
| Trust model and differences from plain Verus | [Trust](trust.md), [divergences](divergences.md) |
| Limitations and open problems | [Known issues](known-issues.md) |
| Error codes | [Errors](errors.md) |
| What changed in a release | [0.2.0 release notes](release-notes/0.2.0.md), [0.1.0 release notes](release-notes/0.1.0.md), [changelog](../CHANGELOG.md) |

Standalone-file development: [verify, watch and MCP](standalone-verification.md)
(available in published 0.2.0; absent from 0.1.0).

## Experimental features

These work but may change in any release, and `sail --help` doesn't list
them.

| Feature | Guide |
|---|---|
| Seed portfolios and fragility measurement | [Divergences](divergences.md) |
| Team caching of signed solver replies over HTTP(S) | [HTTP shared cache](http-shared-cache.md) |
| Requirement assurance from retained jobs | [Requirement assurance](requirement-assurance.md) |
| Temporal jobs (not in release builds; needs a private dependency) | [Temporal jobs](temporal-jobs.md), [private dependencies](private-dependencies.md) |

## Working on Sail

Current ordering: [roadmap](ROADMAP.md),
[next-release mutation feature](design/verus-mutation-release.md),
[ADR 0100](decisions/0100-mutation-first-roadmap.md),
[ADR 0109](decisions/0109-oyster-retired-v030-scope.md) and
[0.2.0 backlog reconciliation](reports/2026-11-04-roadmap-review.md).
Oyster is abandoned and temporal packaging is held. The neutral-contract
surfaces were removed for v0.3.0 ([requirements after Oyster](design/requirements-after-oyster.md)
records what Sail kept and why); other experimental surfaces remain maintained.

- [AGENTS.md](../AGENTS.md): workflow, Rust conventions, gates and
  documentation rules.
- [Charter](CHARTER.md): scope and the correctness contract.
- [Decision records](decisions/README.md).
- [Roadmap](ROADMAP.md).
- [Testing](testing.md), [fault injection](fault-injection.md) and
  [deep Verus checks](deep-verus-tests.md).
- [Native source sync](testing.md): linked-worktree support,
  dirty-input preservation and [qualification](validation/host-source-sync.md).
- [Releasing](releasing.md): release procedure, archive layout, how `sail`
  finds its bundled Verus, and [crate-publication qualification](validation/crates-io.md).
- [Sail's Verus](../engine/verus/README.md): the fork runtime and its
  interactive engine.
- [Managed runs](managed-cold-start.md): what a first run costs and when
  receipts are reused.
- [Shared stream parser](design/checked-stream-parser.md): checked operations,
  [qualification](validation/checked-stream-parser.md) and unproved boundaries.
- [Held solver replies](validation/held-response-ownership.md) and
  [compiler filename tokens](validation/compiler-filename-refinement.md):
  source-shared operations, native adoption and qualification limits.
- [Shared command classifier](design/checked-command-classifier.md): checked shallow
  command/response operations and [qualification](validation/checked-command-classifier.md).
- [Shared cache persistence](design/checked-cache-persistence.md): record framing,
  write sequencing, [qualification](validation/checked-cache-persistence.md) and native limits.
- [Input prefix and delivery](design/checked-input-prefix-delivery.md): owned raw
  input, acknowledged writes and [bounded qualification](validation/checked-input-prefix-delivery.md).
- Validation records for each gate are in [validation/](validation/). Design
  notes are in [design/](design/), milestone reports in [reports/](reports/),
  and research in [survey/](survey/README.md).
- Sail commit identities in these records name commits on `main` wherever an
  equivalent exists. Some records also cite commits from unmerged research,
  evidence and release-candidate branches. Those commits are not part of the
  published history; such a record's identity is the SHA-256 it cites for its
  evidence, archives and runtimes.
- [Verus issues](verus-issues/README.md): local reproductions and workarounds
  for upstream behaviour.
