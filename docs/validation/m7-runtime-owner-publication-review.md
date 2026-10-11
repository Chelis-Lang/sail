# Runtime ownership during repair publication

This file-only review examines source `827fecdaea26375194e5f42247cb9147dcb5e879`
and metadata candidate `8b3e4f61296669bb8621f1e85a3d9f1b41a2625b`. No Rust code,
runtime, target, daemon or proof was changed or executed. The immediate next
measurement should evaluate the metadata candidate's unchanged-owner path;
splitting runtime ownership is a conditional design, not a selected feature.

The follow-up below reads integrated source
`c35abdbdab22ec3e68ef1f58df7144eb46fe3c9b`. It refines the minimum change and its
limits without implementing it or attributing any portion of the measured
interval to runtime work. The current-product publication controller will
measure the metadata change separately; neither review supplies repair or
reader acceptance credit.

## What the observed interval includes

The [original publication diagnostic](../../bench/validation/m7-linux-publication-timing-original-repair-db4c65f-failure-2026-10-11.json)
retains two completed workers with freshness establishment times of 4.881175
and 4.731040 seconds. Document parsing took 361 and 725 microseconds. The
120-second fixture failed, and its cold publication row was overwritten. These
two observations do not establish whether runtime hashing, namespace traversal,
helper construction or another fill stage dominates.

The timer in [worker.rs](../../crates/sail-daemon/src/jobs/worker.rs) wraps
`store::establish_freshness`, which first rereads stdout through `project_bytes` and parses it
through `project`, then invokes `FreshnessIndex::establish`. Thus its duration
is wider than `fill`, and the earlier `read_document_us` is not a measurement
of this second parse. Neither the retained rows nor setup timers distinguish
same-owner reuse from cold reconstruction. The [earlier worker review](m7-worker-freshness-publication-review.md)
identifies the duplicate parse, but the current evidence does not make that
small operation the explanation for several seconds.

## Current runtime and source ownership are combined

Despite its name, `RuntimeValidation` in
[freshness.rs](../../crates/sail-verus/src/managed/freshness.rs) retains a complete
project `InputSnapshot` and a single `ReadLease` over both project and runtime
files. `fill` acquires the namespace Observer before reading the runtime
identity, combines `observer.files()` with runtime paths, and acquires that
lease before fresh content validation. It then runs the original project
snapshot, `runtime_current` hashes, observation-plan equality, complete file
membership, namespace and lease barriers, and final runtime-identity equality.
Only that successful transaction can publish the owner.

[runner.rs](../../crates/sail-verus/src/managed/runner.rs)'s `observation_plan`
includes package directories, configuration ancestors, source and build-script
declarations, runtime paths, all three local declaration ledgers, the exact
context `.link` and selected identity JSON. A source write or watched ledger
replacement can therefore invalidate the same owner that protects runtime
bytes. `inspect_started` clears both `Projection.observer` and
`Projection.runtime`; synchronous `establish` also drops both before a new fill.
A dirty lease cannot certify its apparently unchanged runtime subset.

An existing clean owner already avoids repeated project and runtime hashing:
`establish` attempts `request.refresh` first, preserving both namespace barriers,
the complete lease validation, generation and Arc identity. The eligibility
boolean only removes redundant runtime-sidecar parsing after independently
guarded cold reads; it does not split ownership or bypass runtime validation.
Entries bind exact cache/workspace path bytes, context and environment. They
are not shared merely because two requests name the same runtime digest.

Cold acquisition also creates a new helper observer, hashes its runtime closure,
spawns the lease helper and binds kernel/file/mount identities in
[lease.rs](../../crates/sail-verus/src/managed/lease.rs). Those are plausible
reconstruction costs, not separately measured costs in the retained interval.
Retiring the previous owner can also kill and join its helper; this work belongs
to the complete establishment interval when the last reference is dropped.
The optional [sysroot handoff](../decisions/0031-guarded-sysroot-digest-handoff.md)
already retains independent sysroot coverage, but the freshness fill does not
import it. That endpoint covers its exact eligible sysroot inventory, not every
executable, engine, helper, configuration and runtime file in this owner;
using its digest rows alone would not establish complete runtime coverage.

## Evaluate unchanged metadata first

Candidate `8b3e4f6` avoids replacing byte-identical private Linux `.link`,
`input-state.json`, `source-inputs.json` and local `build-inputs.json` files.
It compares the complete original serialized bytes with bounded reads and
before/after descriptor and named identities. Any uncertainty takes the
original atomic-write path. Compiler-family export consumption/publication,
proof receipts and content-addressed identity publication remain unchanged.
Its equality check avoids a write; existing continuous guards remain the
only freshness authority.

