# Native-binding reader workflow: prerequisite timeout

The one original workflow on product `46e1da6` is closed and failed its bounded
baseline ordinary-full verification prerequisite. The command timed out at its
unchanged 600-second deadline (600.058976 s including owned termination), with
exit code −2. No Python or Rust census sample was collected, and retained
CLI/MCP integration was not attempted. This result provides no reader latency
credit and does not replace the [earlier measured failure](m7-rpc-stage-workflow.md).

The [portable terminal record](m7-rpc-binding-reader-terminal.json) binds the
[prepared identities](m7-rpc-binding-reader-prepared.json), complete preparation
reports, partial streams, observer metadata, timers and immutable postseals.
Production source `46e1da6b1d932f72b78136dabd6747b8009574e9` passed its separate
566-test complete Linux gate and 39 solver controls before this run. The
selected engine is the b346 baseline, not the unqualified source candidate.
Fresh independent source-only copies and a new cache were used; no previous
target, receipt or digest authority was copied. Original controllers,
collectors, 600-second checks, five-second RPCs and fail-fast behavior were
preserved. No proof or workflow was repeated after the failure.

## Successful preparation and failed check

Separately recorded unmeasured preparation completed for both workspaces:

| Workspace | Ordinary full verification | Repair-context preparation | Total preparation |
|---|---:|---:|---:|
| Baseline | 238.055071 s | 292.795251 s | 531.311237 s |
| Active | 161.077651 s | 257.641150 s | 419.332375 s |

Both public results were complete, accepted and Current, with no notes or
errors. Each reported all 1,871 vstd and 331 HumanEval functions verified and two
actual compiler invocations. These are genuine preparation proofs, not
one-driver, speedup or reader acceptance claims. No declaration retry was
needed. Total unmeasured preparation was 950.643612 s.

The subsequent bounded ordinary verification used the same baseline context
and input revision. Its retained `context-ready` event reports complete source
observation. The last progress event starts the HumanEval target; no subsequent
completion event is retained. Standard output is empty, and the 88-byte stderr
ends with `Checking human_eval_verus`. This does not establish whether a
compiler, solver, Cargo coordination or another stage caused the timeout.

## Observation limits and closure

A bounded, metadata-only snapshot taken 559.924987 s after the context run
start retains six actors: Sail, two read-lease helpers, cargo-verus, sail-cargo
and Cargo. It includes PID birth ticks, CPU ticks, scheduler counters, bounded
descriptor targets and command arguments, with no environment or pipe reads.
Its traversal followed only each process's main-task children. Cargo can launch
children from another task, so the snapshot omitted verifier/shim descendants
and is not a complete owned process tree. No timeout cause follows from those
six actors. The next observer control must cover every task and bind PID birth
identity before relying on a missing descendant.

The shutdown recorder contains the latest 256 preparation/status rows, started
2,091 observations and overwrote 1,835, with none dropped. There are no census
rows to attribute. Its 83,128-byte export and owned launch header are sealed.

The wrapper took 1,556.512532 s internally and 1,556.903126 s externally. The
owned daemon exited normally with code 0 in 0.118016 s. Cleanup, archival and
finalization error lists are empty; all product, thirteen selected runtime
artifacts, source template and collector postseals match. The launcher,
controller, daemon and ordinary Sail process are absent after closure. Four
captured descendants remain zombies; none is a live captured actor.

The private raw archive contains 57 regular files and 67,688,695 uncompressed
bytes. Its 1,925,860-byte compressed copy was rehashed after transfer, with
SHA-256 `16aa9da3c1987ebb8413212b8de2d9b25a72ca2abc2ca4b9094b1053a814e869`.
Workspace/cache authority and environment-bearing job records are excluded.
The partial actor snapshot and concrete launch/controller records are retained
separately with byte identities. See [reader acceptance](../reader-acceptance.md),
[RPC diagnostics](../result-rpc-diagnostics.md), [the repair guide](../proof-repair.md),
[known issues](../known-issues.md) and [the roadmap](../ROADMAP.md).
