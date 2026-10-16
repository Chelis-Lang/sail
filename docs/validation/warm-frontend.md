# Warm frontend qualification (#162)

**Status:** profile recovered; first implementation candidate built and
unit-checked. No frontend performance change is selected or shipped.

## Candidate and checks

- Fork source `2f9669d2`, based on `a9ce7c13` / runtime `73dea021`: reuse
  existing immutable AST nodes during read-only reachability traversal.
- Local macOS arm64, Rust 1.98.1, pinned `engine/verus/Cargo.lock`.
- Both `vir` library unit tests pass. The new regression compares identity
  rewriting with read-only traversal: exact expression/type/place callback
  order, binding scope and root reconstruction. Clippy of all `vir` targets
  with warnings denied passes.
- Benchmark command regression: all four tests pass. They verify focused
  versus full Cargo-Verus commands, the fork solver/shim/owned socket binding,
  and refusal of unknown modes. No compiler or solver runs in these tests.
- Separate key-projection source `2371ca51` on the `520f3b57` measurement
  branch: four JSON serialization/isolation/scope tests pass; `rust_verify`
  checks successfully; clippy for the selected projection test passes. It
  measures a possible key and never skips fresh lowering or verification.
- No candidate runtime, corpus, exact-SMT comparison or performance
  measurement has been qualified. The tests above do not establish a gain
  or equivalence over real projects.

Raw logs are in the external `frontend-review-2026-10-16` evidence set:

| Artifact | SHA-256 |
|---|---|
| `readonly-walk-test.log` | `d81f488c81e29e7d99eea3820c890651e96deb8b24f5715a5f63428d90d6b969` |
| `readonly-walk-clippy.log` | `a2db7a82dd2f91484b2976605b34d3f8a3421b54e72cb806cf4121e09eadabbf` |
| `key-projection-test.log` | `20ca1717e589501587d25697e33c53fc662ed5e08c5cfe547cae1bf5edafcb77` |
| `key-projection-check.log` | `1951f1dd0bdd7d2c195b9ac7196ec09df9aea98a3353fe843631181f74270b2f` |
| `key-projection-clippy.log` | `95e83b967575ed6a1de786feb1306ee655f450f6265e1d8957212a0e547928aa` |

## Profile scope and failed attempts

The [profile report](../reports/2026-10-16-warm-frontend.md) identifies
source/runtime/environment and each raw input's checksum. Three projects
have 32 successful samples each; these are profiling evidence with the
stated source-attribution and conformance limits. The intended all-hit
IronKV scope still has one solver start. Direct-shim timing samples lack
independent cache coverage counters.

- `520f3b57`, 2026-10-16: all 14 focused managed Anvil attempts fail before
  verification. Cause: `sail verify` correctly reproduces the refusal of
  partial selectors under Cargo-Verus `verify`; the plain arm uses `focus`.
  No paired Anvil speedup or frontend acceptance exists.
- `520f3b57`, 2026-10-16: raw pruned-VIR key proxy does not support measured
  body/comment edits: zero matching keys despite many matching SMT digests.
  Exact cause and a complete stable key remain unresolved. Persistent reuse
  is not implemented.

## Remaining gates

Independent source review, native Linux build/gates, macOS and Linux runtime
regressions, deterministic exact-SMT comparisons, full fast/standard
conformance and native performance measurements remain required. The
comparison must include cold and edited runs and every recorded failure;
previous timings do not qualify the candidate. A positive native gain must
be established before a fork runtime is selected. ADR 0052 remains proposed.
