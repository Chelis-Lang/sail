# Candidate write fidelity

Status: source plan and an unrun stock-library support probe for
[#372](https://github.com/Chelis-Lang/sail/issues/372). No production writer,
cache policy, transport, dependency edge or origin admission changes are selected.
The [cache-origin architectures](cache-origin-admission.md) remain an owner
choice. A `CandidateWrite` would preserve a candidate, not certify that a solver
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

The exact generic signatures and final placement remain subject to source
review after the primitive support check. No code here changes the current
production transition. Existing same-slot native tests remain required.

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
This source inspection does not establish that every compiler-internal path
lacks a usable model. A direct full-file probe must resolve actual support.

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

If this prerequisite fails, stop the identity-dependent production proof at
that precise missing contract. Keep the actual native identity check intact.
Do not replace it with the already conditional `OwnerIdentity` trait. The
actual map lookup with the native key, mutable slot access, locking and
bookkeeping also require support checks before claiming full writer coverage.

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
