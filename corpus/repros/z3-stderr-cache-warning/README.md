# Solver stderr disappears on an unsafe warm cache

Pinned Z3 4.16.0 answers `input.smt2` with `unsat` on stdout and
`WARNING: (2,66): pattern does not contain all quantified variables.` on stderr.
The old cache stored stdout replies without the warning. On a warm run it
could avoid starting Z3 and omit the diagnostic entirely.

Strict deep validation found this in `generics::const_generics_broadcast`
and `triggers::test_trigger_on_lambda_3`. The latter test's libtest outcome
changed, because its upstream harness treats the unexpected warning as a
failure. Neither discrepancy is normalized away. Issue #61 tracks the fix.

The corrected cache forwards stderr, publishes only sessions observed silent
through EOF, and refuses older local/shared entry formats. Warning sessions
always start the pinned solver again. A bounded staging limit may cost cache
misses without changing forwarded output.
