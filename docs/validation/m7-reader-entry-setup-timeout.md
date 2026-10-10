# Frozen reader-stage setup timeout

The [portable record](m7-reader-entry-setup-timeout.json) preserves a failed
fresh reader-stage attempt on source `8e09eaa`, the matching selected `b346`
engine, and the original `6ca` Python/Rust collectors. This is Linux amd64
Docker execution under emulation on macOS arm64 with shared development load.
It establishes no latency, proof or overall M7 acceptance.

The current reader controller created fresh independent HumanEval source copies
and a private cache. It copied neither prior receipts nor Cargo output. Engine
import and metadata succeeded. Ordinary full baseline preparation exceeded its
unchanged 600-second invocation limit at 600.227 seconds; owned interruption
ended the child with signal 2 and the controller exited 1. Its final project
stdout was empty. No baseline was admitted, neither census started, and no
retained CLI/MCP check ran. The 30-second command, five-second RPC, four-client,
200-sample and p95-below-200-ms requirements remain unchanged.

The retained Cargo stderr reached the requested HumanEval crate after dependency
compilation. Its retained dependency driver report completed `vstd` with
2,060 verified and zero errors, reporting 371.595 seconds total and seven
verification threads. This is dependency evidence, not a complete requested
HumanEval proof. Retained timestamps place dependency report completion at
approximately 555.33 seconds after ordinary setup began. The HumanEval driver
was created 559.91 seconds after its owning Sail process, leaving approximately
40.32 seconds before interruption. This demonstrates that most of the setup
budget preceded the requested crate; it does not establish a solver hang. The owned process witness captured a Verus driver and actual Z3
children. Verus-owned SMT transcript logs retained 2,151 completed check responses
across 454 transcripts. These partial responses are not a sealed whole-target
proof. Pending logged queries establish log intent, not receipt of bytes by Z3;
they must not be used to assert a protocol deadlock or completed solver work.

The final HumanEval verifier's kernel counters recorded 26.24 user CPU seconds
and 5.36 system seconds. Five surviving Z3 process counters summed to 1.65 user
and 0.24 system seconds. These are partial, identity-bound process observations,
not a complete run CPU census. They do not establish pure shared-load delay or
resource exhaustion. Earlier ordinary preparations completed in roughly 373 and
519 seconds on the older `a6` product; those runs had separate contexts and cache
state and are not controlled speed comparisons.

The owned daemon exited 0 in 124.5 milliseconds. Cleanup reported no errors and
no live descendants remained in the original process group. Eighteen exited
children remained as zombies owned by the container init process; no unrelated
process was killed. Exact reports, sidecar transcripts, controller logs and
source/collector/runtime hashes remain in the private raw archive identified by
the portable record. No broad repeat or deadline extension followed this failure.

The earlier [live-reader failure](m7-a6-b346-reader-census-failure.json) and
[retained-job cohort diagnostic](m7-reader-entry-coalescing-diagnostic.json)
remain separate evidence. The latter obtained all Current responses on an idle
retained job but still missed latency; it does not replace a real live-verifier
census. The [reader acceptance guide](../reader-acceptance.md) defines admission
for future runs.
