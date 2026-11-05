# How Sail reuses verification

An agent may check the same file several times, change one function, try a bad
repair, then undo it. Sail reuses solver answers, complete target proofs, and
whole unchanged calls. Each saves work at a different point in that loop.

## Solver answers

Verus translates proof obligations into queries for a solver. Sail's solver cache is
keyed on the exact query bytes, solver identity, and effective options. If a later
call asks the same query under the same execution settings, it can use the stored
answer. A changed query needs solving again.

This helps even when the Rust compiler and Verus frontend still have work to do. The
[trust model](trust.md) explains the local, shared, and bundled cache tiers and how
replies are admitted. For SMT-LIB submitted directly through `sail runtime`, the
[runtime guide](runtime.md) describes its separate transcript cache, including
models and unsat cores.

## Complete proofs

A target that verifies fully, without function filters, can establish a proof receipt. When Cargo finds that
target fresh and its receipt still matches, Sail can reuse the report from the
successful run without running Verus for that target. Changing one target need not
repeat every other target's proof.

Receipts cover the selected inputs and verification context, not just the text of a
function. Sources, dependencies, generated inputs, options, tools, and configuration
can affect whether a result is reusable. The [workspace
guide](workspace-verification.md) describes the checks; the [freshness
guide](freshness.md) explains Current, Historical, and Unknown. Current means the
inputs match. It does not mean the proof passed.

## Unchanged calls and undo

When an invocation qualifies for [unchanged-call replay](decisions/0082-unchanged-call-replay.md) and still matches a recorded successful call, Sail
can replay its result without running Cargo or Verus. It rechecks the files,
settings, and tools recorded by the earlier call. If they no longer match, or cannot
be checked, it verifies normally.

Returning to an earlier revision can also restore a retained proof. The [undo
guide](instant-revert.md) describes Cargo eligibility and retention costs.
[Standalone files](standalone-verification.md) have their own input checks and
bounded revision history. Neither workflow treats an unobserved input as unchanged.

## What still costs time

A changed target still needs compilation and Verus frontend work. A query absent
from the local, shared, and bundled caches still needs a solver run. Reading and
validating inputs also has a cost, which can outweigh the saved work on very small
projects.

[Background preparation](background-preparation.md) can do work for the next
unchanged Cargo call after returning the current result. It does work ahead of time
rather than skipping it, and uses additional CPU and disk. Set
`SAIL_BACKGROUND_PREPARE=0` to turn it off.

The [performance page](performance.md) gives measured edit-loop gains and the cases
where Sail was slower. For a fresh CI runner, [CI caching](ci-cache.md) explains
which directories to retain and what each tier saves.
