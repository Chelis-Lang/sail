# Native temporal worker

Status: focused native-library, isolated-process and full macOS host checks
passed. Native Linux and common-runtime integration qualification remain required.
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
  The watch-startup failure is retained without attributing a cause.
