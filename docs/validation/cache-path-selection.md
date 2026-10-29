# Cache selection and physical managed paths

Status: native correctness, full conformance and installed-release first-Serve
witnesses pass on Linux and macOS. Hosted checks remain required before merge.
This addresses #581 and #601 without changing shipped-proof admission.

The installer and runtime share primary/secondary cache selection. Explicit
`SAIL_VERUS_HOME` retains primary precedence; otherwise `SAIL_CACHE_DIR`,
`XDG_CACHE_HOME/sail`, then `HOME/.cache/sail` determines the install root.
Managed CLI paths derive from one physical cache root. Creation or resolution
failure retains the existing fallible path. Unclassified SAIL3138 misses name
the trace route without guessing a dependency mismatch.

On 2026-10-29, native EC2 c7i.8xlarge, us-east-1, source `8acc849a` passes
the complete image gate (1,242 ordinary tests), solver51, full E2E including
extra, full fast and standard conformance. Each of four arms reports 147/147
Same / 3,451 functions for fast and 159/159 Same / 13,166 functions for
standard, with zero unstable, different or infrastructure outcomes. All 21
bounded mutation/control rows agree. The original baselines remain unchanged.

The combined Mac candidate `0d3138d1`, mac-m4.metal/us-east-1b/macOS 27.0,
passes strict Clippy and 1,238 non-solver tests. Its cache implementation is
unchanged from this candidate. The installed-release witness at `1dc5f1a5`
uses a fresh demo and a symbolic cache alias: Current, exit0, one requested
verifier invocation and vstd's release dependency proof. A real custom-cache
`cargo xtask fetch-verus` also installs the pinned stock runtime successfully.
The rebased `a05b948a` passes the full Linux image gate again (1,242 tests).
Its staged release, fork `47fb3be6` and Rust 1.99.1, also serves vstd through
a fresh symbolic cache alias: Current, exit0, exactly one requested verifier
invocation and a release vstd receipt (10.655 s internal wall, unpaired).
These are functional controls, not performance measurements.

Raw logs remain outside Git:

| Log | SHA-256 |
|---|---|
| Linux cache-current | `1c2386df3016937af3d0f0f4810b9ac43ef34da3ef249ed38fff39f5f631caf7` |
| Linux rebased gate and staging | `9242691d227ea927976b8ddfc390b4c503685582cab43cbac0e63ce2748ce47c` |
| Linux installed first Serve | `2d92833b180edfbc4eafac9944285f57851427e39445863a7378e31f34b77b56` |
| Mac corrected combined gate | `a5bee5b2cde7d1424fc74b08b2030e26ad7de1115737ef2a049e2fbcd28d1e66` |
| Mac installed first Serve | `e481990e8dc32c7c6a0ebe42623d77f7fe96f2b840978b7d8a2516074a612c6a` |

Indexed attempts: `2b6a0fd9` failed item-order Clippy; `e37fe6a8` failed an
obsolete import. Both were corrected without weakening checks. The combined
Mac gate at `867fd4ec` failed an unrelated projection-memo fixture's alias
assumption; the physical-root fixture correction passes in `0d3138d1`.

Indexed attempt: the first Linux installed witness stopped because its temporary
parent did not exist. Creating that benchmark directory and installing the
minimal pinned host toolchain succeeds; the full gate had already passed.
