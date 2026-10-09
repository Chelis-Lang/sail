# VI-0009: Partial timing success includes skipped functions

- Status: open, tracked locally
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
