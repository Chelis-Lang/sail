# VI-0042: A structural-equality derive inside a module aborts Verus

- **Source:** pinned stock Verus `0.2026.10.18.7d82994` and the pinned fork, Rust toolchain from `corpus/toolchain.toml`
- **Found:** 2026-11-08, while qualifying neutral profile-three source closures
- **Status:** open upstream; no current Sail surface is affected since the neutral path was removed in v0.3.0

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

None today. Sail emits no Verus source of its own since v0.3.0 removed neutral
lowering ([ADR 0109](../decisions/0109-oyster-retired-v030-scope.md)). Neutral
lowering marked every admitted unit enum with `Structural`, so an enum in any
module could not produce a verdict, and profile three refused unit enums
outside the crate root with `SAIL3140`. A user's own `Structural` derive inside
a module still reaches this Verus error directly.
