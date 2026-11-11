# Builtin source-path conformance

| Field | Current state |
|---|---|
| Tracking | #710; owner-approved [ADR 0112](../decisions/0112-stock-builtin-source-paths.md), question 51 |
| Status | Scoped scanner qualified and independently approved; native Mac diagnostic component passes. Runtime prototype remains unselected; no runtime pin or archive published. |
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
Native Linux controls passed 54 cases and native Mac controls passed 56, with
all 1,313 host tests passing (53 skipped). Independent review rejected that
scanner at `40d4e643`: 30 punctuation-prefix false cleans in 130 probes, and
the real rebuilt builtin rlib still failed because Rust's 0xc1 string delimiter
was treated as a path byte. The replacement retains punctuation and spaces
inside path candidates and recognizes only NUL and Rust's string sentinel as
binary delimiters. It adds both failure reproductions; replacement native
validation and independent review remain pending. These scanner controls do
not qualify the rebuilt runtime.

Rejected-review report SHA-256:
`0aef9401e886e5690151ec16502e408e3a91798f0862f5f1ebd2c3fbf8b3cea5`.
The sentinel is defined by [Rust's serialization source](https://github.com/rust-lang/rust/blob/1.99.0/compiler/rustc_serialize/src/serialize.rs).
No scanner test is claimed by the earlier component rebuild.

Remaining: independently reviewed production build/variant binding; Linux and Mac actual diagnostic
controls; whole-corpus fast/standard/deep and source-suite checks; determinism;
installed runtime and cost qualification; immutable artifact staging and
publication authorization. No release or default-mode acceptance follows from
the component result.

Scanner qualification at `2b0ec250` is complete. Native Linux c7i.4xlarge,
us-east-2, Rust 1.99.0 in the CI image: workspace gate passes 1,326 tests
(53 skipped), pinned-solver suite passes 50, full E2E including extra passes,
and all four PR conformance arms report 33 Same, zero unstable/different/infra
errors (311 functions each; seven units with resource drift). Scanner controls
pass 80 cases as root, with the two permission-only cases explicitly skipped.
These are diagnostic/build-tool changes, not selected verification-runtime
behavior; ADR 0087 requires the fast tier, not standard/deep corpus reruns.

Native Mac scanner controls pass 82 cases. The exact fourteen-file prototype
passes only its Mac-scoped scan; strict, foreign-platform and explicit-token
controls reject. Independent review passes 713 additional native controls
and verifies that 477 relevant Rust/Cargo files are unchanged from the earlier
Mac host gate (1,313 pass, 53 skipped). This is explicit identical-source host
evidence reuse, not a new host-gate execution at `2b0ec250`.
Independent approved-review SHA-256:
`ffd4da7adf2fdce1ee8f6882df226e3617ea4405da94f6ac638e5e8bb0b6c43c`.

Book source tests, mdBook 0.5.2 build and generated-link checks pass in the native
Linux CI image. Node is absent there, so the optional highlighting probe is
explicitly skipped. Initial manual book setup omitted the image's Git ownership
configuration and failed; the corrected run scopes trust to the owned `/work`
checkout. Failed log SHA-256:
`ecbc0e59657fe633dca016ffdb49b926001458b103af93c87a5710104a615a3b`;
passing book log: `165cee9571d00fd83fbe96851641bb0bc6fc4df1bb5d0d946a0e5735e5b41ee7`.
PR report archive: `b97d605596af1e2e3c0eb7af954ea18b0bde0b6664638cf4f993f8ae8b8663d8`.
Gate log: `b141cb7cc5aead38adf3601e5242930a8fe602b376510736643657a1656af992`;
solver: `7242fe35a265fcad4a88e606209025187d28aa9626fb8859d0359d806ea3fe65`;
E2E: `40694bb400da0f805839673ec5e0c809e0b789219eef7a1c1f082e7b370cfe51`.

This qualifies the scanner component only. It changes no corpus entry or
consumer command/skill; release instructions, decision, known issue and
changelog describe the new explicit scan mode. Production compiler-remap
policy, distinct runtime variant, rebuilt platform inventories and the runtime
qualification listed above remain required to fix #710.
