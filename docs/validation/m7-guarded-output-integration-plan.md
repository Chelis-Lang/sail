# Daemon output-owner integration plan

Status: file-only implementation under independent review; no compiler run,
installed daemon activation, RPC benchmark or Currentness claim. Base is `a136edb`; the compiler-family controls and cleanup
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
Requests carry only one charged Frames Arc, a bounded exact native path and
64-hex expected digest. There is no new map, growable queue, per-RPC thread,
constructor retry or foreground `GuardedOutput::begin` call. Each charged frame
family can offer construction at most once; dirty/refused generations fall back.

The existing frame Arc retains its output owner through selected wire output.
Construction owns that complete Frames Arc too, so cache eviction cannot refund owner storage
while construction or a response survives. Physical GuardedOutput disposal
(including owned helper kill/wait) precedes the final reservation refund. The existing retired Frames Arc shell margin also counts the constructor as one
additional retiring owner beyond the 32 endpoint clients. Shutdown
sets the shared cancellation source, clears the queued request, joins the single
constructor, and only then closes its service reservation. Every spawn-error
path performs the same cancellation/join; no detached constructor is permitted.

## Admission under the existing 128 MiB family

Reservations use the SAME `result_cache::Budget` atomic count as encoded frames;
there is no second owner quota. Before allocating a construction request,
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
64 KiB service/control storage, and bounded retiring OutputOwner Box shells.
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

## Resource corrections under independent review

The file-only source correction adds an exact-file Observer constructor for
output/helper paths. It retains binding/ancestor watches and refuses a directory
replacement instead of recursively walking it. The general Observer path is
unchanged. Bounded ReadLease construction now reads each fdinfo with a 64 KiB
limit and cancellation between descriptors, rather than unbounded read_to_string.
These two source controls are not yet compiled or executed.

The interpreter selection uses the explicit supported Linux exec bootstrap
boundary. Initial daemon process environment storage is bounded by that kernel's
exec limit, conservatively charged as 2 MiB; production does not mutate the
process environment. This is the existing trusted OS/helper bootstrap, not an
attestation of arbitrary future getenv callers. Helper JSON path
parsing is independently byte-capped before deserialization; its transient
String/PathBuf growth still belongs in the peak sum. Readiness/proc inventory
vectors, simultaneously live mount snapshots and fdinfo fallback overlap also
belong in that sum. No constructor reserve is itself proof of these bounds.

Shutdown cancels and joins the sole constructor; cached/in-flight Frames can
still own output helpers afterward. Their per-owner reservation remains charged
until those actual Frames dispose the helper. Permanent fixed reservation stays
inside the frame Budget for the complete Budget lifetime. Service shutdown never
pretends to refund surviving owner storage. Spawn failure disables the optional
constructor and preserves the existing job endpoint/read-and-hash path.

The output-seal diagnostic stage accumulates both actual output barriers and any
subsequent original fallback read/hash. Independent freshness remains a separate
stage. Repeated frame lookups likewise accumulate. No total payload or collector
code changes; this prevents understating work by overwriting the first barrier.

Independent allocation review supplies this conservative constructor envelope:
two bounded Observers 32 MiB; mount snapshot/failure/growth extras 6 MiB; helper
closure JSON/decoded paths/combined path copies and request JSON growth 8 MiB;
bindings/readiness/initial inventory 4 MiB; global/fdinfo inventory overlap
11 MiB; initial Linux exec PATH/interpreter transient 2 MiB; remaining bounded
path/hash/helper-command/OS-command storage 1 MiB. This totals 64 MiB and already
counts owner contents; the separately reserved retained 16 MiB overcounts them.
The proposed source controls must verify collection element/layout assumptions.

OutputOwner is boxed so the existing tiny frame-budget controls still exercise
frame admission. Its fields dispose the GuardedOutput before refunding charge;
the final Box shell can remain physically live after refund. The permanent
fixed margin explicitly adds `(MAX_CLIENTS + 1) * size_of::<OutputOwner>()` for
endpoint and constructor retirement, separately from existing Frames Arc shells.
The admission also overcharges the complete OutputOwner inline size in addition
to the primitive's checked retained bytes before its Box allocation.
