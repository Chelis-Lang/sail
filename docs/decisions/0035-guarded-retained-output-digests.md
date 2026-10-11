# 0035: Guarded retained output digests

- **Status:** proposed; daemon integration and affected source controls complete, installed reader acceptance pending
- **Date:** 2026-10-11

## Context

The [descriptor-bound original reader workflow](../validation/m7-descriptor-reader-terminal.md)
failed the unchanged full Result p95 bound at 343.142 ms. Every original sample
passed admission and actual verifier continuity. Rust and retained CLI/MCP were
unattempted. In the latest recorder tail, all 42 completed Current Result calls
hit an already encoded frame. Their output-seal stage p95 was 15.537 ms: the
daemon re-reads persisted worker stdout and compares its complete SHA-256 before
selecting any frame. Frame selection itself took 0.003 ms at p95.

Those 42 rows are not complete 200-call attribution. Output socket backpressure
and full Python parsing remain materially larger costs; removing the output
read/hash alone does not establish that the original bound can pass. Neither a
frame key nor its immutable encoding proves that the persisted stdout remained
unchanged. Metadata, permissions, timestamps and a previously supplied digest
are insufficient authority.

## Proposed decision

Prototype an output-only `managed::GuardedOutput` primitive. It acquires the
original bounded Linux namespace observer, mount-generation coverage and
nonrenewable read leases before independently hashing the exact entire output.
The helper runtime/interpreter remain covered by the original helper observation
and lease checks. The expected persisted digest is only a comparison value.
Every reuse must check the actual continuous owner; no disk digest memo, elapsed
time, TTL or metadata observation authorizes it.

The namespace and kernel arguments are the same as [ADR 0029](0029-kernel-lease-work-bounds-and-freshness.md)
and [ADR 0032](0032-kernel-mount-generation-coverage.md). A writer/mmap break,
outside hardlink write, rename/replacement, restored bytes/mtime, mount event,
helper death or uncertain barrier makes the owner sticky-invalid. A later clean
sweep cannot resurrect it. The owner certifies output bytes only. The original
independent input/runtime freshness checks and Current/Historical/Unknown rules
remain necessary. Prospective integration must check the output namespace before
those input checks and check the same original output owner afterward, so their
observations establish a valid point within the request interval.

The daemon integration candidate uses one owned background constructor and
keeps foreground RPCs on existing coverage or original complete hashing. It is
not yet source-qualified or enabled in an installed product. Before enabling it,
compare the complete prospective validation operation, including the new
namespace/helper/kernel work and fallback on contention, with the original full
read-and-hash operation on the same complete retained report. Any helper overhead
that outweighs the saved hashing rejects the optimization. The unchanged live
four-client/200-sample strict 200 ms and five-second RPC gate remains required.

## Ownership and fallback matrix

| State | Permitted output action | Currentness authority |
| --- | --- | --- |
| Cold, unsupported, capacity unavailable or constructing | Original complete read and SHA comparison | Original independent freshness checks |
| Independently hashed under complete uninterrupted guards, fresh barriers pass | May reuse exact output digest after reviewed activation | Original independent freshness checks |
| Expected SHA mismatch, dirty namespace, lease/helper loss or cancellation | Discard generation; original complete read/hash fallback | Original independent freshness checks; no promotion from output cache |
| Evicted family with in-flight response/construction | Keep all owner/resource charges until the final owning reference closes | No new authority from eviction or generation |

## Resource and lifecycle boundaries

The primitive accepts one exact output path, bounded native path bytes and one
64-character lowercase SHA. It rejects nonregular files with a nonblocking open
and bounds the complete independent content read by the existing 32 MiB frame
limit. It hashes with a fixed 16 KiB buffer and retains no output payload.
Observer/helper acquisition uses the existing bounded constructors, one shared
cancellation source and the original 120-second construction clock. Cancellation
is checked between bounded operations; kernel I/O is not promised interruptible.

Actual requested retained storage is exhaustively charged through the existing
Observer/ReadLease ownership methods and must fit a 16 MiB per-owner allowance.
That allowance is **not** a whole-construction peak or RSS claim. Before any
daemon activation, callers must reserve it from the existing 128 MiB frame-family
budget before construction, retain that reservation through all in-flight and
constructing references, bound aggregate constructor overlap, and account for
every transient buffer. This prototype does not add an uncharged daemon owner.

