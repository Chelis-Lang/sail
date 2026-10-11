# Qualification trace admission review

This review preceded the original qualification launch. Its frozen cross-edit script
constructs both compiler environments by removing every inherited
`SAIL_ENGINE*` key (`test-private-span-cross-edit.py:48`), then supplies its fixed
engine settings at lines 51–55. It does not forward `SAIL_ENGINE_IMPORT_TRACE`
or `SAIL_ENGINE_IMPORT_TRACE_TARGET`. The prepared controller's inherited
settings therefore could not provide actual selected-state evidence. They were
removed before launch, with the unexecuted controller hash retained in
[the handoff record](qualification-handoff.json). The fixture source files and
compiler environment remain original. No compiler diagnostic or qualification
result is attributed to this review.

The dormant implementation is in patched Verus
`rust_verify/src/sail_engine.rs`, source SHA
`f3857c37552423e024d3779bee9e1e45c50a569ea9537518c1205a4801163eda`.
The `provenance` call at line 321 invokes a unit-returning observer before its
existing memo/provenance checks. The trace gates on focus and exact opt-in `1`
at line 403; its known display filter at lines 395–408 precedes admission and
accepts only the exact selected spelling. Unknown tokens skip observation.
None of these settings appears in the preparation key, restoration decision,
source authority or stock resolver.

`trace_import_authority` at lines 402–495 borrows the admitted owner and its
fields. `record`, `authority`, `deferred` and `current_range` are read-only
methods on `OwnedImportMappings` (patched
`rust_verify/src/sail_owned_imports.rs:454–520`), with no loader call, resolver
mutation or owner promotion. The observer admits one serialized writer under
`EMITTER` before expansion. Each complete record is at most 32 KiB; the trace
stops after 4,096 records or 4 MiB minus a reserved 512-byte unavailable marker.
Failure drops the partial serialization and emits only a bounded unavailable
record. The existing observer allowance and single mapping owner remain part
of the unchanged physical preparation accounting; published byte count is not
an additional retained buffer.

Publication opens only the fixture-owned `SAIL_ENGINE_EVENTS` file and appends
that record. The trace never writes or rewrites captured stdout/stderr and
never supplies a verification result. This is a structural side-channel
statement: serialization, mutex acquisition and file I/O still consume time
and can change scheduling. It does not promise deterministic concurrent NOTE
arrival order. Any future separately authorized state diagnostic must retain
that scope and the original strict raw-diagnostic comparisons; it cannot be
counted as original qualification merely because the trace emitted.
