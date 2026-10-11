# Original-descriptor-only delayed role: proposed boundary

Status: reviewed design; a file-only implementation/control draft is pending
independent review. No helper execution, proof, build or candidate activation
follows from this document. The
[original qualification failure](qualification-failure.json) and the
[separate bounded diagnostic](provenance-diagnostic-result.json) remain distinct.
The selected runtime is unchanged.

## Actual evidence and remaining uncertainty

The separate diagnostic used exact revision 0 compiler input, sealed runtime
and original flags/context. Its five eligible Loaded imports precede imported
binding 5705, `vstd/function.rs:138:2: 138:2 (#0)`, original range
406973–411571, expected raw hash `966a15d4a7885d0f3391df29d6d31a1d`. This record
is unavailable with Delayed at both constructor and pre-focus, immediately
before the serialized focus store returns for provenance refusal. This excludes
a whole-family capture error for that observation. It does not establish a
particular descriptor path or Foreign-match count: the unavailable trace does
not retain them. The original fourteen-stage qualification is still failed at
zero hits in revision 0, without cross-edit evidence.

The exact pinned `rust_verify/src/spans.rs:182–197` creates Delayed descriptors
for `remaining_crate_files` absent from the SourceMap Foreign iteration. It uses
the original metadata's filename and expected raw hash, with the original
normalized range. Thus a legal stock delayed recipe need not have any current
Foreign metadata owner. This is a static code fact, not proof that binding 5705
took that particular constructor arm.

At `spans.rs:257–275`, current stock reporting can call `SourceMap::load_file`,
check its actual hash bytes and replace its own Delayed state with Loaded or
None. `unpack_span` then translates the original coordinates through that
current resolver. Preparation must neither perform this work nor attest its
future success. The existing Foreign-associated DeferredLocator deliberately
requires metadata already present in the current SourceMap; it cannot represent
the legal remaining-original recipe above.

## Proposed distinct role and conditions

Keep LoadedForeign and the existing Foreign-associated DeferredLocator intact.
Add a separate proposed **OriginalDescriptorLocator**, with no LoadedFile,
SourceFileHash-kind, StableSourceFileId, current name/index/range or source text.
It owns only the already-bounded actual descriptor path; its enclosing Record
retains original stable crate/range and exact raw expected hash bytes.

Capture requires all of the following, before freezing any worker snapshot:

1. Actual current resolver state is Delayed and its original range is selected
   uniquely within the original stable crate. Original range/hash table
   uniqueness, overlap, overflow and family admission checks remain unchanged.
2. The descriptor hash equals the exact original metadata hash bytes. The
   original metadata filename exists and equals the actual Delayed filename
   byte for byte. This ties the role to the original remaining-file recipe;
   neither display text nor a guessed remapping supplies authority.
3. That exact path is absolute, valid UTF-8, NUL-free and at most 4096 bytes.
   Admission precedes copying or formatting. No canonicalization, symlink
   resolution, file access or loader invocation occurs.
4. There are **zero** current Foreign files with the expected raw hash bytes,
   across **all crates**. This intentionally stricter condition avoids inventing
   a current CrateNum association when no Foreign owner exists. One supported
   same-crate Foreign match continues to use the existing metadata-associated
   role; unrelated equal-hash Foreign files cause a conservative new-role miss,
   and existing duplicate/ambiguous checks remain. No filename-derived crate
   identity or extra stable-crate inventory is introduced. Missing Foreign
   identity never creates a Loaded claim.
5. Retain the exact immutable original metadata range/hash/filename once at
   constructor capture in the admitted family. Immediately before Sources
   handoff, reread only the mutable current descriptor and Foreign namespace on
   the compiler thread: compare its Delayed state/path/hash against that frozen
   original authority and require the same zero-match condition. The original
   metadata table is no longer borrowed and is not reread at handoff. Any new Foreign association or early Loaded, None,
   Busy/poison or descriptor change refuses this role. There is no promotion,
   reconstruction of discarded authority or second snapshot.

The zero-match condition is a current namespace check, not a content assertion.
The all-crate zero-hash test is performed at both barriers using actual borrowed
Foreign entries. It requires no compiler-derived CrateNum for the absent owner;
matching hash kinds/lengths are not inferred. A current local source file does
not become imported Foreign authority by
matching its name or bytes. After the immutable handoff, ordinary current
reporting may naturally create or use a local diagnostic file; the immutable
role remains OriginalDescriptorLocator. It never writes back into SpanContext.

