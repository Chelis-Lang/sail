# VI-0009: Partial timing success includes skipped functions

- Status: open in stock Verus (tracked here; not filed upstream);
  fixed in Sail's Verus fork by `3566421a` (fix: omit unselected functions from function timing).
- Found: 2026-10-09
- Verus: `0.2026.10.04.426d8b0`
- Reproduction: `corpus/repros/verus-partial-success/`

A function-only verification reports success for an unselected failing
function in `times-ms.smt.smt-run-module-times.function-breakdown`.
The per-crate verified count correctly counts only the requested function.
Timing success is computed from absence in `func_fails`, so it does not
establish that an unselected function was checked.

Sail restricts function-only structured reports using the requested module
and function pattern, following the pinned `user_filter.rs` matching rules:
exact match first; unique substring fallback; explicit leading/trailing
wildcards. It never combines function results from different Verus attempts.
Whole-crate timing parsing is unchanged. Unselected functions are omitted
from reports and cannot become confirmed MCP lookup results.

## Sail fork

Fixed in [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus) by
`3566421a` (fix: omit unselected functions from function timing), one commit
with its regression test
`source/rust_verify_test/tests/sail_vi0009_selected_timing.rs`, which fails on
the pinned stock source and passes on the fork. Timing entries are recorded
only for functions the user filter selects. Whole-crate runs select every
function and are unchanged.

Since [ADR 0043](../decisions/0043-default-mode-runs-the-fork.md) Sail runs the fork for every verification, default mode included. Stock runs remain only as the conformance oracle and under the `SAIL_CARGO_VERUS` development override. Sail keeps restricting function-only
structured reports to the requested selector, for stock runs; the restriction
is harmless with the fork.
