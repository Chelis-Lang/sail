# Solver snapshot prototype

Status: local hardening checks pass; native Linux and raw-output acceptance
pending. Disabled by default. No release uses this candidate.

Source: hardening immediately after `88076b62`, with native source
`native/sail-snap/sail_snap.c` SHA-256
`feee55f411ae04ed4af4b25db17fc6198c042b8f76b3211bdc5c5777454a100e`.
The runtime oracle for the bounded macOS correctness checks is the pinned
Z3 4.16.0 from the combined `8a373f81` source runtime. These tiny checks exercise
transport and resource admission; they do not measure performance.

| Gate | Result | Scope and limits |
|---|---|---|
| Native preload regressions, macOS | Pass | 9 checks: NUL/string/comment/all bytes, resumed NUL, single and concurrent count, one-byte memory cap, zero count |
| Rebuilt shim with daemon, macOS | Pass | 2 additional cold checks, armed preload and incompatible preload; actual solver start required, stdout/stderr/exit match plain Z3 |
| Selected Rust tests | Pass | `sail-api`, `sail-shim`, `sail-bench` library tests, 78 total |
| Selected clippy | Pass | Those three packages, release profile, all targets, warnings denied |
| C warnings | Pass | macOS `-Wall -Wextra -Werror` |
| Native Linux, full host/gate and conformance | Pending | No new host or competing measurement job started |
| Revised resource-bound performance | Pending | Earlier candidate timings do not qualify these bounds |
| Raw-byte exactness across the corpus | Pending | Masked statistics are not acceptance |

Raw evidence stays outside the repository, in the snapshot review evidence set:

| File | SHA-256 | Result |
|---|---|---|
| `old-tests.json` | `67783489700cf1042efb905639b4a6aaf394f06925c73cb5f63802445aeb8e65` | Old prototype fails 6 of 9 regression checks; count 8/2 and concurrent 32/2, ignores memory/zero budgets |
| `corrected-tests.json` | `d798add7e412242b8faeb8a73a46d02d3cfc375ab90c87501a2efded83e474fc` | Revised library and rebuilt shim pass all 11 checks |
| `rust-tests.log` | `afc098bdb87429b606ff08b24db3039937438be0d6fec2e811310b21bd5ba883` | 78 selected Rust tests pass |
| `clippy.log` | `a7531379dd862d1e047ebcce1ce5da565ab3ffa88129aa25cf5c35cac54a4f00` | Selected packages and all targets pass |

Failed/interrupted attempts remain visible:

- `88076b62`, 2026-10-16: original preload changes real NUL-containing input
  and exceeds parked limits; deterministic negative controls above.
- Candidate hardening, 2026-10-16: the first shim regression omitted creation
  of its provenance directory, so solver start was unestablished; the malformed
  preload also exposed premature forwarding of loader stderr. The final test
  requires provenance and the implementation defers that forwarding until the
  preload acknowledges its framing.
- Earlier native `exact-ironkv-base` run: statistics comparison differences
  reported; no claim of a raw exactness pass. The recovered experiment's
  zero-record verismo output does not qualify that project.
- Earlier Anvil managed partial-selector measurements: invocation refused
  before verification; retained as failed samples in the recovered archive
  cited by the [report](../reports/2026-10-16-solver-snapshots.md).

The source walk, syscall interposition, stale/partial socket recovery,
platform behavior and active-worker policy need independent review before any
default integration. [ADR 0051](../decisions/0051-exact-solver-snapshots.md)
remains proposed.
