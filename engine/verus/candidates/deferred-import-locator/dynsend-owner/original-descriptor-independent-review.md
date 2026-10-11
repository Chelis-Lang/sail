# Original descriptor authority: independent review

This reviews the proposed OriginalDescriptorLocator role in
[the design](original-descriptor-role-review.md). It records static conclusions,
not executable eligibility, reuse or source qualification. The retained original
fourteen-stage qualification still fails at revision 0 with zero hits. The
separate bounded trace identifies a Delayed refusal after five eligible Loaded
records; it does not retain the refused filename or Foreign-match count.

The proposed zero raw-hash Foreign condition across **all** current crates is
conservative and closes the missing crate-association problem. Absence supplies
no current CrateNum, typed content hash, file coordinates or Loaded attestation.
A filename must never supply a guessed crate association. Any matching Foreign
file prevents this new role; the existing associated role and its stricter
negative controls remain separate.

Original metadata is borrowed during SpanContext construction. Its exact stable
crate, original range, expected raw hash and filename must be captured once,
under the same admitted 8 MiB owner. The original filename must equal the actual
Delayed descriptor before that copy. Pre-focus validation compares the **current**
Delayed descriptor and current Foreign namespace against those retained original
values. It cannot promise to reread a metadata table that is no longer available.
There is no second seed, resolver reset or state promotion.

The raw current binding remains exactly `(id, data, as_string)` with precisely
`Arc<()>`. The current Reporter keeps its own untouched lazy resolver, including
successful loading and failed hash/file resolution. This recipe does not attest
live file contents, a loader algorithm or successful future diagnostics. The
closed consumer audit and complete actual Reporter/codec/source controls remain
required, as do the original strict diagnostics and same-input wire oracle.

Additional controls should retain the original filename while changing only the
current descriptor, reject missing original filenames, reject Foreign appearance
in any crate before handoff, and reject early Loaded/None/Busy. Admission arithmetic
must precede container growth, and physical last-owner disposal must precede quota
release. A new domain/tag must reject all prior-domain entries. Existing bounds,
local hygiene/parent checks, associated-role negatives and original pipeline
assertions remain unchanged.

No static blocker was found in this narrow policy. Actual sealed vstd metadata
eligibility and reviewed implementation are still prerequisites to a build; the
observed refusal cannot be relabeled eligible from this review.
