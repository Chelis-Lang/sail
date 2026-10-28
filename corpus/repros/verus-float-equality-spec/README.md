# Binary64 equality from original bits

`numeric_denotation.rs` proves a mathematical interpretation of every u64 bit
pattern. It decodes sign, exponent and fraction independently of the comparison
predicate. Finite values denote signed integers in units of `2^-1074`: subnormal
magnitude is the fraction, and normal exponent e has magnitude
`(2^52 + fraction) * 2^(e - 1)`. NaNs are unequal to every value; infinities
agree only for the same sign; both zero signs denote integer `0`.

The universal theorem relates the bit comparison to equality of those numerical
denotations. Distinct exponent bands and positive-factor cancellation establish
finite canonicalization. Minimum-normal, 1.0, maximum-finite and subnormal examples
anchor the numeric scale. There is no finite-only premise, floating equality
oracle, inverse-bitcast or injectivity axiom.

Use the pinned Stock executable and environment described in
[`corpus/toolchain.toml`](../../toolchain.toml) and the
[direct checker guide](../../../docs/design/acceptance-proof-checker.md):

```sh
"$SAIL_STOCK_VERUS" corpus/repros/verus-float-equality-spec/numeric_denotation.rs \
  --no-cheating --output-json --num-threads 1
```

Run the entire file with ordinary default resources and a 120-second outer
deadline. The qualified positive result is 38 verified, zero errors and 35
user-function entries. `numeric-denotation-controls.json` contains nine exact
before/after recipes. Apply each recipe exactly once to a fresh independent
copy named `numeric_denotation.rs`, then run the same whole-file command.
Retain every original function. A control qualifies only for an ordinary
verification error, with all 35 user functions and verified+errors=38; compiler,
VIR, timeout and resource-limit refusals earn no credit.

These files are byte-identical to the independently audited source qualification
in the [validation record](../../../docs/validation/binary64-equality-support.md).
They are standalone corpus support, outside the four acceptance-model composition
and production-refinement inventory. The result does not establish native
Rust/Serde/to_bits correspondence, the Stock builtin IEEE equality bridge or
compiler-input/launch completeness. Those obligations remain on their existing
candidate branches; none is removed to make this component pass.
