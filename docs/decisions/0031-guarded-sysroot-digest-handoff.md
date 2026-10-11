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
scopes and 4,096 inventory files per owner. Root/path/frame and retained ownership
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
twelve pointer edges and node metadata. A deliberately conservative 1,024-byte
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
