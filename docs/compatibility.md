# Compatibility matrix

The Verus releases sail is tested against (`corpus/toolchain.toml`), and how
the corpus fares under each one. The primary pin is the release the corpus
is validated against. Secondary pins are compatibility targets (ADR 0010),
run locally with `SAIL_VERUS_RELEASE=<release>` (ADR 0013).

| Release | Role | Z3 | Rust | sail conformance (fast tier) | Notes |
|---|---|---|---|---|---|
| `0.2026.10.04.426d8b0` | primary | 4.16.0 (bundled) | 1.98.1 | 147 of 147 the same (macOS arm64 and Linux x86-64); standard tier 159 of 159 (Linux) | The corpus is validated against this release |
| `0.2026.09.06.8dea4a2` | secondary | 4.16.0 (primary's build; VI-0006) | 1.98.0 | 147 of 147 the same (macOS arm64, 2026-10-09) | See below |

## Plain Verus under the secondary release (fast tier)

The fast tier's in-tree sources (vstd, examples) are those of the **primary**
release's commit. Under the older secondary release, some of them fail
simply because they are newer than that release. sail matches plain Verus on
every unit either way (147 of 147 the same).

- **Exit non-zero under the secondary:** 21 of 147 units.
- **Of those, 5 are expected-failure examples** (`assertions.rs`,
  `basic_failure.rs`, `debug.rs`, `debug_expand.rs`,
  `test_expand_errors.rs`). They fail under the primary too.
- **The rest fail to build or verify** because their sources need the newer
  release: vstd itself, human-eval-verus (it depends on the newer vstd), and
  14 further examples, including `guide/*`, `iterators/*` and `mergesort.rs`.

The secondary pin is meant for external projects written against that
release, such as vest's recent history (`sail-bench history-replay`). It is
not meant for the in-tree tier.

## Optional Singular validation

The binary releases omit Singular support. Use `ci/local-singular.sh` to build
the pinned commit with `--features=singular` and compare the tagged integer-ring
examples and memory allocator in pass-through, fill and warm modes. The separate
image pins the Singular package and uses separate build/cache volumes. No CI
workflow runs this step. Source selection is explicit through
`SAIL_VERUS_SOURCE_BUILD`; artifact identity failures are infrastructure errors.
Results from the takeover validation will be recorded after its local run.
