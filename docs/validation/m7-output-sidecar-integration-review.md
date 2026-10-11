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

## Reviewed bounded correction

The later file-only `8506ed9` correction closes the fdinfo and recursive-walk
issues above. Exact-file observation retains original binding and ancestor
watches, watches the resolved regular file and declines directories. The general
observer is unchanged. Bounded lease construction reads complete fdinfo with a
65,537-byte sentinel and refuses more than 65,536 bytes; its exact parser and
identity checks remain unchanged. It checks cancellation between descriptors.

The subsequent daemon draft `5bf0f882`, following `90bdad4`, has no static
output-authority or ownership blocker. The original frame key remains exact;
the same locked output owner brackets independent input freshness. Failure of
the final output barrier falls through to the original complete output read/SHA
and another independent freshness check. Fields drop the guard before its
charge. A queued or constructing task owns the entire charged Frames Arc; an
in-flight selected response also retains that family. Fixed headroom counts
33 retiring Frames and 33 output-owner Box shells. Optional cancellation starts
before either join on endpoint startup failure.

For this supported Linux exec/bootstrap boundary, each original environment
string is bounded by `MAX_ARG_STRLEN = 32 * PAGE_SIZE`. The observed native page
size is 4,096 bytes; daemon production code has no process-environment mutation
route. A conservative 2 MiB allowance covers PATH/interpreter temporaries. This
is a stated trusted OS/process bootstrap premise, not permission to introduce
arbitrary environment mutation later.

The conservative construction envelope is:

| Simultaneously retained or transient requested storage | MiB |
| --- | ---: |
| Two bounded observer construction envelopes | 32 |
| Mount snapshot growth/failure and additional mount owner | 6 |
| Helper JSON, decoded paths, combined copies and request encoding | 8 |
| Binding/readiness/initial inventory collections | 4 |
| Aggregate-budget proc reads and key-vector growth | 11 |
| Supported initial PATH/interpreter temporaries | 2 |
| Remaining bounded paths, hash and command metadata | 1 |
| Total | 64 |

This includes retained owner contents, which the separate 16 MiB owner
reservation deliberately charges again. It is requested-storage accounting,
not allocator RSS, helper address spaces or kernel memory. In the proc envelope,
global and fallback descriptor bytes share the same original 2 MiB aggregate
read budget. Combined old/new byte-buffer growth fits 6 MiB; minimum 33-byte
valid owned rows bound found-key vector growth by 3 MiB, leaving 2 MiB for the
remaining bounded metadata. The exact single-output path combines at most one
output and 1,025 helper paths. Actual source/type-layout and lifecycle controls
remain required before activation or a performance claim.

The four new daemon controls are meaningful: pending construction cannot invoke
foreground construction; eviction retains its 80 MiB reservation; the fixed
slot and one-shot offer cannot overlap constructors; an actual write between
barriers refuses and permanently invalidates the output owner. A requested
additional store-level control must demonstrate that a failed final barrier
really repeats complete output hashing and freshness and preserves full frame
bytes. The queue-level `None` outcome alone does not establish that integration.

The actual `56bb72e` store control closes that coverage gap. Test-only hooks
observe the real freshness classification and full stdout read; they never
substitute either outcome. Healthy owner, held owner lock and refused inspection
quota use the expected original paths. A restored write after the first actual
freshness call requires one full read and two independent freshness calls, with
the complete selected frame equal to the original encoder's frame. Wrong SHA
and oversized persisted bytes retain their original errors. The owned
constructor is cancelled and joined before outcome assertions.

The first actual affected scope passed six guarded-output controls, one fdinfo
bound control, one exact-file control, 21 daemon jobs unit controls and all six
original daemon jobs integration controls. Its whole result remains FAIL because
strict Clippy rejected startup match spelling. The following test-only
semicolon failure is also retained; the corrected `169d6d7` complete affected
scope is still running at this review point. These passes establish neither a
green complete scope nor the original reader p95. Static quota, shutdown and
full-fallback review has no remaining blocker.
