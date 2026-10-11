# 0031: Guarded sysroot digest handoff between managed commands

- **Status:** proposed
- **Date:** 2026-10-11

## Context

The [original managed acceptance with setup timings](../validation/m7-managed-setup-012707a.md)
passed unchanged, including cold proof, CLI/MCP jobs, missing receipts and a
restored-mtime failing edit. Its unchanged command reused both proof reports
with no verifier invocation but still spent 15.992 seconds hashing its sysroots.
The existing run-local `managed::sysroot::Baseline` safely reuses these hashes
at final validation because namespace observers and nonrenewable read leases
were acquired before the first byte read. Reusing persisted hashes without
continuous coverage would break that argument. Neither metadata nor restored
mtime proves unchanged bytes; mmap, hardlinks, aliases and transient namespace
changes require the existing complete kernel protection. A mountinfo snapshot
comparison alone does not observe a transient bind-mount-and-restore; remote
reuse must additionally require supported continuous kernel mount-generation
coverage. Unsupported coverage declines handoff.

## Proposed decision and review boundary

Retain independently constructed sysroot baselines in a bounded **local daemon**
service. The service has a distinct versioned Unix endpoint and no shared-store,
HTTP or cross-host export. It never accepts caller-submitted hashes. It installs
all original guards and then reads the complete file contents itself.

Each caller still installs its original local sysroot Baseline before requesting
reuse. A successful handoff must match exact roots, complete inventory, native
path bytes, canonical device/inode bindings and mount-namespace identity. The
service validates its continuous original owner before publishing the complete
bounded response. The caller brackets import with its local clean barriers.
The owners therefore overlap: the daemon protects bytes from its first read
through the handoff, and the caller protects the same files from before the
handoff through the original proof and receipt commit barriers.

After this certification, the caller's original local Baseline is the authority;
there is no remote Current token or remote check during proof. Loss of the
remote owner before certification refuses the handoff and invokes original local
content hashing. Loss after certification cannot end the caller's independent
continuous protection. If either local guard fails, the existing rehash,
Historical/Unknown classification and no-receipt rules remain mandatory. No
late response may promote a caller that already selected local fallback.

Root review must approve this overlap/lifetime interpretation before runner
integration. The cache and independent controls can be implemented first.

## Authority and fallback matrix

| State | Digest authority | Action |
| --- | --- | --- |
| Cold, old/missing endpoint or incompatible version | None | Original local full hashing |
| Background fill queued/running, bounded capacity exhausted | None | Original local full hashing; never wait for construction |
| Local coverage unsupported, preexisting writable mapping, nonrepresentable path | None | Original local full hashing and original uncertainty policy |
| Complete guarded daemon entry plus complete matching local owner | Overlapping guarded handoff | Import only eligible hashes; retain all original local barriers |
| Namespace/helper/mount/inode/watch loss before handoff, timeout, partial or malformed frame | None | Refuse entry and perform original local full hashing |
| Namespace/helper/mount/inode/watch loss in caller after handoff | Caller authority lost | Original final rehash and no Current receipt |
| Daemon restart, shutdown or entry eviction after certified handoff | Caller owner continues independently | Original local barriers remain authoritative |
| Different runtime/engine/toolchain selection, root or mount namespace | No matching scope | Independent guarded fill or original local hashing |

Noneligible executable/engine/tool/configuration identities are still read by
content in the caller. Identity sorting, path encoding, context/key bytes,
invocation arguments, verdicts and diagnostics remain unchanged. Solo operation
without a usable daemon remains the original local path.

## Lifecycle and resource contract

Lookup performs no content hashing, guard construction or wait behind a builder.
A bounded short RPC returns an available complete entry or immediate refusal.
The initial cold caller queues construction only after its original full hash
barrier, so there is no duplicate startup hash before that caller reaches it.
One separate maintenance worker constructs entries from its own guarded reads;
it never invokes Verus and does not add a verification worker.

Initial proposed caps are two retained owners, one constructing owner, two queued
scopes and 4, 096 inventory files per owner. Root/path/frame and retained ownership
charges must be checked before publication; original observer, helper and lease
limits remain in force. A small frame-size bound alone is not a decoded-memory
or helper-process RSS claim. Cancellation checks accompany every bounded read
chunk and construction stage. Shutdown cancels construction, releases retained
guards and owns/reaps its maintenance resources. Unsupported or uncertain
coverage must decline reuse without blocking foreground verification.

The exact implemented storage/transport limits and shutdown behavior must be
reviewed and documented before this ADR becomes accepted. A byte-identical disk
record, timestamp, TTL or eventual successfully renewed owner is never authority.

## Consequences and required evidence

Warm commands can avoid repeated large sysroot byte reads while retaining their
original caller guards. Cold behavior, initial guard acquisition and all
noneligible hashing remain. Independent fills add real maintenance work whose
cost must be reported, never hidden as fixture preparation.

