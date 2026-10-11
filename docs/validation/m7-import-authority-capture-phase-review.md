# Single import-authority classification at source handoff

This is a read-only design review of the unselected OriginalDescriptorLocator
candidate, full patch `1ad773b66e1b58dac62936499e37d81179e1138565cc1774d4a79d168fe935a8`.
No implementation, compiler run or qualification retry accompanies this review.
The original fourteen-stage failure remains: eight stages pass, revision-zero
focused preparation then has zero hits and four provenance refusals, with exact
stock diagnostics and full 9/0 verdicts. B's separately retained diagnostic
observes an initially eligible OriginalDescriptor for imported `laws_eq` span
7867: Delayed at both barriers, but matching Foreign raw-hash count grows from
zero to one and late validation returns ChangedSource. The diagnostic record is
`engine/verus/candidates/deferred-import-locator/original-descriptor/provenance-diagnostic-result.json`.
The count alone does not establish that the new Foreign belongs to the right
crate, has an unambiguous typed hash/name/range, or meets the existing policy.

The current pinned call chain provides a later natural compiler-thread boundary.
In `rust_verify/src/verifier.rs`, `after_expansion` constructs SpanContext at
3259 before `construct_vir_crate`. That stock work performs HIR/type checks,
external-item queries and HIR-to-VIR conversion. Trait-conflict and lifetime
checks follow. Only afterward does line 3364 call `sail_focus`, which first takes
import mappings and builds SOURCES at `sail_engine.rs:813`. The observation
establishes growth between these boundaries, not which particular stock query
introduced the file. There is no preparation take/store before source handoff.

A single late classification is feasible without changing stock work. At the
constructor, retain an explicitly **unclassified** bounded seed of original
crate/range/hash authority and the exact initial actual resolver state/recipe.
At the existing compiler-thread handoff, validate that the resolver state and
path/hash still match that seed, then classify once using the current SourceMap
and the existing strict role rules. The callback still has its TyCtxt, so the
current Foreign CrateNum can be checked against the actual stable crate ID there
without retaining TyCtxt. A now-present Foreign may select the existing
Foreign-associated Deferred role only if every existing crate/hash/range/type/
uniqueness check passes. A cross-crate equal raw hash continues to refuse.
This does not promote a discarded OriginalDescriptor state: no role existed
before that single classification.

Retaining only original metadata would be insufficient. The seed must preserve
the initial actual descriptor so changed path/hash, Loaded/None transitions,
Busy and other existing mutation negatives do not become newly accepted. An
unbounded original HashMap, SourceMap, SourceFile Arc, old raw span or compiler
context must not escape into it. Do not rerun SpanContext construction: that
would rebuild stock resolver tables and reset span IDs. Do not retain two owners
or increase the 8 MiB family. Admit the complete fixed seed/final-record shape
before reserve and bounded path growth; move or convert records within that
same owner, accounting for any temporary overlap and disposal before release.
Frontend failure must also drop the seed and retain its admission-time peak.
The existing prospective 500 MiB maximum remains a limit to prove, not a new
qualified allocation claim.

The current exact raw-binding path is separate from that timing change.
`current_bindings` at `sail_engine.rs:524` serializes the current pruned Krate and
Ctx span seeds under the existing collection guard. Prepared Span decoding
requires the complete current `(id,data,as_string)` key and clones its CURRENT
raw Arc. No old coordinate or resolver is installed. Store captures actual SST,
effects and messages, and then validates each span's provenance. Restored
messages retain their original four emission methods and use the current
Reporter. A missing or ambiguous current binding remains a miss. Those codec,
graph/effect/key and replay boundaries should stay unchanged.

A late implementation must prevent a second unnoticed SourceMap change between
classification and SOURCES handoff: use the same current file snapshot/read
boundary where practical, or retain the existing strict late validation against
that handed-off snapshot. Do not infer that source growth stops forever.
Subsequent lazy Reporter loading remains stock execution; it must neither
reclassify the retained capsule nor restore an old resolver state.

Required new controls should distinguish timing from broadened equivalence:

- A legal same-crate Foreign introduced by ordinary work before the SINGLE
  classification may use the existing strict Deferred role; retain the earlier
  capture-then-growth ChangedSource negative unchanged as its own boundary test.
- Equal raw hash in another crate, duplicate Foreign owners, wrong typed hash,
  changed normalized range/name/StableSourceFileId and excessive namespaces
  continue to refuse under the existing role policy.
- Initial-to-handoff descriptor changes, Missing/Busy/None and incorrect original
  filename/hash/range refuse; no eager loading or role promotion occurs.
- Exact current raw-unit and local hygiene/parent negatives, codec current-Arc
  controls, all replay modes/primary-label behavior and old-domain migration
  refusal remain intact. A new domain must separate the timing policy.
- One-family allocation failure, full path-growth headroom, frontend-error
  cleanup, physical last-owner disposal and admission-time peak controls prove
  seed-to-mapping ownership before any ordinary build.

Original source/strict controls, fresh ordinary vstd, all runtime seals and the
unchanged fourteen-stage/600-second stock-diagnostic, same-input raw-wire and
previous-revision focused-hit gates remain required. This review does not prove
that the observed Foreign will be eligible, solve all zero-hit causes, select a
runtime or establish M7 speed.
