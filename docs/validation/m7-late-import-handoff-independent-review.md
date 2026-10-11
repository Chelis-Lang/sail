# Independent late imported-authority handoff review

This is a read-only static review of the unselected file-only source candidate:
full patch `228c78745a977b26c33f52446d1d73ec8276c5a32d797bc19dfe3dd4cb3cd949`,
delta `1d6b9158a346441cdc82a49e6a80955acb761673784b47011e333345c3df62d4`.
The complete/delta and seven applied source hashes match the candidate identity.
No compiler, proof or qualification actor was launched for this review.

The prior unchanged-focus failure remains retained: the initial OriginalDescriptor
observed zero matching Foreign files, while the pre-focus namespace contained
one. Its immutable owner correctly refused reuse. The later file's full role
eligibility was not established by that trace. Moving classification cannot
assume that the new candidate admits it or establishes a hit.

The reviewed seams are concrete:

- `SpanContextX::new` now retains a bounded original-authority seed plus the
  exact initial actual resolver recipe. Pending seeds expose no eligible
  provenance. Unsupported, busy or discarded seeds cannot be reset by a late
  Foreign file.
- `sail_with_imports` consumes the seed exactly once after the fresh HIR-to-VIR,
  trait and lifetime phases. One borrowed `SourceMap.files` guard spans late
  classification, final revalidation and Sources publication. Its stable-crate
  callback follows the already audited borrowed crate-store field path.
- `recipe_unchanged` returns the actual resolver mutex guard, without cloning
  a PathBuf/hash payload. Loaded coordinates and Delayed path/hash must match
  the initial observed recipe before namespace classification. This is a late
  classification of previously ineligible seeds, not promotion of discarded
  authority or a write to the stock resolver.
- The same fixed Record array and eight-MiB admission move from Pending to the
  immutable owner. Seed-name capacity stays charged while late descriptor and
  source-name copies are admitted. Disposal still precedes charge release;
  no SourceFile Arc, compiler context or old raw span escapes through this owner.
- Both `take_bucket` and `store_bucket` hold a fresh borrowed file inventory
  while validating every used imported binding. Loaded/Deferred roles require
  the same exact indexed owner, cnum, typed hash, stable identity, name, lengths,
  coordinates and uniqueness. OriginalDescriptor still requires zero matching
  Foreign files across all crates. Later namespace growth can refuse reuse;
  these checks never reclassify an immutable owner.
- The exact current `(id, data, as_string)` span codec and current raw Arc
  rebinding remain in place. No old resolver state or coordinates are restored.
  Replay messages use the current Reporter. Local hygiene/parent and malformed
  imported-span checks remain unchanged. The v8 preparation domain rejects v7
  keys; the portable codec representation stays unchanged.

No additional static authority or ownership blocker was identified in these
seams. Trace labels now distinguish constructor resolver state from late
classification Foreign counts. The reduced actual API controls reported by the
implementer are supporting evidence; they are not complete-source or replay
qualification. New controls cover recipe mutation, missing/second handoff,
post-handoff ambiguity and zero-Foreign growth, cancellation/drop and natural
current stock resolver transitions. Their complete-source execution, strict
checks, original diagnostics/wire oracles, genuine previous-revision focus hit
and original 600-second pipeline remain required before any selection or reuse
claim. The prospective 500-MiB preparation argument remains separate from
ordinary compiler RSS and actual product performance.
