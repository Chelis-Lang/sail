# Missing dependency stdout after store seeding (#184)

Reproduced on draft rc.5, Sail source `c3687990`, Verus fork `73dea021`,
stock oracle `0.2026.10.04.426d8b0`, Z3 4.16.0, macOS arm64.
With the installed `sail` and pinned stock `cargo-verus` on PATH:

```sh
scratch=$(mktemp -d)
export SAIL_CACHE_DIR="$scratch/cache"
cd "$scratch"
mkdir one two
(cd one && sail new --lib proj && cd proj && sail verify)
(cd two && sail new --lib proj)
cp one/proj/Cargo.lock two/proj/Cargo.lock
(cd two/proj && sail verify)
(cd two/proj && CARGO_TARGET_DIR="$scratch/plain" cargo verus verify)
```

The first Sail run and the stock run print vstd's `2060 verified, 0 errors`
line before the user crate's `1 verified, 0 errors`. The second Sail run
prints only the latter, although its result confirms vstd store reuse.
All exit 0. The correction must restore the missing line once, in order;
subsequent unchanged Sail runs must remain silent.

See the [validation record](../../../docs/validation/dependency-store-output.md).
