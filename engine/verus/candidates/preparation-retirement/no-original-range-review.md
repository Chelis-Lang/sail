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

A follow-up static pass found a test-layout concern missed in the composition
ACK: the new `pending_absence_test` function follows the existing `drop_probe`
test module. Earlier actual pinned strict validation rejected the same pattern
with `items_after_test_module`. This is not an authority change, and the frozen
control scope must retain its actual outcome. If reproduced, move the unchanged
helper before the module in a new frozen identity; do not suppress the lint or
change assertions. No actual lint result for this composition is available yet.

The first frozen scope subsequently failed after 148.315 seconds: VIR's 42
controls and strict check passed, but driver test compilation reported nine
`E0603` private-module accesses. Driver strict validation was unattempted, so
the anticipated ordering lint was not an actual result of that scope. Its
complete raw and closure evidence remain retained in B's `1170b8c`.

The narrow correction was independently applied in small review scratch and
matched all eight source identities: full patch
`d1225e56a98dc09e62e7d520ee0ff65ec308520bf801e04237123bdc8feb22f9`,
delta `b4e50ca99f23265e3f8bc6c31d6f411df76aea42f982525354c074298dfa600f`.
Only three direct-forwarding `cfg(test)` verifier wrappers, their nine new test
call paths, and the unchanged Pending helper's lexical location differ. After
those call-path substitutions, the test assertions and prior prefix are exact;
production engine, roles, validator, budgets, domain, codec and retirement
behavior are unchanged. No static blocker was identified. Corrected controller
`b97d9693138af242529b0d457d6b6d348930ebc546468ea294c21561d88e5356`
requires the exact failed result and source before running a new complete scope.
Corrected source tests and strict results remain pending; this review launches
no compiler, runtime or qualification actor.

That corrected scope compiled but failed after 18.930 seconds: 83 of 108 driver
controls passed. The new memo fixture supplied non-hex short text, which the
unchanged production `ProvenanceMemo::insert` correctly refused. Its subsequent
test-only unwrap panicked, followed by 24 poisoned serial-lock failures. This
disproved the earlier static assumption that the fixture had been admitted.
The complete failed scope remains retained; no cascade is counted as passing.

The new fixture correction was independently checked against full patch
`aaa3b2e4780c6b410b77ac121ef682c9532fb2eb21552a104fa63243e7a95124`
and delta `9415f4a14f8345b09f25978b0deacc7849054b4d20358a2f76e44ad3730b65a9`.
Own scratch application matched all eight source hashes. Only the test helper's
stale value, two added pre-call admission assertions and matching expected
literal changed. The value is exactly 64 ASCII hexadecimal characters; the
unchanged insertion policy admits that shape, and the test checks both the
single entry and exact value before calling `provenance()`. Removing the bypass
would therefore return that admitted stale digest and fail the existing
inequality assertion. Production insertion, bypass, keys, ownership and all
other assertions are byte-exact. Controller
`fe5816dec6ac841fa184001b0f52d0949719e993fd5e94cfe9b3c7df3ac972d2`
binds the second failed result before a new complete scope. Static review found
no blocker; actual corrected source controls remain pending.

The corrected complete source scope then passed: 42 VIR and 108 driver controls
and both all-target strict checks, enclosing 37.197 seconds, result
`a693bbe9dc0d240f499b029bd26a24e94412098e586c301c8a26ecfcf66c7808`.
This closes source-control validation only. It does not erase either earlier
failed scope, build an ordinary runtime, or establish a restored hit.

The independently reviewed counterfactual patch
`c994c23d0c281848f85336e1639e4cc480ddefae0e96bc8f60582557566c61fc`
removes only the six-line production absence memo bypass. Scratch application
matched negative engine
`871bd25334955e841fe790e3a6078e12a096966783316bd3b345a2af5d60a846`;
fixture, admission checks and inequality assertion are unchanged. Final
controller `e8a2e84230ca52483611b04a53d1f79f9666d4d9f3277465800266903a055129`
requires the exact positive result and source, one failed case at the inequality
with identical admitted stale digests on both sides, then exact source restoration
and one positive case. It retains each actual executable and dependency file
before rebuilding, and verifies final source and prior runtime seals. This is
approved for the one private sensitivity scope; execution remains pending.
No ordinary runtime or original qualification result is inferred.

The actual sensitivity scope then passed, result
`5be40db3fba7b031e4ceb38977fd264c63c5712f58637826233bde448647a8ed`:
the unchanged inequality failed with the same admitted stale digest on both
sides, and exact positive restoration passed its one case. Both tested images,
source closure and previous seals are retained. This establishes sensitivity
of that specific source control, without proof or reuse credit.

The ordinary plan for unselected variant `c77e1a1879302e06` was reviewed with
builder `084b0372b4b351ab7833b200d7857cee3ba297473fef82b63d8c03a7bf432c0a`
and build closure
`39ced274bf50a394f471c7652c5afffbd5f8c35f6fbf7b830767568165ad0b37`.
Exact source and sensitivity results, raw logs, source/lock and holder checks
precede builder mutation. Compiler-only seed copies exclude changed VIR,
macros, driver and vstd entries; no top-level runtime, proof or manifest is
copied. Fresh vstd 2060/0, complete new 13-artifact inventory, exact eight source
files and lock, and all four prior runtime seals remain mandatory. No static
ordinary-build blocker was found.

An independent AST comparison of original pipeline controller
`8b7a0cf501cfc1a7401da33c59059ad866101b3bb304d29cd879e8639aed1f19`
confirmed unchanged command, environment and complete 14-script inventory
against the preceding visibility launcher. All 13 original stages, strict
diagnostics and 600-second limits remain intact. The proposed post-qualification
closure still hard-coded the old 9-attempted/8-passed/1-failed outcome; that
reporting bug was flagged to B before launch. It must validate the actual prefix
and primary result, accepting full success only with all 13 stages passed and
retaining any terminal failure and remaining unattempted stages. This does not
change an oracle. Build and qualification execution remain separately pending.
