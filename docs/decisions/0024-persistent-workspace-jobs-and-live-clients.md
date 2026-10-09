# 0024: Persistent workspace jobs and live clients

- **Status:** accepted
- **Date:** 2026-10-09

## Context

Watch previously supervised an independent verifier process and polled file size
and modification time under the current directory. It missed external path
dependencies and same-size edits. MCP lookups acquired the same project lock as
verification, so an agent could not read its previous results while work ran.
Cargo freshness alone does not establish proof freshness (VI-0005).

ADR 0023 introduces managed target artifacts and complete proof receipts. A
shared local job service makes that runner available to the CLI, watch and MCP
without requiring a server deployment or credentials.

## Decision

The daemon hosts a separate version-1 project endpoint alongside its unchanged
version-4 SMT-cache endpoint. Requests and responses are bounded JSON frames;
every RPC has I/O deadlines. Status and output readers hold no verification
lock. One FIFO worker executes managed verification, preserving each submitting
client's environment rather than inheriting the daemon's first client's context.
Non-UTF-8 environments are explicitly rejected instead of transformed.

Job records, complete stdout/stderr, and target events live in private directories
under `projects/jobs`. Records are atomically replaced before acknowledgement.
The socket and files are mode 0600 and directories 0700. Identical in-flight
requests with a known revision coalesce; requests without a known revision do
not. Lifecycle is distinct from proof verdict. Completed reports retain their
establishing run, scope, solver/configuration identity and input revision.

A queued or running job keeps the daemon alive. Cancellation kills the entire
worker process group. A parent pipe terminates orphaned workers when the daemon
dies. Explicit shutdown cancels work; records interrupted by a crash become
infrastructure errors on restart. The last 20 terminal jobs per workspace are
retained; proof receipts live separately and survive history pruning.

Completed result/status reads revalidate the runner's persisted declared-input
manifest and environment outside the job-state lock. Missing coverage becomes
unknown currentness; changed revisions become historical. Listing jobs reports
unvalidated completion currentness as unknown. No prior result is presented as
current merely because the local Rust files match.

`watch` consumes filesystem events across Cargo workspace/path-dependency roots,
lockfiles, inherited configuration and toolchain files. It debounces for 300 ms,
supersedes active jobs, and schedules the latest edit. JSONL reports lifecycle
and target completion; human output follows diagnostics. Watch never infers a
semantic function dependency graph.

MCP gains asynchronous start/status/result/cancel tools. Default blocking verify
uses a forced-fresh job and preserves its report-v1 response, selecting only
requested fresh targets. Explicit solver experiments retain their existing
paths. Cached-result and counterexample tools inspect completed jobs while a
newer job runs; older local-only snapshots are historical evidence with unknown
complete-context freshness.

## Consequences

Single-user CLI usage auto-starts the local service and needs no credentials.
The default cache socket includes a workspace capability suffix; an explicitly
selected compatible older cache daemon can continue running with a jobs-only
sidecar. SMT keys, cache entry formats, unknown policy and solver verdicts are
unchanged.

The worker limit is deliberately one. Parallelism, remote job execution and
semantic dependency scheduling require separate evidence and decisions. Job
output is retained verbatim on disk; RPCs reject oversized reports rather than
truncating proof evidence, and diagnostics can be read in bounded slices.
