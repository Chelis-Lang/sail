# VI-0043: Exported source maps serialize in hash order

- **Found:** 2026-11-15, while investigating Anvil's cold vstd path (#232).
- **Runtime:** selected Linux fork `a0e11cffbdf768eac874996f35149385152f736e`,
  manifest `1d23c074efd3335a60f3ca24f97ab5eb130fafd2454a06f5fc9d279da188c03c`.
- **Evidence:** [Anvil vstd artifact record](../validation/anvil-vstd-artifacts.md).
- **Status:** reproduced; producer correction implemented on fork branch, unselected.
- **Environment:** native x86-64 EC2 c7i.4xlarge, us-east-2c, Sail CI core image
  `6c19f5c8d397b61c`. No Mac or stock reproduction is claimed.

## Reproduction and cause

The [small multi-file export fixture](../../corpus/repros/verus-export-metadata-order/README.md)
uses the bundled fork macros directly, without Cargo. Eight repetitions at
unchanged source/output paths produced eight different VIR exports while Rust
metadata stayed identical. All compiler exits were zero. Earlier actual focused
Anvil replays also failed strict same-mode byte comparisons; those failed
controls remain failures, and the verified-versus-compile-only experiment was
never reached.

`source/rust_verify/src/import_export.rs` derives `Serialize` for
`CrateMetadata`, whose last field is
`original_files: HashMap<Vec<u8>, FileStartEndPos>`. `export_crate` sends this
directly to bincode's legacy serde writer. Independent analysis of retained
Anvil VIR files decoded the final 144-entry map, roundtripped its original order
exactly, and found identical keys, optional filenames, positions and preceding
VIR bytes. Only complete map-entry order changed. Fresh map hashing therefore
leaks into the serialized artifact.

## Sail impact and correction boundary

The variation defeats exact artifact identities and obstructs the #232
prerequisite. Fork commit `1a7e2f6cb1be88fefd382ce05bae4aae11b0e8a5` sorts complete entries
at the Verus serialization boundary; the in-memory map, values and decoder stay
unchanged. This is a producer
determinism correction, not query normalization or permission to ignore differing
artifact bytes. No default selection follows from the reproduction.

Anvil's `.rmeta` variation is a separate unresolved question. Its `arrow_head` /
`arrow_tail` swaps resemble the registry-macro class described in VI-0001, but
this record does not establish their cause or claim that sorting VIR fixes them.
The preliminary corrected runtime produced identical VIR bytes in eight direct
replays of the captured Anvil invocation, while Rust metadata still varied. That
separate same-mode check therefore remains failed. No speedup, shipped artifact
reuse, whole-project acceptance or release readiness is established.
