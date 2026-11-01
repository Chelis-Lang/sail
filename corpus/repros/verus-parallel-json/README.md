# Concurrent Cargo-Verus JSON output loses framing

Found on 2026-10-31 with stock Verus `0.2026.10.18.7d82994`, its pinned Z3
and Rust 1.99.1, native Linux c7i.8xlarge / us-east-1. The valid generated
chain/diamond/shared workspaces each exit zero, but the combined stdout is
not a complete JSON document stream. All three repeated stock controls also
fail JSON decoding, at different offsets. The existing strict comparator
therefore reports differences; no parallel conformance pass follows.

The original complete reproduction has eight packages, four modules each and
two root binaries. Its pinned generated source is
`7e7ec297896d3f9a6c5a4d48d3076622696b5c44`. The smaller two-package shared
workspace below also reproduces the failure. Set
`SAIL_VSTD_SOURCE` to `source/vstd` in the pinned Verus source checkout:

```sh
SAIL_REPRO_OUT="$(mktemp -d)"
python3 scripts/perf-scaling.py --out "$SAIL_REPRO_OUT/project" \
  --shape shared --packages 2 --files 1 --targets 1 --vstd "$SAIL_VSTD_SOURCE"
(cd "$SAIL_REPRO_OUT/project" && \
  CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR="$SAIL_REPRO_OUT/target-1" \
  cargo +1.99.1 verus verify --workspace --all-targets \
  -- --output-json --time > "$SAIL_REPRO_OUT/parallel.stdout" \
  2> "$SAIL_REPRO_OUT/parallel.stderr")
python3 corpus/repros/verus-parallel-json/check-stream.py "$SAIL_REPRO_OUT/parallel.stdout"
```

Repeat with a fresh `CARGO_TARGET_DIR` and separate stdout/stderr captures;
scheduling varies. An unchanged Cargo no-op does not reproduce compiler
output framing. Three native stock runs of the two-package reduction all
exit zero. The first two fail syntax after one and five complete documents;
the third parses all nine documents. All three attempts, including the
non-reproducing round, remain in the external archive with SHA-256
`547fb8df191271c267014541b6713fd992a857806ea54ffbbf619a50fa09e458`.
The generator source is `32f7bba6`; runtime and environment are as above.
The checker only diagnoses JSON syntax, including
duplicate keys; it never normalizes output or judges proof equivalence.

A separate `--jobs 1` Cargo control keeps every workspace and test/binary
target selected. Its streams parse completely, but independent crates change
document order: chain compares same, diamond and shared compare different
because function details are paired by document position. Equal aggregate
counts are insufficient. Its result stays separate from the original parallel
failure. Do not drop unparsed output, missing functions or diagnostic checks
to make the existing comparison pass. Deterministic lossless output capture
and the normal parallel profile remain unresolved.