Kernel resources are separate: the original lease inventory caps combined input
and helper files at 4,096; each bounded Observer caps watches at 8,192. The output
owner has an output Observer and the lease helper's Observer, plus their original
mount guards and private lease IPC. Constructor probe children and the retained
lease helper use the original owned kill/wait cleanup. A future background service
must own and close its constructor workers too; no unbounded per-RPC thread or
constructor retries are permitted.

## Consequences

This can reduce repeated report hashing only if the complete ownership checks
are cheaper in actual measurements. It leaves complete response graphs, exact
diagnostics, stdout bytes, wire order, collector code and deadlines unchanged.
The prototype is a new optional library primitive, not a user switch or a daemon
performance claim. Its negative controls and complete-output comparison must be
retained even if the design remains unselected.

## Recorded component result

The [complete-operation comparison](../validation/m7-guarded-output-components.md)
passed six real output mutation controls and release all-target strict Clippy.
On the exact complete retained stdout, four-client component p95 changed from
25.781 ms for reading/hashing to 17.186 ms with guarded checks and immediate busy
fallback. This supports reviewing bounded integration; it does not enable the
primitive, explain the original 343.142 ms live p95 or meet the original reader
gate. The separately measured parent-group candidate remains test-only because
its tail comparison did not demonstrate an improvement.

## Integration candidate

The [implementation and ownership plan](../validation/m7-guarded-output-integration-plan.md)
keeps the same 128 MiB frame-family budget. Before any request allocation it
reserves 16 MiB retained ownership plus 64 MiB constructor scratch; only one
queued/constructing generation exists. The fixed margin covers a single 1 MiB
constructor stack, 64 KiB service controls and bounded retiring owner/Frames
shells. Each foreground coverage operation reserves 16 MiB scratch before any
barrier. All charges survive actual eviction, construction and in-flight frame
references. Helper/observer disposal precedes owner refund; surviving outer
allocation shells belong to the permanent margin. No output owner establishes
Currentness or changes the complete result payload.

Exact output/helper file coverage avoids recursive directory walks, and bounded
construction fdinfo uses a complete 64 KiB sentinel read. The general legacy
observer and inventory APIs keep their previous behavior. Input/runtime freshness
still runs between the same owner's two barriers; final coverage failure repeats
the complete original read/hash and independent freshness. Missing, busy, quota
refused or sticky-invalid coverage always uses that original fallback.

Cancellation and owned constructor join close queued/active construction.
Cached or in-flight Frames can retain their original owners afterward, and their
charges remain until actual physical disposal. Constructor spawn failure disables
the optional path while preserving the original job service. The supported Linux
exec environment is the existing trusted OS/helper bootstrap; production does
not mutate that process environment, and its clone storage is charged in the
construction envelope. Native kernel/helper resources remain separately bounded.

The corrected candidate passed 35 affected native source controls and strict
all-target checks for both affected packages; see the
[exact source validation](../validation/m7-guarded-output-source-controls.md).
The initial strict failures and tested images remain retained. All original complete payload,
collector, four-client/200-sample and five-second bounds remain required; prior
component measurements are not daemon acceptance or performance qualification.

## Concurrent validation

The [reviewed overlap change](../validation/m7-result-output-overlap-plan.md)
releases output locks during independent input freshness. It retains the same
continuously guarded owner across both barriers, confirms exact owner identity
again, and keeps the original complete fallback on any refusal. An evicted but
continuously guarded owner remains valid and charged; removal or substitution
between barriers cannot certify an earlier prospective result. The fixed
admission margin also covers the new mutex/Arc allocation shell after reservation
refund. No new proof authority or retention ceiling is introduced.

The [native record](../validation/m7-result-output-overlap-native.md) retains
30 daemon controls, eight unchanged primitive controls, strict checks and the
first test-only Clippy naming failure. The original lock algorithm fails the
overlap sensitivity assertion. A fixed-order comparison with four readers and
complete 2,158,309-byte output plus real fixture-input freshness reduces p95
from 19.366 to 12.713 ms with all 128 validations complete. Source is selected
for integration after review; installed full-reader acceptance remains pending.
This 6.653 ms component reduction is not an explanation or remedy established
for the original 343.142 ms, 4,313,743-byte live Result failure.
