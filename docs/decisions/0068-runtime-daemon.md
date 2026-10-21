# 0068: Shared runtime transport and agent tools

- **Status:** implemented, acceptance qualification in progress
- **Date:** 2026-10-21

## Context

Raw SMT execution needs one shared service for Rust, CLI and MCP clients.
The existing cache and Cargo-job protocols have different responsibilities;
adding solver jobs to either would conflate process lifecycle and proof state.

## Decision

Expose the same public runtime request and response through a separate framed
Unix endpoint. Bind transport version, active invocation identity, exact query
content, limits, caller attribution and negotiated backend identity. Keep the
API crate independent of execution. The daemon depends on `sail-runtime`; the
MCP crate connects through `sail-api`, without linking a solver executor.

Require owner-private filesystem permissions, bounded input/output frames,
absolute I/O deadlines and bounded client/execution admission. Cancellation
uses a separate control connection. Active registration lasts through response
delivery so the existing daemon idle guard cannot stop a published invocation.
Disconnect and shutdown cancel active work and join owned workers.

CLI `--socket` connects explicitly without autostart or fallback. The existing
embedded CLI path remains available without that option. MCP autostarts the
configured Sail daemon only when no cache daemon is running; an older live
daemon without the runtime endpoint requires restart. Both tools expose actual
readiness and exact results; unknown or incomplete execution is never proof.

## Validation and limits

Transport controls test actual endpoints, framing, private permissions,
duplicate identities, active limits, separate cancellation and blocked response
delivery. CLI/MCP tests drive the actual stdio protocol and daemon startup.
Unavailable controls never fabricate a solver answer; pinned-solver tests
separately compare RPC, CLI, MCP and embedded execution with original evidence
and shared-cache reuse. Native host and existing conformance gates remain
required. The raw SMT route does not establish Rust-code assurance. These
admission bounds are not hard aggregate RAM enforcement.
