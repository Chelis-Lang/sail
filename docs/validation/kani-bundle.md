# Kani release-bundle fixture

Status: native Linux contributor fixture qualified at `0c31ad8e`. Official
installation, eleven Python control groups, the full local gate and fresh
independent review pass. No Kani executor, CLI backend, nightly Rust
authentication, currentness or reusable proof authority is added.

The authoritative Linux pin names Kani 0.68.0 / CBMC 6.11.0, the official
140,315,343-byte archive SHA-256
`32e2b484d73ede0bbf64a2cf0879c4259422497d8aec4ee67d448de9ae7843d3`,
and 120 original files totaling 507,989,223 bytes, plus the original empty
`scripts/` directory. Independent release API and archive inspection verifies
these identities. All original file hashes and archive bytes remain unchanged.

## Native qualification

On a native c7i.4xlarge in us-east-2, the exact-source runner installs the
archive, independently rehashes every installed file, preserves notices/modes
and the empty directory, and confirms that no external toolchain link exists.
The full local gate passes: 1,403 workspace tests, 57 declared skips, formatting,
Clippy, doctests, rustdoc, layering, resource and dependency checks. The lead
independently rehashes all ten retained original evidence files; archive SHA-256:
`8d962b58c777359206b9293f503ccfc25a8a8a3123e909f012b5b05886bd0e00`.

Fresh independent review approves exactly `0c31ad8e`. Separate native host and
production-core-image runs each install and rehash the complete official
archive and pass 59 independent installer observations, 13 FFI observations,
all eleven candidate groups and 32 rounds of two concurrent publishers.
Controls cover canonical paths/types/identities/bounds, pre-publication faults,
cleanup/retries and preservation of raced files, directories and links.
Unsupported primitives refuse without fallback. Both environments use Python
3.12.3/glibc 2.39; immutable production image:
`sha256:061dd7ee7c0929d175ca2d33ef60fc7c11ba1f42da976a6e60b8490090b6b6ae`.

The lead independently verifies all 1,129 native archive entries: 690 regular
files, 433 directories and six symlink metadata records, without following links.
Native archive SHA-256:
`36d509e1bd786d4aeaadd5f1346ed246d025e58b2b76b14ec60d02a37d20ceb9`.
All 358 final local review records and the 116/218 predecessor records are also
independently verified and retained. No Rust, solver or Kani execution is
inferred from the review controls.

## Scope and retained failures

| Source / date | Result and limit |
|---|---|
| `c6259d9d`, 2026-11-14 | Independent rejection: check-then-rename replaces a raced empty destination. Successor uses typed synchronous Linux renameat2(RENAME_NOREPLACE), never a fallback. |
| `0d2d8694`, 2026-11-14 | Native official installation refuses the unpinned original empty directory; gate not reached. Successor pins only that original empty directory; other extras remain refused. |
| `0c31ad8e`, 2026-11-14 | Native installation/full gate and final review pass. First review image invocation used an archive basename incompatible with its API lookup; corrected invocation passes. Both original logs remain retained. |

This is a Linux contributor fixture. CI runs synthetic controls without a Kani
download. Real Kani/arrayvec, process faults/cancellation, separately authenticated
nightly Rust, input closure, explicit bounds and Mac remain #712 package G.
No selected-runtime, model/refinement or dependency change occurs; this slice
requires fast protected CI and focused installer controls under ADR0087, without
claiming corpus, benchmark, formal or Mac acceptance.

Documentation changed: ADR0137, this record, crate README and changelog.
No released command changes; consumer skill and installation recipes stay
unchanged. Documentation-only amendments after qualification preserve all
installer, test, resource and CI source bytes from `0c31ad8e`.
