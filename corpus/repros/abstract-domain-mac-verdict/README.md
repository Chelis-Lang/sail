# Stock/fork disagreement in a Mac abstract-domain proof

The pinned semi-persistent package `abstract-domains` at
`10de512a3374ef7e8245746e95d114163acdc714` is a full-project reproduction on
native arm64 macOS 27, mac-m4.metal / us-east-1c, Rust 1.99.1. Stock Verus
`7d829946` passes; fork `47fb3be6` with the managed verifier flags and shim
fails `semi_persistent_abstract_domains::tnum::Tnum::add_carry_bitwise_has_eq`.
Three stock runs pass; the captured fork arm exits 101 with a postcondition
error at `abstract-domains/src/tnum.rs:729:17`. The strict comparison remains
Different. The finer artifact memo is not executed by this verifier-level arm.

Run the original package through the unchanged comparator:

```sh
sail-conformance run --corpus corpus/performance-candidates.toml \
  --tier deep --project semi-persistent --filter abstract-domains --jobs 1 \
  --engine-build "$SAIL_PERF_VERUS_BUILD" --out "$SAIL_REPRO_OUT"
```

Keep the original scope and resource limit. The full 23-unit Mac comparison
has 22 same / 1 different, 5,357 function records, no instability or
infrastructure/skipped result, and resource drift in 13 units. The same pins
pass the corresponding Linux comparison. Root cause and a standalone reduced
proof remain unestablished. Stock/shim and fork/bare-shim controls isolate the
integration; they cannot replace the original managed comparison.

Original Mac comparison archive SHA-256:
`388e2a007aa3dea05168d33876134451c4465c13322cea9137f1b7575ee212d8`.
It retains report and original raw outputs; regenerable Cargo target and
scratch project copies are omitted. No proof, resource limit, output, field
or comparator is changed to make this case pass.

Completed controls: stock/bare-shim and stock/managed-flags compare same;
fork/bare-shim and fork/managed-flags fail the same proof. Routing the fork
directly to the pinned stock Z3 executable, without Sail's shim or managed
flags, still fails. Full controls archive SHA-256:
`f71416129dfb411543e700962b76374bade14793830a58467b95b9cdd049bb30`.
These controls localize the behavior outside the artifact memo and shim;
they do not identify the exact fork change or establish a wrong Z3 answer.

Continued literal replay isolates two earlier completed specification
termination scopes, while the later proof query is byte-identical. The
bounded companion-context prototype passes this unchanged full package on
both platforms and all 23 broader units on both, but remains unselected
pending source-suite, full corpus and cost qualification. The original
failed runtime is still a reproduction. See
[VI-0040](../../../docs/verus-issues/VI-0040-macos-abstract-domain-verdict.md)
and [ADR 0094](../../../docs/decisions/0094-isolated-spec-termination-checks.md).
