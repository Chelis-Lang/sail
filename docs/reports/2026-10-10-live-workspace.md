# Live workspace verification and team caching

M6 completes the local developer and agent workflow around the pinned Verus,
with an optional authenticated HTTP team cache. Issues #64–#66 and PR #67
track the implementation and acceptance.

## Product behavior

`verify --incremental` always invokes Cargo and returns a complete per-target
report. Eligible unchanged targets reuse successful proof receipts without
invoking Verus. Receipts bind actual artifacts, context and the target's complete
input closure; publication checks coherent observations and commits last.
Failed, partial, interrupted, missing or incompatible proof evidence requires
fresh verification. Compiler-declared sources bind to their consuming packages
and dependency closures; their global union remains observed for currentness.

`verify --background`, `jobs` and four asynchronous MCP tools share durable jobs
and one local FIFO worker. Status and result inspection remain available while
another verification runs. Cancellation terminates the owned process group;
restart marks unfinished work as an infrastructure failure. Job history and
proof receipts have separate lifetimes. Watch follows workspace and path
inputs, cancels superseded work, and emits lifecycle events as JSON lines.
Normal local use starts its services automatically and needs no credentials,
account, team service or container.

The optional HTTP service transfers existing signed entries and complete-input
silence certificates. Readers verify signing trust independently of transport
credentials. HTTPS validation, bounded messages and I/O, deadlines and immutable
publication preserve fallback on remote failures. The runtime image runs as
UID 10001 and persists private credentials; operator TLS termination remains an
explicit deployment responsibility.

## Acceptance

The host gate passes all 322 tests, doctests, rustdoc, strict linting, dependency
layering, dependency policy and generated benchmark freshness. The pinned solver
and fault-injection suite passes all 36 real-solver cases, including a fresh HTTP
reader answering exact solver replies without launching Z3.

Native Linux CI passes every required check. The retained `a518a104` managed
acceptance proves complete cold reports, unchanged reuse with zero verifier
invocations, installed CLI and async MCP jobs, missing-receipt recovery, and a
failing edit with restored mtime. The legacy and managed phases took 316 and
348 seconds; no job exceeded its existing ten-minute limit. The later `e21922d`
workflow also passes all checks.

Full fast conformance matches 147 units and 3,450 functions in each of the
pass-through, fill and warm modes. Full native standard matches 157 units and
11,446 functions, with two explicit missing requirements. Separate zero-skip
verismo-main and Linux Anvil runs recover both: combined standard coverage is
159 units and 12,726 compared functions per mode. Every comparison has zero
verdict/diagnostic differences, instability and infrastructure errors. These
integration runs overlapped frontend fixes; they do not claim one frozen
executable throughout the full corpus. The exact tested Anvil binaries are
retained separately.

The final native same-name library/binary oracle returns fresh complete proofs
for both requested targets. Their entire solver inputs match plain Verus
byte-for-byte. The observation wrapper is identified explicitly; three separate
vstd declaration-order differences are preserved under VI-0001. Historical
currentness in this cold oracle does not count as current reuse.
A separate frozen emulated Linux oracle also matches both complete target inputs
exactly and produces a Current report with three fresh verifier invocations.
Its deliberately unavailable daemon forces pinned-Z3 trace capture; this is
fresh forwarding acceptance, not cached-reply or performance evidence.

The frozen Linux server deployment probe validates real startup, reader/writer
authentication, unprivileged UID, private file modes and credentials retained
after container replacement. TLS termination in an operator's proxy is outside
that loopback probe.

Portable evidence lives in `bench/acceptance/`. The former `bench/validation/`
summaries and generators are archived outside the repository (sha256
`dcf30c1827434a157d17d15825305cdb8eac1265e99642489f8f37b97887322c`) and remain in git history before
rescue lane L14.

## Frontend measurements

Both native macOS fixtures pass three complete paired rounds and a real proof
edit, retaining spread, RSS, binary identity and development load. Every reuse
returns Current complete reports with zero verifier invocations; the edit
invokes one verifier and agrees with plain Verus. Context/content validation
still costs time:

