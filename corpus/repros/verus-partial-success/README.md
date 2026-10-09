# Partial verification timing reports skipped functions as successes

Pinned Verus `0.2026.10.04.426d8b0`, Z3 4.16.0, 2026-10-09.

Run the pinned launcher directly:

```sh
verus input.rs --crate-type=lib --verify-root --verify-function good --output-json --time
```

The verification result reports one verified obligation and zero errors.
The timing function breakdown includes `input::bad` with `success: true`,
zero rlimit and zero solver time, even though `bad` was skipped and contains
`assert(false)`. Verifying the whole file fails on `bad`.

Verus computes timing success as absence from `func_fails`; a skipped
function never enters that set. Source: `rust_verify/src/main.rs` and
`rust_verify/src/verifier.rs::run_commands_queries`, at the pinned commit.

Sail must apply the explicit function selector to structured reports. It must
not treat every timing entry in a partial run as a verified function.
Tracked locally as VI-0009; no upstream report is filed.
