# VI-0038: Literal addition infers nat in a proof local

**Status:** expected pinned-language typing, not a verifier defect. A proposed
performance edit, found on 2026-10-31, was invalid; the benchmark outcome check correctly refuses
it. No solver or compiler workaround is selected.

On stock Verus `0.2026.10.18.7d82994`, Rust 1.99.1, native Linux c7i.4xlarge /
us-east-1, the minimal [reproduction](../../corpus/repros/literal-addition-nat/rejected-int.rs)
returns E0308: the right-hand side of `let result: int = 1 + 1` has type
`nat`, while the local requires `int`. Verification reports zero verified and
zero verification errors; the process exits one. This is a compiler failure,
not a failing proof. It illustrates why an `errors` outcome alone cannot
establish a deliberately failing proof's semantics.

The [matching nat local](../../corpus/repros/literal-addition-nat/accepted-nat.rs)
passes the same native stock command: one verified, zero verification errors,
exit zero. A separate `assert(1 + 1 == 3)` control reports one verification
error and exits one. The compiler-error and proof-failure outcomes therefore
remain distinct. Do not change the catalogue's expected outcome to accept
the rejected int edit. The original real-project capture remains a failed qualification
attempt. No upstream report is needed for an invalid benchmark edit.

```sh
verus corpus/repros/literal-addition-nat/rejected-int.rs --crate-type lib --output-json --time
verus corpus/repros/literal-addition-nat/accepted-nat.rs --crate-type lib --output-json --time
```

Native control archive SHA-256:
`e72b42af72cfaeb69986d0bf0f0e218b872e52926c486b40f6087354282648da`.
