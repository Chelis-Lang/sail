# Dependency receipt preflight: unimplemented plan

The unchanged benchmark on product `387ba808794c2e5d283a35d1150196c0611da84a`
has established warm repair contexts but has not yet established measured reuse
or the required speedup. The retained GCD warmups had two and three compiler
drivers respectively; the first was Historical with declarations established,
and the second was Current and accepted. The first concatenate warmup was Current
and accepted with three drivers. These counts identify extra work, but do not
attribute the recorded wall times to a particular phase.

## Source mechanism

In [the managed runner](../../crates/sail-verus/src/managed/runner.rs), the initial
cleanup scans existing readable receipts for changed input revisions. Missing,
unreadable or otherwise unusable dependency receipts are discovered after the
first Cargo invocation by `target_records`. A successful invocation with missing
receipts then cleans those packages and invokes Cargo again, replacing its
artifact journal and pending receipts. When the independent script bootstrap
owner exists, that retry also rebuilds supported script packages to preserve the
original fresh-compilation requirement.

A declaration-transition run cannot commit proof receipts without the original
Current transaction. Its next run can therefore verify the requested crate,
discover a missing dependency receipt, and verify that requested crate again
after rebuilding the dependency. The [repair engine](../../crates/sail-verus/src/engine.rs)
obtains focus and full evidence from one managed run; they do not themselves
require two Cargo invocations. The [original benchmark](../../scripts/m7-repair-bench.py)
separately requires exactly one compiler driver for each measured M7 repair.

## Conservative candidate

Before the initial invocation, under the existing proof-context and compiler-family
locks, identify reachable verified dependency packages whose actual target role
can be resolved from the supported Cargo selection. If their proof-context-local
receipt cannot meet the original package, manifest, target, kind, input-revision,
commit, dependency-byte and artifact-byte checks, schedule ordinary package clean
before the first Cargo invocation. This only requests fresh delegated verification.
It does not make a result complete or authorize reuse.

Retain `target_records` and the entire missing-receipt retry. A preflight receipt
cannot prove which artifacts Cargo will actually emit; the original post-invocation
identity and exact artifact-set checks remain decisive. Ambiguous dependency roles
retain the current path. Do not import another selector's proof receipts, learn
proof authority from ordinary compiler-family exports, clear a requested-target
force flag, or publish receipts before the original final guards.

Only implement this after the running benchmark supplies measured behavior and
the extra work warrants the change. A broader metadata graph may contain inactive
dependencies, so indiscriminate graph-wide cleaning is not an acceptable cost
claim. Additional preflight byte hashing can also cost more than it saves.

## Required meaningful controls

- A real Cargo dependency/requested-target fixture with an ordinary dependency
  artifact but no local proof receipt must invoke the dependency before the
  requested target's first verification. With preflight disabled, the unchanged
  driver-count assertion must expose the old requested-target repeat.
- A valid local committed receipt must retain dependency reuse and fresh forced
  verification of the requested target. Keep the existing post-Cargo receipt
  checks; a preflight hit alone must never produce complete evidence.
- Missing, corrupt, uncommitted, different-selector, different-target and stale
  dependency receipts must cause reproof or the existing fail-safe retry. Include
  restored-mtime dependency and artifact writes, and changed feature/target roles.
- An artifact-set change after preflight must still reach the original retry;
  failed or canceled verification must not publish pending receipts or leave the
  family/context owners held. Inactive or unresolved dependency roles must not
  acquire proof reuse authority.
- Retain complete full/focus reports and compare their verdicts and diagnostics
  with the original path. Then run the unchanged cold 240-second fixture, original
  repair fixture, and original benchmark with its one-driver and timing checks.

Existing setup phase rows and worker ChildWait/FreshnessEstablish rows can separate
runtime acquisition, target cleanup, compiler time and terminal publication.
No new instrumentation, test execution, compiler process, or performance result
is part of this file-only review. This plan does not change acceptance state.
