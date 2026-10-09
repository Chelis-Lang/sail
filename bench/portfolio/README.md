# Portfolio cost probe

Generated from `cost-aarch64-macos-devload-2026-10-09.json`: three paired rounds per scope on a shared macOS arm64 development machine. These are small protocol/selection probes, not Linux reference speedup benchmarks. Both cold and warm runs use `accept-first-proof` mode and its isolated cache.

| Requested scope | Plain median (range), ms | Portfolio cold median (range), ms | Portfolio warm median (range), ms | Outcome |
|---|---:|---:|---:|---|
| good | 2870.7 (1563.7–3079.1) | 3948.2 (2394.7–5140.3) | 4970.7 (2832.5–6873.1) | baseline seed 0 wins; skipped failing function omitted |
| whole-file | 2319.0 (2127.9–3609.3) | 14485.2 (9828.3–15469.5) | 12138.1 (11204.5–17093.8) | all three attempts fail; baseline failure preserved |

Warm runs serve cached checks; compiler and CLI work still run. Alternative configurations carry separate prefix keys. A hard unknown at its resource limit is always re-solved, so this probe does not predict the warm cost of page-table’s fragile allocation proof.

## Independent allocation proof

Retained in `page-table-allocate-aarch64-macos-2026-10-09.json`: seed 4 proved exactly `impl_u::wrapped_token::WrappedMapToken::allocate` on macOS arm64, with 448,058,976 resource units and zero errors. The observed effective limit was 3,000,000,000 (14.9% used): the function-specific attribute overrides the global `--rlimit 20` flag. The unchanged baseline reached its wall timeout. Both attempts, solver hash and proving configuration are retained. This confirms only the requested allocation function, not the entire page-table crate, and is not a reference-platform speedup benchmark.
