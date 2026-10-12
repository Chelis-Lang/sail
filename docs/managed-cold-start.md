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

The implemented [guarded sysroot handoff](guarded-sysroots.md) addresses repeated
runtime byte hashing in unchanged commands. It uses a continuously guarded
local daemon owner and an overlapping original caller owner; it does not copy
compiled artifacts or proof receipts into a cold target. Missing, busy, invalid
or incompatible endpoints retain original local hashing. Complete integrated
cold/warm acceptance and measured savings are still pending.

A newly isolated target has no ordinary compiled dependency artifacts or proof
receipts. Even a one-function project can first compile its macro dependency
chain and verify vstd. Different projects and different engine, flag or runtime identities own
separate compiler targets. Focus selectors within the same compiler-artifact
family can share ordinary dependencies; their proof contexts and receipts stay
separate. Native CI's retained cold
example required two verifier invocations for vstd and the requested demo; its
next unchanged run reused both reports with zero invocations. The
[local timeout comparison](validation/m7-managed-cold-777-vs-f94.md) records a
separate unresolved 240-second failure during this cold path. Neither ordinary
dependency preparation nor compiler startup may be described as an established
warm proof check. The separate [timeout stage witness](validation/m7-managed-cold-stage-777.md)
observes actual vstd verification after pre-Cargo setup and ordinary dependency
compilation; none of those stages alone is established as the entire cause.

The implemented [compiler artifact families](decisions/0036-managed-compiler-artifact-families.md)
separate ordinary Cargo dependencies from proof contexts. Only a source-engine
function selector is omitted from the new compiler-family key; all other runtime,
argument, configuration and environment identities remain bound. Optional daemon sysroot hint selection likewise omits only this selector under
a new domain; independent caller kernel/namespace guards, import barriers and
local-hashing fallback remain unchanged. A shared family
has stable executable aliases and a nonblocking exclusive owner through cleaning,
compilation and receipt publication. Concurrent focus contexts sharing that
family can refuse with SAIL3104 rather than mutate each other's target.

Proof receipts, currentness guards, runtime sidecars, source observations and
committed reports remain private to each complete proof context. Selected repair
packages remain forced fresh. A fresh Cargo artifact without the exact proof
receipt still triggers the existing clean-and-reverify path.

Ordinary build-script contracts may be reused from the compiler family after
fresh validation of every declared input and actual Cargo output. A consumed
family generation prevents aborted builds from leaving stale contracts behind;
all output units must validate again after the original input guards are live.
Missing or uncertain coverage forces original cleaning, including registry
scripts. Old local declarations can supply watch paths but cannot bypass that
cleaning. Proof-specific or unaudited Cargo-injected environment declarations
refuse sharing. See the [decision](decisions/0036-managed-compiler-artifact-families.md)
for the local cache trust boundary and effective environment binding.

Optional independent ordinary-build recovery passes the complete local controls,
emission sensitivity and original installed repair fixture on c2. Real-project
benchmark recovery remains unqualified: the fresh
[completed c2 benchmark](validation/m7-benchmark-c2a4b23-warmup-terminal.md)
stops at required Current admission with a declarations-established note, even
though verification exits zero. No measured samples are admitted.
When a compatible generation is unavailable, original script cleaning and
regeneration still occur. Before that mutation, the candidate acquires its own
continuous coverage of supported script inputs, Cargo configuration and runtime
bytes. Only a complete successful Cargo run with matching compilation roles,
declarations, output seals and final guard checks can establish ordinary contracts.
Initial regeneration does not guarantee the next invocation is Current:
proof-source coverage and independent script publication have separate
requirements. The [first real-project publication review](validation/m7-c2-first-bootstrap-publication-review.md)
identifies the retained evidence still missing for the c2 refusal.
The original complete proof transaction is unchanged: Unknown proof evidence
remains Unknown and cannot create a successful receipt or accepted repair.
Unsupported paths, aliases, directory declarations, Cargo overrides, environment
or unavailable guards refuse optional retention and preserve original cleaning.
The [candidate contract and resource limits](validation/m7-compiler-script-bootstrap/README.md)
and [independent review](validation/m7-ordinary-script-bootstrap-review.md) bind the
supported role. The [602 failed positive controls and three exact Cargo path captures](validation/m7-bootstrap-602ec67-gate-failure.md)
remain distinct from corrected native validation; there is no measured recovery
or explanation of the historical 730 failure yet.

A fully observed Current negative full verification can retain those ordinary
dependency contracts when all requested targets finished normally, the complete
full result contains proof errors without a VIR error, and input declarations
were already covered before the run. Compiler errors, partial or interrupted
runs, new declarations and uncertain inputs withdraw the generation. Failed
proofs remain failed, produce no successful receipt and do not skip the next
selected verification. The original c2 repair fixture validates the installed
workflow; real-project benchmark validation remains pending. Original repair
deadlines are unchanged.

The [script-observation diagnostic](validation/m7-compiler-script-observations.md)
records the avoided cleaning seam and its control scope. The compiler-family
implementation and affected controls are separate from pending whole-product
performance validation. No repair fixture
deadline, cold acceptance bound or benchmark requirement is lengthened. Actual
whole-product performance and the [roadmap](ROADMAP.md) requirements remain open.

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

The same sink now observes three publication boundaries: `result-publication`
is the first complete result-document write; `receipt-publication` is the
existing pending-receipt writes and guarded commit, including any original
Unknown demotion and replacement result write; `compiler-family-publication`
is the existing ordinary dependency-generation publication attempt. These
observations do not admit a receipt or generation. Error exits retain the
original error and record a finished interval when the timer is dropped.
The existing result `execution.wall_ms` is sampled before these boundaries and
is unchanged. A retained worker stdout document proves neither its emission
time nor terminal daemon publication before a deadline. Daemon child-exit,
document-read, optional freshness establishment and status persistence can be
observed separately through the existing
[optional daemon diagnostic ring](result-rpc-diagnostics.md).

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

The current daemon Result path enables an optional background output-owner service
under [ADR0035](decisions/0035-guarded-retained-output-digests.md). It concerns
persisted stdout bytes, not verification or input Currentness. One background
constructor may establish continuous output coverage within the existing frame
budget; foreground requests use that coverage only around their independent
input/runtime freshness check. Missing, busy, refused or invalid coverage retains
the full read/hash fallback. The integration passed
[35 affected native source controls and strict checks](validation/m7-guarded-output-source-controls.md).
The later [b895 installed product](validation/m7-compiler-script-bootstrap/scheduling-recovery.json)
passes the original full reader and retained CLI/MCP workflow. That whole-product
pass does not isolate this shortcut's latency contribution or establish repair
speed. Newer source-inventory/baseline changes require their own product validation.
