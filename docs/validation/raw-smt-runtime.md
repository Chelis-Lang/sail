# Exact raw SMT runtime

The first raw-SMT runtime slice is qualified locally on source `e26b7b65`.
The subsequent merge of main `4636ff20` takes its Cargo benchmark work without
changing any crate source, `Cargo.toml` or `Cargo.lock` from that candidate.
Protected CI remains required before merge. [ADR 0065](../decisions/0065-oyster-runtime.md)
and the [runtime specification](../spec/runtime.md) define this slice.

## Environment and results

Native Linux validation ran on a dedicated `c7i.4xlarge` in `us-east-1`,
using the standard MCP CI image tagged `b66b86dca7b240b2` and Rust 1.99.0.
No performance lane shared the host. The image supplies the pinned Z3 4.16.0
and Verus engine `576558fc76ee`. The source adds a raw-SMT route; ordinary
Verus verification keeps its implementation and oracle.

| Gate | Actual result |
|---|---|
| `ci/local.sh gate` | Passed; 889 tests passed, plus formatting, Clippy, doctests, rustdoc, layering, dependency and documentation checks |
| `ci/local.sh solver` | Passed; 38 actual-solver and fault tests passed |
| `ci/local.sh e2e` | Passed every phase and `extra`, including real MCP client and repair acceptance |
| `ci/local.sh conformance fast pr` | Pass-through, cold/warm cache and engine arms each compared 33 units and 311 functions: no verdict/diagnostic differences or infrastructure errors; six units retain reported rlimit drift |
| Release CLI | Actual pinned readiness, named unsat core, exact warm replay with caller reattribution and forged-answer admission refusal passed |
| macOS `ci/run.sh host` | Passed all 878 host tests on arm64 macOS 26.5, with two test threads and unchanged assertions and timeouts |

The runtime's direct pinned-Z3 oracle runs before caching. It compares exact
stdout for models, values, named unsat cores, enum datatypes, large integers
and incremental push/pop transcripts. Additional checks exercise cold/warm
cache attribution, exact whitespace and limit keys, corruption, explicit
uncached unknown, command forgery, output limits, deadlines, cancellation and
inherited pipes. These establish raw solver behavior, not Rust contracts or
requirement assurance tiers. No legacy-path performance gain is claimed.

## Retained evidence

Raw logs stay outside the repository. Hashes bind the complete files:

| Artifact | SHA-256 |
|---|---|
| `sail-m10-gate.log` | `fa877bfae33692db84ce666da6ca860d1ac94657efeec0001f2f3df1a2711a72` |
| `sail-m10-solver.log` | `79fef5624b0d6bcab02178e5dcd9efcbdcde387fa52560d30416e5dd56c6b662` |
| `sail-m10-e2e.log` | `cf8d32bc1e4b6f774dbccbbdb641a28ee2694ad4c5b545499d64ccb5e37c0afd` |
| `sail-m10-conformance.log` | `811ca9da670d8284719ed6069424fdb688aeab885e92b06053e10d9b619cfa82` |
| `sail-m10-cli-smoke.json` | `6e4151489c33e364a9e117cbb1a0eb9811a6304fc1d247f572caa4900657e127` |
| `sail-m10-macos-host-bounded.log` | `87ca7e905a9cd2a5d95e79f7122f44639348aee7e6701f52dbe32a4331354a81` |

Earlier attempts remain visible:

| Source/date | Result | Cause or limit |
|---|---|---|
| `e26b7b65`, 2026-10-21, full-parallel macOS host | Failed one existing watch test; isolated repeat passed | Extra startup notification observed; cause not established by this slice; raw log `7aef92980a92b965af8962b4f9020e754f347f4560267e86263a4a96dbc58ec9` |
| `e26b7b65`, 2026-10-21, second full-parallel macOS host | Failed output-limit process fixture and an existing shim completion fixture | Timeout/startup ordering observed; cause not established; raw log `1b8982d2f68da4b9fc7465d2019bb6028765667fab3ddb24f08a693222025f9e` |

| `e26b7b65`, 2026-10-21, release CLI evidence capture | Assertions passed but evidence-file write failed | Bind-mounted temporary file permission failure; stdout capture then completed successfully |

The complete two-thread run passed without changing production deadlines,
fixture deadlines, assertions or conformance rules. Daemon, temporal backend,
neutral-contract proofs and the Oyster production adapter require their own
acceptance records.
