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
Native Linux and full Verus integration acceptance remain pending for this slice.

Earlier attempt: `c67ae9a5`, macOS host, 889 passed and two failed. Both failures
were exact MCP tool inventories missing the two implemented runtime tools;
the corrected inventories preserve exact full-list checks. No solver verdict
or backend evidence check was relaxed.