This can prevent unnecessary rebuilding for an unchanged request whose owner
was established before the command. It cannot eliminate a first cold fill,
reuse across a new context/environment, revive an already dirty owner, or
retain project validity across a real edit. A declaration change can also
legitimately require new coverage even when project revision bytes match.
The candidate's same-Arc and original-writer sensitivity controls should run
before attributing any product benefit to it.

The next original publication measurement should keep the same 120/30/5
fixture and separate its unchanged repetition from actual source changes.
Compare complete post-child `freshness_establish_us`, terminal visibility,
Current admission and exact results; retain overwritten/missing observations.
If the no-op controls preserve the owner but the original operation remains
slow, optional bounded observations of owner reuse versus fill, namespace
construction, lease/helper acquisition, project snapshot and runtime hashing
plus retired-owner disposal would locate the remaining cost. No such
instrumentation or new run is part
of this review. A synthetic owner pass alone supplies no real repair speedup.

## Minimum conservative implementation boundary

The first implementation should separate **runtime digest authority**, while
retaining the original complete source transaction. It should not begin by
removing runtime paths from `observation_plan`, weakening the original source
lease, or changing `refresh_context_validated`'s project hashing. The proposed
retained shapes are:

| Owner | Contents and responsibility | Invalidation |
| --- | --- | --- |
| Independent runtime owner | Exact request binding, runtime identity SHA, raw `.link` and selected `.json` coverage, complete runtime file/alias namespace Observer and nonrenewable ReadLease | Any runtime binding/content, sidecar, helper, interpreter, kernel, mount or coverage failure |
| Source projection | Original complete observation plan and full lease, freshly captured complete InputSnapshot, generation and reader-overlap state | Original source, declaration, configuration and namespace changes |

The runtime owner stores no prior source revision or Current result. Its digest
may replace only `runtime_current`'s redundant content reads after independent
cold establishment and fresh validation of that same continuous owner. The
source projection still covers the complete original union of source and runtime
files. Thus this initial seam retains source helper acquisition and original
runtime binding checks within that lease. It may save runtime hashing, but does
not remove all reconstruction work or establish a several-second saving.

The runtime inventory must be discovered under native coverage, not trusted
from an earlier disk list. Acquire exact raw `.link` namespace coverage before
discovery; acquire the selected JSON and complete discovered runtime coverage
before eligible content hashing. Read the identity and hash every declared
runtime file independently, then require final identity equality, exact raw
sidecar lease membership and all clean barriers before publishing eligibility.
If either sidecar or any discovered binding changed during acquisition, decline
reuse. Unavailable or oversized discovery follows the original full path. A
previous SHA or canonical-only sidecar alias cannot initialize this owner.

The identity is broader than a toolchain fingerprint: `runner::context` includes
invocation configuration and any selected custom target-description file.
Runtime/source inode overlap is therefore permitted and must remain guarded by
both owners. Never assign an overlapping input exclusively to runtime or source
based on directory spelling. `precise::Observer` already watches every path
ancestor and symlink target and filters events against exact tracked roots;
runtime coverage should use the existing exact-file constructor for this
complete file inventory. It must not recursively track an unrelated project
directory just because it contains a runtime configuration file. The unchanged
source Observer keeps its recursive package and negative-path coverage.

### Joint publication transaction

1. Select the retained runtime owner under the exact request key and retain
   that same charged owner outside projection locks. Validate its namespace and
   full ReadLease before using it. Dirty source state may clear the source
   projection, but cannot clear or renew a valid runtime owner implicitly.
2. Rebuild the original source observation plan and full source lease before
   hashing the new project inputs. All original build-script environment,
   declaration completeness, missing-input and content-path membership checks
   remain. `refresh_context_validated` may receive a runtime-valid callback only
   for the already independently established retained owner.
3. Before publishing the new snapshot, require original plan equality, complete
   source file membership and source lease/namespace barriers. Revalidate the
   same runtime owner, raw sidecar selection and namespace. Keep identity
   fallback parsing when exact raw membership is absent; no new eligibility is
   inferred from a successful source refresh.
4. Under the short publication lock, require the original source generation and
   selected runtime owner identity still match. Reject replaced owners and late
   fills. An evicted but retained owner is not invalid merely through eviction;
   its continuous guards and physical charge must remain alive. Never substitute
   a freshly created equal-digest owner between the two barriers.
5. Only this joint transaction can supply the new source revision. A failed
   joint barrier cannot publish or return its prospective complete snapshot as
   Current. It follows original invalidation/refill and
   Current/Historical/Unknown behavior. An intact runtime owner certifies runtime
   bytes only, never the edited source or an old proof receipt.

