# Namespace candidate: independent qualification failure audit

This is a read-only audit of the first unchanged revision in the original
qualification of candidate `b3b07d47d7542817`. The full candidate patch is
`955cd584c19f99a5dece3d1f69da49b55530457a6ace334d56baf5af724bcf5e`.
The ordinary runtime and original launcher review remain separate in
[the runtime review](m7-preparation-namespace-runtime-independent-review.md).
No compiler, proof, retry or diagnostic actor was launched for this audit.

The first eight qualification stages passed. Stage nine stopped at revision
zero of previous-revision-focus: **zero restored hits**, zero same-input oracle
checks and no focused restores. Stock and engine both verified nine functions
with zero errors and byte-identical 234-byte stderr. The positive reuse assertion
failed; there was no served unchecked hit, diagnostic-order disagreement or
accepted old-domain collision. Later revisions and remaining stages were not
attempted. The original failure and all deadlines remain unchanged.

The retained events contain three `prepared-unavailable` records with stage
`namespace` and one with stage `source-map`. The summary reports four misses and
10,201,751 bytes peak retained preparation ownership. The observed peak does not
prove which import authority failed.

## Exact refusal boundaries

In the frozen `rust_verify/src/sail_engine.rs`, `store_bucket` first serializes
SST, graph reads, effects and recorded messages while collecting their span
bindings. It calls the namespace validator on each collected binding **before**
calling `provenance`. Consequently, the existing optional import trace inside
`provenance` cannot report these earlier failures. A trace with no records would
not distinguish their causes.

`validate_used_namespace` can reject an unsupported raw owner or a unit span
whose data does not contain exactly two words before looking up an import.
Imported validation additionally rejects an unavailable mapping family,
unknown/ambiguous original range, invalid packed range, a frozen unavailable
role, the 4096-file bound, or a changed/ambiguous current namespace. In
`sail_owned_imports.rs`, `validate_binding_files` preserves the exact owner and
role: an OriginalDescriptor refuses any newly present Foreign raw-hash match;
Loaded/Deferred require the captured current owner, typed hash, name, ranges,
crate identity, unique content and non-overlapping range. The raw events do not
identify which of these checks failed.

The fourth event is a different boundary. `with_namespace` returns `Option<R>`,
where the store operation itself returns `Option<()>`. An inner namespace
refusal therefore produces `Some(None)`. The outer `admitted.is_none()` test
emits `source-map` only when namespace admission itself failed, such as absent
map/queue, failed control admission, queue disconnect or failed Ready handshake.
It does **not** relabel an inner namespace refusal. Parallel event interleaving
does not establish which bucket owns an adjacent event.

One concrete safe-miss possibility exists at that separate boundary:
`Lease::pair` admits only when its global active-owner counter is zero. A worker
can drop its lease and release `PREPARATION_GATE` while the compiler thread still
owns the final Request/Shared allocation. The next worker can then fail the
counter's compare-exchange. The queue/retirement reserve covers the allocation;
it does not make admission succeed. This source-level possibility is not an
attribution of the actual fourth event and cannot explain the three inner
namespace refusals.

## Smallest useful next evidence

A reviewed, opt-in, failure-only observation should retain the exact rejected
binding at the validator boundary, not move or relax that boundary. For imports,
the compiler-thread validator can expose its existing `CaptureFailure` and
packed words under the same live files guard. The worker can associate its
exact current span identity and bounded spelling; unsupported raw kinds need a
separate fixed classification. Broker admission must separately identify
control admission, queue failure or Ready failure. No loader, file clone, owner
reclassification, additional source-map pass or inferred state is needed.

The existing source controls test authority checks and provenance directly;
the broker controls test fixed-slot transport with a mock predicate. A useful
composite regression would collect actual prepared spans and run the exact
validator through both direct and broker paths, preserving all existing negative
checks. Such a control is proposed, not executed here. A fresh diagnostic run
would be separate evidence and would not replace this failed qualification.

Raw record hashes and the scope of this audit are retained in
[the audit JSON](m7-preparation-namespace-qualification-independent-audit.json).
