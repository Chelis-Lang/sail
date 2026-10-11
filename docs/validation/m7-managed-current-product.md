# Current product managed-receipt acceptance

The sealed `46e1da6` Linux product passes the unchanged managed-receipt fixture
in `ci/test-managed.py`. This is the small default managed workflow, distinct
from source-engine repair, HumanEval/Vest performance and real reader latency.

| Operation | Seconds | Observed result |
| --- | ---: | --- |
| Cold verification | 220.375 | Current complete proof; requested target and vstd freshly verified |
| Unchanged verification | 12.867 | Current complete reports; both targets reuse receipts |
| Missing-receipt recovery | 21.192 | Current complete proof; requested target refreshed |
| Restored-timestamp failed edit | 19.347 | Current failing report, expected exit 101; requested target never reused |

The original CLI submit/result and MCP checks also pass. Whole-fixture elapsed
time is 335.794 seconds. Each verification retains its original 240-second
bound; command and RPC limits and all assertions are unchanged. This is an
acceptance check, not a repeated benchmark or a causal performance comparison.
The selected optional source engine is not used in this fixture.

The [machine-readable record](../../bench/validation/m7-linux-managed-46e1da6-emulated-2026-10-11.json)
binds the tested product, frozen source archive, command, all six recorded
operations and 26 raw files. Host and native evidence hashes match. Independent
post-run scans of both containers find no owned fixture/product actors or
unreadable live processes, and the fixture directory has been removed. The
source runtime, tested executables and raw evidence are retained.

Execution used local Linux amd64 Docker emulation on arm64 macOS. The separate
HumanEval reader setup and private source work ran concurrently. No timing
cause or quiet-machine condition is inferred. Earlier frozen cold timeouts
remain separate failed evidence; this pass does not recertify those runs or
accept M7.

The previous [0f31c1d pass](../../bench/validation/m7-linux-managed-0f31c1d-emulated-2026-10-11.json)
remains separate. These single acceptance runs are not a causal performance
comparison or the required three-round repair benchmark.
