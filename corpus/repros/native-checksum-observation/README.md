# Native checksum-algorithm observation regression (#599)

Fork `c9af9534` compiles this ordinary Rust input correctly with either Cargo
checksum algorithm. Its native observation is complete with SHA-256 but absent
with BLAKE3: the loader observer hardcodes SHA-256, then rejects the compiler's
BLAKE3 source-map checksum. The real shipped-proof factory consequently cannot
establish native prerequisites. This is a conservative provenance refusal,
not an altered verdict or demonstrated stale proof.

Choose the runtime's `verus` as `VERUS`. From a separate copy of these two inputs,
use a fresh absolute observation directory for each algorithm:

```sh
SAIL_REPRO_OUT="$(mktemp -d)"
for algorithm in sha256 blake3; do
  mkdir "$SAIL_REPRO_OUT/$algorithm"
  env '__VERUS_DRIVER_VIA_CARGO__=1' RUSTC_BOOTSTRAP=1 \
    SAIL_NATIVE_COMPILER_INPUTS="$SAIL_REPRO_OUT/$algorithm" \
    SAIL_NATIVE_COMPILER_INPUTS_BINDING=0000000000000000000000000000000000000000000000000000000000000000 \
    "$VERUS" rustc native.rs --crate-name native_probe --crate-type lib \
    --emit=dep-info,metadata -Zchecksum-hash-algorithm="$algorithm" \
    -o "$SAIL_REPRO_OUT/native.rmeta"
done
```

C9 writes only `sha256/inputs.json`. The correction must write both complete
native documents, each using the requested algorithm and actual loader bytes.
`engine/verus/test-native-inputs.py` automates both arms against stock, including
non-UTF-8, empty and CRLF includes, exact compiler streams and complete dep-info.
The stock compiler's dep-info supplies independent checksums and lengths.
No input file is reopened by the observer to guess earlier compiler reads.
