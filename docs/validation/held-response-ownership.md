# Held-response ownership qualification

Status: a bounded production adoption is implemented in a candidate based on
main `e9862433`. Native whole-query discovery passes at `0349afa6`: 291 verified
checks, zero errors and all 476 functions, retaining the former 464 and adding
12 held-response functions. The complete affected controls and native runtime
acceptance remain pending. No performance result is claimed.

`query_holding.rs` is extracted from `97550ed3` without parser, output-owner,
key, compiler-input or cache-storage representation changes. The production
shim holds its original `BoundPair<Expect, Vec<u8>>` separately from effort
metadata. Three constructive controls cover original-pair preservation,
the actual `[1,3,2,4]` publication order and orphan-metadata refusal. Six
inherited executable mutations remain required ordinary verification failures.
All 139 prior query controls and their complete function inventory remain.
The manifest locks the actually discovered enlarged positive count. The pure
`release_before` convenience and its constructive witness remain checked; the
native adapter uses `take` and its existing pushes to avoid an extra allocation.

The checker captures the entire native caller and checks its reviewed digest;
its source roles explicitly exclude that adapter from proved bodies. The
private `Holding` fields, imports, same-body syntax, complete dependency and
control inventories remain checked. Native regressions cover check replacement,
original effort/reply identity, orphan annotations, exact-once final release,
and permanent stopping. Existing statistics/reason ordering checks remain.

| Gate | Current result |
|---|---|
| Source/checker tests | All 59 pass at committed `0349afa6`; final-head rerun pending |
| Complete query positive and ordinary controls | Native discovery 291/476 passes; locked positive plus six new ordinary controls pending |
| Model suite and unchanged-input evidence | Pending exact input comparison |
| Native workspace, solver and full E2E | Pending |
| Full fast/standard conformance | Required: production query association is selected |
| Added validation cost | Pending bounded native comparison; no performance claim |

All Stock calls retain normal whole-crate scope, one thread, default resource
limits and the 120-second deadline. Signals, syntax errors and resource failures
do not qualify as successful controls. No complete service, OS/I/O, cache-origin
or cryptographic theorem follows from this component.

| Date | Source | Attempt | Result |
|---|---|---|---|
| 2026-10-28 | Uncommitted extraction | Initial source suite | 58/59 pass; the final-drift fixture correctly captures committed Git sources and cannot find the new uncommitted module. No Stock call or proof credit. Commit first, then rerun unchanged checks. |
| 2026-10-28 | `0349afa6` | First native setup | The root controller correctly refuses the Ubuntu-owned checkout before image build or Stock calls. An exact-checkout, per-process `safe.directory` setting fixes only orchestration; the original exit 128 remains a failed setup. |
| 2026-10-28 | `0349afa6` | Native whole-query discovery | One whole positive passes 291/476 with all 177 registry entries preflighted and final source/runtime/Rust guards. This is count discovery, not execution of the complete controls. |
