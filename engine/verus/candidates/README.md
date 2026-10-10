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
`qualifications/2421-linux-first-attempt.json`. A fixture-only correction added
two reachable spec declarations without changing the runtime, assertion or
deadline. The positive and all three genuine restored-hit negative controls
then passed; AIR confirms both declarations and the order fault changes actual
wire bytes. See `qualifications/2421-linux-hit-controls.json`. The resumed qualification later failed the unchanged HE actor deadline, as
recorded below. It is not selected by the installer or the public runtime pin.

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

The first actual `2421` component capture around that edit changed only
`global.datatype_graph_span_infos` (801,177 to 801,178 encoded bytes). The
remaining 30 baseline components compared byte for byte. Both bounded focused
runs verified nine functions without errors. This vector contains spans from
the complete call-graph construction as well as datatype checks; its sole
ordinary reader is the fresh, pre-lowering trait check. See
`qualifications/2421-first-input-delta.json`. The evidence identifies an
overbroad preparation dependency; it does not change the key or establish
cross-edit reuse. A private, routed accessor with unsupported-read misses and
new exact oracle qualification would be required before narrowing it.

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

`qualify-same-input.py` runs all source stages against a sealed ordinary build.
Its optional `--hit-controls OUTPUT` resumes from an explicitly completed
control tree only after checking the exact verifier and solver identities,
every completed case and each required positive/negative comparison. It copies
that tree's raw files into the new pipeline. The original failed pipeline stays
unchanged. A passing source pipeline still does not constitute real-project
product or performance qualification.

The resumed `2421` source pipeline passed the eleven stock fixture controls,
graph/checkpoint controls, cancellation and exact first-input capture. Its full
HE unchanged-revision run then reached the unchanged 600-second actor deadline.
All 106 restored-hit shadow comparisons completed with equal actual ordered
wire bytes, effects, diagnostics and per-bucket solver outcomes, but the separate
authoritative primary was still solving `human_eval_129::min_path`. Its sent
input is an exact prefix of the completed stock input, ending at a 600,000,000
rlimit `check-sat`; no response or full compiler seal was available. These
shadow results do not establish a completed authoritative proof. The body edit
and final Linux library stages were not reached. Raw streams and deadline
metadata are retained; see `qualifications/2421-linux-resumed-failure.json`.
The candidate remains unqualified and unselected. Narrowing the measured unused
span-vector dependency does not establish a remedy for this solver timeout.

## Private span-seed dependency candidate

`private-span-boundary.patch` is a separate, unselected development candidate
based on `2421`. It removes only the unused whole
`GlobalCtx.datatype_graph_span_infos` vector from the preparation baseline.
The field is private. Its only ordinary reader, `recursive_types::check_traits`,
still runs against the fresh complete graph and vector before bucket lowering.
The accessor invalidates an active preparation capture. Current callback span
registration uses that same accessor before capture, so current raw-span owners,
used-span bindings and provenance remain exact. The interpreter clone factory
and diagnostic oracle fork invalidate active parent capture and retain a small
bounded read-epoch lineage when creating independent capture owners. This
lineage catches routed span or graph reads through a clone created before its
parent begins lowering, even if an intermediate owner has already been dropped.
It retains epochs only, with no ancestor ledger or mutable compiler effects.
Sibling owners remain independent. Unknown allocation, depth above 64 or epoch
overflow disables admission. An unexpected future routed read therefore costs a
miss; this does not claim coverage of arbitrary unguarded compiler data.