Required controls include daemon/owner restart and loss; existing/new mmap
writers; outside hardlinks; restored bytes/mtime; alias/namespace/mount changes;
overflow; isolated scopes; malformed/truncated/old/late RPCs; busy/cold fallback;
bounded construction/cancellation and eviction. A cached handoff and an original
local run must produce exactly identical ordered runtime identities and context
keys. The original full managed acceptance and affected workspace/solver/corpus
gates remain required. This ADR neither lengthens those limits nor qualifies
M7 source-engine, reader p95 or edit-loop performance.

## Requested-storage accounting implementation

The first admission primitives charge actual String/PathBuf and Vec capacities
with checked arithmetic. On the pinned 64-bit toolchain, observer and kernel
inventory BTree nodes have at most eleven entries of at most 48 inline bytes,
twelve pointer edges and node metadata. A deliberately conservative 1, 024-byte
charge per entry plus one root covers sparse nodes; it is not a length-only
payload estimate. See the pinned [Rust node layout](https://github.com/rust-lang/rust/blob/1.99.0/library/alloc/src/collections/btree/node.rs).

HashMap usable capacity is not allocated bucket count. The observer accounting
charges twice the next power of two of usable capacity plus 32 buckets, with
32 additional bytes per bucket for control bytes, padding and alignment. It
refuses unaudited pointer widths and tuple alignments. The observer also charges
the inotify wrapper's shared descriptor allocation once; the pinned wrapper
owns one Arc and watch descriptors hold Weak references, rather than separate
allocations per descriptor. Its exact version is bound by Cargo.lock.

These primitives count requested Rust storage and deliberately overcount nested
inline structures. They do not claim an allocator RSS bound or include kernel
watch/lock memory, descriptor tables or the separate Python owner's address
space. Resource-count limits and construction/transient reservations remain
separate requirements before the daemon handoff is enabled. Exhaustive field
destructuring makes added guard fields require an explicit accounting decision.
Overflow refuses admission rather than wrapping to a smaller charge.

## Optional endpoint wire contract

The independent `sail_api::sysroot` endpoint uses version 1 frames capped at
1 MiB. It appends `.sysroots-v1` to the existing local cache socket path; neither
the job protocol nor SMT cache keys change. Requests contain at most eight
sorted exact UTF-8 absolute roots, their actual canonical device/inode bindings,
the process filesystem root identity, mount namespace and mount inventory
content digest, and the pre-sysroot runtime-selection content digest. Unsupported
native encodings decline reuse instead of converting path bytes lossily.

A Lookup response must echo the exact scope and return a sorted complete
inventory of at most 4,096 bound files. The caller remains responsible for
comparing that inventory against its independently guarded actual local files.
No syntactically valid response alone is authority. Prepare is only a bounded
maintenance hint; it carries no caller hashes. Collection counts are bounded
while decoding, and oversize output is refused before any frame bytes are sent.
Old versions, trailing/truncated JSON, wrong variants and late full responses
refuse reuse. One 100 ms budget covers connect/backlog, write, read and validation;
there is no retry or late promotion. Socket operations remain safe Rust through
the pinned socket 2 dependency. The byte/work caps are explicit; elapsed checks
do not pretend to interrupt kernel I/O or scheduler suspension.

## Construction and endpoint implementation review

The optional implementation uses one maintenance worker and one endpoint worker.
Lookup never performs construction or content hashing. A queue contains at most
two exact scopes; at most two independently guarded owners are retained, each
admitted only when exhaustive Rust-owned storage accounting is at most 16 MiB.
An invalid or evicted owner is released before another helper is constructed.
One endpoint owns at most one accepted socket and one complete response clone;
it has a finite kernel backlog and creates no per-connection thread. Kernel
backlog capacity is not a bound on Linux kernel networking memory.

The 96 MiB construction reservation is a Rust storage ledger, not RSS,
Python allocator usage or a total process peak. Three bounded namespace/helper
observers each precharge at most 16 MiB before watched collections grow. Inventory
traversal stops at 16,384 visits, 4,096 files and 512 KiB of admitted original path
capacity. Canonical row capacity is separately bounded before hash-map, capture
and response growth. Each helper closure report is capped at 1 MiB and 1,024 paths
while decoding; helper baseline/final content reads each stop at 128 MiB. Content
reads use 64 KiB chunks, reject nonregular files with a nonblocking open, and stop
at 4 GiB per construction. Request and response encodings each cap 1 MiB before
sending a frame. The typed helper request avoids allocating a JSON Value for
every native path byte.

The finite peak must additionally charge retained owners, the constructing
owner, original queued scopes, the endpoint's decoded request, its complete
snapshot clone and response frame, helper protocol buffers and temporary
inventory/hash collections. File descriptors, inotify watches, leases and helper
processes are separate resource counts; they are not represented as claimed RSS.
Admission controls and their implementation ledger require review before the
runner is enabled.

Cancellation propagates one shared stop source and the original construction
start through inventory, helper observation, helper hashes and final barriers.
Nested observers cannot restart a fresh 120-second construction clock. Shutdown
sets index cancellation before joining the endpoint, wakes a waiting maintenance
worker even after lock poisoning, and joins both owned workers outside RPCs.
Construction observes stop between bounded operations; this is not a promise to
interrupt kernel I/O or scheduler suspension. A cancelled owner cannot publish.

The local endpoint requires a cache socket owned by the current user with mode
0600, and refuses foreign files and foreign-owned existing sockets. Older
daemons with a public socket mode decline this handoff; an upgraded cache daemon
must set its owned socket private immediately after bind and before accepting.
An active endpoint is never unlinked; stale replacement requires a refused
connection and an exact inode recheck. Its trust boundary is the same private,
trusted local daemon as existing cache/job sockets. Snapshot shape validation
cannot make a malicious same-user daemon trustworthy. The caller still holds
its original independently acquired guards before the request, compares the
complete actual inventory and path/canonical/device/inode rows, and captures
only files eligible after those guards began. The FileDigests alias map is never
populated from remote rows. Every failure retains the original local path.

Root review permits staged runner integration of the endpoint and import
primitives. The original caller Baseline is acquired before lookup, imported
rows remain confined to its eligible captured map, and independent fill hints
follow only a complete original local hash barrier. `sail serve` owns the
optional endpoint and joins it on shutdown; its cache socket is private to the
user. All failures retain local hashing. Primitive transport/ownership controls
do not establish original managed cold/warm acceptance or a measured product
speedup. This ADR remains proposed until the integrated acceptance is retained.

The index reserves 96 MiB before starting its single builder and refuses a charge
above 160 MiB including up to 32 MiB retained owners and a 16 MiB endpoint/queue
reserve. The latter covers two queued scopes plus constructing/accepted scope
copies, one bounded input frame and typed decode, one snapshot clone and its
bounded output frame. These limits apply to one index/endpoint family, not every
unrelated daemon in a process or the system. Retained admission remains 16 MiB per
owner; the larger construction reserve covers concurrently built temporary
collections before retained admission. It does not allocate 96 MiB eagerly.

The construction ledger covers the three 16 MiB observer reserves, conservative
BTree node charges for original/eligible/captured/canonical-digest inventories,
512 KiB path-capacity inventories and clones, file row/descriptor/identity Vecs,
helper baseline/final digest vectors, helper request/readiness/closure buffers,
mount guards and hash chunks. Path capacity is checked before collection growth,
with capacity shrinking on optional inventory paths; canonical row capacities
are also admitted before capture/hash-map insertion. The same original local
inventory path does not acquire these optional restrictions.

Per daemon index, there are at most two retained lease helpers and one
constructing helper (a conservative upper bound; eviction releases the old
helper before constructing its replacement). Each helper opens at most 4,096
leased regular files. Each of the three observers per owner retains at most 8,192
watches, giving a conservative 73,728-watch bound across all three owners. Parent
Rust descriptors are bounded by these fixed owners, their inotify/mount/socket
handles, one listener, one accepted endpoint and the finite temporary helper
probe. These are kernel resource counts, not Rust heap or Python RSS claims.

The optional pre-sysroot selection identity streams exact sorted byte-key/value
environment pairs into SHA-256. JSON object keys cannot represent those OS byte
keys. The stream declines optional reuse after 256 KiB of encoded input without
retaining a serialized buffer or copied pair vector. The original complete
context encoding and proof/cache keys are unchanged. The first staged activation
failed to construct this scope and retained local hashing; its separate cold
timeout remains [recorded](../validation/m7-sysroot-activation-7da9579.md).
The [corrected selector controls](../validation/m7-sysroot-selection-controls.json)
pass actual byte-key/non-UTF8, exact encoding, changed input and overbudget cases;
complete integrated acceptance is still required.

## Socket readiness within the original budget

The optional client waits for Unix socket readiness through the already pinned
`rustix` dependency instead of sleeping one millisecond after each nonblocking
`WouldBlock`. This adds a direct external dependency to `sail-api`; it changes
no internal crate dependency edge. The borrowed descriptor has no additional
worker or retained owner. Readiness permits only another read or write attempt.
EOF, peer closure, socket errors and incomplete frames remain ordinary refusals.

The request-entry 100 ms deadline is unchanged. Interrupted waits recompute the
remaining time from that same start; checks still bracket every operation and
follow decoding and complete snapshot validation. Late readiness cannot grant
reuse, including on platforms that round a polling timeout upward. The existing
complete-response control closes its peer after sending the frame; the existing
disconnect, partial trickle, late response, wrong variant and full backlog
controls remain unchanged.

The preceding installer gate failed a positive handoff control at its original
deadline. That observation does not establish its cause or demonstrate that
readiness waiting fixes it. Validation of this transport change is separate from
the real-project acceptance and performance measurements.

The [independent affected-crate validation](../../bench/validation/m7-linux-sysroot-readiness-34d04c9-emulated-2026-10-11.json)
passes strict all-target checks and all seven original sysroot controls on the
local Linux container. It retains the original deadline and assertions. A
complete current workspace gate and real-project acceptance remain required.
