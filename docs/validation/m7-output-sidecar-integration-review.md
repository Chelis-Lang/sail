# Output and runtime sidecar integration review

This is a read-only source review. No compiler, proof, endpoint or reader
acceptance actor was launched. Original reader failures and repair deadlines
remain unchanged.

## Runtime identity sidecars

The six real Linux controls and strict check in `f11afd2` support the narrow
eligibility change described in [the retained component record](m7-runtime-sidecar-components.md).
The reviewed production-enable draft adds one boolean to the existing Linux
runtime validation owner. It retains no parsed graph or separate owner.

`Request::key` includes exact absolute cache and workspace path bytes, context,
and environment. Lookup uses that key's projection; final publication still
checks its generation, the exact runtime Arc and the namespace observer.
Eligibility is set only after the original cold transaction succeeds: both raw
sidecar paths belong to that exact acquired lease, the original initial and
final identity reads agree, and every original cold barrier passes. On each hot
read the same owner still performs all file-binding, interpreter, helper,
mount and kernel checks, bracketed by the original namespace barriers.
Canonical-only aliases and missing raw membership retain the original parse.
There is no static authority or ownership blocker in that narrow promotion.

The tests cover both sidecars, restored bytes and mtime, identical recreation,
replacement aliases, outside hardlinks, mmap, ancestor redirection, helper death
and a held owner after eviction. The fixed-order comparison used 403 generated
pairs over one file; its small reduction is not a live reader performance claim.
The actual production draft still requires its affected controls and strict
check after promotion.

## Guarded output constructor

The reviewed service proposal preserves independent output authority: an
existing owner checks namespace before input freshness and that same locked
owner checks the complete native lease afterward. Failed final validation must
repeat the original complete output read/hash and independent freshness.
Frames-owned output storage, including a queued or constructing frame Arc,
is preferable to a separate uncharged owner index.

The proposed 64 MiB construction reservation is not yet a proof of the maximum
physical allocation. The existing code has these concrete review gaps:

- `kernel_leases::Inventory::bind` reads each fdinfo with uncapped
  `read_to_string`. A byte cap or explicit pinned kernel maximum is required.
- `Observer::walk` is recursive. `GuardedOutput` can observe a replaced directory
  before its final regular-file hash check. The proposed 1 MiB service stack
  needs a depth/stack bound; a visit count alone does not bound stack use.
- `Observer::begin_with` allocates a mount snapshot before charging its
  construction budget. The reservation must include that failure transient.
- Helper closure path decoding allocates each PathBuf before its length check;
  byte-buffer and vector capacity growth, both observers, closure/path copies,
  baseline/final digests, mount snapshots, readiness decoding, proc row vectors
  and overlapping fdinfo fallback must be included in the maximum-live sum.
  Post-allocation retained-size rejection cannot establish a construction cap.
- Interpreter PATH/path allocation needs an explicit supported-environment
  bound or local admission argument.

The existing permanent retired-Frames allowance counts endpoint workers. A
constructor introduces an additional possible retiring Frames shell and fixed
request storage; include that overlap instead of assuming the old 32-worker
margin covers it. Joining the service disposes its thread and queued task, but
cached or in-flight frame owners can remain alive. Their per-owner reservations
must survive through actual GuardedOutput/helper disposal. Cancellation checks
between operations do not promise interruptible kernel I/O.

These are allocation and lifecycle requirements for prospective activation,
not an explanation for the historical Result tail. No payload, collector,
socket setting, quota ceiling, authority barrier or deadline change is approved
by this review.
