# Candidate write fidelity

Status: an unsupported stock-library probe and a generation/accounting
replacement with full-root Stock positive proofs for
[#372](https://github.com/Chelis-Lang/sail/issues/372). The implementation candidate
now wires the actual cache/writer/server paths; full negative/native qualification
is incomplete and it is unselected for rollout. No transport, dependency edge or
origin admission policy is changed.
The [cache-origin architectures](cache-origin-admission.md) remain an owner
choice. A `CandidateWrite` preserves a candidate; it does not certify that a solver
produced it or authorize its reuse.

The initial source base is `58806916`. The existing native code already carries
one immutable `Job` through its FIFO, calls `DiskCache::put_scoped` with fields
borrowed from that job, and settles pending memory only when `Arc::ptr_eq`
matches its original value. This proposal moves those actual operations into
checked shared source. A parallel abstract ledger, a caller-supplied success
flag and a new assumed ownership adapter cannot replace the production steps.

## Actual operations to preserve

| Operation | Actual source | Required correspondence |
|---|---|---|
| Create the candidate | `sail-daemon/src/server.rs::{State::put,State::keep}` | Preserve the original key, `Arc<Value>`, command, solver and provenance; raw PUT and imported values stay candidates |
| Queue and retain | `sail-daemon/src/writer.rs::Writer::submit` | The pending-memory clone and queued job retain the same original allocation; duplicate, closed, capacity and wait behavior stay intact |
| Dequeue and write | `sail-daemon/src/writer.rs::Writer::run` | The actual `put_scoped` arguments are this job's key, reply, command, solver, mode, provenance and effort, with no re-resolution or replacement |
| Settle success | `sail-cache/src/memory.rs::MemoryTier::settle` | `Written(stamp)` changes only the selected key's still-pending slot holding that original allocation, preserving its payload |
| Settle refusal/error | The same method | `Exists` and the actual write error remove only that same pending allocation; a replacement, on-disk slot or stale completion is untouched |
| Retire the job | `sail-daemon/src/writer.rs::Writer::run` | Queue keys, original cost, notification order and logging use the original completed job |

These statements do not imply that the candidate was admitted. Pending memory
is already serveable before persistence. No source-shared constructor may turn
its pending/on-disk state into a solver-origin fact.

## Proposed checked boundary

The first implementation must share the actual records and executable
transitions, rather than construct a second snapshot of them:

1. Retain the real native key type and `Arc<Value>` in a private-field candidate
   record. If records move into the existing dependency-free acceptance leaf,
   use type parameters for native key/effort/stamp types and native aliases;
   do not add cache, key or protocol dependencies to that leaf. Preserve the
   actual slot/value layouts and budget accounting, and avoid new per-job
   payload copies or owner allocations.
2. Derive a borrowed write-argument view directly from that record. Its fields
   must be the same original key, mode, reply, effort, command, solver and
   provenance consumed by the actual native `put_scoped` call. A getter proof
   without the real call-site join is insufficient.
3. Bind completion to that call's actual `Result<PutOutcome, Error>` and original
   record. An unrestricted `from_success`, externally supplied Boolean or
   constructor that merely accepts an asserted outcome cannot establish the
   call correspondence. If a narrow I/O contract remains necessary, identify
   the actual operation, arguments and returned value separately from physical
   storage truth; keep that provider obligation explicit.
4. Move the actual pending-slot transition into checked shared source, including
   its direct `Arc::ptr_eq` operation. The selected slot, current state, original
   job and actual completion must flow into it. The surrounding key lookup,
   mutex ownership, counters, removal and LRU bookkeeping still need their own
   exact joins; do not silently assume them by verifying a toy `Option<Slot>`.

The Arc support probe has now run and failed before verification. The
source-only replacement in `sail-acceptance/src/cache_candidate.rs` contains
an actual checked clock and owned slot/ticket transitions; it is not a parallel
ledger. The real native map stores its `Slot`, the real writer queue holds a
sealed `PreparedWrite`, and the server keeps original read observations through
stale-read removal. Both raw PUT and shared/bundled keep paths construct the
same immutable `CandidateWrite`. Existing same-slot assertions remain, with
native tests prepared for equal-byte/different-Arc replacement, same-Arc ABA,
two tiers with equal local ordinals, permanent exhaustion and actual disk
Written/Exists/error outcomes. None have run at this candidate, and no
production writer transition is claimed qualified.

The intended native representation reuses the actual checked LRU clock: an
insertion snapshots its issued tick as immutable generation; touching changes
only the LRU tick. Generation values are local to a retained endpoint. Pending
handles own their original inner endpoint; read/removal handles borrow their
original tier, avoiding a new endpoint clone on read hits. Kernel endpoint and
ticket fields stay paired and private, with checked projections rather than an
identity comparison. The kernel's `matches_ticket` predicate compares only the
supplied key/generation within that endpoint; it does not assert global owner
identity. The actual lock/map/accounting joins remain explicit.

A newer insertion is distinct even when it reuses the same Arc. This is a
conservative difference from the old Arc-based ABA check: an old completion or
read removal refuses instead of affecting that newer insertion. Existing
queued-key serialization is preserved. Counter exhaustion must clear/refuse
memory safely without resetting the clock; disk candidates continue normally.
The native `PreparedWrite::write` borrows all seven disk-request fields from
its actual candidate, calls the real `DiskCache::put_scoped`, and feeds that
exact returned Result through the checked `observed_completion` projection.
Its private endpoint dispatch is not a public `finish(success)` API. Candidate
value, slot-state and disk-outcome native names are aliases of the shared real
records; the disk format and mode/solver/provenance fields are preserved.

Each slot charges the original 128-byte logical allowance plus the actual
nonnegative carrier-size growth from insertion identity, including padding.
Private layout-only types preserve the complete prior state/slot shape; they
are never instantiated or used as cache state. The portable const conversion
checks its round trip before adding the delta; overflow saturates to a charge
that memory admission refuses. The measured native layout grows from 80 to 96
bytes, so this charge adds 16 bytes rather than only the eight-byte field
([#486](https://github.com/Chelis-Lang/sail/issues/486)). This does not establish
an allocator/RSS bound or quantify spare margin in the original allowance.
The configured memory budget also reserves three native pointer
words for the additional retained tier endpoint metadata (24 bytes on the
supported 64-bit hosts). Queue cost adds the actual `size_of` carrier difference
between prepared and raw candidate records; it is not guessed from selected
fields. Read hits retain the existing value Arc clone but add no endpoint clone.
The original five-slot LRU fixture includes the newly charged fixed metadata,
retaining every eviction/count/budget assertion. New boundary tests reject a
reply just above the usable quarter limit and a second queued record whose
handle metadata exceeds the original queue budget. The original source candidate's
104 cache/daemon tests pass; the layout correction adds an actual-type admission
regression and a separate undercharged native mutation control, pending execution.
Allocator bookkeeping and RSS are not this logical budget. Layout, cost and
conformance are unmeasured; no performance result is claimed.

A separate source/native-control family replaces the original value clone with
an equal-payload allocation. The native Arc comparison assertions must reject
that change; it receives no Stock-negative credit. Stock's Arc value abstraction
is not relabeled a pointer-identity theorem. The freshness proof uses insertion
generation and private endpoint routing, never payload equality.

## Allocation identity is a prerequisite

The pinned stock source provides the following evidence:

| Source | Source finding |
|---|---|
| Verus `7d829946`, `source/vstd/std_specs/smart_ptrs.rs` | `Arc::new` specifies the dereferenced payload. This file has no `Arc::ptr_eq` specification |
| The same revision, `source/vstd/view.rs` | An `Arc` view projects its payload's view |
| The same revision, `source/rust_verify/src/fn_call_to_vir.rs` | Arc clone is special-cased as an identity operation in the SMT encoding |
| Rust `58cf8f9c`, `library/alloc/src/sync.rs` | Native `ptr_eq` compares the addresses of the actual `ArcInner` pointers; native clone retains that pointer and increments the strong count |

The exact sources are [stock smart-pointer specifications](https://github.com/verus-lang/verus/blob/7d829946231d6413a398f6e4160cdfe951261dad/source/vstd/std_specs/smart_ptrs.rs),
[stock views](https://github.com/verus-lang/verus/blob/7d829946231d6413a398f6e4160cdfe951261dad/source/vstd/view.rs),
[stock call translation](https://github.com/verus-lang/verus/blob/7d829946231d6413a398f6e4160cdfe951261dad/source/rust_verify/src/fn_call_to_vir.rs)
and the [pinned Rust implementation](https://github.com/rust-lang/rust/blob/58cf8f9c35b355199727627d07ae34a4e2846d67/library/alloc/src/sync.rs).
The subsequent direct full-file probe confirms that the actual operation is
unsupported at all five calls ([#473](https://github.com/Chelis-Lang/sail/issues/473)).
It exits 1 with no resource marker and passing final source/runtime guards,
so zero proofs qualify. See the [owning record](../validation/cache-candidate-refinement.md).

The [support probe](../../proofs/support/candidate_write_arc.rs) calls actual
`std::sync::Arc::{new,clone,ptr_eq}`. It requires a nonempty original, a retained
clone that matches it, equal-byte distinct live allocations that do not match,
and an equal-valued replacement that does not match the retained original.
It introduces no user assumptions, external-body declarations, raw-pointer
substitution, provider trait or invented allocation number. There is no copied
writer ledger in the probe.

Run the whole file through the validated pinned stock runtime, with the same
pinned Rust environment as the canonical checker. For example, inside the
native core image after its existing runtime validation:

```sh
/opt/verus/verus-x86-linux/verus \
  /work/proofs/support/candidate_write_arc.rs \
  --cfg sail_verus --no-cheating --output-json --num-threads 1
```

Use the existing 120-second external deadline and default verifier resources.
Preserve the complete output and actual function/obligation inventory. Do not
focus functions, add a new `assume_specification`, infer allocation identity
from payload equality, or raise resources to conceal missing support. A
missing operation/model or failed witness is an unsupported prerequisite,
not a qualified ordinary negative or proof of the candidate-write theorem.
The probe is not selected by the canonical component drivers.

The unsupported pointer proof is retained as history. The coherent source
candidate replaces the native Arc guard in its actual callers. No conditional
`OwnerIdentity` trait is added. The replacement is an actual insertion-issued
generation guard, not an assertion that pointer identity is now supported.
The actual map lookup with the native key, mutable slot access, locking and
bookkeeping also need their real joins before full writer coverage can be claimed.

## Actual owned accounting transition

The next source slice replaces the actual `Inner.bytes` and `Inner.pending`
fields with a private-field shared `Accounting` record. There is no parallel
ledger. Installation reserves both additions with overflow refusal before either
counter changes. Native disablement clears this record while preserving the
separate exhausted lifetime clock. Eviction and the reported counters read the
same actual record.

Settlement, stale-read removal and administrative removal all consume the actual
owned map entry through `EntryUpdate`. Before shared accounting, native `apply`
computes its existing charge from that same owned slot. `Accounting::apply`
returns the exact original transition and payload: unchanged/stale entries keep
both counters; a written pending entry only decrements pending; retirement
subtracts its supplied charge and decrements pending only for a pending slot.
The existing saturating subtraction behavior is preserved. The native caller
then restores that returned entry or removes its actual LRU ordinal.

This theorem covers **declared, supplied-charge accounting**. The native cost
function, map membership/uniqueness, locking, complete LRU relation and physical
endpoint correspondence remain distinct joins. It does not prove that an
arbitrary supplied cost equals allocator usage or global map occupancy. Counter
initialization is private to a fresh native `Inner`; source policy rejects
reassignment or unchecked mutable escape. The record has exactly the same two
u64 counter fields as before and introduces no retained allocation.

Two nonempty constructive operations exercise the actual shared counter path,
including stale replacement, written/exists/failed outcomes, retirement,
overflow refusal and clear. Thirteen additional full-envelope runtime controls
retain all existing control descriptors; the native omission control exercises
the actual owned-map/counter caller. Complete Stock roots prove the shared
operations at `494c091c`; ordinary controls and native execution remain required.

## Actual ordered retirement boundary

The next source slice moves the real `Inner::apply` accounting and ordered-index
retirement into `apply_accounted_order`. It consumes the same owned `EntryUpdate`,
mutates the actual supplied `Accounting` and `BTreeMap<u64, [u8; 32]>`, and returns
that exact update. Removed entries delete only their original slot tick;
Written and Unchanged preserve the entire ordered map. All original accounting
contracts/bodies remain unchanged and the new operation invokes them directly.
The native caller forwards its actual two fields exactly once, then restores the
returned slot or completes retirement. No extra retained object or runtime
ledger is introduced. The native lock, slot-map selection and global relation
between slots, ticks, clock and counters remain separate obligations.

Its constructive witness uses two nonempty actual slots issued by the same
Clock, including an independently retained on-disk slot, and exercises Removed,
Written and Unchanged transitions. Seven additional full-envelope controls and
an actual native index-omission control are prepared. The prior native accounting
omission control is rebased to retain the original LRU removal, so it continues
to isolate omitted accounting. These additions are unqualified source until
fresh complete Stock/native runs establish their actual inventories/results.

The next slot-map join must use the actual native key and methods. Pinned
[hash.rs](https://github.com/verus-lang/verus/blob/7d829946231d6413a398f6e4160cdfe951261dad/source/vstd/std_specs/hash.rs)
conditions its functional HashMap specifications on `obeys_key_model`; its
documented primitive/Box axioms do not themselves supply the native byte-array
key premise. The actual `remove_entry` and `get_mut` calls have a direct
support check. `proofs/support/cache_candidate_map.rs` stages those exact
operations over the production shared Slot shape, plus the same key's model
obligation and a nonempty primitive-key control. It adds no assumed key model,
replacement ledger or new specification. At `d6f3d144` the whole file exits
before verification because both methods are unsupported
([#498](https://github.com/Chelis-Lang/sail/issues/498)); it earns no proof/control
credit and does not determine the separate key-model obligation. The follow-up
`cache_candidate_map_key.rs` support root uses specified APIs and requests that
prerequisite independently, without granting it. No native HashMap defect is
claimed. Primitive u64 comparison
laws for ordered retirement are already present in pinned
[laws_cmp.rs](https://github.com/verus-lang/verus/blob/7d829946231d6413a398f6e4160cdfe951261dad/source/vstd/laws_cmp.rs).

Recovery still cannot reconstruct the model's prior recorded-origin membership
from a file stamp, checksum or plausible reply. This retirement step neither
selects a Q40 architecture nor admits a candidate as solver truth.

## Controls and remaining obligations

After support and source review, require a nonempty end-to-end candidate write
through the actual writer and memory transition. Ordinary proof controls must
mutate the complete real operation while preserving its contracts and loops.
They must reject swapped keys or metadata, altered reply/mode/effort, another
original Arc with equal bytes, stale completion, a replacement slot, premature
success, and refusal/error promotion. Native tests must preserve existing
queue/drop/budget/error behavior and exercise those same callers.

An enabled zero-function or empty payload case cannot replace the nonempty
constructive witnesses. Changes to the canonical source closure, private
records and actual callers require the ordinary closed capture, full-root
stock positives, all inherited/new controls and native gates. No counts are
predicted or frozen before successful full discovery.

Even that candidate-fidelity result would leave admission before pending
memory, public PUT truth, arbitrary initial disk, restart recovery, protected
origin authentication, shared/bundled writers, physical persistence and
solver-origin correspondence open. Neither Q40 architecture is selected by
this plan.
