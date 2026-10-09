# VI-0003: `--output-json` reports `success: true` for a failed bit-vector query

- **Status:** open (tracked here)
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-08, building function attribution (ADR 0014); first
  noted from the source in `docs/survey/verus-z3.md` §6
- **Reproduction:** `crates/sail-cli/tests/fixtures/attr/input.rs`, function
  `arith::bv_bad` (`assert(x & 1 == 0) by (bit_vector);`)
- **Impact on sail:** `sail verify --output json` reports such a function as
  `failed`, which differs from Verus's `success` field but agrees with Verus's
  diagnostics and exit code. When attribution is unavailable, sail can only
  preserve Verus's raw `success` but cannot trust it in a failing crate.
  Uncertain successes are reported `unknown` with an attribution note, never
  confirmed as verified.

## Description

When a function's only failing query is a `by (bit_vector)` assertion, Verus:
- reports `error: bitvector assertion not satisfied`;
- exits with status 1;
- counts the error in `verification-results.errors`;

but still lists the function with `"success": true` under
`times-ms.smt.smt-run-module-times[*].function-breakdown`.

The cause: `success` is computed as "not in `func_fails`"
(`rust_verify/src/main.rs:394-411`). `func_fails` is filled on rlimit
cancellation and on located failures, but not on the `Invalid(None, …)` path
that bit-vector queries take (`rust_verify/src/verifier.rs:883-899`).

## Workaround

sail attributes the failed spun-off bit-vector obligation (`Function-Def`,
spinoff reason `bitvector`, first answer `sat`) to the function and reports
`failed` (`sail_ledger::FunctionRecord::outcome`). `crates/sail-cli/examples/check_output.rs`
treats exactly this case as agreement.

When attribution is partial or unavailable and the crate reports an error,
Sail does not confirm successful timing entries. The raw field, diagnostics
and exit code are retained. A successful crate with zero errors keeps its
ordinary Verus results. This conservative fallback closes the unresolved
false-verification gap without guessing which function failed.
