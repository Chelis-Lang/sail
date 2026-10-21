# Exact raw SMT runtime

The raw-SMT runtime slice, including its sealed execution environment, is
qualified locally on source `612745eb`. The branch also takes main's Cargo
benchmark and watch/store fixes. Protected CI remains required before merge.
[ADR 0065](../decisions/0065-oyster-runtime.md) and the
[runtime specification](../spec/runtime.md) define this slice.

## Environment and results

Native Linux validation ran on a dedicated `c7i.4xlarge` in `us-east-1`,
using the standard MCP CI image tagged `b66b86dca7b240b2` and Rust 1.99.0.
No performance lane shared the host. The image supplies the pinned Z3 4.16.0
and Verus engine `576558fc76ee`. The source adds a raw-SMT route; ordinary
Verus verification keeps its implementation and oracle.

| Gate | Actual result |
|---|---|
| `ci/local.sh gate` | Passed; 890 tests passed, plus formatting, Clippy, doctests, rustdoc, layering, dependency and documentation checks |
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
| `sail-m10-final-gate.log` | `c03a41c4ff7572b7c2df42e52059a19c6e09915a62ec7a66e0a78012562f370d` |
| `sail-m10-final-solver.log` | `fca2b4892b4547fd7f3f076a0c1226a34e382b464b54007bddfc80d1ff14093b` |
| `sail-m10-final-e2e.log` | `be3948162302bbe044c7098f7518a3eb100ec2a167dcbe8078b92b6c056beede` |
| `sail-m10-final-conformance.log` | `5aca354aca6ca8b730cf8752aa7e2f8684db00481fae86f49a0945d19aa57092` |
| `sail-m10-final-cli.json` | `6e4151489c33e364a9e117cbb1a0eb9811a6304fc1d247f572caa4900657e127` |
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

The sealed-environment followup also passed all 11 focused macOS runtime tests,
including the actual pinned solver and inherited-environment regression, on the
stacked daemon candidate. The original full host run above qualified the first
slice; the daemon's separate acceptance record qualifies its added transport.

After merging main's watch/store fix `35b09255`, the complete native workspace
gate passed again on `e8834444`. Its retained log
`sail-m10-current-main-gate.log` has SHA-256
`4f9adb8ea0089405d1946918772617166f3d9fa503f44d776c6c50049708174c`.
