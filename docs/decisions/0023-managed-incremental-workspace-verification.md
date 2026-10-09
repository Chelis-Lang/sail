# 0023: Managed incremental workspace verification

- **Status:** accepted
- **Date:** 2026-10-09

## Decision

Explicit `sail verify --incremental` always invokes Cargo and owns its artifact
and receipt directories under Sail's cache. It never cleans or rewrites the
caller's target directory. Private `sail-cargo` and `sail-verus-driver` aliases
reuse the installed Sail executable; no additional binary installation is needed.
The Cargo adapter forwards metadata unchanged, replaces the Cargo-Verus wrapper
only for builds/checks, and records Cargo artifact messages. The driver forwards
original argument bytes and leaves compiler probes, build scripts, and ordinary
Rust dependencies alone. Each verification target gets isolated JSON, AIR,
SMT-transcript, and shim-provenance sidecars, fixing the shared-log limitation of
VI-0004 without changing solver input.

A fresh Cargo artifact is insufficient proof evidence (VI-0005). Report reuse
requires a complete successful receipt, a checksum over that receipt, matching
Cargo package/target/kind, the observed artifact paths, and content hashes of
artifacts including Verus `.vir` files. The receipt context binds ordered flags,
exact environment bytes, toolchain configuration, actual Cargo/Rust driver and
Verus binaries, Sail, the shim, and Z3. Both Cargo's compiler and the verifier's
actual Rust runtime are identified: the adjacent Verus launcher reports its active
toolchain through `--version --output-json`. All installed sysroot target libraries
are content-bound and watched, including implicit standard libraries. Phase-local
hash deduplication avoids rereading executable aliases; it never carries hashes
across observations or trusts timestamps. Custom sysroots and JSON target
specifications fail explicitly because v1 cannot establish their complete runtime
identity.
Missing or invalid receipts trigger a targeted clean of Sail-owned artifacts and
fresh verification. Failed, cancelled, scoped, and incomplete results are never
whole-target reuse receipts. Explicit force-fresh runs establish reports but do
not authorize later automatic reuse.

Input revisions hash workspace files, lockfiles, manifests, Cargo configuration,
resolved local, registry and git dependency contents, and declared build-script
paths. Only Cargo's actual output directory and Sail's managed cache are excluded;
a source directory named `target` is still an input. Build-script output
`rerun-if-changed` paths and `rerun-if-env-changed` values are recorded separately;
content changes force the script package to refresh even when size and mtime are
restored. Without explicit rerun declarations, the package directory is hashed,
matching Cargo's default declared input contract. Native filesystem observation
also invalidates work when an edit is restored during a running proof. Canonical
external symlink targets are watched, and compiler dep-info records external
`include!` inputs for subsequent snapshots. Newly discovered external compiler
inputs make the establishing run historical until a fresh, fully observed run;
unreadable dep-info gives unknown currentness. Lost input
coverage gives unknown currentness; changed inputs give historical results and
invalidate newly established reuse receipts. Undeclared external build-script
side effects remain outside Cargo's input contract and cannot authorize reuse.

Managed output is `sail-project-result` envelope version 1. Embedded verifier
reports retain schema version 1 and their original counters and provenance.
Every target identifies the invocation that established its report and whether
it was reused. Current invocation counters are separate. Declared input state is
persisted so job status can check freshness without starting Cargo or blocking
behind a verifier worker. A `context-ready` event exposes an established runtime
and declaration context for job coalescing, without invoking Cargo during job
submission. Cold contexts cannot coalesce until compiler declarations have been
observed. Force-fresh applies to requested workspace targets while successful
dependency receipts remain eligible.

## Consequences

The default CLI and SMT cache keys are unchanged. Workspace attribution and
complete reports now survive Cargo's fresh-artifact path. Input uncertainty costs
fresh work, rather than manufacturing a verified verdict. True persistent Verus
frontend internals, semantic function dependency slicing, and shared frontend
receipts remain deferred. `sail-verus` may depend on `sail-api` for the existing
report types and new project envelope; this is acyclic and preserves the shim's
small dependency boundary.

## Validation

The pinned Cargo-Verus integration runs through the actual private aliases and
produces isolated reports for the toy target and vstd. The first run established
1,872 function proofs; unchanged subsequent runs invoke zero target verifiers.
Regression tests cover restored-mtime source/build inputs, restored edits during
verification, receipt corruption and altered artifacts, selector coverage, and
partial-scope exclusion. Whole-workspace and differential acceptance results are
recorded by the milestone lead before merge.
