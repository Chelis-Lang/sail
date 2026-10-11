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

The [deferred-locator consumer audit](deferred-locator-consumer-audit.md) records
the next read-only authority review after the LoadedForeign candidate's actual
Delayed refusal. It proposes no source implementation, selected-runtime change
or exemption from the original qualification gates.

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

## Combined source checks

The source application uses the preceding bounded private-span candidate and
this exact production overlay. The actual VIR namespace differs from the
standalone harness name; its test-only Arc/Box `Large` inventory therefore uses
the exact `type_name` of those fixture types. This changes no production registry
entry or admission rule. The original prototype identity remains retained.

The combined pinned VIR suite passes 33/33 tests, including the actual
span/message/SST schema, all ownership controls and the graph lineage checks.
Compilation uses two jobs at niceness 19; bounded driver and source-oracle
execution runs at ordinary priority. The actual driver passes 22/22 tests, and
both VIR and driver strict all-target Clippy pass. The complete unselected
[combined patch](../ownership-private-span.patch) and
[source check record](combined-source-checks.json) retain the identities and
preceding failures. The direct driver launch failed before any test because
macOS required Cargo's rustc-driver library setup; its ordinary Cargo test
invocation then passed. An untracked allocation-only example also failed strict
Clippy for an unread decoded effects field. Reading its inline owner size in
diagnostic output fixes that example without changing the candidate, admission
or assertions.

The physical phase accounting remains the
[decode allocation audit](../decode-budget/README.md#aggregate-phase-accounting)
plus the [complete observer allowance](../observer-budget/README.md). The largest
simultaneous allowance remains 492 MiB under the unchanged 512 MiB limit. The
ownership tokens and registry add borrowed static names and bounded stack state,
not retained heap buffers. Dynamic collection/pointee/string admission and
physical drop ordering remain unchanged. This is a closed-source allocation
argument, not an allocator-arena, compiler RSS or telemetry-peak measurement.

The independent ordinary Linux build passes at ordinary priority with both Cargo
and source compiler job limits set to two. Its vstd verification reports 2,060
verified and zero errors. The [build record](ordinary-linux-build.json) binds the
sealed runtime, source patch, solver, source lock and raw build log. Independent
source and full proof gates are running; this build does not establish a restored
proof or select the runtime.

## Retained checks and disposable intermediates

After the completed host checks, low disk capacity required reclaiming only the
owned, inactive Cargo intermediates. Exact test/probe executables and dynamic
loader files were retained and verified before removal; source, raw output,
prepared entries, sealed runtimes and the active Linux build were left intact.
[The cleanup record](intermediate-cleanup.json) preserves the original logical
sizes, retained inventories and measured available-space change. The historical
check results remain in `combined-source-checks.json`; recompiling host compiler
objects requires rebuilding those disposable intermediates.

The owner's later cleanup request also covered seven earlier, completed worktree
builds. Only their inactive Cargo `target/debug` directories were removed, after
retaining exact test/CLI/example/loader bytes and Cargo identities in the current
owned evidence set. Worktree source, other raw output and runtime freezes were
preserved. [This second record](prior-worktree-cleanup.json) distinguishes
original logical size from observed filesystem availability; concurrent activity
prevents attributing the availability change solely to this cleanup.

## First combined runtime qualification failure

The ordinary c39 runtime passes graph/private-span audits, old-domain migration,
actual same-input HIT and order/body/effect fault controls, eleven source fixtures,
checkpoints and cancellation/restart. The real HE input-component comparison
finds no changed baseline component after the other-module body edit.

Qualification then stops at revision zero of the previous-revision FOCUS gate.
Stock and engine both prove all nine module-013 obligations with identical
diagnostics, but the engine restores no preparation, so the required checked-HIT
assertion fails. The actual producer identifies `ledger-scratch-budget`; it does
not identify a decode allocation refusal. The whole-graph scratch estimate
charged unrelated nodes and owned span text even for scalar and single-component
queries. [The immutable failure record](c39-qualification-failure.json) retains
that result. Full HE qualification did not run. The candidate remains unselected;
no previous-revision proof reuse or product qualification is established.

The follow-up [scoped-query allocation audit](scoped-graph-scratch.md) binds a
[separate combined patch](../scoped-query-private-span.patch). It retains the
8 MiB scratch limit and all preceding source identities and failures. The
actual pinned VIR suite passes 37/37 and the driver suite passes 22/22; both
strict all-target Clippy checks pass. Ordinary-runtime qualification remains
separate; no checked real cross-edit hit
has yet been established by this follow-up.
