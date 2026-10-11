# Independent NoOriginalRange draft review

This is a static review of B's file-only prototype, not a source-test, runtime,
reuse or performance result. The original b3b07 qualification remains failed:
eight stages passed, revision 0 had zero checked hits and four preparation
misses, and the remaining stages were unattempted. The separate boundary
diagnostic identified one `pair-busy` admission failure and three
`UnknownCrate` range-lookup refusals. Retirement acknowledgement addresses the
first mechanism; the proposed absence role addresses only a complete, known
immutable imported namespace that contains no matching low position.

The reviewed helper snapshot has SHA-256
`ff162c13db2200de08cc207f4f25cfb0b80b6ebd5f8c1b68be1dcaa988c77af9`;
the engine snapshot is
`edc9c71a05c2eee8cccad984050e23f02539ad846709c1e1e123ea648df7022e`.
These identify an evolving draft. A final export and its composition with the
acknowledgement overlay require a separate check.

The constructor's completeness argument is conservative. Every captured
imported row already requires one exact original hash/range match; duplicate
or overlapping inclusive ranges refuse. Equal per-crate counts therefore
establish a bijection with the actual borrowed original metadata table. The
digest streams the crate, record count, ordered ranges, hash length and bytes,
and optional original filename's exact OS bytes. Overlong names disable this
new authority; existing recorded-role checks remain intact. This proves
completeness of that actual table, not completeness of all source files that
ever contributed to the exporting compiler. Foreign state and arrival time
in the historical exporting callback remain unobserved.

The stock resolver first searches its immutable imported ranges using the
packed low position. An absent range returns `None` before a loader call.
The new lookup uses the same inclusive low-position predicate, requires a
known complete namespace and ordered low/high words, and never turns an
arbitrary `record()` error into success. The original lookup API still refuses.
Private imported ranges cannot be extended after construction; ordinary
resolver transitions mutate only a row's `info`. Current SourceMap growth
cannot create a new imported-range row. A covered but unavailable resolver is
therefore a different case and must retain the old refusal.

The proposed provenance binds the complete namespace digest, packed words,
current span ID, exact data and spelling, with an explicit raw-unit/no-coordinate
tag. Its production path bypasses the old word-only memo before lookup. The
existing prepared-span codec restores the actual current binding, and both
take and store still validate every used span under the namespace guard.
Neither cached display text nor a cache-owned raw object supplies authority.
Unknown raw owners, malformed words, unknown crates and covered unavailable
rows continue to miss. The new v10 preparation domain separates old entries.

Physical ownership remains in the same admitted 8 MiB family. The optional
32-byte digest is inline in each Record, so both pre-reservation arithmetic and
the actual-capacity check use the enlarged `size_of<Record>`. Digesting borrows
metadata without copying paths or allocating another registry. Existing outer
map, original-row, imported-row and current-file limits remain 4096. Seed to
classified-owner movement and physical disposal before refund remain unchanged;
the prospective aggregate stays 500 MiB plus the existing 80 KiB broker margin,
within 512 MiB. Actual pinned layout and drop controls are still required.

The [stock controls](absence-control/README.md) already establish actual typed
decoding of the sealed vstd metadata and stock `None` with zero loader calls for
the three observed packed ranges. They also distinguish a covered unresolved
row from genuine absence. They do not establish historical callback ownership,
full current-Arc replay, a successful preparation, or cross-edit reuse.

Before full-source validation, retain controls for incomplete/extra/ambiguous
constructor coverage; a covered unavailable row; current ID, words, spelling
and raw-kind changes; a pre-existing word-memo entry; exact fresh Arc restoration;
current namespace growth; admission/drop order; and old-domain rejection. The
memo pair control must exercise `provenance()`, not only its uncached helper.
Composition with the acknowledgement overlay must preserve its real adapter
retirement sensitivity test and every prior role/assertion. No static blocker
was identified in the reviewed draft; full-source tests, strict checks, ordinary
build and the unchanged original qualification remain pending.

The frozen composition in B's `bedefef` was subsequently checked against full
patch `93c91bef65f1793d87968dec7c3a5271f9855a4a379d1019bc78cc25a125394d`
and role delta
`6dedaadc2428979a0d956c875e51038dfe567f119f6f3ffa1b5c0442ac17174f`.
All eight composed source hashes matched its identity. Independently reversing
only that role delta returned the byte-exact eight-source positive retirement
overlay. The existing helper tests remain an exact prefix, and the retirement
implementation and real adapter sensitivity control are preserved.

The final added controls exercise a prepopulated word memo, covered stock
`None`, pending ownership, exact original hash/range changes, current raw-kind
and literal changes, and codec-rebound spans through both actual direct and
parallel namespace adapters. Removing the memo bypass would fail the retained
stale-value assertion; accepting any stock `None` would fail the covered-row
assertion. These are source-level sensitivity findings, not executed results.
Controller `326632795b00d68e8f22351372a3bba3462fc014e94140b4cbab7502be4be6ba`
retains the complete serial library controls and both all-target strict checks,
prior seals, source closure and engine-selector removal. Static composition is
approved for that one private source-control scope. Full-source execution is
pending, and the domain inequality control proves encoded-format separation
only. Actual cache rejection remains mandatory in the original runtime gate.
