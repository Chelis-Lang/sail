# 0141: Name reported crates from consistent compiler paths

- **Status:** accepted implementation; native Linux qualification and bounded review pass
- **Date:** 2026-11-14
- **Tracking:** #746, #712

## Context

A local trait implementation for a foreign type can have a friendly function
name such as `alloc::Vec::tag`, while its compiler path belongs to the local
crate. Choosing the first timing row's friendly namespace incorrectly names
the crate `alloc` and makes the display depend on timing-row order. The defect
is reproduced on the baseline with exact source matching current main.

## Decision

When any timing row supplies a compiler path, require every row's path to
establish the same nonempty crate namespace. Incomplete, malformed or
conflicting paths leave the optional display name unavailable. With no
compiler paths, preserve the legacy friendly-namespace display only when all
rows agree; conflicting or unqualified friendly names remain unavailable.
This fallback is a display hint, never proof or target identity.

Validate a qualified path before deriving its namespace: the namespace uses
alphanumeric or underscore characters plus Verus's exact generated crate
escapes (`$~` before a digit-leading name and `$~u$~HEX$~` Unicode scalars).
Preserve the encoded namespace verbatim; this validation never rewrites solver
input. Every component is nonempty, has no
remaining colon or control character, and canonical components have no
whitespace. Legacy friendly type tails may retain pretty-printing spaces.
Unsupported or malformed evidence leaves the optional name unavailable.

The existing partial-report selector filters consume the name as a namespace.
Correcting it also fixes selected report rows: an actual shipped-compiler
`--verify-function alloc*` run checks two foreign-type methods; the old name
omits both, while the corrected namespace retains both. This is a scoped
reporting fix, not a purely cosmetic change. Per-function records outside
selector projection, diagnostics, query identities, provenance, cache keys
and physical target/receipt admission policy remain unchanged. Do not add an
attribution failure merely because the optional name is unavailable. No
dependency change.

## Consequences

Crate display names become independent of row order and foreign self-type
names. Existing evidence and the original failed broad stability assertion
remain retained. Native positive/negative/order controls, standard conformance
and independent review are required; no fixture pass is real-project acceptance.


The first c1a4 candidate passes lint and 17 focused tests but independent
native probes expose three accepted malformed path strings and the selector
coupling. Those original failures remain retained; the successor addresses
validation and records actual selected-report behavior. Full native gates,
standard conformance and fresh scoped review remain required.


Actual shipped-compiler controls reject the 7112 successor: valid Unicode and
digit-leading crate names are encoded in VIR and its alphanumeric-only guard
omits selected functions. The next successor validates the pinned compiler's
`krate_ident_to_string` escape grammar and preserves encoded names. Malformed
paths and malformed escape spellings remain refusals. Original actual compiler
outputs and the old/new selected-report differences remain retained.
