# Check-sat-using response cardinality

Run the pinned Z3 directly:

```sh
"$SAIL_TEST_Z3" -smt2 -in < corpus/repros/sail-check-sat-using-cardinality/input.smt2
```

It exits successfully and prints the exact two responses in `expected.stdout`.
`check-sat-using` is a satisfiability check and emits one response under the
default `:print-success false` protocol. [Sail #397](https://github.com/Chelis-Lang/sail/issues/397) records this defect.
The original Sail shallow classifier
recognized the command and counted it as a check, but its output policy omitted
this variant and returned `Output::None`. The focused `output_expectations` test
reproduces that error and guards the corrected variant.

This demonstrates incorrect response accounting. It does not demonstrate a
false-proof exploit: the existing complete-publication gate refuses unmatched
live replies. Solver semantics and complete process/stream coverage remain
separate obligations from this dispatch fix.

The actual ignored `tactic_check_responses_keep_their_own_keys_and_warm_reuse`
shim test compares full stdout, stderr and status against direct execution,
then requires exactly two cached replies and no new solver on the warm run.
The reproduction uses pinned Z3 4.16.0. The directly tested macOS executable
SHA-256 is `edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb`.
