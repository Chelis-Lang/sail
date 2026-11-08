# VI-0042: A structural-equality derive inside a module aborts Verus

- **Source:** pinned stock Verus `0.2026.10.18.7d82994` and the pinned fork, Rust toolchain from `corpus/toolchain.toml`
- **Found:** 2026-11-08, while qualifying neutral profile-three source closures
- **Status:** open; Sail refuses the form in neutral profile three

## Reproduction

```rust
use vstd::prelude::*;
verus! {
pub mod shapes {
    use vstd::prelude::*;
    #[derive(PartialEq, Eq, Structural)]
    pub enum Mode { Wide, Tall }
}
pub fn wide(mode: &shapes::Mode) -> (ret: bool)
    ensures ret == (*mode == shapes::Mode::Wide)
{ *mode == shapes::Mode::Wide }
}
```

`verus FILE --crate-type lib` panics with `Internal Verus Error: The thir_body
query is running for item ... shapes::_ which may require erasure, but the
VerusErasureCtxt has not been initialized`. Naming the derive by its full path
(`vstd::prelude::Structural`) or importing only `Structural` gives the same
panic, as does dropping the `Debug` derive. The same enum at the crate root
verifies. The derive's anonymous constant inside a module appears to reach the
erasure query before the erasure context exists; this cause is not established.

## Impact on Sail

Neutral lowering marks every admitted unit enum with `Structural`. An enum in
any module, inline or a module file, therefore cannot produce a verdict.
Profile three refuses unit enums outside the crate root with `SAIL3140`.
Profiles one and two keep their existing admission and bytes.
