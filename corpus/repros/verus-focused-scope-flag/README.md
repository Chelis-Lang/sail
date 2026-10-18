# Focused module reports whole-crate metadata

With the pinned stock Verus `426d8b01`, run:

```sh
"$STOCK_VERUS" input.rs --crate-type lib --verify-only-module selected --output-json --time
"$STOCK_VERUS" input.rs --crate-type lib --output-json --time
```

The selected call succeeds with one verified check and zero errors. The whole
call fails with one verified check and one error from `omitted::not_checked`.
Both documents report `is-verifying-entire-crate: true`. The selected function
breakdown contains only `selected`, so that flag is not whole-crate authority.

See [VI-0028](../../../docs/verus-issues/VI-0028-focused-scope-flag.md).
