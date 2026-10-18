# VI-0028: focused module reports whole-crate metadata

Status: reproduced in pinned stock `426d8b01` and observed in fork `e5365c05`.
No fork emitter change is selected. No issue was filed upstream.

The driver's `is_verifying_entire_crate` checks `verify_function`,
`verify_module` and `verify_root`, but omits `verify_only_module`. Consequently
`--verify-only-module` can skip a failing proof while its JSON still says
`is-verifying-entire-crate: true`.

The [five-line reproduction](../../corpus/repros/verus-focused-scope-flag/)
contains one passing selected module and one failing omitted module. A genuine
macOS stock run verifies only the selected proof, succeeds with 1 verified and
0 errors, and reports the flag as true. Removing the selector yields 1 verified,
1 error and a failing exit. Its flag is also true. Raw record SHA-256:
`4fa58b02422f3ba42c5544f370eb7aba45f72506c76818604669e5fe81144854`.
The initial reproduction attempt omitted `--crate-type lib` and stopped at
E0601 before verification; it remains a failed attempt in the raw evidence.
This is a correctness witness, not a performance or full Anvil qualification.

The native 25-call Anvil cohort independently observes the misleading field in
the genuine stock and normal fork arms. That whole cohort remains failed; its
spinoff arms additionally panic and cannot be qualified by repairing this reader.

The partial research reader now binds the pinned emitter's literal true value.
It establishes its explicitly partial scope from the exact selector and complete
selected module/function catalogue, and still requires the full stock verdict,
exit and diagnostic comparisons. Unexpected metadata, missing/extra selectors,
foreign functions and incomplete captures refuse. It does not produce Current,
publish a proof or alter `actual_work` complete-run admission. Sail's default
proof or cache policies are unchanged. The emitter field alone is never evidence
that all functions were verified.
