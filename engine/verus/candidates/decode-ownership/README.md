# Prepared decode ownership prototype

This unselected source overlay refines allocation admission only. It keeps the
128 MiB cumulative decode cap, depth 128, 8× element and borrowed-byte growth
charges, 128-byte control allowance, suppressed capacity hints, truncation checks
and the separately admitted Frames exception. It changes no preparation key,
span rebinding, graph comparison, solver command or proof acceptance check.

The old guard admitted 36 of 105 retained HE Portable v3 bodies and safely refused
69. The first ownership prototype admitted 77 and refused 28 under the same cap.
These are allocation-only, checksummed-body probes with placeholder span bindings,
not proof validation, confirmed cache hits or previous-revision reuse. No entry is
assigned to a function from its opaque filename. The complete per-entry identities
and outcomes are in `probe.json`. Earlier failures remain in
`../decode-budget/primitive-charge-profile.json`.

## Closed ownership routes

A probe supplies a **nomination inventory**, never new runtime authority. The
frozen table in `prototype.patch` permits only exact value/visitor pairs justified
by the pinned standard serde implementations or the pinned source's generated
fixed-field visitors. `registry-audit.json` binds that source inventory. There is
no production prefix match, dynamic learning or generic custom-visitor waiver.
Unknown pairs retain the original conservative admission.

The audited source fields in VIR AST/SST, definitions, context and messages, and
AIR AST/messages use derived Deserialize without `with`, `deserialize_with` or
`flatten` hooks. The hand-written Span conversion retains its outer seed charge;
its inner derived Encoded record uses fixed-field ownership. Owned span-key/data
and string conversions retain their element/borrowed-byte admission. BigInt and
BigUint custom conversions keep conservative outer admission. Frames alone uses
its independently admitted implementation; the bypass additionally requires the exact compiled Frames value and OwnerVisitor identities, so a tag string alone is insufficient. Interpreter fields
with `serde(skip)` are outside this Portable ownership inventory.

The allowed pinned serde routes are primitive, String, Option, tuple/array, Vec,
HashMap and HashSet visitors with the exact recorded instantiations. String/byte
ownership is still admitted before the borrowed visitor copies bytes. Growing
collections receive no size hint and pay `8 × size_of(element) + 128` per element
(or per key and value). This covers initial minimum capacity, geometric growth,
old/new buffer overlap, map control bytes, alignment and duplicate decoded nodes.
It is not an allocator-arena or process RSS bound.

A paid token names the **exact inline value** whose storage is admitted. An
exact standard PhantomData seed in an audited fixed field may inherit its paid
parent's inline storage. An exact admitted visitor returning that same value
need not charge the same storage again. The outer derived enum admits its largest
inline variant before reading it; matching variant fields inherit that token.
Root values retain fresh admission. Growing Seq/Map accesses reset fixed ownership.

Arc/Box transparent deserialization changes the requested value to a different
pointee visitor value. That mismatch admits the full pointee before allocation;
the unchanged 8×/128 charge covers Box→Arc overlap and control blocks in these
closed pointer forms. Pointee strings/containers retain their separate dynamic
byte/element charges. Option and custom newtype transitions reset the token and
retain conservative admission. Unknown/custom seeds cannot forge a paid token.

Ownership metadata is borrowed static names plus bounded-depth stack values; no
allocation, global memo or release of decode quota is introduced. Existing
physical-drop ordering and preparation phase arithmetic remain unchanged. A
smaller cumulative charge cannot establish a cache hit: all complete consumption,
key, graph, source/span and reporter validation still runs independently.

## Checks and remaining qualification

The private pinned-toolchain helper preserves all five original malformed-input,
string-before-allocation, large-inline/deep-box, capacity/DAG and overflow checks.
Seven additional checks cover fixed/root values, actual Arc/Box large pointees in
fixed fields and growing collections, enum/nested collection ownership, Option
and custom transitions, a custom visitor returning an otherwise known type,
custom seeds, forged graph tag rejection, and sorted exact registry/unknown-pair rejection. Both helper tests
and strict all-target Clippy pass.

The all-105 probe validates entry checksums and full byte consumption twice, with
placeholder bindings for the second decode. It neither replays effects nor
validates a proof. Genuine previous-revision focused reuse, actual full driver
checks, source oracles and normal runtime qualification remain required before
selection. The remaining 28 refusals are retained; fitting every largest body is
not a requirement that justifies weakening admission.
