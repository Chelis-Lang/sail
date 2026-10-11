# Bounded span-provenance diagnostic

This [separate source patch](../provenance-diagnostic.patch) diagnoses the
[failed scoped-query cross-edit gate](scoped-query-qualification-failure.json).
It does not change span equivalence, keys, preparation admission, restoration,
proof outcomes or any qualification assertion. The selected runtime remains
unchanged. [The exact identity record](span-provenance-diagnostic.json) marks
actual build/tests and mismatch capture as pending.

The failing invocation edits task023 while the selected task013 function remains
unchanged. Its persisted FOCUS key passes decoding and graph validation but fails
the exact span-provenance comparison. Local provenance currently binds safe raw
fields and its complete containing source owner; imported provenance binds the
encoded crate/range pair and every foreign owner. The failure stage alone does
not identify which field changed, or authorize dropping a dependency.

`SAIL_ENGINE_PROVENANCE_TRACE=1` is a private diagnostic control in this isolated
variant. Only FOCUS emits records. Store records carry the exact encoded span
identity, provenance digest, safe raw field values and contributing source owner
fields. The first failed comparison records the same identity with expected and
actual digests and current owners. Local raw fields retain `lo`, `hi`, syntax
context and parent. Imported records are explicitly labelled. `SpanData` Debug
is never invoked because its formatter can call compiler TLS on worker threads.

Serialization borrows already-admitted owners and runs under the existing
observer mutex and 1 MiB writer bound. It does not build a `Value`, clone owner
tables or allocate a foreign-owner list. Store tracing has a separate limit of
3,072 records and 3 MiB; mismatch tracing reserves 1,024 records and 1 MiB.
Each byte allowance leaves 512 bytes for its single incomplete marker. Exhausting
store tracing cannot consume or disable mismatch tracing. An arm stops on its
first cap/serialization failure and emits only a bounded unavailable control;
partial JSON is never published. These controls affect observation only.

New actual-type controls check Local/imported owner fields and a borrowed 2 MiB
owner-name overflow that publishes only unavailable, while its original
provenance and owned source remain unchanged. The source build and those tests
must complete before the trace is used. The original cross-edit assertions and
600-second compiler limits remain unchanged in the separately labelled
instrumented reproduction. No mismatch trace or successful unit test establishes
reuse correctness or product qualification.
