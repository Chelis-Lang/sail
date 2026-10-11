# Daemon output-owner integration plan

Status: file-only design; no daemon activation, compiler run, RPC benchmark or
Currentness claim. Base is `a136edb`; the compiler-family controls and cleanup
remain retained on `fix/m7-compilation-context`.

The new path belongs only to the retained Result wire-frame cache. The worker's
persisted Projection remains its exact expected SHA. It does not authorize an
owner: the one background constructor independently reads all stdout bytes only
after acquiring the existing namespace/helper/kernel coverage. Status, ordinary
project loading, repair/explanation/baseline projection and foreground managed
verification retain their original paths.

## Authority and fallback

`jobs/store.rs::result_frame` may look up the exact existing frame key before
reading stdout. Only an existing independently constructed output owner permits
skipping that read. A nonblocking owner mutex and scratch admission precede its
namespace barrier. The original `projected_currentness` runs between that barrier
and the SAME owner's final `clean`. The lock survives both checks. A miss, busy
lock, failed barrier, unsupported platform or unavailable scratch admission runs
the original complete `project_bytes` read/SHA followed by original freshness.
A failed second barrier discards the prospective currentness and repeats those
original operations. Output identity never supplies input/runtime freshness.

All three immutable frame variants, full diagnostics, wire order, encoder,
MAX_FRAME and five-second socket limits remain unchanged. A frame miss always
reads and parses the original sealed stdout. No digest-only key, metadata/mtime,
TTL or later clean generation revives a failed owner.

## Service and lifetime

One daemon-owned constructor thread, started by JobServer::spawn, consumes one
fixed request slot. A single admission flag covers queued AND constructing work;
it is released only after constructor-local buffers and scratch charge drop.
Requests carry only one charged output cell, a bounded exact native path and
64-hex expected digest. There is no new map, growable queue, per-RPC thread,
constructor retry or foreground `GuardedOutput::begin` call. Each charged frame
family can offer its cell at most once; dirty/refused generations fall back.

The existing frame Arc retains its output cell through selected wire output.
Construction owns its cell too, so cache eviction cannot refund owner storage
while construction or a response survives. Physical GuardedOutput disposal
(including owned helper kill/wait) precedes the final reservation refund. An Arc
shell retiring after refund is covered by a separate fixed margin. Shutdown
sets the shared cancellation source, clears the queued request, joins the single
constructor, and only then closes its service reservation. Every spawn-error
path performs the same cancellation/join; no detached constructor is permitted.

## Admission under the existing 128 MiB family

Reservations use the SAME `result_cache::Budget` atomic count as encoded frames;
there is no second owner quota. Before constructing an output cell/request,
reserve the full 16 MiB retained allowance. Before queue publication, also reserve
one conservative 64 MiB constructor scratch allowance. Only one constructor can
hold that scratch allowance. Both reservations precede allocation; capacity
unavailability declines the optimization without evicting proof authority.

A fresh guarded foreground operation reserves 16 MiB inspection scratch before
any owner barrier. This independently covers the original bounded proc read and
inventory vectors, including old/new Vec growth and fdinfo fallback overlap.
Owner mutex contention refuses immediately. Scratch refunds only after the
complete barrier operation and all its temporaries have closed.

The permanent admission margin additionally covers a 1 MiB constructor stack,
64 KiB service/control storage, and bounded retiring output-cell Arc shells.
Actual type/layout controls must validate that fixed-shell allowance. These
bytes reduce the existing frame budget; they do not raise its 128 MiB ceiling.
The constructor reserves 80 MiB total (16 retained + 64 scratch), so large frame
families may correctly receive no output owner. After construction closes only
16 MiB remains charged, allowing subsequent ordinary frame admissions.

The 64 MiB constructor scratch is a separate bound, not inferred from
GuardedOutput::retained_bytes. The audit must enumerate simultaneously live
output/helper Observers (each original 16 MiB construction admission, including
visited sets), helper closure/path copies, bindings, baseline/final digests,
1 MiB helper JSON old/new buffers, 2 MiB mount snapshots, and 2 MiB proc inventory
old/new buffers plus row/fdinfo fallback overlap. Complete stdout hashing uses
the existing fixed 16 KiB buffer and 32 MiB content ceiling; it never retains the
complete stdout. Helper process address spaces, watches/FDs and kernel memory
remain separately bounded resources, as ADR0035 states. The exact scratch sum
must be reviewed before implementation is enabled or any compiler launch.

## Required executable controls

- Real mutation/restored bytes+mtime, replacement, outside hardlink/mmap,
  helper death and dirty generation retain full fallback and exact frame bytes.
- Input freshness changes between output barriers cannot obtain Current from
  output identity; second-barrier failure repeats original validation.
- Missing/busy/quota-refused/constructing owners use full read/hash, including
  wrong persisted SHA error and oversized output refusal.
- Preconstruction reservation, single queued+active constructor, eviction with
  in-flight frame and construction, last physical drop/refund and paused Arc
  shell retirement stay within the unchanged shared family ceiling.
- Cancellation of queued/active work, spawn failure and shutdown join release
  every owner/helper/charge. Foreground never calls constructor or waits on it.
- Actual full response equality and all original reader/collector deadlines
  remain mandatory later. Component/unit controls establish no live p95 credit.

ADR0035 and user docs will describe actual activation only after these source
controls pass; original reader acceptance remains a separate required scope.
