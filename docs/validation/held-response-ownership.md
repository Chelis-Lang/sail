# Held-response ownership qualification

Status: a bounded production adoption is implemented in a candidate based on
main `e9862433`. Frozen `144db36d` passes its whole-query positive, all six new
ordinary controls and focused native Rust checks. The whole positive reports
291 verified checks, zero errors and all 476 functions, retaining the former
464 and adding 12 held-response functions. The complete combined control and
runtime gates remain pending. No performance result is claimed.

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
| Source/checker tests | All 59 pass at committed `0349afa6`; combined-source qualification remains separate |
| Bounded direct Stock checks | Frozen `144db36d`: whole positive 291/476; all six controls fail ordinarily with 290 verified, one error and all 476 functions |
| Complete combined control matrix | Pending; the 177-case registry is preflighted, not fully executed here |
| Model suite and unchanged-input evidence | Pending exact input comparison |
| Focused native Rust | Strict Clippy for acceptance/shim with all targets; all 12 shim library tests pass, zero skipped, including both new regressions |
| Native workspace, solver and full E2E | Pending combined acceptance |
| Full fast/standard conformance | Required: production query association is selected |
| Added validation cost | Pending bounded native comparison; no performance claim |

All Stock calls retain normal whole-crate scope, one thread, default resource
limits and the 120-second deadline. Signals, syntax errors and resource failures
do not qualify as successful controls. No complete service, OS/I/O, cache-origin
or cryptographic theorem follows from this component.

Qualification uses native x86-64 Linux on a c7i.4xlarge in us-east-1, normal
core CI image inputs `587662778c7e8f88`, Sail Rust 1.99.0 and the pinned stock
Verus `0.2026.10.18.7d82994` with Rust 1.99.1. All seven final calls retain
complete captured programs and normal termination. Final source/runtime/Rust
guards pass. The earlier one-call inventory discovery at `0349afa6` remains a
separate prefix; eight actual Stock calls were made in total.

Closed private artifact:
`experiments/2026-10-28/held-response-144db-bounded-native.tgz`,
16,771,294 bytes, SHA-256
`824e17f2b97cc7b8a10d93569471083ea8280070e7c619326fbf8aa1982965fb`.
Immutable PUT and complete fresh GET/hash comparison pass. The read-only packet
audit checks canonical Git source, every generated driver/dependency byte,
all raw Stock results and the exact native test names/counts. Its source and
result are retained as companion `.audit.py` and `.audit.json` objects. These
are audits of the bounded calls, not execution of the remaining controls.

The separate 59-test source log is retained as
`experiments/2026-10-28/held-response-0349-source-tests.log`, SHA-256
`853b3695a475ccfc6b5e1f937ebb03a6786781462fe38903d979ec8940bf8b25`.
Its immutable PUT and complete fresh GET/hash comparison also pass.

| Date | Source | Attempt | Result |
|---|---|---|---|
| 2026-10-28 | Uncommitted extraction | Initial source suite | 58/59 pass; the final-drift fixture correctly captures committed Git sources and cannot find the new uncommitted module. No Stock call or proof credit. Commit first, then rerun unchanged checks. |
| 2026-10-28 | `0349afa6` | First native setup | The root controller correctly refuses the Ubuntu-owned checkout before image build or Stock calls. An exact-checkout, per-process `safe.directory` setting fixes only orchestration; the original controller exit 1 remains a failed setup. |
| 2026-10-28 | `0349afa6` | Native whole-query discovery | One whole positive passes 291/476 with all 177 registry entries preflighted and final source/runtime/Rust guards. This is count discovery, not execution of the complete controls. |
| 2026-10-28 | `144db36d` | Locked bounded matrix and focused native tests | Whole positive, all six ordinary controls, strict Clippy, 12/12 native tests and final guards pass. Complete combined acceptance remains pending. |
