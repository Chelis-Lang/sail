# 0136: Return the actual optional-retention decline stage

- **Status:** implemented; native Linux integration and focused review pass
- **Date:** 2026-11-14
- **Tracking:** #712 package C, #595

## Context

Optional retained-proof plan admission returns an Option. Some checks explain
why they decline; malformed metadata, unsupported target kinds, inaccessible
storage, unreadable tools and unavailable caller-environment observations can
silently return None. The retained spike records the last named step in a
thread-local string. That would make a later explanation depend on mutable
ambient state and could confuse separate operations.

## Decision

Plan admission returns a private typed decline from the actual failing check.
The existing caller records its SAIL3146 explanation and takes the same normal
compiler fallback. Existing profile and unqualified-manifest notes retain their
text and codes. No observer state is consulted by an admission check.

Selection, check order, runtime identity, filesystem operations, hashes and
source/proof authority are unchanged. Unrequested retention produces no new
note. Human output continues to show SAIL3146 only with SAIL_EXPLAIN=1; JSON
retains the explanation. This does not introduce a cost threshold or recovery
policy, refresh a missing export or infer why an I/O operation failed.

This first slice covers plan admission. Typed restore/publication work is
separate package C work; the missing build-script-export explanation already
landed in #708. The prototype's unbounded publication journal is not adopted.

## Consequences

A caller can distinguish a missing metadata field from a rejected runtime or
failed storage preparation without guessing from a later filesystem state.
A refusal remains a refusal. Focused controls, complete local gates, original
retained-reuse acceptance and independent review must pass before merge.
No dependency, selected runtime or platform-specific operation changes.
