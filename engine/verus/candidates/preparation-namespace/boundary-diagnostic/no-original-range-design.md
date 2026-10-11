# Explicit absent original-range authority (proposed)

The bounded diagnostic established three `UnknownCrate` refusals for current
unit spans from core/alloc and a separate `pair-busy` admission failure. The
error name alone is ambiguous: `OwnedImportMappings::record` also uses it when
the crate exists but no original range contains the packed low position.
Read-only inspection of the exact sealed vstd metadata found 145 original rows
under the same stable crate word, with none containing any of the three low
positions. The pinned exporter stamps every span with its exporting crate's
stable ID, but exports `local_files`, which excludes Foreign source files.
Actual pinned metadata decode and stock `unpack_span`/zero-loader controls are
required before this proposal becomes executable authority.

The new role would describe the stock resolver's immutable absence result. It
would supply no source content, loaded owner, coordinates or locator. The
current callback's exact raw unit, two packed words, span ID and spelling remain
mandatory. A decoded preparation still rebinds to that actual current span;
neither a cache record nor its spelling can invent a current binding. Unknown
raw owners, malformed words, invalid low/high order and truly unknown crates
continue to refuse. Existing Loaded, Deferred and OriginalDescriptor checks
remain intact.

At constructor capture, a known-crate completeness entry must be established
from the actual original metadata and immutable `imported_crates` rows. Each
original hash/range must have exactly one corresponding imported row; no
missing, extra, duplicate or ambiguous row may produce the completeness entry.
The entry retains the stable crate ID and an exact digest of the complete
original range/hash/filename inventory. It is not inferred from a display path
or from seeing another valid span. Empty or inconsistent helper namespaces
refuse. Capture failure remains failure for the whole family.

The single late handoff moves this admitted entry with the existing owner. It
does not promote an earlier unavailable span or perform loading. A separate
lookup can classify `NoOriginalRange` only when the complete known-crate entry
exists, the two words are valid, and no original row contains the low position.
The existing `record()` API keeps its old refusal behavior. The new explicit
lookup must distinguish this role from every recorded role and from all errors.
Current SourceMap growth cannot add an immutable imported-range row. Current
stock Reporter remains the sole resolver and returns `None` at its unchanged
range lookup, before any loader call. No compiler coordinates are reconstructed.

Provenance needs a distinct tag and new preparation domain. It binds the
complete original namespace digest, exact packed words and existing exact
current-span key. Intact entries from the old domain must miss. The original
strict diagnostics, same-input lowering oracle and genuine previous-revision
reuse checks remain mandatory; no UnknownCrate-to-success shortcut is allowed.

Completeness storage must be admitted before allocation in the SAME 8 MiB
family, with actual Vec capacity and seed/record overlap charged. A bounded
per-crate digest can be streamed from the immutable constructor rows, using
their audited range order without copying the metadata table. Any unsorted or
ambiguous input must refuse, rather than normalize silently. Registry buffers
drop physically before owner quota release. The 500 MiB plus 80 KiB total and
512 MiB aggregate cap stay unchanged; admission overflow produces a miss.

Required controls include actual pinned metadata/full consumption for the
three observed ranges, actual stock None with zero loader calls, exact current
unit rebinding, changed word/ID/spelling/raw owner negatives, unknown crate and
malformed range negatives, missing/extra/duplicate original-row completeness
refusal, a newly supplied containing imported range refusing the absence role,
SourceMap growth preserving the stock absence result, old-domain rejection,
admission-before-capacity growth and physical release ordering. Complete actual
Reporter diagnostics and the unchanged qualification pipeline remain separate
gates. A's retirement acknowledgement correction is independent of this role.