These requirements remain conservative when actual callback conditions are
unsupported. A source lacking an exact original filename, a changed remapping,
relative/non-UTF8/oversized path, ambiguous original metadata or a changed
handoff namespace is still a miss. The actual sealed-vstd metadata/control now establishes a supported original
recipe for this selected hash/range in a controlled empty Foreign namespace;
its absolute filename is retained by SHA. It does not establish the historical
5705 callback namespace, nor full fresh-store admission. See the
[implementation and controls](../original-descriptor/README.md).

## Current binding, consumers and domain

The role is limited to the same reviewed optional interactive ghost callback.
The [closed consumer audit](../../decode-ownership/deferred-locator-consumer-audit.md)
still applies: lowering propagates semantic VIR/SST span identity, and current
Reporter/SpanContext alone loads, hash-checks, translates and emits diagnostics.
The recipe does not attest a FileLoader algorithm, live bytes or successful
resolution; original hash bytes have no stored kind tag and none is inferred.
The invocation's existing runtime/input identity and observation contract
remain independent requirements for any public Current proof.

Prepared decoding still rebinds the exact current `(id,data,as_string)` entry to
its current raw `Arc<()>`. Require precisely two packed original words and the
unique checked original span range; unknown raw owners, malformed words and
Local lo/hi/hygiene/parent retain their current policies. No old raw span,
rendered snippet, SourceMap coordinate or resolver mutation is restored.

A new proposed domain, `sail-prepared/graph-ledger/import-locator-authority/v7`,
and a distinct OriginalDescriptorLocator provenance tag bind that role,
original words/stable crate/range/hash and exact absolute descriptor. Existing
LoadedForeign and Foreign-associated DeferredLocator identities remain exact.
No solver, wire, semantic-key, graph-ledger, effect or diagnostic normalization
is proposed. Old v6 and earlier intact/misfiled bodies must be rejected; no
artifact relabeling or resealing. Portable version 3 may remain only if the
representation itself stays unchanged.

## Physical ownership and required controls

Use the same single process-wide 8 MiB mapping family. A new role must fit the
existing maximum enum/record-size admission or its new actual `size_of` must be
charged before reservation. Bound path capacity and growth overlap before
allocation. No source text, imported compiler metadata clone, current SourceMap
Arc, mutable descriptor Arc or extra seed family is retained. The same admitted
owner moves constructor → handoff → Sources; physical disposal precedes quota
release on every failure/drop. The prospective phase maximum remains 500 MiB
within 512 MiB; it is not certified by this design or the approximately 10 MiB
observed driver peak.

Required actual pinned controls before any ordinary build:

- A real Delayed descriptor from original metadata with an empty Foreign
  namespace captures without file I/O; its role has no current-range method.
- Exact current raw binding plus scalar Reporter replay yields current stock
  primary/label fields for successful lazy load and wrong-hash/missing-file
  failure; repeat rendering uses the same current resolver. Complete formatted
  stderr and queued/group ordering remain original-pipeline obligations.
- Exact-original filename/hash/range changes, missing filename, unsupported
  path encodings/length, malformed words, unknown raw owners and early
  Loaded/None/Busy or Foreign-namespace change all refuse.
- Existing LoadedForeign and metadata-associated DeferredLocator wrong
  crate/kind/hash/name/StableID/range/duplicate-owner and handoff negatives remain
  unchanged. A missing association cannot be relabeled into the new role at
  revalidation. Composite unrelated-invalid-record controls distinguish global
  family failure from a per-record unsupported recipe.
- Concurrent capture refusal, reserve overflow and physical drop-before-release
  prove one-family ownership; serialization/refusal observer remains bounded.
- New-domain rejection and current-binding codec/provenance controls run in the
  actual source, followed by all library/strict gates, fresh ordinary vstd and
  exact runtime seals, then the original fourteen stages and 600-second bounds.
  Genuine previous-revision FOCUS and strict stock diagnostics remain required.

A separate role changes span equivalence and therefore requires the proposed
[ADR 0033 amendment](../../../../../docs/decisions/0033-deferred-import-diagnostic-locators.md)
and independent review. It is not deletion of the existing Foreign requirement
and does not repair the retained qualification result by interpretation.
