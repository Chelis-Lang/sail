# Internal compiler failure was indistinguishable from a completed error

This is an optional Sail engine defect in its driver completion contract. The
source is pinned to Verus `426d8b01e7ffb910a36c15e1f870ff985bb61eef` and Rust
`48a229ceaefd4985c50990b14116b6d856af0985` (1.98.1).

The inherited driver catches every panic from `rustc_driver::run_compiler` and
returns `Err(())`. The first engine implementation then emitted an unsuccessful
`compiler-sealed` event. Main could still print a complete-shaped verification
JSON document and exit 1. That output is indistinguishable from a legitimate
late EXEC error by exit status and report shape alone. No incomplete result was
accepted, but the required stock fallback could be missed.

The isolated source tests reproduce the distinction without a solver:

```rust
compiler_result(false, || panic!("test internal compiler failure"));
// Stock returns Err(()), retaining its established behavior.
compiler_result(true, || panic!("test internal compiler failure"));
// Interactive mode unwinds; the following seal is unreachable.
compiler_result(true, || {
    std::panic::resume_unwind(Box::new(rustc_errors::FatalErrorMarker));
});
// An expected compiler error returns Err(()) normally.
```

The correction uses the pinned `rustc_driver::catch_fatal_errors` only for the
explicit interactive mode. It catches exactly `FatalErrorMarker` and resumes
unexpected panics. A normal driver terminal carries
`payload.normal_termination: true`, including legitimate proof, frontend, VIR
and late EXEC failures. The flag records completion, never proof success.
Unexpected panics, signals, malformed output and absent completion require
stock fallback. Default verification keeps the original catch-all path.

The local source differential gate includes a failing proof, a frontend type
error and a late EXEC borrow error. It compares full diagnostics, verdicts and
solver bytes against stock and requires the normal terminal flag. Cancellation
must still produce no compiler seal. No prepared encoding changed, so its
version remains 2; patch and runtime identities change independently.
