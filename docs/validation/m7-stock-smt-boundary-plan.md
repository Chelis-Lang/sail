# One stock AIR transport component control

This is a prepared diagnostic, not an executed result. It follows the
[failed reader prerequisite](m7-rpc-binding-reader-terminal.md) and
[bounded wait audit](m7-rpc-binding-reader-wait-audit.md). The original
600-second failure remains unresolved. No compiler, solver, proof, endpoint or
reader census was launched to prepare this control.

The [controller](../../scripts/m7-stock-smt-process-diagnostic.py) extracts the
exact command vector from the sole complete QUERY block in the sealed module
052 log. The 358,298-byte vector has SHA-256
`ff1aebf727168498cac2186ddfbebd556073b726fa1d8417df11b043f1b2f1bf`.
Stock AIR appends its original newline and `(echo "<<DONE>>")` framing; the
358,317-byte expected writer stream has SHA-256
`faa2a5db20d8337c831d66417dedcfe8bc0fb655ecea7460c812ce6e30dd3ed8`.
These are intent-derived reconstructed bytes, not captured historical wire.

The selected b346 AIR file is byte-identical to the pinned stock file. The
controller refuses any change to its exact source hash or replacement
boundaries. Its transformation adds metadata calls before/after enqueue,
write, newline, echo, flush, read, DONE recognition, reader delivery and receiver
completion. It preserves the original operations, channels, threads, solver
arguments, command bytes, flushes and stock `Drop`. The generated harness
contains no unsafe code. It links the already-built selected yansi dependency
with the matching Rust 1.98.1 compiler in a private directory, without Cargo or
shared-target writes. Compilation has a separate 120-second lifecycle ceiling.
Its source and compiler flags are retained independently of execution.

The real arm has one unchanged 600-second whole-operation deadline. It starts
the sealed product 46 shim, which forwards to the exact selected pinned Z3 with
`-smt2 -in`. The daemon socket is absent: this is a forwarding-only component
control. It does not recreate the historical cache contents, compiler bucket
concurrency, workload, process scheduling or proof context. A successful arm
only proves that this reconstructed batch completes in this separately
observed transport. A failed arm localizes its own last confirmed boundary;
it cannot retrospectively explain the failed project verification.

## Observations and ownership

Each marker attempts one nonblocking Unix datagram containing four little
endian integers: sequence, elapsed nanoseconds, dropped count and stage ID.
No command, response, environment, path or diagnostic text enters that channel.
The collector retains the latest 256 rows and reports overwrites, sender drops,
missing completed rows and unknown tail loss when completion counters are
absent. It performs a bounded final drain before closing. Missing observations
cannot establish that an operation did not occur. A successful `write_all`
marker confirms an OS write, not solver parsing or historical reception.

Three metadata snapshots use the existing birth-bound all-task
[owned-process observer](m7-reader-process-observation.md), with its unchanged
128-process, 128-task, 64-FD, 1 MiB and 0.5-second work bounds. The controller
retains its `Popen` handle and initial root birth identity. It neither reads
solver pipes nor signals discovered processes. A timeout terminates only the
owned process group, with bounded five-second interrupt and kill/reap windows;
cleanup and evidence failures remain explicit. The operation deadline does not
claim it can interrupt a kernel diagnostic read. Solver responses are read
only by the stock reader, then written to a separate bounded response file.

## Sensitivity controls

Four separately labelled tiny fake-peer controls pause at enqueue-before,
write-before, read-before or delivery-before. Their three-second deadlines are
diagnostic fault controls, not replacements for the real arm's 600 seconds.
They must observe the selected boundary, reject the following boundary, and
reject sequence gaps or dropped observations. A fifth normal fake-peer control
requires complete delivery and stock child shutdown. Each compares the peer's
actual stdin bytes with the untouched command/newline/DONE stream; deliberately
pre-write pauses require the exact empty prefix. These executable sensitivities
are pending. The eight
[pure preparation controls](../../scripts/test-m7-stock-smt-process-diagnostic.py)
pass on both host and Linux; they cover extraction, changed boundaries, loss
handling, classification and compiler-timeout terminal preservation,
and do not substitute for the pending executable controls.

The private static configuration binds all 13 selected runtime artifacts,
manifest, compiler, yansi and shim by byte identity. It declares only explicit
solver choices and excludes the daemon socket. Subsequent build/run modes
recheck the complete inventory and generated sources, retain raw output and
normal/timeout closure, and seal artifacts again. The
[portable static record](m7-stock-smt-boundary-plan.json) describes these
identities without exporting the private configuration's local paths.

Execution order is: private build, normal fake peer, the four fault controls,
then one real forwarding arm. Stop at a failed control; do not retry a failed
real arm. The executable observations remain pending source/lifecycle review.
No result counts as Current proof, source qualification, M7 completion,
performance or live-reader acceptance. See [reader acceptance](../reader-acceptance.md),
[known issues](../known-issues.md) and [the roadmap](../ROADMAP.md).
