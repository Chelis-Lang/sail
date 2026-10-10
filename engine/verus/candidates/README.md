# Exact graph-read candidate

Candidate `2459` completed an ordinary Linux source build with 2,060 vstd
functions verified and passed its library, fixture, checkpoint and cancellation
controls. Its real HE qualification failed an independent-stock exact SMT
comparison, so it remains unqualified and unselected. Fixed declaration-order
controls preserve both fresh compiler orders and their exact cached/fresh output
pairs. The separate other-module edit matched all stock solver bytes, but
obtained no preparation reuse from the previous revision. See
`qualifications/2459-linux.json` and `qualifications/2459-order-controls.md`.
Neither candidate changes `interactive.patch`, the installer or a qualified
runtime. See ADR 0028.

Apply `graph-ledger.patch` to a separate checkout of the primary pinned Verus
commit. Do not apply it on top of `interactive.patch`: the candidate is a complete
patch including that baseline. Before building, run:

```sh
python3 engine/verus/candidates/audit-graph-ledger.py --source VARIANT/source
```

After building the candidate in a separate source variant, run the seven ledger tests:

```sh
cargo test --release -p vir --lib sail_graph::tests
```

Use the variant's pinned source toolchain and normal source-build environment.
Then run the existing source tests and the local-only exact oracle:

```sh
python3 engine/verus/candidates/test-graph-ledger.py \
  --engine VARIANT/verus --stock STOCK/verus --z3 PINNED/z3 --out OUTPUT
```

The oracle uses the same input paths and compares original full diagnostics,
function outcomes and solver-input bytes without normalization. It retains
unchanged-callback, body-edit, stale-span, corrupt/misfiled/oversized checkpoint
checks. Cancellation and real-project catalogue gates remain separately
mandatory. Neither the scripts' presence nor static coverage is a runtime pass.

`reporter-ownership.patch` is a narrow overlay for the currently qualified
baseline or the previous graph candidate. It disposes diagnostic message vectors
and all cache-owned nested Arc references before releasing their process-wide
quota. It is not an installer-selected patch. `graph-ledger.patch` already
includes the overlay; do not apply it a second time. Its blocked-destructor
regression exercises both recording and focus reporters, while a separate Arc
lifetime check preserves compiler-owned references.

The allocation-counter destructor audit covers graph Recording, Ledger and final
Capture ownership plus diagnostic RecordingReporter and FocusReporter. The only
other decrement is the preparation miss counter, which owns no allocation.
Span-collection guards restore callback-local state and have no quota to release;
serialized entry/provenance/source storage uses explicit global gates or direct
ownership accounting, rather than destructor counters. Compiler-owned message
clones are working state outside cache ownership; removing cache references does
not invalidate those clones.

## Same-input raw-byte oracle candidate

`graph-ledger-oracle.patch` is a complete, separate candidate patch. The owner
selected a byte comparison between restored and fresh lowering from the same
compiler input, retaining independent stock verdict and diagnostic checks and
reporting stock transcript instability. The historical `2459` qualification
failure remains preserved. This new candidate passed an ordinary Linux build
with 2,060 vstd functions verified and no errors, plus host library regressions.
Its first executable qualification passed a real restored-hit positive, then
failed the mandatory order-fault control: the tiny fixture had only one spec
declaration, so swapping a spec and an empty proof function did not change the
wire. The failed control and its raw streams are retained in
`qualifications/2421-linux-first-attempt.json`. Complete executable qualification
is pending. It is not selected by the installer or the public runtime pin.

The diagnostic oracle runs only when `SAIL_ENGINE_ORACLE_DIR` is set and a
preparation has passed the normal key, provenance and graph-ledger validation.
It constructs independent fresh and restored `Ctx::new` owners from the same
immutable current pruned IR. Name, graph capture, chosen-trigger, qid, log,
reporter, solver and verifier outcome owners are independent. It compares
post-lowering and final effects, grouped/immediate and deferred diagnostics,
typed function outcomes, and the ordered list of actual solver-writer byte
streams. Each arm invokes the explicit real pinned solver. Partial writes,
capture errors, nonzero solver termination, missing launch/closure or unknown
state fail the oracle. An empty session list is exact empty evidence, not a
solver proof. Shadow work cannot advance primary producer ordinals or cache
counters; it does not alter the authoritative primary lowering.

The oracle's actual-written streams, diagnostic files and summaries have a
256 MiB process-wide retained disk quota. Each diagnostic file is bounded by
16 MiB and each summary by 64 KiB. There are at most 4,096 session allocations
per arm. These diagnostic files and the two temporary compiler/verifier working
states are separate from the production preparation cache's memory budget;
this opt-in qualification mode is not a product performance path. Capsule
comparisons use the same global preparation serialization lock and bounded
encoders, and drop temporary buffers before unlocking. The production quota
physical-drop fixes from `graph-ledger.patch` remain present.

After an ordinary verified build of the distinct variant, run:

```sh
python3 engine/verus/candidates/test-same-callback.py \
  --engine VARIANT/verus --stock STOCK/verus --z3 PINNED/z3 --out OUTPUT
```

The fixed controls fill a preparation, prove a real restored-hit positive with
actual sessions, then inject ordered-function, spec-body and latent-effect
corruption into the independent restored arm **after successful validation**.
The primary cache contents remain unchanged. The first two faults must fail
actual wire equality; the last must fail complete effect equality. Positive
shadow work must preserve primary obligation identities/order and authoritative
stock verdicts and diagnostics. These executable controls, existing stock
fixture verdict/diagnostic checks, checkpoint/cancellation controls and both
real-project edit catalogues remain required before activation.

`SAIL_ENGINE_ORACLE_COMPONENTS` plus `SAIL_ENGINE_ORACLE_COMPONENT_MODULE`
optionally records exact baseline components in their original order before
lookup. It uses the existing typed encodings, a 64 MiB retained serialization
bound under the global gate and a separate 128 MiB process-wide disk bound.
These records diagnose the first changed input; they do not broaden any cache
equivalence or suppress a miss.

`test-key-delta.py --engine VARIANT/verus --z3 PINNED/z3 --project HE --out OUTPUT`
runs two bounded callbacks focused on module 013 around a module 023 body edit.
It preserves both source forms and directly compares each ordered component's
bytes before reporting the first change. It always restores its private source
copy. Its result is a miss-cause diagnostic, not conformance, useful cross-edit
reuse or a speed claim.

`test-real-he-same-input.py` is the newly selected real-source byte gate. It
retains the original stock exit, full/function verdict and exact diagnostic
checks, reports each independent stock transcript difference with both raw
streams and hashes, and requires a completed same-callback raw/effect/diagnostic
comparison for **every** actual preparation hit. It is separate from the earlier
`test-real-he.py` and its retained failure. Its unchanged-revision control must
restore an existing focused preparation; its other-module edit records actual
reuse without claiming previous-revision reuse from a disk-hit counter alone.
All actor deadlines remain 600 seconds, and a timed-out owned process group is
terminated with raw stdout/stderr and deadline metadata retained.
