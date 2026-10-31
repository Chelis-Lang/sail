# Broken generated binary control

This preserves the generator defect found on 2026-10-31. It is an intentionally
invalid project, not a performance baseline. With pinned Verus
`0.2026.10.18.7d82994` / Rust 1.99.1:

```sh
cargo verus verify --manifest-path corpus/repros/scaling-missing-prelude/Cargo.toml --all-targets -- --output-json --time
```

The binaries omit `use vstd::prelude::*;`. Verus refuses them because the
required builtin crate is not imported. Concurrent Cargo failures can finish
different binary/test targets first; the complete native three-shape attempt
reports three conformance differences, including those Cargo diagnostic
trailers and partially completed crate output. Keep those refusals; do not
relax the comparator or count the matching primary error as successful
program qualification.

The generator correction supplies the prelude and a `verus!` main that calls
the generated root library's checked probe and asserts its result. The same
complete workspace/all-targets command must be rerun. Importing the prelude is
ordinary Verus program setup, not a compiler workaround. This reproduction
has no upstream defect claim.
