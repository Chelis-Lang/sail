# Non-UTF-8 compiler-read metadata (#599)

`proof.rs` passes with the supplied one-byte `data.bin`. Emptying the binary
makes fresh Verus fail. With fork `0ad405eb` and Rust 1.99.1, compiler-read
metadata nevertheless describes SHA256(empty), size zero, for this binary.
This reproduces incorrect metadata, not a demonstrated stale Sail proof.

From this directory, select the pinned fork's `verus` as `VERUS` and use a
fresh observation directory:

```sh
mkdir observation
SAIL_COMPILER_INPUTS="$(pwd)/observation" \
SAIL_COMPILER_INPUTS_BINDING=0000000000000000000000000000000000000000000000000000000000000000 \
RUSTUP_TOOLCHAIN=1.99.1 "$VERUS" proof.rs --output-json --time \
  -Zchecksum-hash-algorithm=sha256
```

The expected actual-byte digest is
`a8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89`, size one.
Inspect both phases of `observation/inputs.json`. Keep raw output outside the
repository. Reproduce the failing proof in a separate copy with an empty
binary; do not overwrite this pinned positive input.
