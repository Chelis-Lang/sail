# Runtime daemon, CLI and MCP qualification

Focused execution passed on native macOS arm64 on October 21, 2026, with two
Cargo jobs and two test threads. This record covers the daemon implementation
stacked on raw-runtime commit `e26b7b65`, not full integration acceptance.
The pinned Z3 4.16.0 executable SHA-256 was
`edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb`.

Commands, with `SAIL_TEST_Z3` naming that existing pinned executable:

```sh
CARGO_BUILD_JOBS=2 cargo test -p sail-daemon runtime::tests -- --include-ignored --test-threads=2
CARGO_BUILD_JOBS=2 cargo test -p sail-cli --test runtime -- --include-ignored --test-threads=2
CARGO_BUILD_JOBS=2 cargo clippy -p sail-api -p sail-daemon -p sail-cli -p sail-mcp --all-targets
cargo xtask check-layering
cargo fmt --all -- --check
git diff --check
```

All nine RPC controls passed, including actual pinned execution/cache parity
with embedding, actual cancellation/disconnect, malformed frames, wrong result
identity, private permissions, duplicate requests, responsive control capacity,
shared idle activity and blocked response delivery. Both actual CLI/MCP tests
passed: stdio MCP autostart and explicit CLI socket use preserve unavailable
outcomes without fabricated checks; the pinned positive case preserves exact
solver output, checks and identity across MCP and a CLI cache hit. Clippy,
layering, formatting and diff checks passed.

Raw logs remain outside the repository, identified by SHA-256:

| Record | SHA-256 |
| --- | --- |
| RPC controls | `0da1e2d5b0af69c22fa37ee29a80dbb208f7f0480bd2584874f2e14917bf084b` |
| CLI/MCP controls | `4415f013543ef4004a9646748355bdc633369af20bfd8e59b2280d04ca4029f1` |
| Clippy | `ee82428ac0fb0fae42ccff5f1fec9c80c8553ef4c99b9f379e1ce64f2fbf82e7` |
| Layering | `f644c6cfa236e3d417c3fba8df302a50f0ecfc698e499454465f7b6208df0f4c` |

Full macOS host, native Linux gate and existing Verus conformance/end-to-end
qualification remain separate gates. These controls do not qualify temporal
execution, neutral contracts, Rust-code assurance or hard aggregate RAM bounds.

The final macOS host gate at source `ff2f4366` passed all 891 non-solver tests
with 43 explicit skips and workspace Clippy, using two Cargo jobs and two
nextest threads. The sealed-environment followup from `612745eb` is included as
`c67ae9a5`; all eleven focused runtime tests also passed with the actual pinned
solver, including the ambient-environment regression. Host log SHA-256 is
`1cee3648ff6be8c154beb93b28500c7d46b3664ff65ebdb4e25d598b28c6db0a`;
focused runtime log SHA-256 is
`6e5ad0be295846245568340715706a18c138d962db6f704610450f96ae5c56c9`.
Native Linux and full Verus integration qualification follows below.

Earlier attempt: `c67ae9a5`, macOS host, 889 passed and two failed. Both failures
were exact MCP tool inventories missing the two implemented runtime tools;
the corrected inventories preserve exact full-list checks. No solver verdict
or backend evidence check was relaxed.


Native Linux source `e4a384dd` completed the full required sequence with exit
zero: gate, solver, full end-to-end plus extra, PR conformance, full fast and
standard conformance. Gate passed 907 tests; the solver lane passed 41 tests.
All four PR arms matched 33 units and 311 functions. All four full-fast arms
matched 147 units and 3,449 functions. All four standard arms matched 159 units
and 12,725 functions, with no differences, instability or infrastructure errors;
19 standard units had recorded resource-limit drift. The engine arm included
158 focused units. This qualification is correctness evidence, not a
performance measurement.

The original gate attempt passed its tests but failed the installed MCP
inventory example. The corrected exact inventory was rerun through the full
sequence above. Complete original logs and conformance reports are retained in
`sail-daemon-full-native-evidence.tar.gz`, SHA-256
`ad814460d9a648ab1e12e93874883deb0efc7c9d10379b6f6c27edf77cad2e17`.

The branch then merged main `a0e5649f`, including independently qualified raw
runtime and neutral-contract lowering. The merged implementation `f35156e4` passed its current-source native full
gate with 930 tests and all formatting, strict workspace Clippy, documentation,
layering, dependency and benchmark-page checks. Its current-source macOS host
gate passed 919 tests with 44 explicit skips and strict workspace Clippy.
Actual pinned runtime, RPC and CLI/MCP followups passed 11, 9 and 2 controls,
respectively. The complete standard result above belongs specifically to
`e4a384dd`; it is not represented as a standard run of the later merge.

Current-main logs, retained outside the repository:

| Record | SHA-256 |
| --- | --- |
| Native full gate | `72f984002a06586608c81684c224fff7daf6a99e6dfe6330a7ffd2ec3fa955e4` |
| macOS host gate | `c59cab2386c5f5826966cabba005fca1d8e6c8f741cb68b85dd908b07710c3a3` |
| Actual pinned macOS runtime/RPC/CLI | `44b934ac16411b1099cebfe29971fb9787d588d09ec679ca52bff5f724a0ce0c` |