The new dependency domain is
`sail-prepared/graph-ledger/private-span-seed/v4`. The portable representation
remains version 3; its data representation has not changed. All other 30 baseline
components, pruned IR, ordered graph answers/function schedule, options and
mutable effects remain bound. `audit-private-span-boundary.py` checks privacy,
the closed raw-read inventory, both clone-escape guards and fresh trait-check
ordering. The initial 12-test prototype record remains in
`qualifications/private-span-host.json`. The epoch-lineage revision passed all
17 host release VIR regressions, including pre-created clones, destroyed
intermediates, sibling isolation, admission refusal, depth/epoch overflow and
last-owner physical quota release; see
`qualifications/private-span-epoch-host.json`. This is library evidence, not a
built or qualified runtime. Tiny lineage nodes share the existing 16 MiB graph
allocation quota, and their nested ancestry is physically dropped before charge
is released. The quota regressions run serially because they inspect that
process-wide counter.

After combining the independently reviewed decoded-allocation correction and
ordinarily building a separately sealed runtime, run:

```sh
python3 engine/verus/candidates/test-private-span-cross-edit.py \
  --engine VARIANT/verus --stock STOCK/verus --z3 PINNED/z3 \
  --project HE --patch CANDIDATE.patch --out OUTPUT
```

The additive control checks a complete module-013 proof against stock before
and after an actual module-023 body edit. It requires a persistent restore in
**FOCUS**, before the invocation can write any full-phase preparation, with
previous-revision files retained. Every real hit must pass the same-input raw
wire/effect/diagnostic oracle. This scoped control cannot replace complete
HE/Vest qualification, mandatory stock checks or the two whole-loop performance
gates. The retained `2421` primary timeout is not assumed solved by this seam.
A combined runtime build is deliberately pending the decoded preparation
allocation fix; no aggregate 512 MiB claim is made from these seam tests.

The combined-candidate source pipeline accepts `--patch CANDIDATE.patch
--private-span-boundary --old-engine OLD2421/verus`. It validates the exact
sealed patch identity, runs genuine old-domain migration/misfile controls and
the prior-revision focused body-edit control before complete HE verification.
Complete HE also requires a focused persistent restore after the edit. The
migration control reuses identical source and caller context across actual old
and new runtimes, preserves old entries, checks new keys, then misfiles an
intact correctly checksummed old payload under new keys and requires a fresh
proof. These new controls remain unexecuted until the combined ordinarily
verified runtime is built; Python parsing alone is not qualification.

For the next combined qualification, `qualify-same-input.py` enables bounded
read-only Linux observations on the real HE actors. The sidecar records the
owned process group’s start time, argv, CPU ticks, I/O counters, wait channels,
thread state and actual solver executable hash. It never reads process
environments or unrelated groups, and it does not alter solver priority, inputs
or the unchanged 600-second deadline. Samples have an 8 MiB disk limit; process,
thread, argv and solver-identity inventories are separately bounded and report
truncation. Unavailable observations cannot authorize a proof or turn a timeout
into a pass. Non-Linux execution records that `/proc` is unavailable.

Observation threads are prepared before launching their actor. All sidecar and
`/proc` I/O stays on the daemon thread; binding an actor performs no file I/O.
The existing actor deadline is anchored immediately before `Popen`, and
`communicate` receives only its remaining time. The sampler pins the actual
leader’s parent, process group and start tick, rechecks leader identity before
payload reads and sample publication, and uses fd-relative process payloads so
numeric PID reuse cannot redirect reads. Leader exit, disappearance or identity
change stops sampling. Optional diagnostic shutdown cannot prevent killing and
draining the real actor at its deadline.

## Combined bounded preparation candidate

`bounded-private-span.patch` combines the private span-seed read boundary and
its epoch lineage with the independently reviewed decoded-allocation, focused
report and source-observer bounds. It is a complete patch for the pinned source,
not an overlay to apply to `interactive.patch`. The selected `b346` runtime and
the immutable failed `2421` qualification remain unchanged.

The guarded serde decoder admits allocations before visiting each nested value,
owned string, sequence or map, and suppresses untrusted collection size hints.
It covers the complete portable SST/effects/bindings/diagnostics schema, not just
the outer file size. Graph frames retain their independently bounded decoder;
each supported replay query admits conservative scratch before executing the
original graph algorithm. Replayed diagnostics keep their admitted owner through
oracle comparison and replay, and physically drop their vectors and nested Arc
references before releasing quota. Unsupported schemas, depth, malformed input
or insufficient admission cause a preparation miss.

