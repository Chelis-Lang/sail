# External enum accessor order (VI-0020)

**Reduction status: unexecuted.** This is a small source reduction prepared
from the confirmed IronKV observation, not a claimed native reproduction.
The actual reproduction is two fresh unchanged IronKV captures using the
same compiler, source and resolved dependency lock: `external_fns[96]` and
`[97]` reverse `message_t::impl&%0::{arrow_key,arrow_value}`. Both selected
buckets change literal JSON/bincode; their complete raw SMT multisets match.
See [VI-0020](../../../docs/verus-issues/VI-0020-external-function-order.md).

The source retains a trusted module and an enum with two surviving direct
accessors across multiple variants, so the old macro produces external
`arrow_key` and `arrow_value` methods. The unrelated proof creates a verified
bucket. The Git dependency matches the actual IronKV lock. A fork compiler
still compiles Cargo's resolved macro source; the standalone fork macro's
existing order fix does not replace that dependency.

On a quiet native Linux research host, copy this directory to a new owned
work directory first. Use the exact qualified research fork's `cargo-verus`
and its pinned Z3. Do not patch the dependencies to the fixed fork macro:
this deliberately exercises the compiler with the original macro source.
Retain the resolved Cargo.lock, metadata, every raw stdout/stderr/status,
runtime/source hashes and original VIR logs.

Prepare once, then repeat at least eight times from the same source path:

```sh
cargo +1.98.1 clean --manifest-path "$fixture/Cargo.toml" \
  --target-dir "$out/target" -p sail-external-order-repro
VERUS_Z3_PATH="$runtime/z3" "$runtime/cargo-verus" verify \
  --manifest-path "$fixture/Cargo.toml" --target-dir "$out/target" -- \
  --output-json --log vir --log-dir "$out/round-N"
```

The initial prepared run must use the same command without the clean and
with a separate log directory. Confirm the proof actually verifies with no
errors before comparing anything. `external_fn` lines in the complete VIR
logs expose the member order; retain full logs rather than treating a
filtered comparison as full conformance. For a literal ordinary-AST/bincode
key claim, additionally qualify an explicitly source-bound observer on
these original dependencies; the current fork-library capture controller
correctly refuses a foreign macro origin. Neither execution nor a compiler
candidate is qualified by this README. No runtime pin, query normalization
or frontend reuse changes here.
