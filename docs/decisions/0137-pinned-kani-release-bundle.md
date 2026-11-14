# 0137: Authenticate Kani's original release bundle separately from execution

- **Status:** implemented; contributor fixture qualified on native Linux
- **Date:** 2026-11-14
- **Extends:** [ADR0128](0128-kani-bounded-observations.md)
- **Tracking:** #712 package G, #135

## Context

The strict decoder recognizes Kani 0.68.0 / CBMC 6.11.0 exports but grants no
execution or reuse authority. An executor also needs a reproducible tool input.
The official Linux archive has 120 original regular files, including compiler,
solver, Rust libraries and original license notes. Its declaration names
nightly-2026-08-21 and rustc 1.100.0-nightly (8925ea358 2026-08-20).

## Decision

Pin the official Linux x86-64 archive digest/size and every original file's
size, digest and executable bit, plus the original empty scripts directory,
in engine/kani/bundle-linux.json. Copy the pin
unchanged into the unpublished sail-kani package through crate-resources.
The contributor-only installer verifies the archive, validates the complete
canonical file inventory and prepares final file/directory modes in a private
same-filesystem staging tree before one atomic Linux renameat2(RENAME_NOREPLACE). An isolated stdlib
ctypes binding declares the fixed scalar/path argument types and retains both
NUL-free byte buffers for the call. Unsupported libc, kernel or filesystem
cases fail without fallback. Existing destinations, including racing ones, refuse;
links, special files, aliases, duplicates and extra or missing members refuse.
Downloads, pin size, member count, expanded bytes and path depth remain bounded.
All original files and notices are retained. No third-party dependency is added.

This does not select a backend or confer bounded proof authority. The bundle's
Rust version declaration is not an authenticated Rust installation. Official
Kani setup adds an external toolchain symlink; this fixture neither creates nor
follows it. The executor must separately authenticate the nightly toolchain,
actual command, input closure, original output/completion and explicit bounds.
No cache is enabled. Mac pins/qualification and CLI integration remain open.
CI runs only small synthetic installer controls; it never downloads Kani or
adds the toolchain to the shared CI image.

Primary evidence: [official release](https://github.com/model-checking/kani/releases/tag/kani-0.68.0).
The asset digest from the release API matches the independently downloaded
140,315,343-byte archive. All 120 file entries are derived from original bytes,
not from a locally modified setup directory.

## Consequences

Later executor work can name exact compiler/CBMC bundle bytes without trusting
printed version labels alone. The fixture remains distinct from Rust setup,
backend acceptance and real-project proof/cache authority. Native Linux full
gate and focused independent extraction/publication review pass for this
contributor slice; see [qualification](../validation/kani-bundle.md). No Mac
or executable-backend readiness follows.
