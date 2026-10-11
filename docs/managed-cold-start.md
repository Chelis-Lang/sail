# Managed cold compilation and proof reuse

`sail verify --incremental` owns its Cargo target and complete proof receipts
under the cache's project/context directories. The project key binds the exact
workspace path bytes. The context binds invocation arguments, engine selection,
runtime/configuration bytes and relevant environment. Source input revisions
and compiler-declared inputs separately bind each retained proof. Unchanged
complete receipts can skip verification; deleting evidence, changing input
content or losing observation coverage requires fresh work. Cargo saying
`fresh` is insufficient proof evidence. See [ADR 0023](decisions/0023-managed-incremental-workspace-verification.md)
and the [freshness contract](freshness.md).

The staged [guarded sysroot handoff](guarded-sysroots.md) addresses repeated
runtime byte hashing in unchanged commands. It uses a continuously guarded
local daemon owner and an overlapping original caller owner; it does not copy
compiled artifacts or proof receipts into a cold target. Missing, busy, invalid
or incompatible endpoints retain original local hashing. Complete integrated
cold/warm acceptance and measured savings are still pending.

A newly isolated target has no ordinary compiled dependency artifacts or proof
receipts. Even a one-function project can first compile its macro dependency
chain and verify vstd. Different projects and different engine/function/flag or
runtime contexts currently own different targets. Native CI's retained cold
example required two verifier invocations for vstd and the requested demo; its
next unchanged run reused both reports with zero invocations. The
[local timeout comparison](validation/m7-managed-cold-777-vs-f94.md) records a
separate unresolved 240-second failure during this cold path. Neither ordinary
dependency preparation nor compiler startup may be described as an established
warm proof check. The separate [timeout stage witness](validation/m7-managed-cold-stage-777.md)
observes actual vstd verification after pre-Cargo setup and ordinary dependency
compilation; none of those stages alone is established as the entire cause.

The current product does not share Cargo dependency artifacts across those
targets. A future production compilation cache could remove repeated ordinary
macro compilation, but it needs its own decision and evidence. A safe key must
bind the actual compiler/toolchain, target kind/specification, effective flags,
wrapper behavior, dependency graph and source/configuration/build-script inputs
by content. Produced artifacts need immutable publication and complete byte
binding; consumers need private writable targets, so concurrent Cargo runs
cannot mutate another context's artifacts. Diagnostic paths and compiler-defined
path-sensitive output must remain correct. Cargo metadata or timestamps alone
cannot establish this identity.

Verified dependencies such as vstd also require complete matching proof evidence;
copying their compiled artifacts alone cannot authorize reuse. Existing missing
receipt handling deliberately cleans and verifies such targets again. A cache
for ordinary non-verification dependencies and reuse of verified dependency
reports are different contracts. No shared-target or prewarming change has been
implemented from the cold-start hypothesis, and no cold acceptance bound has
been lengthened. The [roadmap](ROADMAP.md) still requires correctness and actual
product acceptance for any performance claim.

`ci/test-managed.py` now writes per-invocation timing and observation-only
progress events outside its temporary fixture. On timeout it captures only the
owned descendant tree and run-sidecar inventory: CPU ticks, process identity,
crate selectors, working directories and FD targets where available. It never
reads a full process environment or archives runtime configuration contents.
These files are private diagnostics; portable records select nonsecret facts and
hashes. Capture bounds are 1 MiB of read/link bytes, 128 processes, 512 inventory
entries, 128 tasks and 64 FDs per process, with a half-second scheduling budget.
Kernel operations cannot be interrupted by an elapsed-time check; capture may be
partial and cannot prove a cause. Capture errors still reach the original owned
kill/reap path. The 240-second verification timeout and all acceptance assertions
remain unchanged; evidence work is cleanup after a failed deadline, never an
extension that can admit a late result.
Reads use nonblocking, no-follow descriptors and refuse nonregular files, so a
substituted FIFO cannot make diagnostics wait for a writer. The small evidence
controls run in the workspace gate; they do not prewarm or invoke Verus.

With a nonempty existing `SAIL_JOB_EVENTS` opt-in, managed setup also writes adjacent
`*.setup-PID-SERIAL.jsonl` files. They record fixed phase names, PID and relative
start/elapsed milliseconds. They contain no arguments, paths, source,
configuration, environment values, diagnostics or solver/model contents. Each
file is created exclusively with mode 0600; an existing file, alias or special
file disables that output. Limits are 64 files per process, 64 rows and 16 KiB
per file. Failure, exhaustion or missing parent directories disables this
optional evidence without changing verification or error handling. An unset or
empty destination is a no-op before allocating any file slot; it creates no
relative timing file. There are
no observation threads, dependencies or changes to identity inputs.

The phases separate Cargo metadata/initial configuration; complete runtime
context construction; executable and optional engine byte validation; Python
closure identity; toolchain probes; sysroot observer/lease acquisition; initial
sysroot hashing and validation; context configuration/sealing; owned aliases;
initial dependency/source snapshot; coherent input guards; the second snapshot;
input validation; target cleanup; and invocation sealing. `runtime-context` is
an inclusive parent of its constituent phases, so its duration must not be
added to theirs. Snapshot and guard phases retain all their original reads,
probes and barriers. They are timings of those existing blocks, not evidence
that one specific hash or syscall caused a delay.

Timing begins in the managed runner, after CLI configuration and daemon
connection setup. A final row is written when its block exits, including error
returns; a forcibly killed unfinished block may have no completed row. Append
I/O adds observation overhead and kernel operations are not interruptible by
these limits. These records diagnose stage costs; they do not extend the
240-second check or qualify performance. The [first complete timed standalone
check](validation/m7-managed-setup-012707a.md) passed all original assertions,
including cold verification, receipt reuse, CLI/MCP jobs and a restored-mtime
failing edit. Its repeated sysroot hashing remains a measured warm-call cost;
no production hash reuse or compiler-artifact sharing change follows from that
one run, and the earlier timeout records remain separate.
Choose an event destination outside the workspace and other declared inputs;
otherwise writing diagnostics can itself invalidate source observation. The
managed acceptance harness already does this. The timer's no-follow creation
protects its final filename; its parent directory is an explicitly chosen
private diagnostic destination, not a trust boundary for verification.

The [timer product freeze](validation/m7-setup-timer-product-build.json) records
the exact Linux Sail/shim bytes built after the complete 498-test workspace gate
and 38-control solver suite. It is a build scope only; cold-start measurements
retain their original deadlines and remain separate from source/runtime and
reader qualification.
