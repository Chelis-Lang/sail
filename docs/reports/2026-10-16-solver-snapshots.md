# Solver continuation: findings and blocked acceptance

Both solver-fork and one-use live-prefix candidates remain unselected.
The strict raw-output gate fails; no changed-check speedup or completion of
[#158](https://github.com/Chelis-Lang/sail/issues/158) is claimed. The complete
research source is archived rather than integrated into the shim/API/CI.
[ADR 0051](../decisions/0051-exact-solver-snapshots.md) stays proposed.

## Fork-state findings

The original preload consumed genuine NUL input as its control protocol and
admitted more parked processes than configured: a single solver parked
eight with a limit of two, and concurrent solvers parked 32. The archived
correction uses lossless framing, a version handshake before solver input,
and per-admission count/resident-memory checks. These bounds do not imply a
global active-worker memory limit.

Process copying remains insufficient. Z3 commit
[`ddb49568`](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/util/scoped_timer.cpp)
registers an atfork prepare handler which finalizes idle timer workers. On
actual pinned macOS Z3, the timer reproduction observes one thread initially,
two after a timed query and one after the old fork marker. The parent's
implementation state changes too. A global timeout's active worker is not
inherited by a forked child. The archived candidate conservatively refuses
observed unsupported threads, timers, descriptors and process limits; that
is not a complete synchronization or state-equivalence proof.

Historical native fork/replay comparisons mask statistics and include real
`num-allocs` differences. Their apparent successes are not raw acceptance.
Z3's [memory manager](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/util/memory_manager.cpp)
contains implementation allocation state; its presence does not establish
the cause of those particular differences. The recovered Anvil timings
justify investigating the cost, not adopting the implementation. Some old
Anvil commands also refused partial selectors before verification; those
failed samples remain visible in the archived evidence.

## One-use live-prefix controls

An alternative prepares a fresh pinned process on the exact prefix, keeps
its actual timer threads and consumes it once. It changes no SMT command
and injects no extra check. Preparation CPU, total CPU, memory, expiry and
fallback all count as work. Complete Session/ledger eligibility and default
integration remain unimplemented.

Independent review reproduced an executable-path ABA: a foreign image ran
between a pathname check and process creation, then the original pathname
was restored. That source returned `sat` where fresh pinned Z3 returned
`unsat`. The revised Linux prototype holds and checks the actual running
executable and pidfd before prefix input and claim; macOS live reuse refuses
and falls back freshly. All 28 bounded native controls pass, including the
actual foreign-image negative, same-process timer-worker continuation,
one-use claims, resource/input ceilings and exact tiny responses. These
controls are not arbitrary-SMT or real-project acceptance.

## Actual real-trace fresh controls

The complete changed IronKV and Anvil transcripts come from the preserved
native archive. IronKV has 23 solved checks in the exact common prefix and
one remaining check; Anvil has seven and two. Their full edited inputs are
2.46MB and 6.29MB. Command boundaries come from captured response metadata;
no query is normalized or added.

Three independent fresh runs per trace execute actual pinned Linux Z3 on a
native c7i.8xlarge in us-east-1. Each has a 300 s run bound, a 16MiB input
ceiling and a 1GiB sampled resident-memory budget. All six executable images
are checked. Every process exits 0 with empty stderr and the same replies:
24 `unsat` for IronKV, nine for Anvil. No `sat` or `unknown` occurs.

| Trace | Guarded fresh wall seconds | Literal raw gate |
|---|---|---|
| IronKV body-late | 2.190 / 2.178 / 2.177 | Fail: `:time` varies |
| Anvil body-late | 55.068 / 55.910 / 55.853 | Fail: `:time` varies |

Every changed stdout line in these recorded fresh comparisons is a `:time`
field. Other bytes, including `num-allocs`, match in this particular scope;
this does not explain or erase old fork/resume allocation differences. One
IronKV sample also records `ESRCH` reading `smaps_rollup` during process exit;
it remains a failed observer sample, without an error exemption.

Both literal fresh gates fail. The controller explicitly skips both live
samples and exits 1. No one-use latency measurement runs and no live gain
is claimed. This distinguishes equal check replies from literal clock-byte
equality without weakening the current gate. ADR 0011's existing stored
timing behavior raises a documented acceptance question, not permission to
mask fields or change the oracle.

## Reproducibility and limits

The [qualification record](../validation/solver-snapshots.md) identifies
exact source/runtime/environment, archived source bundles and every failed
attempt. The repository includes the tiny timer input and its instructions;
invasive experimental runtime code remains in the complete archived branch.
No default/runtime pin, verdict, diagnostic, query key or product behavior
changes with this report. Whole-corpus correctness and useful performance
remain open gates for any future implementation.
