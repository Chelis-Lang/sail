# Independent Loaded-import authority review

This is a static review of proposal commit `5f96e28`, including its inert
`owned-import-mapping/capture.rs` helper and eleven pinned-API controls. It does
not apply a source change, run another proof, approve span equivalence, or
qualify a runtime. The [causal diagnostic](span-provenance-causal-diagnostic.json)
and both original strict failures remain unchanged.

The helper validates a complete original crate/range/hash association against
one current Foreign file. It checks normalized range length, current source
hash bytes, stable crate, unique current bounds and same-crate content-owner
ambiguity. Its checked translation refuses crossing the original file boundary.
Unavailable Delayed, missing and busy records remain unavailable; capture does
not call the stock loader. Records contain owned values rather than compiler
contexts, source-file Arcs or old raw spans. Physical records and names are
destroyed before the single 8 MiB family admission is released. These checks
have no identified static authority blocker in the inert helper.

The future constructor/pre-focus integration still needs the following exact
boundaries:

- Preserve original expected hash/range authority until pre-focus validation.
  A current file hash alone cannot reconstruct original metadata authority.
  Seed, late validation, snapshot, Arc header and any memo belong to the same
  admitted family; a second uncharged snapshot is unsupported.
- Check record storage arithmetic before reservation, followed by an actual
  capacity check. The present fixed Record shape and 4,096-record limit fit the
  family, but adding seed fields must not move the admission check after a
  potentially larger allocation. Include old/new name-growth overlap.
- Accept only a current raw **unit** import, with exact current
  `(id, data, as_string)` binding. An arbitrary non-local Any object containing
  two words does not establish an import. Local raw position, hygiene and parent
  checks remain exact.
- Validate the current original-to-loaded translation on the compiler thread,
  then publish one immutable mapping. Retain typed current hash kind/value,
  stable file ID, full bounded name and both lengths. Current coordinates may
  serve as checked translation values only under an explicitly reviewed new
  domain, with old-domain disk and batch rejection controls.
- Record the actual selected state from this admitted mapping. The first
  mismatch's original words are
  `[2208441662476345389, 316229852143522]`. The f833 trace records contributing
  SourceMap files, not resolver state. Existing Foreign files do not prove
  Loaded, as the real Delayed-plus-existing-Foreign control demonstrates.

The pinned stock constructor in `rust_verify/src/spans.rs:120` selects the
remapping Delayed branch only when `RealFileName::local_path()` is available.
The captured contributor's formatted name has `local: None`; this is a useful
observation to check against the pinned accessor, not a substitute for the
selected resolver state. Capture must retain unsupported states rather than
load a file to establish eligibility.

The consumer boundary supports this narrow investigation. Prepared VIR
lowering/effect paths propagate typed spans and their exact identities; they
have no SourceMap or SpanContext from which to read foreign coordinates.
`vir/messages.rs:73–85` rebinds decoded spans to current raw Arcs. The current
stock Reporter at `rust_verify/src/verifier.rs:94/107` resolves imported words
through the current SpanContext, while local diagnostic sorting at line 2267
uses local raw SpanData. Thus current raw binding is necessary, but it cannot
replace resolver authority. Grouped/immediate diagnostics, fresh frontend and
late EXEC remain independent requirements.

The observed 25-byte relocation identifies why the old broad imported-owner
digest misses. It does not authorize omitting owners or starts from that old
digest. The reviewed candidate must still pass relevant-file/hash-kind/name/
namespace/normalization/range/ambiguity/state negatives, current binding and
local hygiene controls, physical admission/drop controls, all original source
tests, fresh vstd, and the original 600-second previous-revision FOCUS and full
same-input gates. No build is authorized by this review alone.