Hot inspection must validate both owners, retaining caller-entry checkpoint,
both namespace barriers, same-pointer/generation checks and the existing 100 ms
owner wait. Coalesced snapshots must identify the joint source/runtime owner
pair; a checkpoint from the old source projection cannot authorize a sequential
reader or a new source generation. Busy ownership stays unavailable for that
read and does not destroy independently valid leases. No cache-age test, extra
wait, helper renewal or late promotion is introduced.

### Exact benefit and ownership limits

The minimum proposal remains within one exact cache/workspace/context/environment
key. The original repair fixture changes the function selector from `selected`
to `missing`; `context_hashes` includes the full engine request and arguments.
Those are separate proof contexts even when compiler artifacts are shared.
The two retained worker timing rows do not include context IDs or an owner-reuse
field. A same-key split therefore cannot be claimed to save both observed fills.
Its supported target is a real body edit under unchanged invocation selectors
and declarations after an independent runtime owner already exists.

Cross-context runtime sharing would need a separate reviewed identity contract:
equal runtime SHA alone does not bind a new context's raw `.link`, exact path
view, environment or source interval. It is not part of this minimum proposal.
Neither are removing runtime coverage from the source lease, sharing a lease
helper or borrowing a proof-time/sysroot owner. Such changes require their own
complete overlap, inventory and lifecycle arguments.

Before implementation, reserve every added retained owner and construction
overlap, with lifetime charges through final Arc disposal. Indexed context
limits do not bound evicted owners still held by readers. Cap total helpers,
native descriptors/watches, inventory/path storage and concurrently constructing
owners, and release helper resources before refunding their reservation. Keep
the original admission limit of 4,096 combined files, including helper runtime
closure; splitting must not admit twice that inventory. Account for duplicate
helper closure/namespace storage and joint-barrier scratch. Existing legacy
loaders do not provide a new whole-owner memory ceiling; do not claim a 16 MiB
peak or RSS bound from post-construction `retained_bytes` alone. Resource refusal
must retain original complete validation, without an unbounded constructor
queue or hidden retry. Helper kill/wait and retired-owner disposal belong to
whole-operation measurement; moving them to an unowned reaper is not a saving.

No new crate edge is needed for an implementation within existing managed
observer/lease/freshness modules. Selecting independent runtime authority needs
a numbered ADR extending the joint-owner argument in
[ADR 0029](../decisions/0029-kernel-lease-work-bounds-and-freshness.md), with
[mount-generation coverage](../decisions/0032-kernel-mount-generation-coverage.md)
and an explicit physical/resource ledger. This review does not allocate an ADR
number or select a new authority path.

## Required decision evidence

The next current-product publication run should first establish whether the
metadata no-op preserves an existing owner on unchanged commands. If it removes
most of the original cost, a second runtime owner may not earn its construction
and barrier overhead. If actual source edits remain slow, measure complete
establishment, including disposition of the old owner, source observation,
source/helper lease acquisition, source hashes, runtime content hashes and joint
validation. The retained aggregate 4.73–4.88-second values do not isolate these
operations. Keep the original 120/30/5 fixture and all complete reports unchanged.

Before selecting a split, real controls must cover:

- A real source-body edit under the same request: runtime owner pointer stays
  unchanged, source owner/revision changes, original source hashing and final
  checks run, and the old source result cannot remain Current. Original unified
  ownership must fail the runtime-retention assertion as the sensitivity check.
- Source mutation/restoration through direct writes, outside hardlinks, mmap or
  ancestor redirects between capture and either final barrier: the old source
  generation never certifies the new result. Runtime/source inode overlap must
  invalidate both as required.
- Actual `.link`, selected JSON or runtime bytes replaced, restored, aliased or
  deleted/recreated; helper death, interpreter change, kernel loss and transient
  mounts: retained runtime reuse refuses and original full validation applies.
  Cold setup still performs original independent runtime reads and hashes.
- Changed selector, environment, cache/workspace raw bytes or missing exact
  membership: same-key reuse is refused, with no canonical or equal-SHA shortcut.
- Source refill raced with runtime replacement, held eviction, concurrent readers
  and shutdown: no substituted owner, old checkpoint or late generation is
  published; held owners stay charged through resource disposal. Preserve the
  existing delayed-publication and restored-write reader controls.

Compare complete establishment and terminal publication on actual edited source,
not just `runtime_current` or a generated runtime-only microbenchmark. Until those
checks and the whole-operation comparison justify implementation, current users
retain the original hashing/freshness behavior; the public contract and original
reader and repair deadlines are unchanged.
