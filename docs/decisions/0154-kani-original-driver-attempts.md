# 0154: Bound uncached attempts through the original Kani driver

- **Status:** bounded native Linux qualification and independent review pass; unselected
- **Date:** 2026-11-14
- **Extends:** [ADR0153](0153-kani-owned-execution-tool-layout.md)
- **Tracking:** #712 G

## Context

An actual native routing experiment uses the original staged driver, its original
Rust Cargo, a fresh explicit environment and the pinned arrayvec 0.7.8 harnesses.
All three cases produce the expected distinct observations: bounded success,
counterexample and insufficient bound. Sources stay exact and tools recheck.
The pinned driver selects Cargo mode when its first argument is kani; it needs
no Rustup shim. This is execution-route evidence, not Sail backend acceptance.

## Decision

Add an unselected Linux original-driver runner with one exact harness, explicit
unwind and a 1 ms–600 s child-process budget. Authenticate the existing private
Stage before and after execution. Use a fresh private output directory, explicit
offline Cargo home and cleared child environment. Retain exact argv/environment,
stdout/stderr, exit and original export; decode only a completed native result
with the existing strict decoder. Reject cancellation, signals, missing or
excessive captures and tool changes; retain partial attempts without outcomes.
Each stream/export stays within the existing 16 MiB admission bound.

Give the child a new process group. Observe exit with waitid NOWAIT, kill its
inherited group before reaping the leader, and reap on every error path. This
keeps the original group identifier reserved through cleanup. Files prevent a
descendant holding an output pipe open from blocking capture. The process budget
does not promise interruption of filesystem authentication or bound all Cargo
target disk usage. Caller-owned immutable inputs/private parents remain required.

Reuse the existing rustix process feature; no dependency or layering change.
The contributor inspector is not sail verify --backend kani. Input/configuration/
dependency/build-script closure is deliberately unauthenticated and recorded as
such. There is no receipt, currentness, proof authority, cache admission or
ordinary verified verdict. This runner must not be wired to those paths until
their separate closure and acceptance work is complete.

## Consequences

This makes the actual driver route concrete for subsequent backend integration.
Native process/cancellation/refusal controls, original full captures, independent
review and real harness execution are required. Full arrayvec/direct comparison,
source/environment/bound authority, CLI/MCP, cache separation, Mac qualification
and release gates remain open. Original Tools/Stage authentication stays strict.

See [original-driver qualification](../validation/kani-original-driver.md) for
actual results and scope. This selects no backend or source-closure/cache authority.