| Fixture | Plain warm SMT median (range) | Managed complete reuse median (range) |
|---|---:|---:|
| Two functions | 0.75 s (0.66–0.75) | 2.14 s (2.09–2.24) |
| 300 functions | 1.05 s (0.95–1.52) | 2.05 s (1.97–2.06) |

These easy synthetic fixtures establish complete proof reuse and lower verifier
work; they do not establish a wall-time gain. They are secondary-platform
observations, not Linux reference benchmarks. Shared-parent and concurrent
macOS attempts that became Historical are retained separately. Private fixture
parents and serialized proof setup reduce test-induced namespace changes;
currentness checks and deadlines remain unchanged. Safe freshness-cost work is
tracked in [#69](https://github.com/Chelis-Lang/sail/issues/69).

Separate final uninstrumented Linux/amd64 emulated runs also pass all three
Current complete reuse rounds and the proof edits for both fixtures. They use
the frozen `a518a104` executable and the unchanged 600-second invocation limit.
Two-function medians are 4.11 s plain versus 13.01 s managed; 300-function medians
are 3.55 s versus 10.41 s. These supplementary observations establish receipt
reuse, not native Linux reference performance or a wall-time improvement. Their
success leaves the earlier emulated wait's cause unresolved.

## Defects found during acceptance

A native Linux check exposed unnecessary reverification after declaration
establishment: the application's source appeared in a global target binding,
invalidating vstd's earlier receipt despite unchanged contents. The retained
`managed-reuse-defect-linux-2026-10-10.json` records the exact frozen build,
Current report and two unexpected verifier invocations. The fix assigns
compiler declarations to their owning package closures and finalizes staged
bindings while coherence checks still bracket the reads. Original staged
checksums are verified before resealing. A regression fails with the previous
global implementation; another rejects staged corruption and checks includes
consumed from a different already observed package.

The earlier 240-second emulated cold-build timeout is retained separately in
`installed-managed-emulation-timeout-linux-2026-10-10.json`. Neither its deadline
nor the reuse assertions were relaxed. Native Linux CI runs both complete
end-to-end phases in parallel; the required aggregate rejects a failed,
cancelled or skipped phase. Long conformance and performance probes remain
local-only.

A frozen Linux/amd64 emulated installed run passed cold and warm reuse but its
job-result command returned exit 2. A separate synthetic diagnostic reproduced
explicit five-second RPC errors under four concurrent reads of the original
150-file, 1.408 GB runtime identity. Idle and two-concurrent controls succeeded;
no unvalidated result was labeled Current. The original command did not retain
stderr, so the matching exit alone does not prove its cause. Failure capture now
retains stdout, stderr and duration before reporting a failed command. Native
real CLI/MCP checks passed without changing deadlines.

A separate uninstrumented emulated managed-fill reached its unchanged
600-second deadline. No complete result was produced. The three previously
stalled solver inputs subsequently replayed from a copy of that original cache:
ten hits each, no solver launch, complete response frames and exact proof replies
and rlimit counts. An instrumented retry completed all three Current reuse pairs
and the proof edit; all 737 recorded sessions exited successfully with balanced
frames. Instrumentation changes scheduling and context, so its timings are not
unwrapped benchmarks and its success does not assign the original cause.
The unresolved emulated verifier wait is tracked in
[#68](https://github.com/Chelis-Lang/sail/issues/68). No upstream report was filed.

## Limits and deferred scope

Linux is the reference platform for frontend receipt reuse. macOS retains
conservative parent-directory checks for absent configuration files; unrelated
activity can make a result historical and require another proof. Default
verification and the SMT cache remain available on both platforms. Managed v1
explicitly rejects runtime/configuration selectors whose identity it cannot
establish, including custom sysroots and JSON targets. See
`docs/workspace-verification.md` and ADR 0023.

This milestone does not add persistent Verus internals, semantic function
slicing, racing portfolios, stack-only keys, new SMT backends or shared frontend
receipts. Default verdict policy, the 50% unknown serving cap and existing
whole-scope portfolio semantics remain unchanged.
