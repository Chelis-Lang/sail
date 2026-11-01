# Benchmark diagnostic order

Complete diagnostic blocks extracted, in original order, from the first
IronKV failing-edit block of the native `d18961a1` / fork `4cd6bd5f` matrix
(c7i.8xlarge, us-east-1, 2026-11-01). The pinned corpus and edit recipe are
`corpus/corpus.toml` and `scripts/perf-real.sh`'s IronKV `fail` edit.

Both arms exit 101 and print the same assertion error, three recommendation
warnings, locations and 312-verified/one-error summary. The stock arm prints
warnings before the error; Sail prints the error first. ADR 0042 already
admits this complete-message reordering. The benchmark's ordered headline
comparison incorrectly rejected it. These fixtures retain every message byte
and count plus the common summary; Cargo status lines are outside this minimal
fixture. Original complete logs remain external:

- Stock SHA-256: `8b36333aba3b5a75a435c5bc2c20c86869815f31791c8574d7e5a2e577679dbd`.
- Sail SHA-256: `5d6bc3cc0f6b9839ec0b2818b4a71f6206988402c9d4c2dc713e0d43b4977e65`.

The Rust diagnostic comparator accepts the unchanged pair and rejects a
changed assertion body. Benchmark checks use that same splitter and preserve
message multiplicity and the ordered remainder.
