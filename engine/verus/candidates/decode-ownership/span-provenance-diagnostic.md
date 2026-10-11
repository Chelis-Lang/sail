# Bounded span-provenance diagnostic

This [separate source patch](../provenance-diagnostic.patch) diagnoses the
[failed scoped-query cross-edit gate](scoped-query-qualification-failure.json).
It does not change span equivalence, keys, preparation admission, restoration,
proof outcomes or any qualification assertion. The selected runtime remains
unchanged. [The exact identity record](span-provenance-diagnostic.json) froze
the pre-build state; subsequent records below retain the actual results.

The subsequent [ordinary Linux build record](span-provenance-diagnostic-build.json)
closes the build stage: fresh vstd verification passed 2,060/0, and all 13 sealed
runtime artifacts match the recorded manifest. The emulated build took 2,279.6
seconds at ordinary priority with two build jobs. Copied inactive compiler
intermediates excluded the changed VIR/driver crates; no runtime artifacts or
proof receipts were copied. Actual trace tests and mismatch capture remain
separate: both new actual driver trace controls passed in 51.3 seconds, with
all 13 sealed artifact hashes unchanged. This diagnostic runtime is neither
selected nor qualified; the later causal collection is recorded separately.

The [causal diagnostic record](span-provenance-causal-diagnostic.json) retains
two separate results. The unchanged strict replay failed at revision zero:
the first two module/function progress notes arrived in a different order from
stock. Both arms verified 9/0, but the exact stderr assertion remains failed.
This is a progress-note discrepancy, not an established VI0001 SMT-order cause.
Two subsequent fixed-count stock controls showed identical note order; they do
not establish stock nondeterminism or erase the original difference.

An independent controller then ran the original stock/engine commands and all
three original revision contexts, each with its unchanged 600-second limit.
It did not edit, bypass or continue through assertions in the original test.
Every arm verified 9/0 with identical stderr in this separate diagnostic. The
unchanged-input revision restored prior FOCUS preparation; the task023 edit
again failed to restore it. All actual restored HIT oracles passed, but the
collection is not qualification.

The first failed provenance comparison is an imported span from
`vstd/arithmetic/div_mod.rs:519:50–56`. All 454 contributing foreign owners
change only their session-global `start`, uniformly by 25 bytes; their index,
name, length, hash and foreign flag remain exact. The store trace reached its
cap before this identity. Eleven captured imported stores nevertheless share
one complete prior owner inventory. Independently encoding the original legacy
bincode tag/data and owner tuples reproduces every captured store digest and
both the mismatch's expected and actual digests. Thus the owner-field delta is
verified without treating a missing trace record as complete.

This evidence identifies coordinate relocation in the current broad foreign
inventory check. It does not identify the selected span's original-to-current
mapping or its Loaded/Delayed state. Those require a separately bounded,
compiler-authoritative capture before any changed equivalence policy. Current
span identity and current raw-Arc rebinding remain exact; no provenance, key,
budget or qualification check has changed.

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
