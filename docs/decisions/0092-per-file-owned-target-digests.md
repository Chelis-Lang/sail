# 0092: Per-file owned-target digest observations

- **Status:** proposed; owner approval required before adoption
- **Date:** 2026-10-31
- **Amends:** [ADR 0091](0091-settled-source-and-target-digests.md), item 2
- **Tracks:** #560, #630

## Context

The selected Linux target memo admits a record only when the entire original
owned target tree, complete source, metadata/selection, context and receipt
bytes match. One unrelated target entry or directory stamp change refuses all
stored digests. The Mac default remains off. A finer byte-observation boundary
could avoid hashing unchanged files when unrelated Cargo bookkeeping changes.
The initial broader run-scoped candidate is mostly flat/slower in its
two-round screen; its composed closing-barrier candidate is still being
measured. The [review](../design/general-performance.md) records native
filesystem/hash diagnostics across three structurally different projects.
There is no measured product gain for this proposal.

This cannot solve the first unchanged call after a cold build: newly created
artifacts were unsettled at the original cold run's start. Waiting on a later
call does not make those original observations eligible. The first call must
still read them afresh; this proposal does not authorize moving that work into
the cold call, synthesizing a transcript or promoting an inherited observation.

## Decision (proposed)

Prototype a separately controlled, local owned-target **content digest** memo.
Keep the current whole-tree memo and its platform defaults unchanged until
independent native qualification. The prototype uses a separate explicit
`SAIL_MANAGED_FILE_TARGET_DIGEST_MEMO=1` switch, off by default on every
platform. No proof revision, receipt or verdict may come directly from the
finer memo. Unknown switch values refuse this reuse.

1. Preserve fresh, complete owned-target namespace and physical-binding
   discovery around every consumer. Both current before/after trees must
   agree, including full stamps. An unreadable, ambiguous, aliased or special
   entry refuses the phase exactly as today. New/deleted names remain visible
   to ordinary cleanup and input discovery.
2. Across calls, a proposed file digest requires the exact absolute lexical
   and physical file binding, regular-file type and complete original
   device/inode/size/mode/mtime/ctime stamp. Every physical ancestor must retain
   its original device/inode/type/mode identity and binding. Directory times
   may differ from the previous call, but complete directory stamps and the
   complete namespace must agree across the **current** phase's barriers.
   This is the proposed trust-boundary change requiring owner approval.
3. Preserve the existing complete source readback, metadata/selection,
   compiler family, context and receipt-byte checks before artifact hash reuse,
   plus the ordinary closing checks. A source, selection, context or receipt
   change refuses this first prototype's whole record. Changed/unsettled files
   take complete fresh reads; dirty packages still clean. This deliberately
   does not introduce per-target source trust or reuse across edits.
4. Seed only actual original reads from successful complete Current calls.
   Each file retains its original Current run id/start, original full stamp
   and digest from equal before/after read stamps. Its ctime must be at least
   two seconds older than that **original** run's start. Inherited entries keep
   their original provenance. A newly eligible entry needs a new qualifying
   fresh read, never a rewritten timestamp or run id.
   A phase-local digest supplied by another memo is not a fresh read. Without
   that memo's qualifying original per-file provenance, read the file afresh
   before seeding a new entry; never relabel an inherited digest as fresh.
5. Pure revision proposals remain governed by the existing whole-tree record
   and closing source barrier. The finer memo supplies only digests for files
   freshly selected by the existing consumer. All logical keys, receipt
   admission, exact solver input bytes and output rules stay unchanged.
6. Use a distinct kind/schema/file from the existing whole-tree record.
   Validate sorted unique complete entries, namespace/binding metadata, strict
   fields and checksum under the existing 8 MiB / 100,000-entry ceilings.
   Missing/damaged/over-limit records take fresh paths. Keep retention bounded
   by the existing invocation contexts. No shared/team import and no non-Unix
   trust are permitted.

## Consequences and qualification

Before any platform default changes, measure native Linux and Mac on/off
whole calls, including publication cost, against previous Sail and stock.
Include the original four projects and the broader held-out projects; keep
cold, first-unchanged, replay, full path, body/spec/dependency edits, failing
edits, fixes and reverts distinct. All refused samples remain failures.

Required controls include same-size/restored-mtime edits; original under-two-
second reads that never age in; new/deleted/unrelated entries; directory
replacement/mode changes; symlink/ancestor retargeting; unreadable/special
files; changed source/receipt/selection/compiler context; damaged records;
retry and non-Unix refusal. Preserve exact Currentness, dirty-clean, native
compiler replacement, output/replay and fault-injection checks. Run ordinary,
solver, complete E2E and fast/standard conformance. Release acceptance retains
its original deep/source-suite, determinism, timing and memory requirements.

Select a platform only for a demonstrated material whole-call gain across the
broader scope. Reject a slower prototype even if its component read benchmark
looks faster. Directory churn is isolated without making a cached byte digest
proof authority. First-after-cold parity and complete frontend reuse remain
separate problems, not implied benefits of this design.
