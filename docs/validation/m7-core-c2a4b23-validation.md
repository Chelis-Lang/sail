# Frozen c2a4b23 local validation

Frozen source `c2a4b2363b415aa2157fc38dc388192125b87109` completes the full Linux
workspace gate, then the original pinned-solver suite. Both terminal records are
PASS with no uncertainty.

| Scope | Result |
|---|---|
| Complete workspace gate | 651/651 tests and every remaining gate step PASS; 532.894 seconds including compilation/checks, 215.915 seconds in nextest. |
| Independent ordinary-build recovery controls | All ten controls PASS, including actual Cargo publication, empty-generation recovery, workspace-member resolution, mutation, alias, environment and helper-loss refusal. |
| Original pinned-solver suite | 41/41 PASS; 87.685 seconds enclosing phase, 83.317 seconds in nextest. |

The [machine-readable record](m7-core-c2a4b23-validation.json) binds every retained
host phase log, source archive/inventory, terminal result and controller file by
length and SHA. Rebuilding the safe archive inventory independently matches all
recorded file bytes, modes, links and directory membership. Original test
deadlines, two compiler jobs, one independent nextest case and internal fixture
concurrency are unchanged. Measurements are Linux amd64 Docker emulation on
arm64 macOS with concurrent development work, not native reference performance.

Original terminal checks report equal source/protected bytes and PID1. Container
identity, image and birth match before/after. Known groups have no live survivor
or unavailable row. The gate retains seven terminated unreaped processes and
the solver ten; each has complete recorded task evidence containing only dead
`Z` tasks. This review verifies those rows and hashes on the host, without a new
native census. These observations do not certify arbitrary orphan absence.

Three retained executable copies are explicitly operator-selected. Their hashes
do not prove execution. The log separately names five example test executables
as running; it does not provide their execution-time byte hashes. Nextest PASS
rows bind test names and outcomes, without identifying copied ELF bytes. All
Cargo artifacts were not archived, and no missing execution identity is inferred
from filename or presence.

The [602 failed positive controls](m7-bootstrap-602ec67-gate-failure.md) and
[earlier strict failures](m7-core-244d51d-validation.md) remain failed. This scope
qualifies the corrected local controls, not installed recovery or real-project
performance. Separate remove-only-publication sensitivity, coherent product
delivery and unchanged original repair/benchmark scenarios remain outstanding.
No reader, corpus, benchmark or historical-730 changed-binding claim follows;
proof currentness, receipts and acceptance rules remain unchanged.
