# Registry macros vary native Rust metadata

This extends VI-0001 to native compiler metadata. It uses **plain Rustc**, not a
Verus verification run. Use the selected runtime's Rust 1.99.1 toolchain and the
actual builtin, vstd and macro libraries from a matching Cargo-Verus build:

```sh
mkdir -p output
RUSTC_BOOTSTRAP=1 rustup run 1.99.1 rustc input.rs \
  --crate-name accessor_metadata --crate-type lib --edition 2021 \
  --emit metadata --out-dir output --cfg verus_keep_ghost --cfg verus_only \
  --extern verus_builtin_macros="$SAIL_MACROS" \
  --extern verus_builtin="$SAIL_BUILTIN" --extern vstd="$SAIL_VSTD" \
  -L "dependency=$SAIL_DEPS"
```

Repeat at fixed source/output paths, preserving each original metadata file,
stdout, stderr and exit status separately. With the actual Anvil registry macro
library, eight runs produced eight different metadata hashes. With **only** the
macro library argument changed to the fork's bundled ordered macro, keeping the
same registry builtin/vstd libraries and every other argument, eight runs
produced one metadata hash. All compiler exits were zero. A preceding diagnostic
also found the contrast with separate complete registry/bundled dependency
pairs; it is not the single-input control.

The exact checked-in seven-field input bytes match the native reduction. Its
compiler-input library hashes and raw outputs are retained. No Rust
metadata bytes are normalized for comparison. No proof verdict, default macro
routing change, whole-Anvil artifact equivalence, speedup or release selection
is established by this reduction. The actual captured vstd invocation is a
separate gate in the [Anvil artifact record](../../../docs/validation/anvil-vstd-artifacts.md).
