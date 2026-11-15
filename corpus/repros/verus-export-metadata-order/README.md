# Exported source-map entry order

VI-0043 records nonrepeatable exported VIR bytes on the selected Linux runtime,
fork `a0e11cffbdf768eac874996f35149385152f736e`, manifest
`1d23c074efd3335a60f3ca24f97ab5eb130fafd2454a06f5fc9d279da188c03c`.

From this directory, repeat the **same command at the same absolute paths**:

```sh
VERUS/verus lib.rs --crate-name export_order --crate-type=lib --edition=2021 \
  --no-verify --compile --emit=metadata --out-dir output \
  --export output/export_order.vir
```

Create `output` first. Copy each output to a separate capture directory after
every run, then remove the output files before the next invocation. Keep stdout,
stderr, exit status and the exact runtime identity. Compare the original bytes;
do not sort, decode/re-encode, remap or otherwise normalize them for acceptance.
The initial native reduction used module names `part0` and `part1`; the exact
checked-in source is also exercised separately, with its own original captures.

Eight direct native runs of that reduction all exited zero and emitted identical
15,689-byte Rust metadata, while all eight 3,000-byte VIR exports had different
hashes. No Cargo process or registry proc macro was involved. A preceding
16-module reduction had the same outcome in three runs. These are compile-only
diagnostics, not evidence of verification, artifact reuse or performance.

The selected source writes `CrateMetadata.original_files`, a `HashMap`, directly
through bincode's legacy serde map encoding. Independent retained-byte analysis
of the larger Anvil exports found the same complete map entries in different
serialized orders, with an identical preceding VIR body. The fork correction
must make this producer deterministic while preserving every key and value and
the existing decoder. See [VI-0043](../../../docs/verus-issues/VI-0043-export-metadata-map-order.md).
