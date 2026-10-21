# Native temporal worker

Status: focused native-library, isolated-process, full macOS host and native
Linux workspace/solver/end-to-end/conformance checks passed. Common-runtime
integration and private-dependency protected CI remain required.
The candidate is based on Sail `0581d6f4` and pins D.I.V.E.
`1025cd6a0510183bc16abc1e6e3eea4f9305c5a9`. No TLC, Java or D.I.V.E.
command executes in these checks. Existing Verus routes are unchanged.

On native Apple silicon macOS, the focused source inventory
`sail-dive-focused-source.json` (SHA-256
`79565c37382165c321f0f944b6911b956d47fdba91948405b1c028f055fed7ea`)
passes formatting, strict all-target Clippy, all 14 focused tests and doctests,
and the layering check (17 crates). The eight native tests take 0.08 seconds;
the six process tests take 1.86 seconds. These are correctness checks, not
isolated performance measurements. Raw logs remain outside the repository:

| Artifact | SHA-256 |
|---|---|
| `sail-dive-tests.log` | `09ab169511457e440806e966b2c89c8377b48489f51e42db4e51d4b116bed61c` |
| `sail-dive-clippy.log` | `75c9c1997a750563af391acd69cc6a06ec3b872ea6c5c3e087755e5264f2c379` |
| `sail-dive-layering.log` | `5e10ee9758f16a0e8759c99cca730c9a5a2736503cfee860a42b80e298f64610` |

The full macOS host gate passed all 884 tests with two test threads and
unchanged deadlines. Raw log `sail-m10-dive-macos-host.log` has SHA-256
`27e4034432952c190c1367167ed9fbcecb8756b88241cadd2772368756486f31`. The subsequent
credential-directory isolation and observed-child cancellation fixes passed
their focused macOS checks and strict Clippy.

## Scope

The focused tests exercise actual finite safety, fairness-qualified temporal
success, real invariant and temporal counterexamples, independent replay and
forged Init rejection, state-budget stops, finite sampled simulation, complete
trace transport, mathematical integers, source/host refusals and direct pinned
library parity. Isolated worker checks additionally cover request and worker
identity, positive and negative native parity, cancellation after observing the actual child for a large ASSUME
input, malformed/trailing/oversized output, unexpected stderr and a leader
that exits while its descendant retains pipes. There is no release-performance
claim and no hard RAM claim.

Commands:

```sh
cargo test -p sail-dive -p sail-runtime-worker -- --test-threads=2
cargo clippy -p sail-dive -p sail-runtime-worker --all-targets
cargo fmt --check -p sail-dive -p sail-runtime-worker
```

## Pinned native completion discrepancy

A minimal native request is the three-state counter below with configuration
`INIT Init` and `NEXT Next`, exact store and `TemporalMode::FinalShort`:

```tla
---- MODULE M ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x+1 ELSE 0
====
```

At this pin, `CheckReport::result()` returns typed safety success with an
empty remaining queue. Its original JSON says `check_kind: "temporal"` and
`execution.capture_completed: false`, because final/short was selected but
there are no temporal properties. Those bytes remain unchanged in evidence.
The adapter records `Completion::Safety`, an empty requested PROPERTY list
and the actual requested checks. It does not infer a temporal proof from the
JSON discriminator or invent a completed temporal graph. The fairness-qualified
positive control records `Completion::Temporal`, PROPERTY `P`, and a real
complete graph. The regression pins both observations explicitly.

## Earlier attempts

- Initial temporal positive fixture omitted fairness, and D.I.V.E. correctly
  returned a validated stuttering counterexample. The fixture now includes
  actual weak fairness; the original no-fairness violation remains a control.
- Initial parent validation trusted the native JSON discriminator for graph
  completion and refused a real safety success. The typed completion and exact
  requested-check inventory above fix this without changing native evidence.
- Initial fault fixtures were executable shell scripts. On this macOS host,
  four tiny script controls took 16.49 seconds and exceeded their per-call
  deadlines in parallel. Native Rust fault controls replace those scripts;
  production deadlines are unchanged. The fixture is compiled only as an
  uninstalled build artifact and is never a verifier backend.
- Source-origin admission alone did not cover native overrides installed by
  module name in a user-provided file. Static review found that boundary
  before qualification. Pre-binding IR override/effect admission and a custom
  `TLC.tla` regression now reject that capability without executing it.

- The first native Linux gate could not run the offline credential test from
  a bind-mounted worktree whose `.git` reference names a host-only path. The
  test now executes Git from its own temporary directory, independent of the
  checkout layout. No network or real credential is used.
- The next native Linux gate failed the cancellation fixture because its
  fixed 150 ms delay expired before the actual worker started; it also failed
  an existing watch-startup fixture. The corrected cancellation test observes
  this test process's actual worker with the host process inventory before
  requesting cancellation. Its production and fixture deadlines are unchanged.
  The global Cargo vendor override made the existing watcher scan Cargo
  home. Scoping the override to this workspace restored the unchanged watch
  fixture (1.97 seconds), and the complete gate then passed.

## Native Linux qualification

Source `b6a370fe` passed the complete native `ci/run.sh gate`, `solver`, all
`e2e` phases plus `extra`, and `conformance fast pr` on a dedicated
`c7i.4xlarge` in `us-east-1`, using MCP image `b66b86dca7b240b2`.
The workspace gate passed 895 tests and the solver lane passed 35 tests.
Pass-through, cache fill, warm cache and engine conformance each compared
33 units and 311 functions with no verdict/diagnostic differences or
infrastructure errors; six units retain reported rlimit drift.

The private Git source was externally vendored from the exact locked revision.
A read-only workspace-scoped Cargo source replacement selected it, preserving
ordinary registry access for the verifier fixtures. No credential entered the
host, image, source or retained logs. Tests used two threads and unchanged
assertions and deadlines. This qualifies the native backend and worker, not
Oyster's production route or a requirement assurance tier.

| Retained artifact | SHA-256 |
|---|---|
| `sail-m10-dive-native-gate.log` | `c3fd78d253bbb0e145814b29985a801322553b9de8bc8d51350f3fc2b93ec330` |
| `sail-m10-dive-native-solver.log` | `f31d215f93e64cbc4b39a5805ceaae221dc1998c49f378783b1714a5fed415d4` |
| `sail-m10-dive-native-e2e.log` | `06ff4692ba313470f1e7690eb56ac46f120ce50744b056e99f3d5e9bbf002499` |
| `sail-m10-dive-native-conformance.log` | `2744022b6ae69f639c7a0d33a8e93227fc40d1d92315a0320fc2e13bcdeb3518` |

After the main merge, source `2862d138` also passed the complete native gate
(900 tests), all 14 native/backend and process controls in the release profile,
and the full macOS host gate (889 tests). No assertions or deadlines changed.
The release process controls include actual-child cancellation, native parity,
request/worker identity, malformed and oversized output, unexpected stderr,
and inherited-pipe cleanup.

| Additional artifact | SHA-256 |
|---|---|
| `sail-m10-dive-current-main-gate.log` | `537576a459848b7a35af8c94f8f43dc6ab00eab3fc418a89c6dfb93d3d1076ba` |
| `sail-m10-dive-release-tests.log` | `7165744a9a3ceb518fb714becbb5032977f37b645007e11b2b61ab1d882baa11` |
| `sail-m10-dive-current-macos-host.log` | `341c7569b719008f28152fc965070995c3221c3032d187e1e1061c3c2fa8f2c4` |
