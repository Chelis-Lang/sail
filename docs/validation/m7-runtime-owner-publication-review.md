# Runtime ownership during repair publication

This file-only review examines source `827fecdaea26375194e5f42247cb9147dcb5e879`
and metadata candidate `8b3e4f61296669bb8621f1e85a3d9f1b41a2625b`. No Rust code,
runtime, target, daemon or proof was changed or executed. The immediate next
measurement should evaluate the metadata candidate's unchanged-owner path;
splitting runtime ownership is a conditional design, not a selected feature.

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

## Conditional runtime-through-edit design

If actual edits still spend substantial time rebuilding unchanged runtime
coverage, the narrowest sound split initially stays within one exact request
key. Independently acquire a runtime-only namespace/lease owner before its
complete initial identity and content reads. Retain it separately from the
project/configuration/declaration owner and revision. A source edit invalidates
the latter; its new snapshot must be completely rebuilt under fresh guards.
The retained runtime can contribute only after its own full clean barriers
prove continuous coverage across that construction interval. Bracket both
owners and the exact context selector within the caller's interval, retain
pointer/generation checks, and discard a late project fill after replacement.

The partition must bind exact raw sidecar and runtime path spellings,
canonical identities, full ordered identity contents and helper/interpreter/
mount coverage. Overlapping project/runtime files remain invalidating inputs
to both owners. Keep the context `.link` selection freshly guarded rather than
using the JSON digest as selection authority. Any missing, dirty or ambiguous
runtime coverage uses original full acquisition and hashing. Restart, eviction
or replacement cannot transfer authority to a new owner; a still-live evicted
owner is usable only while the same guards and accounting remain intact.

This needs an ADR and an explicit bound on retained and in-flight owners,
construction overlap, inventories, helpers, descriptors and memory before
implementation. Existing limits of 32 indexed contexts and two revalidations
do not themselves bound the physical cost of another owner. A split may add
helper and barrier work, so its whole-operation gain must be measured; no
fraction of the observed 4.7 seconds can be promised in advance.

Required controls would distinguish a source edit from runtime invalidation:

- Reuse unchanged runtime coverage during a real source edit while rejecting
  the old source revision; require a fresh complete source snapshot and all
  original joint barriers before Current.
- Restore source bytes/mtime, redirect ancestors, or write through an outside
  hardlink/mmap during construction; no stale source Current can survive.
- Mutate either identity sidecar, replace identical runtime bytes, change
  selection/environment, redirect an alias, lose a helper or observe a mount
  event; runtime reuse must refuse and take the original full path.
- Hold an owner across eviction and overlapping fills; charge its entire
  lifetime, reject owner substitution and late publication, and close owned
  resources on shutdown without broadening admission.

The [public freshness contract](../freshness.md) remains unchanged. This review
adds no Current cache, proof memo, receipt transfer or asynchronous completion;
the original reader, repair and benchmark bounds remain open product gates.
