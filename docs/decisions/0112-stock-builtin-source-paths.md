# 0112: Preserve stock's builtin source paths in the runtime

- **Status:** accepted by owner, 2026-11-11; scoped scanner qualified, runtime implementation/qualification pending
- **Date:** 2026-11-11
- **Tracking:** #710, open question 51

## Context

Stock Verus's builtin library records its release builder's source paths.
Rust type errors can print those paths in a `function defined here` note.
Sail's runtime records a different builder's paths, so default diagnostics
differ. No current divergence permits this, and Sail must not rewrite the
diagnostic stream to hide it.

A native Mac component experiment used the exact pinned fork and source lock,
isolated Rustup/Cargo roots, and a compiler path remap restricted to the
builtin source directory. The rebuilt runtime and its release-style vstd
each verified 2,059 functions with zero errors. Its #710 reproduction then
matched stock stdout and stderr byte for byte, with both processes exiting 1.
This is a component result, not whole-corpus or release qualification.

The original archive scanner rejects every `/Users/` and `/home/` occurrence.
Scanning the exact current runtime file inventory from the rebuilt candidate
rejects the canonical stock paths in `libverus_builtin.rlib` (four hits),
`libvstd.rlib` (one) and `vstd.vir` (382). The other eleven runtime files
have no such hits. Raw inventory SHA-256:
`99d5c504ce585a4b8aab72021525580bc37aab4ba1dcd687be16023275fd9f05`;
strict shipping-file scan SHA-256:
`0cb729dde51dd64cc75ce8982543f019e350819680bfe364132d658b24609a54`.
The original failed scan remains failed.

## Decision

1. Build the builtin library with a compiler path remap to the pinned stock
   release's canonical builtin source directory. Do not remap user inputs or
   normalize queries, replies or diagnostics in Sail.
2. Give this build policy its own runtime variant identity, rebuild both
   platforms, and retain all provenance, artifact and solver checks. Existing
   artifacts are never overwritten or relabelled.
3. The owner approved the scoped exception on 2026-11-11: permit the known stock builtin source
   namespace in the three compiler-metadata files above. Restrict this to the
   expected runtime-relative file locations and the canonical directory for
   the active platform. Prefix lookalikes and all other home-directory paths
   still fail. Symlink targets and caller-supplied scan tokens keep their
   original strict checks. Unreadable or unexpected files still fail closed.
4. Preserve every existing scanner regression and add positive controls for
   canonical metadata plus negative controls for other files, other users,
   prefix lookalikes, symlinks and explicit scan tokens.
5. Qualify real diagnostic controls, exact conformance, fork/source tests,
   determinism, installed behavior and cost before runtime selection. A
   scanner-policy approval establishes neither qualification nor publication
   authority.

## Consequences

Default diagnostics can match the independent stock oracle without hiding
their source. The scanner distinguishes these fixed public source labels
from build-host home paths; this is an explicit exception to its blanket
rule, explicitly approved in question 51. Runtime selection and publication
still require their separate qualification and exact-candidate approval.
