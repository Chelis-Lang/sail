# Testing Sail

The complete gate and testing tiers are defined in [AGENTS.md](../AGENTS.md).
Run Linux checks on a native EC2 host with `ci/local.sh`; use `ci/run.sh host`
for the macOS platform check. Performance measurements require a quiet native
host, separate from builds and gates.

## End-to-end phases

CI builds the release binaries once, then runs nine independent jobs. Each
job retains the existing ten-minute hard limit. The required `End to end`
aggregate accepts only nine successful results: failure, cancellation and a
skipped job all fail the aggregate.

| `ci/local.sh e2e PHASE` | Scope | Image |
|---|---|---|
| `legacy` | New projects, cold/warm output parity, JSON, MCP and portfolio | core |
| `output` | Edit-loop output, compile errors and partial verification | core |
| `flags` | Cargo/Verus invocation flags | core |
| `loader` | Loader-variable fallback | core |
| `managed` | Failing-first vstd reuse, complete receipts, jobs/MCP, missing evidence, edits, prune and Cargo metadata | mcp |
| `managed-copies` | Own passing donor, two fresh workspace-member proofs, Current adoption and each project's own receipt reuse | core |
| `managed-native` | Own passing donor, fresh plain summaries, unchanged silence and same-PATH compiler replacement against Sail and stock | core |
| `scripts` | Generated includes, build scripts and declared environment inputs | core |
| `repair` | Repair CLI/MCP against the pinned prebuilt engine | core |

The two fresh-project phases each own their workspace, target directories,
daemon, private compiler wrapper and passing donor. They share no receipts or
compiled artifacts with other jobs. Their requested-target and vstd verdict,
cardinality, Current and publication-refusal assertions remain required.
Only `managed` invokes the real external Claude Code tool-listing check;
the ordinary MCP checks remain there too.

`ci/local.sh e2e` runs the complete union, including both extracted fresh-project
groups once using the already established managed donor, then the local-only
`extra` flag/workspace cases. Validate standalone `managed-copies` and
`managed-native` as well: their independent donor setup is an additional entry
path. No phase or assertion can be omitted to authorize a merge.

## Managed timeout evidence

The managed fixture writes an operation record and a flushed progress line
before each bounded verification starts, then records its exit and elapsed
time. Verifications retain their 240-second child limit; the independent failing
stock oracle retains 300 seconds. These limits are unchanged by sharding.
Partial producer output is drained after owned process groups are killed.

Managed jobs request bounded fixture evidence on failure or cancellation.
GitHub's hard job timeout can bypass that upload step, so a missing artifact
does not locate the blocked command or establish a product failure. The original
containment cancellation and the revised layout's pending native scope are in
the [containment record](validation/dependency-store-fail-closed.md).
