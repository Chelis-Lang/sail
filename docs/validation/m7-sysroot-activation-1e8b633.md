# Original managed scenario after selector correction

One fresh original managed scenario on sealed source `1e8b633` failed the first
cold 240-second check. The original harness, timeout helper and stage collector
were unchanged. There was no preparation, manually submitted Prepare request,
copied proof or altered deadline. Product/runtime/source identities, all
completed phase rows, raw hashes and cleanup observations are in the
[portable record](m7-sysroot-activation-1e8b633.json).

The external controller returned exit 1 after 251.503 seconds inclusive. The
first managed operation timed out under the original 240-second limit and
produced no proof report. Warm reuse, public CLI/MCP and mutation checks were
not reached. The last progress record started `vstd`; stderr ended while
compiling `verus_builtin_macros`, with no completed bucket event. The bounded
timeout observation was limited and captured no processes, so it provides no
CPU or lifecycle evidence that establishes a timeout cause.

The corrected scope reached the automatic fill-hint branch after the original
local hash barrier. Handoff took 27.169 ms and the hint took 12.843 ms. This is
evidence that the prior byte-key serialization defect was bypassed, not proof
that the daemon accepted the queue item, published a complete guarded snapshot
or supplied a later import. No warm call was reached.

Cold runtime context took 62.513 seconds, including 39.664 seconds hashing
sysroot bytes and 14.757 seconds acquiring the original guards. These actual
costs are larger than the earlier runs; they do not establish why. A separate
owned source compiler started after that entire context span and overlapped
later proof work. Its overlap cannot explain the earlier span. Other corpus
and gate work was concurrent, without causal attribution. Execution used
Linux x86-64 Docker Desktop emulation on a shared development host.

Recorded wrapper, harness and managed PIDs were absent after the original
cleanup, and the private scratch directory was removed. Product and pinned
runtime hashes matched the grant afterward. The limited observation does not
claim a complete helper or host process inventory.

This result remains separate from the
[original 7da failure](m7-sysroot-activation-7da9579.md) and the passing
[selector controls](m7-sysroot-selection-controls.json). No warm speedup or
overall M7 acceptance is claimed. The original managed limits and correctness
checks remain required; see [guarded-sysroots.md](../guarded-sysroots.md) and
[ADR 0031](../decisions/0031-guarded-sysroot-digest-handoff.md).
