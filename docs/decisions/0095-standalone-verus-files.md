# 0095: Standalone Verus files through the managed job lifecycle

- **Status:** accepted for implementation (owner-approved release plan, 2026-11-01); native Linux/macOS qualification complete
- **Date:** 2026-11-01
- **Amends:** ADR 0046 for explicit standalone-file invocations

## Context

Issue #164 excludes tutorial files, proof experiments and agent-generated
Verus files from Sail's ordinary workflow. Creating a Cargo package merely
to use Sail is unnecessary friction. The next release includes standalone
verification through CLI, watch and MCP alongside the qualified core fixes.
Existing performance gaps remain visible without serializing all feature work.

## Decision

An explicit first positional `.rs` file selects direct pinned Verus:
`sail verify proof.rs [SAIL OPTIONS] [-- VERUS ARGS]`. No manifest or source
rewrite is generated. Cargo requests retain their existing interpretation.
MCP adds `source_file`, relative to `project_dir`. Jobs explicitly negotiate
standalone support before submitting that execution mode to a daemon.

The standalone runner shares managed runtime identification, compiler-read
observations, output reconstruction and the result/job lifecycle. Its target
kind is `standalone`; its package identity uses a distinct content-addressed
namespace derived from the absolute source path, never a Cargo package ID.
The existing project-result envelope records the invocation directory as
`workspace`, with the original source argument in its requested target.

Only complete successful verify-only proofs may skip execution. Bind the
full invocation and environment, runtime and solver, original compiler-read
checksums and current file contents, source namespaces and symlink bindings.
Read contents freshly; add no cross-call source-stamp shortcut. External
reads bind every ancestor directory up to the observed workspace or filesystem
root: nested inline modules can search intermediate directories that contain
no read source file. Directory entries bind their followed file kind and the
`mod.rs` candidate beneath directory entries, including symlink targets. This
covers both [Rust module layouts](https://doc.rust-lang.org/reference/items/modules.html#module-source-filenames)
and broken alternatives becoming files. The versioned context discards earlier
incomplete records. Unrelated namespace changes can conservatively miss. Bound history
to 16 revisions and 1 GiB, retaining the first successful revision. Checksummed
records preserve the establishing report and exact user output. Fresh checks
with incomplete compiler observations remain non-reusable. Binary includes
whose reported checksum disagrees with actual bytes likewise remain fresh.

Unknown options, custom dependency searches, compilation, output-producing
requests and partial verification execute freshly with the solver cache.
An unsupported observation never prevents ordinary Verus execution. A
cancelled worker cannot publish a successful proof or Current completion.
Interactive repair and solver experiments are outside the initial standalone
surface. Watch and MCP use the same runner and durable jobs as the CLI. Standalone
watch rejects compilation and explicit output-writing options to prevent
self-triggered rebuild loops; the CLI executes those requests freshly. Watch
also requires its source and discovered inputs beneath the invocation directory,
refusing external expansion before subscribing to ancestor namespaces. This
prevents a source read from turning into a recursive whole-filesystem watch.
CLI and MCP verification retain their broader input coverage.

## Consequences

The first feature provides an actual no-Cargo editing workflow, not a second
solver-only execution mode. The baseline observes namespaces conservatively;
large trees or unsupported inputs may prevent receipt reuse. Further cuts
need measured end-to-end gains and preserve all admission checks. Linux/Mac
installed real-file tests, inherited Cargo gates and source-input mutation
controls qualify release selection; implementation alone is not acceptance.
