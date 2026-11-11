# Builtin source-path conformance

| Field | Current state |
|---|---|
| Tracking | #710; owner-approved [ADR 0112](../decisions/0112-stock-builtin-source-paths.md), question 51 |
| Status | Native Mac component diagnostic comparison passes; original strict scanner fails; owner approved the scoped exception on 2026-11-11. Prototype unselected; no runtime pin or archive published. |
| Source | Sail `98042f07`, fork `a0e11cffbdf768eac874996f35149385152f736e`, upstream `7d829946231d6413a398f6e4160cdfe951261dad` |
| Runtime inputs | Existing source lock SHA-256 `04efc53f189f719c2ecd206ce8e15fe7d3c02643efc3f5a7835864c0dc594ee6`; Rust 1.99.1 with rustc-dev/llvm-tools/rustfmt, no rust-src; pinned Mac Z3 SHA-256 `edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb` |
| Environment | Native EC2 mac-m4.metal, us-east-1c, macOS 27.0; isolated build, Cargo and Rustup roots outside the home directory |
| Change | Build-time remap of the builtin source directory to the pinned stock Mac release's canonical builtin directory; no source, query or diagnostic rewriting |

The [minimal type-error input](../../corpus/repros/verus-builtin-build-path/repro.rs)
has SHA-256 `7d3f53ef14ac49d35063752e3462474c00ea2663c8f829dea37bc50f37ffcec3`.
The selected stock and fork binaries each exit 1. Their stderr differs only in
the builtin definition's recorded source directory: stock stderr SHA-256
`c75d1d653497ee6f1cc141d046406fefc6343f710a64d399ee208b85c3f51cd3`,
fork `0cc7cdd3c2ee0d3c63ed1810aceb0503cd5d4b2dbd581ff98b208913302fb26d`.
Both stdout streams are empty. The raw files are retained outside the repository.

The rebuilt prototype verifies vstd both through `vargo build --release` and
the release-style Cargo-Verus vstd build: each reports 2,059 verified, zero
errors. Its reproduction then matches stock stdout and stderr byte for byte,
including exit 1. That establishes this diagnostic component only, not default
conformance across projects, runtime installation, determinism or cost.

The shipping-file probe copies exactly the current pinned manifest's fourteen
runtime file names from the new build. Three contain canonical stock paths:
`libverus_builtin.rlib` (four), `libvstd.rlib` (one), `vstd.vir` (382).
The other eleven have no `/Users/` or `/home/` hits. Existing scanner exit 1
is preserved. Inventory SHA-256
`99d5c504ce585a4b8aab72021525580bc37aab4ba1dcd687be16023275fd9f05`;
shipping-file scan SHA-256
`0cb729dde51dd64cc75ce8982543f019e350819680bfe364132d658b24609a54`.
The unpruned intermediate-tree scan is broader and is not substituted for
this shipping-file result. No qualifying engine manifest is synthesized.

| Date | Source | Attempt | Result and limit |
|---|---|---|---|
| 2026-11-11 | `98042f07`, selected `a0e11cff` | Native stock/fork type-error control | Different builtin note path; original streams preserved |
| 2026-11-11 | Same fork/lock, builtin compiler remap | Rebuild, vstd and diagnostic control | Component passes; strict shipping-file scanner fails on the canonical source paths. Unselected. |

The owner approved the exact scoped scanner exception on 2026-11-11. The
working scanner implements the explicit platform/position boundary, keeps
caller tokens on original bytes and preserves all existing regressions.
Its new native controls and independent review are pending; no scanner test
is claimed by the earlier component rebuild.

Remaining: independently reviewed production build/variant binding; Linux and Mac actual diagnostic
controls; whole-corpus fast/standard/deep and source-suite checks; determinism;
installed runtime and cost qualification; immutable artifact staging and
publication authorization. No release or default-mode acceptance follows from
the component result.