Focused reports serialize borrowed typed diagnostics directly into a bounded
writer after focus workers have joined. Source observations likewise serialize
borrowed fields only under the single emitter lock. The observation writer,
name formatting, progress owners and ordinal table have separate bounds; an
oversized observation publishes an unavailable record rather than a partial
JSON frame. The original full diagnostic text and JSON graph remain unchanged.
Nested diagnostic object-member ordering may differ from the former temporary
JSON value map; ordered arrays and diagnostic contents remain exact.

The decoder/focus allocation argument is documented in
`decode-budget/README.md`. The observer adds a separately bounded 8 MiB allowance
to those phase maxima, yielding a conservative largest preparation-phase bound
of 492 MiB. This is an audited allocation argument, not a measured RSS or runtime
qualification claim. Compiler working IR after a validated handoff and the
explicit diagnostic-oracle arms remain separately reported working memory;
replay bindings, diagnostics and validation scratch cannot be excluded as
compiler working IR before that handoff.

The combined source passed all 26 host release VIR tests, strict all-target
VIR clippy and the private graph/span boundary audits. The first full RustVerify library compile failed before tests with 132 repeated
macro-resolution errors: the helper tests had placed `sail_engine` at crate root,
but the real source nests it under `verifier`. That failed source identity and
exact diagnostics are retained in
`qualifications/b6da-bounded-private-span-first-compile.json`.
The candidate now uses the actual nested macro path, independently checked with
a matching helper layout, and renames the unused parameter without suppressing
its lint. The corrected complete source identity is recorded in
`qualifications/bounded-private-span-host.json`. The corrected source compiled
and ran all 22 actual driver library tests: 21 passed, while the observer byte
comparison failed. Verus enables serde_json's `preserve_order` feature; the
isolated helper omitted it and masked the typed writer's sorted-field change.
The actual assertion and exact output are retained in
`qualifications/d3cb-bounded-private-span-observer-order-failure.json`.
The candidate now preserves the original field insertion order and validates
duplicate fields before serialization. The unchanged actual driver tests pass
22/22 with the pinned JSON features, including the real MessageX graph, focused
JSON, reporter/replay disposal races and raw-wire oracle controls. Strict actual
all-target driver clippy also passes after equivalent lint fixes. These host
checks do not qualify a runtime or establish previous-revision proof reuse.
An allocation-only probe of the 105 retained, checksummed actual HE portable
preparations admitted 36 and refused 69 at the 128 MiB decode budget. Admitted
entries also decoded with explicit key-matched placeholder raw-span owners;
this does not establish live provenance, graph/domain validity or a proof hit.
The majority refusal is a product limitation, not a passing performance gate.
See `qualifications/bounded-decode-retained-he.json`; raw entries and probe
output remain preserved. The unchanged-guard per-type charge profile is in
`decode-budget/primitive-charge-profile.json`: duplicate primitive dispatch
charges account for about 6.5 MiB at refusal, so removing that duplication alone
cannot be assumed to admit the large real preparations. Allocation precision is
under review before an ordinary combined runtime build, without increasing the
allowance or weakening admission checks.
An ordinary verified Linux build, genuine old-domain rejection,
previous-revision focused restore, complete source oracles and both real-project
performance gates remain pending. It is unselected and unqualified.

The inactive `2421` compiler `deps`, `build` and `.fingerprint` directories were
removed after checking process references. All 15 sealed direct runtime artifact
hashes matched before and after; source, runtime, failure records and raw
qualification data remain. No intermediate seed or qualification receipt was
copied. The exact resource-only operation is recorded in
`qualifications/2421-intermediate-cleanup.json`. It does not requalify the runtime.
