# Preparation-only live namespace service

Status: unselected file-only correction. Ten reduced helper and actual pinned
SourceMap API controls pass; complete-source compilation/strict checks, ordinary
fresh vstd, sealed runtime and all original fourteen qualification stages remain
pending. The original late-handoff migration failure stays recorded in
[qualification-failure.json](../deferred-import-locator/late-handoff/qualification-failure.json).
No fixture, oracle, compiler flag, thread count or deadline is changed.

The failed new actor verified two functions with empty stderr, but stored no
preparation: stock parallel workers deliberately pass `None` for the diagnostic
SourceMap; the new namespace guard required `Some`. This positive-fill failure
was not an accepted old-domain collision. The full and narrow patch plus eight
source identities are bound by [identity.json](identity.json).

## Compiler-thread service

The worker's diagnostic SourceMap stays `None`. A separate preparation queue
parameter flows only to lookup/storage. After acquiring the existing global
PREPARATION_GATE, the worker sends one boxed namespace request on the original
reporter queue. The compiler thread borrows the real `map.files()` guard and
publishes Ready. The worker then performs its existing bindings/codec/graph/
provenance/effects logic; only used Imported two-word identities travel through
a single fixed command/reply slot. The compiler thread invokes the unchanged
`validate_binding_files` against the same immutable Sources owner.

The file guard remains held through decode/effect restoration or serialized cache
publication. Worker disposal disconnects the service after the operation. No
SourceMap, SourceFile, raw span, TyCtxt, SST, diagnostic or variable-sized namespace
snapshot crosses the bridge. There is no new loader, resolver mutation, role
reclassification or authority substitute. The constructor seed, one late handoff,
exact current span bindings and every existing role/owner negative remain intact.
The new explicit v9 domain rejects intact v8 identities as an additional control.

Stock queued diagnostic messages remain in their original FIFO queue while this
pure service runs. The service does not dequeue nested messages, render or sort
diagnostics, call `unpack_span`, load/register source files or change worker
count/order. It resumes the existing receiver loop after the guard drops. The
receiver remains live until every worker's existing Done, which follows its
preparation operation. Default rustc SourceMap is not standard Send: no FromDyn
token is forged, no compiler dynamic mode is changed and no source owner is
transported into stock `std::thread::spawn` workers.

## Ownership and failure

PREPARATION_GATE admits one active lease. A separate 16 KiB active fixed-control
reservation precedes allocation of one Arc state, its mutex/condition variable,
and one boxed reporter request. Command and reply are fixed inline Options;
there are no strings, buffers or growable queues in this bridge. The box keeps
the existing reporter enum's largest payload unchanged. The Source owner Arc
shares its already admitted contents. Shared fixed fields dispose before its quota owner releases, but the final
Arc allocation and boxed reporter request can still be retiring when a new lease
is admitted. A permanent 64 KiB reservation covers that overlap and the new
slot's marginal block allocation in the existing reporter mpsc queue. It is
reserved before first allocation and not refunded during the process lifetime;
ordinary queued diagnostics retain their existing compiler ownership scope.
Only one prep request can be queued/served; the main receiver cannot serve the
next while retiring the previous request. Worker-last disposal precedes gate
release. Thus at most two Shared/box shells overlap. A full-source layout control
bounds those shells plus pinned native initialization candidates by 4 KiB and
three whole mpsc blocks plus that margin by 64 KiB. The full-source assertion
has not yet compiled, so the total physical proof remains pending that gate.

The exact pinned std source [audit](platform-allocation-audit.json) records source
URLs and hashes: Linux uses inline futex state; Darwin OnceBox allocates fixed
pthread objects (at most two racing candidates for each object from two peers).
The actual pinned std-libc type control verifies each native type fits its
256-byte ceiling, using the exact libc dependency recorded by pinned std metadata.
The existing reporter list uses 31 slots plus one next pointer per block; the
three-block reserve includes a current/retiring block and one sender boundary
allocation. No arbitrary number of ordinary diagnostic blocks is relabeled as
preparation retention. The proposed phase maximum is 500 MiB plus 80 KiB, below
the unchanged 512 MiB aggregate, with independent review and complete-source
layout proof still required. This is not process RSS or runtime qualification.

Mutex poison, dropped service or failed queue submission refuses preparation.
Worker cancellation/panic disconnects the service; validator unwind drops its
request and wakes a waiting worker. Drop marks disconnection and notifies; it
never waits for an acknowledgement. No peer holds the state mutex while running
validation or the worker's preparation operation. Other workers may wait on the
existing preparation gate, but cannot enqueue a second active lease. Full proofs
continue on the original miss/fallback path.

## Executed controls and pending scope

[api-controls.json](api-controls.json) retains exact source/raw hashes and 60-second
compile/30-second test limits. Controls run serial independent cases, preserving
their actual worker threads. They cover default rustc dynamic mode refusing a
Send bridge; real compiler-thread SourceMap guard/service and deferred queued
source mutation; exact positive/negative words; cancellation before Ready;
dropped service; validator/worker unwind; last-owner budget refusal; and 1,000
requests through one fixed slot. Additional controls check inline/native sizes
and pause after active quota refund while the old Arc allocation is still live,
then admit a new pair under the permanent retiring reserve. Nine helper controls are appended as source-only
unit tests (the separate native libc ceiling control stays in the API probe) in the actual producer module. The reduced API probe does not compile
the Sources adapter or prove complete Reporter stderr equivalence.

```sh
python3 engine/verus/candidates/preparation-namespace/test-api.py \
  --work-dir PRIVATE_NEW_OUTPUT
```

All original static/domain/fixture/checkpoint/cancel/checked-hit/cross-edit/raw
stock diagnostic controls remain required under their original 600-second
compiler bounds. User-facing proof repair remains experimental and this runtime
has not been built or selected.

The [retained scopes](retained-control-scopes.json) keep the first seven-control
result and subsequent sizing controls separately. The test-only paused-drop hook
initially failed E0603 in an external test module; importing the byte-exact source
module including its cfg(test) tests corrected the harness without widening
visibility. Native-size setup initially had ambiguous libc candidates and a
metadata-only rlib; final selection follows the pinned std metadata dependency
identity and binds its full metadata hash. No production guard or assertion was
changed to repair those setup failures.

The complete-source controller is `run-source-controls.py`, bound in
`source-control-plan.json`. It reuses the closed private compiler target after
dual-container holder checks, validates the preceding actual-source result,
preserves the old source before applying the exact delta, and compares all three
prior runtime artifact/source/lock inventories before and after the checks.
It runs two Cargo jobs at ordinary priority and one independent test case at a
time; every test's internal concurrency remains unchanged. This is source
control compilation, not an ordinary runtime build or qualification run.
