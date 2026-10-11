# Original managed acceptance with setup timings

The unchanged standalone `ci/test-managed.py` completed successfully against
frozen product `012707a`, retaining the original 240-second per-managed-call,
120-second plain-Verus, 30-second CLI and five-second RPC bounds. Its total
447.827-second duration includes the complete original script. No dependency
prewarming, copied receipt, compiler flag, input or assertion change was used.
The [portable record](m7-managed-setup-012707a.json) binds source/archive,
Sail/shim, pinned runtime, controller/helper/timer and every retained raw file.

This was Linux x86-64 under Docker Desktop emulation on a shared development
host. It is one standalone acceptance result, not a native reference benchmark,
full end-to-end aggregate replacement, live reader latency pass or qualification
of an optional source engine. The [original timeout witness](m7-managed-cold-stage-777.md)
and [native CI comparison](m7-managed-cold-777-vs-f94.md) remain separate evidence.

| Original check | Seconds | Result |
| --- | ---: | --- |
| Cold | 216.080 | Current, complete demo 1 + vstd 1,871; two verifier invocations |
| Unchanged | 31.947 | Current; zero invocations, both reports reused |
| Missing selected receipt | 42.092 | Current; one fresh selected proof, vstd report reused |
| Restored-mtime failing edit | 31.273 | Current failure, exit 101; one fresh invocation, matches plain Verus |

The original CLI and MCP job assertions also passed with complete retained
proof reports. The failing edit preserved file size and restored its mtime;
its failed proof was neither reused nor marked complete. Currentness describes
freshness of the evidence and does not turn this failure into verification.

All four direct managed calls recorded all 16 completed setup rows. Durations
below are seconds; `runtime-context` is an **inclusive parent**, not additional
work to sum with its child rows. Timing begins inside the runner, after CLI
configuration and daemon setup. These are existing block timings, not exclusive
per-file hash, syscall or dependency-compiler costs.

| Phase | Cold | Reuse | Receipt refill | Failing edit |
| --- | ---: | ---: | ---: | ---: |
| metadata | 3.860 | 1.373 | 1.954 | 1.335 |
| runtime-binaries | 0.723 | 0.618 | 0.794 | 0.902 |
| helper-identity | 0.403 | 0.230 | 0.458 | 0.360 |
| toolchain-probes | 1.922 | 2.554 | 3.077 | 1.767 |
| sysroot-guard | 3.084 | 2.586 | 2.653 | 1.677 |
| sysroot-hash | 15.878 | 15.992 | 14.911 | 10.794 |
| sysroot-barrier | 0.019 | 0.027 | 0.011 | 0.009 |
| context-configuration | 0.019 | 0.017 | 0.005 | 0.005 |
| runtime-context | 22.049 | 22.023 | 21.909 | 15.516 |
| owned-paths | 0.003 | 1.505 | 1.380 | 1.777 |
| initial-snapshot | 1.858 | 0.075 | 0.074 | 0.069 |
| input-guards | 1.019 | 1.059 | 1.346 | 1.158 |
| monitored-snapshot | 0.074 | 0.072 | 0.121 | 0.058 |
| input-barrier | 0.045 | 0.045 | 0.051 | 0.038 |
| target-cleanup | 0.001 | 0.157 | 0.108 | 1.199 |
| invocation-seal | 0.228 | 0.165 | 0.165 | 0.159 |

Cold setup reached the end of invocation sealing at 29.139 seconds from runner
entry. Its inclusive runtime context took 22.049 seconds, of which initial
sysroot hashing took 15.878 seconds. On the unchanged receipt-reuse call, the
context again took 22.023 seconds and sysroot hashing 15.992 seconds, despite
zero verifier invocations. This identifies repeated runtime validation as a
concrete warm-call investigation target; it does not authorize digest reuse
without continuous complete kernel coverage or establish the cause of earlier
timeouts. No production optimization was selected or implemented from this run.

A separately owned two-job source build began after cold and unchanged calls
completed; later calls may overlap it. Its exact start and actor identity are
retained without assigning a causal load effect. After the script finished,
known wrapper/harness/managed/result-worker PIDs were absent and the fresh
fixture had been removed. The original harness terminated and waited for its
owned daemon in `finally`; no unrelated actor was stopped and no global
process absence was inferred.

The [cold-start guide](../managed-cold-start.md) describes target isolation,
proof-receipt requirements, opt-in timing privacy and future compilation-cache
constraints. Original acceptance bounds and correctness checks remain intact.
